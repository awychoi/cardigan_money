//! Self-contained authenticated backup containers, unified manual/automatic
//! creation, listing and retention, legacy portable containers, and
//! crypto-erase. Restore lives in `services::restore`.
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
use crate::security::cipher::{cipher_integrity_check, open_ledger, sqlite_integrity_check};
use crate::security::crypto::{self, LockedKey, SecretString};
use crate::security::files::{
    ensure_private_dir, file_exists_no_follow, fsync_dir, remove_file_if_exists,
};
use crate::security::native_dialog;
use crate::security::secure_input::{self, SecurePurpose};
use crate::security::vault::{load_header, VaultHeader, HEADER_LEN as VAULT_HEADER_LEN};
use crate::services::vault::VaultState;
use crate::state::AppState;
use crate::storage::events::record_event;
use crate::storage::migrations::supported_schema_version;
use crate::storage::settings::{load_settings, save_settings, Settings};
use crate::storage::snapshot::{create_encrypted_snapshot, create_encrypted_snapshot_with_key};
use crate::storage::time::now_rfc3339;
use crate::storage::vault_meta;

// Restore lives in its own module because of the scope/identity matrix; keep a
// single public import surface for existing backup callers.
pub use crate::services::restore::{
    execute_restore, install_restore_confirmed, prepare_restore, PendingRestore, PlaidTokenChoice,
    RestoreOptions, RestorePlan, RestorePreview, RestoreResult, RestoreScope,
};

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

    pub fn as_str(self) -> &'static str {
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
    fn new(
        kind: BackupKind,
        vault_header: VaultHeader,
        db_len: u64,
        hash: [u8; 32],
        dek: &LockedKey,
    ) -> AppResult<Self> {
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
        let db_len =
            u64::from_le_bytes(bytes[OFFSET_DB_LEN..OFFSET_DB_LEN + 8].try_into().unwrap());
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
                bytes[OFFSET_CREATED..OFFSET_CREATED + 8]
                    .try_into()
                    .unwrap(),
            ),
            db_len,
            db_sha256: hash,
            seal_nonce: nonce,
            seal_tag: tag,
        })
    }

    pub(crate) fn verify_seal(&self, dek: &LockedKey) -> AppResult<()> {
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
    pub listed: bool,
}

pub fn backups_dir(state: &AppState) -> PathBuf {
    state.app_data.join("backups")
}

fn container_name(kind: BackupKind, header: &VaultHeader, now: i64) -> String {
    let stamp = chrono::DateTime::<chrono::Utc>::from_timestamp(now, 0)
        .map(|value| value.format("%Y%m%dT%H%M%SZ").to_string())
        .unwrap_or_else(|| "19700101T000000Z".to_string());
    let nonce = crypto::random_array::<4>()
        .map(hex::encode)
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

pub const RETIRED_PREFIX: &str = "cm-retired-";

/// A retired container keeps the original valid backup name after a prefix.
/// Retired files are never listed with the normal backups and never pruned.
fn valid_retired_filename(name: &str) -> bool {
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    if !lower.starts_with(RETIRED_PREFIX) || !lower.ends_with(".cmbak") {
        return false;
    }
    lower.contains("cm-backup-")
}

fn valid_any_backup_filename(name: &str) -> bool {
    valid_filename(name) || valid_retired_filename(name)
}

pub(crate) fn open_read_nofollow(path: &Path) -> AppResult<(std::fs::File, std::fs::Metadata)> {
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

pub(crate) fn read_header_with_policy(
    path: &Path,
    allow_non_private: bool,
) -> AppResult<(BackupHeader, bool)> {
    let (mut file, meta) = open_read_nofollow(path)?;
    if meta.len() < CONTAINER_HEADER_LEN as u64
        || meta.len() > CONTAINER_HEADER_LEN as u64 + MAX_BACKUP_BYTES
    {
        return Err(AppError::HeaderInvalid("backup file size"));
    }
    let non_private = meta.mode() & 0o077 != 0;
    if non_private && !allow_non_private {
        return Err(AppError::PermissionDenied(
            "backup permissions are not owner-only",
        ));
    }
    let mut bytes = vec![0u8; CONTAINER_HEADER_LEN];
    file.read_exact(&mut bytes)
        .map_err(|_| AppError::HeaderInvalid("backup header"))?;
    let header = BackupHeader::decode(&bytes)?;
    let expected_total = CONTAINER_HEADER_LEN as u64 + header.db_len;
    if meta.len() != expected_total {
        return Err(AppError::HeaderInvalid("backup length mismatch"));
    }
    Ok((header, non_private))
}

fn read_header(path: &Path) -> AppResult<BackupHeader> {
    read_header_with_policy(path, false).map(|(header, _)| header)
}

/// Read the public container header without failing solely because the file
/// mode is not owner-only. The caller must obtain an explicit acknowledgement
/// before using the same flag for the full restore validation.
pub fn inspect_backup_header(path: &Path) -> AppResult<(BackupHeader, bool)> {
    read_header_with_policy(path, true)
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
    let (len, hash) = hash_file(&snapshot)?;
    let header = BackupHeader::new(kind, vault_header.clone(), len, hash, dek)?;
    let name = container_name(kind, vault_header, header.created_at);
    let destination = backup_dir.join(&name);
    let destination_existed = file_exists_no_follow(&destination)?;
    let result = (|| -> AppResult<BackupItem> {
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
    // A failed verification must not leave a corrupt container that could be
    // mistaken for a pre-restore backup.
    if result.is_err() && !destination_existed {
        let _ = remove_file_if_exists(&destination);
    }
    result
}

/// Retention: 7 daily, 5 each pre-migration/pre-restore/pre-change, 10 manual.
/// Never removes the last valid backup overall. Strict filename pattern only.
pub fn prune_backups(backup_dir: &Path) -> AppResult<usize> {
    prune_backups_for_vault(backup_dir, None)
}

/// Vault-aware retention. When `vault_id` is supplied only that vault's normal
/// backups count toward, or are removed by, retention. Retired containers are
/// excluded by the strict normal-name pattern.
pub fn prune_backups_for_vault(backup_dir: &Path, vault_id: Option<&[u8; 16]>) -> AppResult<usize> {
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
            if vault_id.is_some_and(|vault_id| header.vault_header.vault_id != *vault_id) {
                continue;
            }
            items.push((name, header.kind, header));
        }
    }
    items.sort_by_key(|item| std::cmp::Reverse(item.2.created_at));
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

/// The vault ID the app considers current, available while locked because the
/// header is on disk. `None` means a fresh install (or an unreadable header).
pub fn current_vault_id(state: &AppState) -> Option<[u8; 16]> {
    if let Ok(guard) = state.session.lock() {
        if let Some(header) = guard.header.as_ref() {
            return Some(header.vault_id);
        }
    }
    load_header(&state.paths)
        .ok()
        .map(|loaded| loaded.header().vault_id)
}

pub(crate) fn backup_item(name: String, header: &BackupHeader, size_bytes: u64) -> BackupItem {
    BackupItem {
        id: name,
        kind: header.kind.as_str().to_string(),
        created_at: chrono::DateTime::<chrono::Utc>::from_timestamp(header.created_at, 0)
            .map(|value| value.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .unwrap_or_else(|| "unknown".to_string()),
        size_bytes,
        generation: header.vault_header.generation,
        schema_version: header.vault_header.schema_version,
    }
}

pub fn list_backups(state: &AppState) -> AppResult<Vec<BackupItem>> {
    let dir = backups_dir(state);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    // The normal list only contains containers for the current vault. A
    // cross-vault restore retires the previous vault's files, but older
    // installations may still have other-vault files on disk.
    let Some(vault_id) = current_vault_id(state) else {
        return Ok(Vec::new());
    };
    let mut items = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|_| AppError::Io)? {
        let entry = entry.map_err(|_| AppError::Io)?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !valid_filename(&name) {
            continue;
        }
        let path = entry.path();
        if let Ok(header) = read_header(&path) {
            if header.vault_header.vault_id != vault_id {
                continue;
            }
            let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
            items.push(backup_item(name, &header, size));
        }
    }
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(items)
}

/// Retired backups from a previous vault. They are deliberately separate from
/// the normal list, are never pruned, and remain until the owner deletes them.
pub fn list_retired_backups(state: &AppState) -> AppResult<Vec<BackupItem>> {
    let dir = backups_dir(state);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|_| AppError::Io)? {
        let entry = entry.map_err(|_| AppError::Io)?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !valid_retired_filename(&name) {
            continue;
        }
        let path = entry.path();
        if let Ok(header) = read_header(&path) {
            let size = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
            items.push(backup_item(name, &header, size));
        }
    }
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(items)
}

pub fn delete_backup(state: &AppState, id: &str) -> AppResult<()> {
    if !valid_any_backup_filename(id) {
        return Err(AppError::InvalidInput("backup id"));
    }
    let path = backups_dir(state).join(id);
    if !file_exists_no_follow(&path)? {
        return Err(AppError::NotFound);
    }
    remove_file_if_exists(&path)?;
    Ok(())
}

/// Rename every normal backup in the default directory with the retired
/// prefix. Used only after a successful full cross-vault restore. Returns the
/// number of files renamed; a rename failure is reported but does not roll the
/// restored database back.
pub fn retire_normal_backups(state: &AppState) -> AppResult<usize> {
    retire_normal_backups_except(state, None)
}

/// Retire normal backups except containers that already belong to
/// `keep_vault_id`. A full cross-vault restore keeps the restored vault's own
/// backups (including the source file if it was stored in the default
/// directory) and retires the previous vault's containers, including the
/// pre-restore backup.
pub fn retire_normal_backups_except(
    state: &AppState,
    keep_vault_id: Option<&[u8; 16]>,
) -> AppResult<usize> {
    let dir = backups_dir(state);
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut renamed = 0usize;
    for entry in std::fs::read_dir(&dir).map_err(|_| AppError::Io)? {
        let entry = entry.map_err(|_| AppError::Io)?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !valid_filename(&name) {
            continue;
        }
        if let (Some(keep), Ok((header, _))) = (keep_vault_id, inspect_backup_header(&entry.path()))
        {
            if header.vault_header.vault_id == *keep {
                continue;
            }
        }
        let target_name = format!("{RETIRED_PREFIX}{name}");
        let target = dir.join(&target_name);
        if file_exists_no_follow(&target)? {
            // Never overwrite an existing retired container. Leave the source
            // in place; the next restore attempt can resolve it deliberately.
            continue;
        }
        if std::fs::rename(entry.path(), &target).is_ok() {
            renamed += 1;
        }
    }
    if renamed > 0 {
        let _ = fsync_dir(&dir);
    }
    Ok(renamed)
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
    let pruned = prune_backups_for_vault(&dir, Some(&header.vault_id))?;
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
    Ok(BackupResult {
        backup: item,
        pruned,
        listed: true,
    })
}

fn same_directory(candidate: Option<&Path>, directory: &Path) -> bool {
    let Some(candidate) = candidate else {
        return false;
    };
    if candidate == directory {
        return true;
    }
    match (candidate.canonicalize(), directory.canonicalize()) {
        (Ok(candidate), Ok(directory)) => candidate == directory,
        _ => false,
    }
}

fn ensure_backup_extension(mut path: PathBuf) -> PathBuf {
    if path.extension().is_none() {
        path.set_extension("cmbak");
    }
    path
}

fn choose_backup_destination<R: tauri::Runtime>(
    app: &AppHandle<R>,
    directory: &Path,
    suggested_name: &str,
) -> AppResult<Option<PathBuf>> {
    let directory = directory.to_path_buf();
    let suggested_name = suggested_name.to_string();
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let picked = rfd::FileDialog::new()
            .add_filter("cardigan_money backup", &["cmbak"])
            .set_directory(&directory)
            .set_file_name(&suggested_name)
            .set_title("Save an encrypted cardigan_money backup")
            .save_file();
        let _ = tx.send(picked);
    })
    .map_err(|_| AppError::NativeInput)?;
    rx.recv_timeout(std::time::Duration::from_secs(300))
        .map_err(|_| AppError::NativeInput)
}

fn create_backup_at(
    conn: &Connection,
    vault_root: &Path,
    destination: &Path,
    kind: BackupKind,
    header: &VaultHeader,
    dek: &LockedKey,
    parent_private: bool,
) -> AppResult<BackupItem> {
    ensure_private_dir(vault_root)?;
    let snapshot = vault_root.join(format!(".backup-snapshot-{}.cmdb", uuid::Uuid::new_v4()));
    let destination_existed = file_exists_no_follow(destination)?;
    let result = (|| -> AppResult<BackupItem> {
        create_encrypted_snapshot_with_key(conn, &snapshot, dek)?;
        let (len, hash) = hash_file(&snapshot)?;
        let backup_header = BackupHeader::new(kind, header.clone(), len, hash, dek)?;
        write_container(destination, &backup_header, &snapshot, parent_private)?;
        let read = read_header(destination)?;
        read.verify_seal(dek)?;
        verify_db_bytes(destination, &read)?;
        let id = destination
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .ok_or(AppError::Io)?;
        Ok(BackupItem {
            id,
            kind: kind.as_str().to_string(),
            created_at: chrono::DateTime::<chrono::Utc>::from_timestamp(
                backup_header.created_at,
                0,
            )
            .map(|value| value.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .unwrap_or_else(now_rfc3339),
            size_bytes: CONTAINER_HEADER_LEN as u64 + len,
            generation: header.generation,
            schema_version: header.schema_version,
        })
    })();
    let _ = std::fs::remove_file(&snapshot);
    if result.is_err() && !destination_existed {
        let _ = remove_file_if_exists(destination);
    }
    result
}

fn record_manual_backup_success(
    conn: &Connection,
    item: &BackupItem,
    listed: bool,
) -> AppResult<Option<Settings>> {
    let _ = record_event(
        conn,
        "backup_created",
        "success",
        Some(serde_json::json!({ "kind": "manual", "listed": listed })),
    );
    let _ = record_event(
        conn,
        "backup_verified",
        "success",
        Some(serde_json::json!({ "kind": "manual" })),
    );
    let mut settings = match load_settings(conn) {
        Ok(settings) => settings,
        Err(_) => return Ok(None),
    };
    settings.last_backup_at = Some(item.created_at.clone());
    settings.last_backup_status = Some("verified".into());
    if save_settings(conn, &settings).is_err() {
        return Ok(None);
    }
    Ok(Some(settings))
}

/// Create a user backup at an explicit destination with explicit passphrases.
/// The native wrapper supplies the file picker and secure-sheet values. A
/// blank `backup_passphrase` uses the vault header/DEK; while locked a
/// `vault_passphrase` is mandatory and no UI session is created.
pub fn create_manual_backup_to(
    state: &AppState,
    destination: &Path,
    backup_passphrase: SecretString,
    vault_passphrase: Option<SecretString>,
) -> AppResult<BackupResult> {
    state.require_real("create a backup in demo mode")?;
    let backup_dir = backups_dir(state);
    ensure_private_dir(&backup_dir)?;
    let destination = ensure_backup_extension(destination.to_path_buf());
    let in_default_directory = same_directory(destination.parent(), &backup_dir);
    let listed = in_default_directory
        && destination
            .file_name()
            .map(|name| valid_filename(&name.to_string_lossy()))
            .unwrap_or(false);
    let unlocked = {
        let guard = state.session.lock().map_err(|_| AppError::Internal)?;
        guard.is_unlocked()
    };

    if unlocked {
        let header = {
            let guard = state.session.lock().map_err(|_| AppError::Internal)?;
            guard.header.as_ref().ok_or(AppError::Locked)?.clone()
        };
        let custom_passphrase = !backup_passphrase.expose().is_empty();
        if custom_passphrase {
            crypto::validate_new_passphrase(&backup_passphrase)?;
        }
        let target = if custom_passphrase {
            Some(VaultHeader::create(
                &backup_passphrase,
                header.params,
                header.vault_id,
                supported_schema_version(),
            )?)
        } else {
            None
        };

        let mut guard = state.session.lock().map_err(|_| AppError::Internal)?;
        let conn = guard.conn.as_ref().ok_or(AppError::Locked)?;
        let (target_header, target_dek) = match &target {
            Some((target_header, target_dek)) => (target_header, target_dek),
            None => (&header, guard.dek.as_ref().ok_or(AppError::Locked)?),
        };
        let item = create_backup_at(
            conn,
            &state.paths.root,
            &destination,
            BackupKind::Manual,
            target_header,
            target_dek,
            in_default_directory,
        )?;
        let pruned = if listed {
            prune_backups_for_vault(&backup_dir, Some(&header.vault_id))?
        } else {
            0
        };
        let updated_settings = record_manual_backup_success(conn, &item, listed)?;
        if let Some(settings) = updated_settings {
            guard.settings = Some(settings);
        }
        drop(guard);
        drop(backup_passphrase);
        drop(vault_passphrase);
        Ok(BackupResult {
            backup: item,
            pruned,
            listed,
        })
    } else {
        let Some(vault_passphrase) = vault_passphrase else {
            return Err(AppError::Locked);
        };
        if vault_passphrase.expose().is_empty() {
            return Err(AppError::InvalidInput("vault passphrase"));
        }
        let custom_passphrase = !backup_passphrase.expose().is_empty();
        if custom_passphrase {
            crypto::validate_new_passphrase(&backup_passphrase)?;
        }
        // Serialize with restore and with an unlock that may be racing this
        // sessionless backup. The guard stays held until the backup is verified
        // and recorded.
        let session_guard = state.session.lock().map_err(|_| AppError::Internal)?;
        if session_guard.is_unlocked() {
            return Err(AppError::Blocked("the vault was unlocked; retry"));
        }
        let header = load_header(&state.paths)?.header().clone();
        let vault_dek = header.unwrap_dek(&vault_passphrase)?;
        let (conn, _cipher) = open_ledger(&state.paths.database, &vault_dek, false)?;
        cipher_integrity_check(&conn)?;
        sqlite_integrity_check(&conn)?;
        let meta = vault_meta::load(&conn)?;
        if meta.vault_id != header.vault_id_hex() {
            return Err(AppError::IntegrityFailure);
        }
        if meta.schema_version > supported_schema_version() {
            return Err(AppError::SchemaTooNew);
        }

        let target = if custom_passphrase {
            Some(VaultHeader::create(
                &backup_passphrase,
                header.params,
                header.vault_id,
                supported_schema_version(),
            )?)
        } else {
            None
        };
        let (target_header, target_dek) = match &target {
            Some((target_header, target_dek)) => (target_header, target_dek),
            None => (&header, &vault_dek),
        };
        let item = create_backup_at(
            &conn,
            &state.paths.root,
            &destination,
            BackupKind::Manual,
            target_header,
            target_dek,
            in_default_directory,
        )?;
        let pruned = if listed {
            prune_backups_for_vault(&backup_dir, Some(&header.vault_id))?
        } else {
            0
        };
        let _ = record_manual_backup_success(&conn, &item, listed);
        drop(conn);
        drop(session_guard);
        drop(vault_dek);
        drop(vault_passphrase);
        drop(backup_passphrase);
        Ok(BackupResult {
            backup: item,
            pruned,
            listed,
        })
    }
}

pub fn create_manual_backup<R: tauri::Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
) -> AppResult<BackupResult> {
    state.require_real("create a backup in demo mode")?;
    let backup_dir = backups_dir(state);
    ensure_private_dir(&backup_dir)?;
    let unlocked = {
        let guard = state.session.lock().map_err(|_| AppError::Internal)?;
        guard.is_unlocked()
    };
    let header = if unlocked {
        let guard = state.session.lock().map_err(|_| AppError::Internal)?;
        Some(guard.header.as_ref().ok_or(AppError::Locked)?.clone())
    } else {
        Some(load_header(&state.paths)?.header().clone())
    };
    let suggested_name = container_name(
        BackupKind::Manual,
        header.as_ref().ok_or(AppError::VaultMissing)?,
        now_unix(),
    );

    if unlocked {
        let Some(mut values) = secure_input::ask(app, SecurePurpose::BackupPassphrase)? else {
            return Err(AppError::Cancelled);
        };
        let backup_passphrase = values.pop().ok_or(AppError::NativeInput)?;
        drop(values);
        let Some(destination) = choose_backup_destination(app, &backup_dir, &suggested_name)?
        else {
            return Err(AppError::Cancelled);
        };
        create_manual_backup_to(state, &destination, backup_passphrase, None)
    } else {
        let Some(mut values) = secure_input::ask(app, SecurePurpose::BackupUnlock)? else {
            return Err(AppError::Cancelled);
        };
        if values.len() != 2 {
            return Err(AppError::NativeInput);
        }
        let vault_passphrase = values.remove(0);
        let backup_passphrase = values.pop().ok_or(AppError::NativeInput)?;
        drop(values);
        if vault_passphrase.expose().is_empty() {
            return Err(AppError::InvalidInput("vault passphrase"));
        }
        let Some(destination) = choose_backup_destination(app, &backup_dir, &suggested_name)?
        else {
            return Err(AppError::Cancelled);
        };
        create_manual_backup_to(
            state,
            &destination,
            backup_passphrase,
            Some(vault_passphrase),
        )
    }
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

/// Legacy portable-container creation helper for compatibility tests. There
/// is no renderer or gateway action for this anymore; existing portable
/// containers remain restorable through the unified restore flow.
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
        let header = BackupHeader::new(
            BackupKind::Portable,
            export_header.clone(),
            len,
            hash,
            &export_dek,
        )?;
        write_container(destination, &header, &snapshot, false)?;
        let read = read_header(destination)?;
        read.verify_seal(&export_dek)?;
        verify_db_bytes(destination, &read)?;
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

pub fn crypto_erase<R: tauri::Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
) -> AppResult<VaultState> {
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
    // Remove provider Keychain material before dropping the encrypted DB, so a
    // crypto-erase does not leave Plaid credentials behind in the login keychain.
    let credential_refs: Vec<String> = state
        .with_db(|conn, _h, _s| {
            let mut stmt =
                conn.prepare("SELECT credential_ref FROM connections WHERE provider = 'plaid'")?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .unwrap_or_default();
    for credential_ref in credential_refs {
        let _ = state
            .credentials
            .delete(&crate::providers::plaid::access_token_account(
                &credential_ref,
            ));
    }
    // Catch tokens whose credential reference is no longer reachable (for
    // example after a destructive restore). Best effort; the Keychain may
    // deny enumeration.
    let _ = state
        .credentials
        .delete_prefix(crate::providers::plaid::ACCESS_TOKEN_PREFIX);
    let _ = state
        .credentials
        .delete(crate::security::keychain::ACCOUNT_CLIENT_ID);
    let _ = state
        .credentials
        .delete(crate::security::keychain::ACCOUNT_SECRET);
    state.lock(app, "crypto_erase");
    remove_tree(&state.app_data.join("vault"));
    remove_tree(&state.app_data.join("backups"));
    remove_tree(&state.app_data.join("demo"));
    // Remove any stray staged/snapshot files at the app-data root.
    if let Ok(entries) = std::fs::read_dir(&state.app_data) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("restore-staged-")
                || name.starts_with(".backup-snapshot-")
                || name.starts_with("vault-install-")
                || name.starts_with("restore-migrate-")
            {
                if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                    remove_tree(&entry.path());
                } else {
                    overwrite_and_remove(&entry.path());
                }
            }
        }
    }
    let _ = fsync_dir(&state.app_data);
    Ok(crate::services::vault::status(state))
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
        assert!(valid_filename(
            "cm-backup-daily-deadbeef-gen1-20261023T120000Z.cmbak"
        ));
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
