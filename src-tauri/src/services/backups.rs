//! Self-contained authenticated backup containers, restore, portable encrypted
//! export, retention, and crypto-erase.
//!
//! Container v1: fixed 256-byte header + SQLCipher database bytes.
//! The header embeds a complete vault header (KDF params + wrapped DEK), so a
//! backup is self-contained. The database hash and public manifest are bound by
//! an AEAD tag whose key is derived from the DEK (available only with the
//! passphrase). Automatic backups therefore need no retained passphrase.

use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use crate::error::{AppError, AppResult};
use crate::security::cipher::{
    cipher_integrity_check, open_ledger, rekey_ledger, sqlite_integrity_check,
};
use crate::security::crypto::{self, LockedKey, SecretString};
use crate::security::files::{
    ensure_private_dir, enforce_private_file, file_exists_no_follow,
    fsync_dir, remove_file_if_exists,
};
use crate::security::native_dialog;
use crate::security::secure_input::{self, SecurePurpose};
use crate::security::vault::{
    VaultHeader, HEADER_LEN as VAULT_HEADER_LEN,
};
use crate::services::vault::VaultState;
use crate::state::AppState;
use crate::storage::events::record_event;
use crate::storage::migrations::{run_migrations, supported_schema_version};
use crate::storage::settings::{load_settings, save_settings};
use crate::storage::snapshot::{create_encrypted_snapshot, create_encrypted_snapshot_with_key};
use crate::storage::time::now_rfc3339;
use crate::storage::vault_meta;

const MAGIC: &[u8; 8] = b"CMBAK01\0";
const CONTAINER_VERSION: u16 = 1;
const CIPHER_ID: u8 = 1;
const OFFSET_VAULT_HEADER: usize = 14;
const OFFSET_CREATED: usize = OFFSET_VAULT_HEADER + VAULT_HEADER_LEN;
const OFFSET_DB_LEN: usize = OFFSET_CREATED + 8;
const OFFSET_DB_HASH: usize = OFFSET_DB_LEN + 8;
const OFFSET_NONCE: usize = OFFSET_DB_HASH + 32;
const OFFSET_TAG: usize = OFFSET_NONCE + 24;
pub const CONTAINER_HEADER_LEN: usize = OFFSET_TAG + 16 + 2;
pub const MAX_BACKUP_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackupKind {
    Daily,
    Manual,
    PreMigration,
    PreRestore,
    PreChange,
    Portable,
}

impl BackupKind {
    fn code(self) -> u8 {
        match self {
            Self::Daily => 1,
            Self::Manual => 2,
            Self::PreMigration => 3,
            Self::PreRestore => 4,
            Self::PreChange => 5,
            Self::Portable => 6,
        }
    }

    fn from_code(code: u8) -> AppResult<Self> {
        match code {
            1 => Ok(Self::Daily),
            2 => Ok(Self::Manual),
            3 => Ok(Self::PreMigration),
            4 => Ok(Self::PreRestore),
            5 => Ok(Self::PreChange),
            6 => Ok(Self::Portable),
            _ => Err(AppError::HeaderInvalid("backup kind")),
        }
    }

    fn token(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Manual => "manual",
            Self::PreMigration => "pre-migration",
            Self::PreRestore => "pre-restore",
            Self::PreChange => "pre-change",
            Self::Portable => "portable",
        }
    }

    fn as_str(self) -> &'static str {
        self.token()
    }
}

#[derive(Clone)]
pub struct BackupHeader {
    pub kind: BackupKind,
    pub vault_header: VaultHeader,
    pub created_at: i64,
    pub db_len: u64,
    pub db_sha256: [u8; 32],
    pub seal_nonce: [u8; 24],
    pub seal_tag: [u8; 16],
}

impl std::fmt::Debug for BackupHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackupHeader")
            .field("kind", &self.kind)
            .field("generation", &self.vault_header.generation)
            .field("db_len", &self.db_len)
            .finish_non_exhaustive()
    }
}

fn seal_key(dek: &LockedKey) -> AppResult<LockedKey> {
    let mut hasher = Sha256::new();
    hasher.update(b"cardigan_money backup seal v1");
    hasher.update(dek.expose());
    let bytes = hasher.finalize();
    LockedKey::from_slice(&bytes)
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl BackupHeader {
    fn new(kind: BackupKind, vault_header: VaultHeader, db_len: u64, hash: [u8; 32], dek: &LockedKey) -> AppResult<Self> {
        let nonce = crypto::random_array::<24>()?;
        let mut header = Self {
            kind,
            vault_header,
            created_at: now_unix(),
            db_len,
            db_sha256: hash,
            seal_nonce: nonce,
            seal_tag: [0u8; 16],
        };
        let tag = crypto::seal(&seal_key(dek)?, &nonce, &header.aad(), b"")?;
        if tag.len() != 16 {
            return Err(AppError::Internal);
        }
        header.seal_tag.copy_from_slice(&tag);
        Ok(header)
    }

    fn aad(&self) -> Vec<u8> {
        let mut bytes = self.encode();
        bytes.truncate(OFFSET_TAG);
        bytes
    }

    fn encode(&self) -> Vec<u8> {
        let mut out = vec![0u8; CONTAINER_HEADER_LEN];
        out[0..8].copy_from_slice(MAGIC);
        out[8..10].copy_from_slice(&CONTAINER_VERSION.to_le_bytes());
        out[10] = self.kind.code();
        out[11] = CIPHER_ID;
        out[OFFSET_VAULT_HEADER..OFFSET_VAULT_HEADER + VAULT_HEADER_LEN]
            .copy_from_slice(&self.vault_header.encode());
        out[OFFSET_CREATED..OFFSET_CREATED + 8].copy_from_slice(&self.created_at.to_le_bytes());
        out[OFFSET_DB_LEN..OFFSET_DB_LEN + 8].copy_from_slice(&self.db_len.to_le_bytes());
        out[OFFSET_DB_HASH..OFFSET_DB_HASH + 32].copy_from_slice(&self.db_sha256);
        out[OFFSET_NONCE..OFFSET_NONCE + 24].copy_from_slice(&self.seal_nonce);
        out[OFFSET_TAG..OFFSET_TAG + 16].copy_from_slice(&self.seal_tag);
        out
    }

    fn decode(bytes: &[u8]) -> AppResult<Self> {
        if bytes.len() != CONTAINER_HEADER_LEN {
            return Err(AppError::HeaderInvalid("backup header length"));
        }
        if &bytes[0..8] != MAGIC {
            return Err(AppError::HeaderInvalid("backup magic"));
        }
        let version = u16::from_le_bytes(bytes[8..10].try_into().unwrap());
        if version != CONTAINER_VERSION {
            return Err(AppError::HeaderInvalid("backup version"));
        }
        if bytes[11] != CIPHER_ID {
            return Err(AppError::HeaderInvalid("backup cipher"));
        }
        let kind = BackupKind::from_code(bytes[10])?;
        let vault_header = VaultHeader::decode(
            &bytes[OFFSET_VAULT_HEADER..OFFSET_VAULT_HEADER + VAULT_HEADER_LEN],
        )?;
        let db_len = u64::from_le_bytes(
            bytes[OFFSET_DB_LEN..OFFSET_DB_LEN + 8].try_into().unwrap(),
        );
        if db_len == 0 || db_len > MAX_BACKUP_BYTES {
            return Err(AppError::HeaderInvalid("backup size"));
        }
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&bytes[OFFSET_DB_HASH..OFFSET_DB_HASH + 32]);
        let mut nonce = [0u8; 24];
        nonce.copy_from_slice(&bytes[OFFSET_NONCE..OFFSET_NONCE + 24]);
        let mut tag = [0u8; 16];
        tag.copy_from_slice(&bytes[OFFSET_TAG..OFFSET_TAG + 16]);
        Ok(Self {
            kind,
            vault_header,
            created_at: i64::from_le_bytes(
                bytes[OFFSET_CREATED..OFFSET_CREATED + 8].try_into().unwrap(),
            ),
            db_len,
            db_sha256: hash,
            seal_nonce: nonce,
            seal_tag: tag,
        })
    }

    fn verify_seal(&self, dek: &LockedKey) -> AppResult<()> {
        crypto::open_sealed(
            &seal_key(dek)?,
            &self.seal_nonce,
            &self.aad(),
            &self.seal_tag,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupItem {
    pub id: String,
    pub kind: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub generation: u32,
    pub schema_version: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupResult {
    pub backup: BackupItem,
    pub pruned: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub restored: bool,
    pub kind: String,
    pub pre_restore_backup_id: String,
}

pub fn backups_dir(state: &AppState) -> PathBuf {
    state.app_data.join("backups")
}

fn container_name(kind: BackupKind, header: &VaultHeader, now: i64) -> String {
    let stamp = chrono::DateTime::<chrono::Utc>::from_timestamp(now, 0)
        .map(|value| value.format("%Y%m%dT%H%M%SZ").to_string())
        .unwrap_or_else(|| "19700101T000000Z".to_string());
    let nonce = crypto::random_array::<4>()
        .map(|bytes| hex::encode(bytes))
        .unwrap_or_else(|_| "0000".to_string());
    format!(
        "cm-backup-{}-{}-gen{}-{}-{}.cmbak",
        kind.token(),
        &header.vault_id_hex()[..8],
        header.generation,
        stamp,
        nonce
    )
}

fn valid_filename(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if !lower.starts_with("cm-backup-") || !lower.ends_with(".cmbak") {
        return false;
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return false;
    }
    let parts: Vec<&str> = name.split('-').collect();
    parts.len() >= 6
}

fn open_read_nofollow(path: &Path) -> AppResult<(std::fs::File, std::fs::Metadata)> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| AppError::PermissionDenied("backup file open"))?;
    let meta = file.metadata().map_err(|_| AppError::Io)?;
    if !meta.is_file() {
        return Err(AppError::PermissionDenied("backup file type"));
    }
    if meta.uid() != unsafe { libc::geteuid() } {
        return Err(AppError::PermissionDenied("backup owner"));
    }
    Ok((file, meta))
}

fn hash_file(path: &Path) -> AppResult<(u64, [u8; 32])> {
    let (mut file, _) = open_read_nofollow(path)?;
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|_| AppError::Io)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        hasher.update(&buffer[..read]);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    Ok((total, out))
}

fn read_header(path: &Path) -> AppResult<BackupHeader> {
    let (mut file, meta) = open_read_nofollow(path)?;
    if meta.len() < CONTAINER_HEADER_LEN as u64
        || meta.len() > CONTAINER_HEADER_LEN as u64 + MAX_BACKUP_BYTES
    {
        return Err(AppError::HeaderInvalid("backup file size"));
    }
    if meta.mode() & 0o077 != 0 {
        return Err(AppError::PermissionDenied("backup permissions are not owner-only"));
    }
    let mut bytes = vec![0u8; CONTAINER_HEADER_LEN];
    file.read_exact(&mut bytes).map_err(|_| AppError::HeaderInvalid("backup header"))?;
    let header = BackupHeader::decode(&bytes)?;
    let expected_total = CONTAINER_HEADER_LEN as u64 + header.db_len;
    if meta.len() != expected_total {
        return Err(AppError::HeaderInvalid("backup length mismatch"));
    }
    Ok(header)
}

fn verify_db_bytes(path: &Path, header: &BackupHeader) -> AppResult<()> {
    let (mut file, _) = open_read_nofollow(path)?;
    file.seek(SeekFrom::Start(CONTAINER_HEADER_LEN as u64))
        .map_err(|_| AppError::Io)?;
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|_| AppError::Io)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        hasher.update(&buffer[..read]);
    }
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&hasher.finalize());
    if total != header.db_len || hash != header.db_sha256 {
        return Err(AppError::IntegrityFailure);
    }
    Ok(())
}

fn write_container(
    destination: &Path,
    header: &BackupHeader,
    source_db: &Path,
    parent_mode_private: bool,
) -> AppResult<()> {
    let parent = destination.parent().ok_or(AppError::Io)?;
    if parent_mode_private {
        ensure_private_dir(parent)?;
    } else if !parent.is_dir() {
        return Err(AppError::Io);
    }
    if file_exists_no_follow(destination)? {
        return Err(AppError::VaultExists);
    }
    let suffix = uuid::Uuid::new_v4().to_string();
    let temp = parent.join(format!(".cm-backup-{suffix}.tmp"));
    let result = (|| -> AppResult<()> {
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)
            .map_err(|_| AppError::Io)?;
        out.write_all(&header.encode()).map_err(|_| AppError::Io)?;
        let mut input = std::fs::File::open(source_db).map_err(|_| AppError::Io)?;
        std::io::copy(&mut input, &mut out).map_err(|_| AppError::Io)?;
        out.sync_all().map_err(|_| AppError::Io)?;
        std::fs::rename(&temp, destination).map_err(|_| AppError::Io)?;
        fsync_dir(parent)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

pub fn create_backup_inner(
    conn: &Connection,
    vault_header: &VaultHeader,
    dek: &LockedKey,
    vault_root: &Path,
    backup_dir: &Path,
    kind: BackupKind,
) -> AppResult<BackupItem> {
    ensure_private_dir(backup_dir)?;
    let snapshot = vault_root.join(format!(".backup-snapshot-{}.cmdb", uuid::Uuid::new_v4()));
    create_encrypted_snapshot(conn, &snapshot, dek)?;
    let result = (|| -> AppResult<BackupItem> {
        let (len, hash) = hash_file(&snapshot)?;
        let header = BackupHeader::new(kind, vault_header.clone(), len, hash, dek)?;
        let name = container_name(kind, vault_header, header.created_at);
        let destination = backup_dir.join(&name);
        write_container(&destination, &header, &snapshot, true)?;
        // Verify what was written, including the seal.
        let read = read_header(&destination)?;
        read.verify_seal(dek)?;
        verify_db_bytes(&destination, &read)?;
        Ok(BackupItem {
            id: name,
            kind: kind.as_str().to_string(),
            created_at: chrono::DateTime::<chrono::Utc>::from_timestamp(header.created_at, 0)
                .map(|value| value.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
                .unwrap_or_else(now_rfc3339),
            size_bytes: CONTAINER_HEADER_LEN as u64 + len,
            generation: vault_header.generation,
            schema_version: vault_header.schema_version,
        })
    })();
    let _ = std::fs::remove_file(&snapshot);
    result
}

/// Retention: 7 daily, 5 each pre-migration/pre-restore/pre-change, 10 manual.
/// Never removes the last valid backup overall. Strict filename pattern only.
pub fn prune_backups(backup_dir: &Path) -> AppResult<usize> {
    if !backup_dir.is_dir() {
        return Ok(0);
    }
    let mut items: Vec<(String, BackupKind, BackupHeader)> = Vec::new();
    for entry in std::fs::read_dir(backup_dir).map_err(|_| AppError::Io)? {
        let entry = entry.map_err(|_| AppError::Io)?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !valid_filename(&name) {
            continue;
        }
        let meta = match std::fs::symlink_metadata(entry.path()) {
            Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => meta,
            _ => continue,
        };
        if meta.len() < CONTAINER_HEADER_LEN as u64 {
            continue;
        }
        if let Ok(header) = read_header(&entry.path()) {
            items.push((name, header.kind, header));
        }
    }
    items.sort_by(|a, b| b.2.created_at.cmp(&a.2.created_at));
    let mut removed = 0usize;
    let mut remaining = items.len();
    for kind in [
        BackupKind::Daily,
        BackupKind::Manual,
        BackupKind::PreMigration,
        BackupKind::PreRestore,
        BackupKind::PreChange,
    ] {
        let keep = match kind {
            BackupKind::Daily => 7,
            BackupKind::Manual => 10,
            _ => 5,
        };
        let mut seen = 0usize;
        for (name, item_kind, _) in items.iter() {
            if *item_kind != kind {
                continue;
            }
            seen += 1;
            if seen > keep && remaining > 1 {
                let path = backup_dir.join(name);
                if std::fs::remove_file(&path).is_ok() {
                    removed += 1;
                    remaining -= 1;
                }
            }
        }
    }
    Ok(removed)
}

pub fn list_backups(state: &AppState) -> AppResult<Vec<BackupItem>> {
    let dir = backups_dir(state);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|_| AppError::Io)? {
        let entry = entry.map_err(|_| AppError::Io)?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !valid_filename(&name) {
            continue;
        }
        let path = entry.path();
        if let Ok(header) = read_header(&path) {
            let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
            items.push(BackupItem {
                id: name,
                kind: header.kind.as_str().to_string(),
                created_at: chrono::DateTime::<chrono::Utc>::from_timestamp(header.created_at, 0)
                    .map(|value| value.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
                    .unwrap_or_else(|| "unknown".to_string()),
                size_bytes: size,
                generation: header.vault_header.generation,
                schema_version: header.vault_header.schema_version,
            });
        }
    }
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(items)
}

pub fn delete_backup(state: &AppState, id: &str) -> AppResult<()> {
    if !valid_filename(id) {
        return Err(AppError::InvalidInput("backup id"));
    }
    let path = backups_dir(state).join(id);
    if !file_exists_no_follow(&path)? {
        return Err(AppError::NotFound);
    }
    remove_file_if_exists(&path)?;
    Ok(())
}

pub fn create_backup<R: tauri::Runtime>(
    state: &AppState,
    _app: &AppHandle<R>,
    kind: BackupKind,
) -> AppResult<BackupResult> {
    state.require_real("create a backup in demo mode")?;
    if kind == BackupKind::Portable {
        return Err(AppError::Unsupported);
    }
    let mut guard = state.session.lock().map_err(|_| AppError::Internal)?;
    let conn = guard.conn.as_ref().ok_or(AppError::Locked)?;
    let header = guard.header.as_ref().ok_or(AppError::Locked)?.clone();
    let dek = guard.dek.as_ref().ok_or(AppError::Locked)?;
    let dir = backups_dir(state);
    let item = create_backup_inner(conn, &header, dek, &state.paths.root, &dir, kind)?;
    let pruned = prune_backups(&dir)?;
    let _ = record_event(
        conn,
        "backup_created",
        "success",
        Some(serde_json::json!({ "kind": kind.as_str() })),
    );
    let _ = record_event(
        conn,
        "backup_verified",
        "success",
        Some(serde_json::json!({ "kind": kind.as_str() })),
    );
    if let Some(mut settings) = guard.settings.clone() {
        settings.last_backup_at = Some(item.created_at.clone());
        settings.last_backup_status = Some("verified".into());
        save_settings(conn, &settings)?;
        guard.settings = Some(settings);
    }
    Ok(BackupResult { backup: item, pruned })
}

pub fn maybe_daily_backup(state: &AppState) {
    if state.require_real("daily backup").is_err() {
        return;
    }
    let settings = match state.session.lock() {
        Ok(guard) => guard.settings.clone(),
        Err(_) => None,
    };
    let Some(settings) = settings else { return };
    let due = settings
        .last_backup_at
        .as_deref()
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|last| chrono::Utc::now().signed_duration_since(last).num_seconds() >= 86_400)
        .unwrap_or(true);
    if !due {
        return;
    }
    let app = match crate::APP_HANDLE.get() {
        Some(app) => app,
        None => return,
    };
    let result = create_backup(state, app, BackupKind::Daily);
    if result.is_err() {
        if let Ok(mut guard) = state.session.lock() {
            if let Some(conn) = guard.conn.as_ref() {
                let _ = record_event(conn, "backup_failed", "failure", None);
            }
            if let Some(settings) = guard.settings.clone() {
                let mut next = settings;
                next.last_backup_status = Some("failed".into());
                if let Some(conn) = guard.conn.as_ref() {
                    let _ = save_settings(conn, &next);
                }
                guard.settings = Some(next);
            }
        }
    }
}

fn stage_database(source: &Path, destination: &Path, header: &BackupHeader) -> AppResult<()> {
    let (mut input, _) = open_read_nofollow(source)?;
    input
        .seek(SeekFrom::Start(CONTAINER_HEADER_LEN as u64))
        .map_err(|_| AppError::Io)?;
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destination)
        .map_err(|_| AppError::Io)?;
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer).map_err(|_| AppError::Io)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        hasher.update(&buffer[..read]);
        out.write_all(&buffer[..read]).map_err(|_| AppError::Io)?;
    }
    out.sync_all().map_err(|_| AppError::Io)?;
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&hasher.finalize());
    if total != header.db_len || hash != header.db_sha256 {
        let _ = std::fs::remove_file(destination);
        return Err(AppError::IntegrityFailure);
    }
    enforce_private_file(destination)?;
    Ok(())
}

pub struct RestorePlan {
    pub header: BackupHeader,
    pub backup_dek: LockedKey,
    pub staged_path: PathBuf,
    pub kind: String,
}

/// Validate a backup completely and stage its database, without touching the
/// live ledger. The caller must remove `staged_path`.
pub fn prepare_restore(
    state: &AppState,
    backup_path: &Path,
    passphrase: &SecretString,
) -> AppResult<RestorePlan> {
    let header = read_header(backup_path)?;
    let current_header = state
        .with_db(|_conn, header, _settings| header.cloned().ok_or(AppError::Locked))?;
    if header.vault_header.vault_id != current_header.vault_id {
        return Err(AppError::Blocked("backup belongs to a different vault"));
    }
    let backup_dek = header.vault_header.unwrap_dek(passphrase)?;
    header.verify_seal(&backup_dek)?;
    verify_db_bytes(backup_path, &header)?;
    let staged = state
        .paths
        .root
        .join(format!("restore-staged-{}.cmdb", uuid::Uuid::new_v4()));
    stage_database(backup_path, &staged, &header)?;
    let result = (|| -> AppResult<()> {
        let (conn, _cipher) = open_ledger(&staged, &backup_dek, false)?;
        cipher_integrity_check(&conn)?;
        sqlite_integrity_check(&conn)?;
        let meta = vault_meta::load(&conn)?;
        if meta.vault_id != header.vault_header.vault_id_hex() {
            return Err(AppError::IntegrityFailure);
        }
        if meta.schema_version > supported_schema_version() {
            return Err(AppError::SchemaTooNew);
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&staged);
        return Err(error);
    }
    Ok(RestorePlan {
        kind: header.kind.as_str().to_string(),
        header,
        backup_dek,
        staged_path: staged,
    })
}

/// Install a staged restore. Requires the vault to be unlocked so the staged
/// database can be rekeyed to the current DEK and the current header retained.
/// Native-confirmed restore. Tests call `install_restore_confirmed` directly.
pub fn install_restore<R: tauri::Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    plan: RestorePlan,
) -> AppResult<RestoreResult> {
    let confirmed = native_dialog::confirm_destructive(
        app,
        "Replace the current ledger?",
        "Restoring this backup replaces accounts, transactions, rules, imports, and settings. A pre-restore encrypted backup is created first. The current passphrase remains in effect.",
        "Restore backup",
    )?;
    if !confirmed {
        let _ = std::fs::remove_file(&plan.staged_path);
        return Err(AppError::Cancelled);
    }
    install_restore_confirmed(state, plan)
}

/// Install a validated staged restore without a native prompt. The caller must
/// have confirmed with the owner.
pub fn install_restore_confirmed(
    state: &AppState,
    plan: RestorePlan,
) -> AppResult<RestoreResult> {
    let mut guard = state.session.lock().map_err(|_| AppError::Internal)?;
    if !guard.is_unlocked() {
        let _ = std::fs::remove_file(&plan.staged_path);
        return Err(AppError::Locked);
    }
    if guard.mode() != crate::state::SessionMode::Real {
        let _ = std::fs::remove_file(&plan.staged_path);
        return Err(AppError::Blocked("restore is unavailable in demo mode"));
    }
    let current_header = guard.header.clone().ok_or(AppError::Locked)?;
    if current_header.vault_id != plan.header.vault_header.vault_id {
        let _ = std::fs::remove_file(&plan.staged_path);
        return Err(AppError::Blocked("restore target vault changed"));
    }

    let dek = match guard.dek.take() {
        Some(dek) => dek,
        None => {
            let _ = std::fs::remove_file(&plan.staged_path);
            return Err(AppError::Locked);
        }
    };
    let live_conn = match guard.conn.take() {
        Some(conn) => conn,
        None => {
            guard.dek = Some(dek);
            let _ = std::fs::remove_file(&plan.staged_path);
            return Err(AppError::Locked);
        }
    };

    // Re-key the staged database to the current DEK and reconcile metadata.
    let rekey_result = (|| -> AppResult<()> {
        let (staged_conn, _) = open_ledger(&plan.staged_path, &plan.backup_dek, false)?;
        rekey_ledger(&staged_conn, &dek)?;
        vault_meta::set_generation(&staged_conn, current_header.generation, false)?;
        vault_meta::set_schema_version(&staged_conn, supported_schema_version())?;
        vault_meta::set_integrity_checked(&staged_conn)?;
        run_migrations(&staged_conn, &dek, &state.paths.root)?;
        cipher_integrity_check(&staged_conn)?;
        sqlite_integrity_check(&staged_conn)?;
        Ok(())
    })();
    if let Err(error) = rekey_result {
        guard.conn = Some(live_conn);
        guard.dek = Some(dek);
        let _ = std::fs::remove_file(&plan.staged_path);
        return Err(error);
    }

    // Pre-restore backup of the live ledger.
    let pre = match create_backup_inner(
        &live_conn,
        &current_header,
        &dek,
        &state.paths.root,
        &backups_dir(state),
        BackupKind::PreRestore,
    ) {
        Ok(item) => item,
        Err(error) => {
            guard.conn = Some(live_conn);
            guard.dek = Some(dek);
            let _ = std::fs::remove_file(&plan.staged_path);
            return Err(error);
        }
    };

    drop(live_conn);
    let old_path = state.paths.database.with_extension("cmdb.restore-old");
    let _ = std::fs::remove_file(&old_path);
    let _ = std::fs::remove_file(format!("{}-wal", state.paths.database.display()));
    let _ = std::fs::remove_file(format!("{}-shm", state.paths.database.display()));
    if std::fs::rename(&state.paths.database, &old_path).is_err() {
        guard.dek = Some(dek);
        let _ = std::fs::remove_file(&plan.staged_path);
        return Err(AppError::Io);
    }
    if std::fs::rename(&plan.staged_path, &state.paths.database).is_err() {
        let _ = std::fs::rename(&old_path, &state.paths.database);
        guard.dek = Some(dek);
        return Err(AppError::Io);
    }
    let _ = fsync_dir(&state.paths.root);

    match open_ledger(&state.paths.database, &dek, false) {
        Ok((new_conn, cipher)) => {
            if let Err(error) = cipher_integrity_check(&new_conn)
                .and_then(|_| sqlite_integrity_check(&new_conn))
                .and_then(|_| load_settings(&new_conn).map(|_| ()))
            {
                drop(new_conn);
                let _ = std::fs::remove_file(&state.paths.database);
                let _ = std::fs::rename(&old_path, &state.paths.database);
                if let Ok((conn, cipher)) = open_ledger(&state.paths.database, &dek, false) {
                    guard.conn = Some(conn);
                    guard.cipher = Some(cipher);
                }
                guard.dek = Some(dek);
                return Err(error);
            }
            let settings = load_settings(&new_conn)?;
            let _ = std::fs::remove_file(&old_path);
            let _ = record_event(
                &new_conn,
                "restore_succeeded",
                "success",
                Some(serde_json::json!({ "kind": plan.kind })),
            );
            guard.conn = Some(new_conn);
            guard.cipher = Some(cipher);
            guard.dek = Some(dek);
            guard.settings = Some(settings);
            Ok(RestoreResult {
                restored: true,
                kind: plan.kind.clone(),
                pre_restore_backup_id: pre.id,
            })
        }
        Err(error) => {
            let _ = std::fs::remove_file(&state.paths.database);
            let _ = std::fs::rename(&old_path, &state.paths.database);
            if let Ok((conn, cipher)) = open_ledger(&state.paths.database, &dek, false) {
                guard.conn = Some(conn);
                guard.cipher = Some(cipher);
            }
            guard.dek = Some(dek);
            Err(error)
        }
    }
}

pub fn choose_backup_file<R: tauri::Runtime>(app: &AppHandle<R>) -> AppResult<Option<PathBuf>> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let picked = rfd::FileDialog::new()
            .add_filter("cardigan_money backup", &["cmbak"])
            .set_title("Choose an encrypted cardigan_money backup")
            .pick_file();
        let _ = tx.send(picked);
    })
    .map_err(|_| AppError::NativeInput)?;
    rx.recv_timeout(std::time::Duration::from_secs(300))
        .map_err(|_| AppError::NativeInput)
}

pub fn restore<R: tauri::Runtime>(state: &AppState, app: &AppHandle<R>) -> AppResult<RestoreResult> {
    state.require_real("restore a backup in demo mode")?;
    let Some(path) = choose_backup_file(app)? else {
        return Err(AppError::Cancelled);
    };
    let Some(mut values) = secure_input::ask(app, SecurePurpose::RestoreBackup)? else {
        return Err(AppError::Cancelled);
    };
    let passphrase = values.pop().ok_or(AppError::NativeInput)?;
    drop(values);
    let plan = prepare_restore(state, &path, &passphrase)?;
    install_restore(state, app, plan)
}

pub fn portable_export<R: tauri::Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
) -> AppResult<BackupItem> {
    state.require_real("portable export in demo mode")?;
    let Some(passphrase) = secure_input::ask_new_confirmed(app, SecurePurpose::ExportPassword)?
    else {
        return Err(AppError::Cancelled);
    };
    crate::security::crypto::validate_new_passphrase(&passphrase)?;
    let Some(destination) = choose_portable_destination(app)? else {
        return Err(AppError::Cancelled);
    };
    portable_export_to(state, &destination, passphrase)
}

/// Testable portable export with an explicit destination and passphrase.
pub fn portable_export_to(
    state: &AppState,
    destination: &Path,
    passphrase: SecretString,
) -> AppResult<BackupItem> {
    state.require_real("portable export in demo mode")?;
    crate::security::crypto::validate_new_passphrase(&passphrase)?;
    let guard = state.session.lock().map_err(|_| AppError::Internal)?;
    let conn = guard.conn.as_ref().ok_or(AppError::Locked)?;
    let current_header = guard.header.as_ref().ok_or(AppError::Locked)?.clone();
    let params = current_header.params;
    let (export_header, export_dek) = VaultHeader::create(
        &passphrase,
        params,
        current_header.vault_id,
        supported_schema_version(),
    )?;
    drop(passphrase);
    let snapshot = state
        .paths
        .root
        .join(format!(".portable-snapshot-{}.cmdb", uuid::Uuid::new_v4()));
    let result = (|| -> AppResult<BackupItem> {
        create_encrypted_snapshot_with_key(conn, &snapshot, &export_dek)?;
        let (len, hash) = hash_file(&snapshot)?;
        let header = BackupHeader::new(BackupKind::Portable, export_header.clone(), len, hash, &export_dek)?;
        write_container(destination, &header, &snapshot, false)?;
        let read = read_header(destination)?;
        read.verify_seal(&export_dek)?;
        verify_db_bytes(&destination, &read)?;
        let _ = record_event(
            conn,
            "export",
            "success",
            Some(serde_json::json!({ "kind": "portable" })),
        );
        Ok(BackupItem {
            id: destination
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "portable".into()),
            kind: "portable".into(),
            created_at: now_rfc3339(),
            size_bytes: CONTAINER_HEADER_LEN as u64 + len,
            generation: current_header.generation,
            schema_version: supported_schema_version(),
        })
    })();
    let _ = std::fs::remove_file(&snapshot);
    result
}

fn choose_portable_destination<R: tauri::Runtime>(app: &AppHandle<R>) -> AppResult<Option<PathBuf>> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let picked = rfd::FileDialog::new()
            .add_filter("cardigan_money portable backup", &["cmbak"])
            .set_file_name(format!("cardigan-money-portable-{date}.cmbak"))
            .set_title("Portable encrypted export (choose a strong password in the next sheet)")
            .save_file();
        let _ = tx.send(picked);
    })
    .map_err(|_| AppError::NativeInput)?;
    rx.recv_timeout(std::time::Duration::from_secs(300))
        .map_err(|_| AppError::NativeInput)
}

fn overwrite_and_remove(path: &Path) {
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if meta.is_file() && !meta.file_type().is_symlink() {
            if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(path) {
                let len = meta.len().min(4 * 1024 * 1024);
                if let Ok(random) = crypto::random_array::<65536>() {
                    let mut remaining = len;
                    while remaining > 0 {
                        let chunk = remaining.min(65536) as usize;
                        if file.write_all(&random[..chunk]).is_err() {
                            break;
                        }
                        remaining -= chunk as u64;
                    }
                    let _ = file.sync_all();
                }
            }
            let _ = std::fs::remove_file(path);
        }
    }
}

fn remove_tree(dir: &Path) {
    if !dir.is_dir() {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                remove_tree(&path);
                let _ = std::fs::remove_dir(&path);
            } else {
                overwrite_and_remove(&path);
            }
        }
    }
    let _ = std::fs::remove_dir(dir);
}

pub fn crypto_erase<R: tauri::Runtime>(state: &AppState, app: &AppHandle<R>) -> AppResult<VaultState> {
    let confirmed = native_dialog::confirm_destructive(
        app,
        "Crypto-erase all local cardigan_money data?",
        "This permanently deletes the encrypted ledger, vault header, mirrored header, rotation journal, backups, demo database, and any staged restore files. There is NO recovery. Copies outside this app (exports, Time Machine snapshots, other devices) are not erased.",
        "Erase everything",
    )?;
    if !confirmed {
        return Err(AppError::Cancelled);
    }
    crypto_erase_confirmed(state, app)
}

/// Erase without a prompt. Only call after an explicit owner confirmation.
pub fn crypto_erase_confirmed<R: tauri::Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
) -> AppResult<VaultState> {
    state.require_real("erase data in demo mode")?;
    state.lock(app, "crypto_erase");
    remove_tree(&state.app_data.join("vault"));
    remove_tree(&state.app_data.join("backups"));
    remove_tree(&state.app_data.join("demo"));
    // Remove any stray staged/snapshot files at the app-data root.
    if let Ok(entries) = std::fs::read_dir(&state.app_data) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("restore-staged-") || name.starts_with(".backup-snapshot-") {
                overwrite_and_remove(&entry.path());
            }
        }
    }
    let _ = fsync_dir(&state.app_data);
    Ok(crate::services::vault::status(&state))
}

pub fn backup_public_header(path: &Path) -> AppResult<BackupItem> {
    let header = read_header(path)?;
    Ok(BackupItem {
        id: path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default(),
        kind: header.kind.as_str().to_string(),
        created_at: chrono::DateTime::<chrono::Utc>::from_timestamp(header.created_at, 0)
            .map(|value| value.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .unwrap_or_default(),
        size_bytes: std::fs::metadata(path)?.len(),
        generation: header.vault_header.generation,
        schema_version: header.vault_header.schema_version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_pattern_is_strict() {
        assert!(valid_filename("cm-backup-daily-deadbeef-gen1-20261023T120000Z.cmbak"));
        for bad in [
            "../cm-backup-daily-x.cmbak",
            "cm-backup-daily-x.cmbak/evil",
            "something.cmbak",
            "cm-backup-.cmbak",
        ] {
            assert!(!valid_filename(bad), "accepted {bad}");
        }
    }

    #[test]
    fn kind_codes_round_trip() {
        for kind in [
            BackupKind::Daily,
            BackupKind::Manual,
            BackupKind::PreMigration,
            BackupKind::PreRestore,
            BackupKind::PreChange,
            BackupKind::Portable,
        ] {
            assert_eq!(BackupKind::from_code(kind.code()).unwrap(), kind);
        }
    }
}
