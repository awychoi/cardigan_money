//! Validated restore for encrypted backups.
//!
//! The renderer never receives a backup path or a passphrase. A native file
//! picker selects the container, a native secure sheet collects the backup
//! passphrase (and the current vault passphrase when the vault is locked), and
//! all candidate work happens on a staged copy. Every replacement of an
//! existing vault requires the owner to type `replace`.
//!
//! Restore targets:
//! - same-vault full: keep the current header/DEK and passphrase;
//! - cross-vault full: adopt the backup `vault_id` and passphrase with a fresh
//!   header/DEK, and retire the previous vault's normal backups;
//! - fresh install: adopt the backup identity into a fresh vault directory;
//! - partial scope: keep the current identity and reset unselected data;
//! - destructive no-current-passphrase: full only, no pre-restore backup, with
//!   a typed confirmation and an explicit provider-token choice.

use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tauri::AppHandle;

use crate::error::{AppError, AppResult};
use crate::providers::plaid::{self, access_token_account};
use crate::security::cipher::{
    cipher_integrity_check, cipher_summary, open_ledger, rekey_ledger, sqlite_integrity_check,
};
use crate::security::crypto::{self, LockedKey, SecretString};
use crate::security::files::{
    enforce_private_file, ensure_private_dir, file_exists_no_follow, fsync_dir,
};
use crate::security::native_dialog;
use crate::security::secure_input::{self, SecurePurpose};
use crate::security::vault::{load_header, write_header, VaultHeader};
use crate::services::backups::{
    backup_item, backups_dir, create_backup_inner, inspect_backup_header, open_read_nofollow,
    read_header_with_policy, retire_normal_backups_except, BackupHeader, BackupItem, BackupKind,
    CONTAINER_HEADER_LEN,
};
use crate::state::{AppState, SessionMode};
use crate::storage::events::record_event;
use crate::storage::migrations::{run_migrations, supported_schema_version};
use crate::storage::settings::{load_settings, save_settings, Settings};
use crate::storage::vault_meta;

const ALLOWED_TABLES: &[&str] = &[
    "vault_metadata",
    "settings",
    "security_events",
    "schema_migrations",
    "accounts",
    "connections",
    "account_sources",
    "balance_snapshots",
    "payment_snapshots",
    "manual_payment_metadata",
    "import_batches",
    "transactions",
    "transaction_overrides",
    "merchant_rules",
    "import_rows",
    "sync_runs",
    "sync_steps",
];

const ALLOWED_INDEXES: &[&str] = &[
    "security_events_created_idx",
    "security_events_type_idx",
    "accounts_active_idx",
    "connections_status_idx",
    "balance_snapshots_account_idx",
    "payment_snapshots_account_idx",
    "import_batches_account_idx",
    "transactions_provider_identity_idx",
    "transactions_account_date_idx",
    "transactions_reporting_date_idx",
    "transactions_merchant_idx",
    "transactions_batch_idx",
    "merchant_rules_priority_idx",
    "import_rows_batch_idx",
    "import_rows_fingerprint_idx",
    "sync_runs_connection_idx",
    "transactions_rule_idx",
    "transactions_import_fingerprint_idx",
    "transactions_source_row_idx",
    "balance_snapshots_live_idx",
    "payment_snapshots_live_idx",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestoreScope {
    Full,
    FinancialSettings,
    FinancialOnly,
}

impl RestoreScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::FinancialSettings => "financial-settings",
            Self::FinancialOnly => "financial-only",
        }
    }

    fn resets_connections(self) -> bool {
        !matches!(self, Self::Full)
    }

    fn resets_import_history(self) -> bool {
        matches!(self, Self::FinancialOnly)
    }

    fn resets_settings(self) -> bool {
        matches!(self, Self::FinancialOnly)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlaidTokenChoice {
    /// Delete the old vault's local access tokens.
    DeleteLocal,
    /// Delete local tokens after best-effort remote `/item/remove`.
    DeleteAndRevoke,
    /// Keep the old tokens for rollback.
    Keep,
    /// Keep local tokens and reuse the matching Items for restored
    /// connections, so no new Plaid Item is created.
    ReuseLocal,
    /// Destructive path only: delete every app-managed access token found.
    DeleteAll,
    /// Destructive path only: leave every app-managed access token in place.
    LeaveAll,
}

impl PlaidTokenChoice {
    fn is_destructive(self) -> bool {
        matches!(self, Self::DeleteAll | Self::LeaveAll)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePreview {
    pub backup: BackupItem,
    pub backup_vault_prefix: String,
    pub current_vault_prefix: Option<String>,
    pub fresh_install: bool,
    pub same_vault: bool,
    pub requires_typed_confirmation: bool,
    pub requires_current_unlock: bool,
    pub destructive_available: bool,
    pub requires_plaid_choice: bool,
    pub non_private: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub restored: bool,
    pub kind: String,
    pub pre_restore_backup_id: Option<String>,
    pub scope: String,
    pub vault_id_changed: bool,
    /// True when the restore replaced the header/DEK with the backup
    /// passphrase (cross-vault, fresh, or destructive) even if the vault ID did
    /// not change.
    pub adopted_backup_key: bool,
    pub retired_backups: usize,
    pub vault_key_changed: bool,
    pub pre_restore_protection: String,
    pub unlocked: bool,
    /// Restored Plaid connections that kept their Keychain token and Item.
    pub reused_connections: usize,
    /// Restored Plaid connections that need a new link (no local token).
    pub new_link_connections: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentTargetKind {
    Fresh,
    Readable,
    Unreadable,
}

#[derive(Debug, Clone)]
pub struct PendingRestore {
    pub path: PathBuf,
    pub header: BackupHeader,
    pub non_private: bool,
    pub target_kind: CurrentTargetKind,
    pub current_vault_id: Option<[u8; 16]>,
    pub current_generation: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestoreOptions {
    pub scope: RestoreScope,
    #[serde(default)]
    pub allow_non_private: bool,
    #[serde(default)]
    pub confirmation: Option<String>,
    #[serde(default)]
    pub plaid_choice: Option<PlaidTokenChoice>,
    #[serde(default)]
    pub destructive: bool,
}

pub struct RestoreRequest {
    pub path: PathBuf,
    pub backup_passphrase: SecretString,
    pub current_passphrase: Option<SecretString>,
    pub options: RestoreOptions,
    /// Internal/test escape hatch for a flow that has already obtained an
    /// equivalent confirmation. The interactive and gateway paths never set it.
    pub permit_unconfirmed: bool,
}

pub struct PreparedBackup {
    pub header: BackupHeader,
    pub dek: LockedKey,
    pub staged_path: PathBuf,
    pub source_path: PathBuf,
    pub non_private: bool,
}

/// Compatibility plan used by the lower-level restore tests and the old
/// same-vault helper. The new flow uses [`PendingRestore`].
pub struct RestorePlan {
    pub header: BackupHeader,
    pub backup_dek: LockedKey,
    pub staged_path: PathBuf,
    pub kind: String,
    pub non_private: bool,
}

fn vault_files_present(state: &AppState) -> AppResult<bool> {
    Ok(state.paths.exists()?
        || file_exists_no_follow(&state.paths.header_mirror).unwrap_or(false)
        || file_exists_no_follow(&state.paths.database).unwrap_or(false))
}

fn load_current_target(
    state: &AppState,
    unlocked: bool,
) -> AppResult<(CurrentTargetKind, Option<VaultHeader>)> {
    let present = vault_files_present(state)?;
    if !present {
        return Ok((CurrentTargetKind::Fresh, None));
    }
    let header = if unlocked {
        let guard = state.session.lock().map_err(|_| AppError::Internal)?;
        guard.header.clone()
    } else {
        load_header(&state.paths)
            .ok()
            .map(|loaded| loaded.header().clone())
    };
    match header {
        Some(header) => Ok((CurrentTargetKind::Readable, Some(header))),
        None => Ok((CurrentTargetKind::Unreadable, None)),
    }
}

fn inspect_path(state: &AppState, path: &Path) -> AppResult<RestorePreview> {
    state.require_real("restore a backup in demo mode")?;
    let (header, non_private) = inspect_backup_header(path)?;
    if header.vault_header.schema_version > supported_schema_version() {
        return Err(AppError::SchemaTooNew);
    }
    let (unlocked, mode) = {
        let guard = state.session.lock().map_err(|_| AppError::Internal)?;
        (guard.is_unlocked(), guard.mode())
    };
    if mode != SessionMode::Real {
        return Err(AppError::Blocked("restore is unavailable in demo mode"));
    }
    let (target_kind, current_header) = load_current_target(state, unlocked)?;
    let existing = target_kind != CurrentTargetKind::Fresh;
    let same_vault = current_header
        .as_ref()
        .map(|current| current.vault_id == header.vault_header.vault_id)
        .unwrap_or(false);
    let requires_plaid_choice =
        existing && target_kind == CurrentTargetKind::Readable && !same_vault;
    let size_bytes = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    let item = backup_item(
        path.file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default(),
        &header,
        size_bytes,
    );
    Ok(RestorePreview {
        backup: item,
        backup_vault_prefix: header.vault_header.vault_id_hex()[..8].to_string(),
        current_vault_prefix: current_header
            .as_ref()
            .map(|current| current.vault_id_hex()[..8].to_string()),
        fresh_install: !existing,
        same_vault,
        requires_typed_confirmation: existing,
        requires_current_unlock: existing
            && !unlocked
            && target_kind == CurrentTargetKind::Readable,
        destructive_available: existing && !unlocked,
        requires_plaid_choice,
        non_private,
    })
}

/// Inspect a file selected by the native picker. Does not touch the live vault
/// and does not need the passphrase; hash and seal checks run on apply.
pub fn inspect_restore(state: &AppState, path: &Path) -> AppResult<RestorePreview> {
    inspect_path(state, path)
}

pub fn prepare_pending_restore(state: &AppState, path: PathBuf) -> AppResult<RestorePreview> {
    let preview = inspect_path(state, &path)?;
    let (target_kind, current_header) = {
        let unlocked = state
            .session
            .lock()
            .map(|guard| guard.is_unlocked())
            .unwrap_or(false);
        let (kind, header) = load_current_target(state, unlocked)?;
        (kind, header)
    };
    let (header, non_private) = inspect_backup_header(&path)?;
    let pending = PendingRestore {
        path,
        header,
        non_private,
        target_kind,
        current_vault_id: current_header.as_ref().map(|header| header.vault_id),
        current_generation: current_header.as_ref().map(|header| header.generation),
    };
    let mut slot = state
        .pending_restore
        .lock()
        .map_err(|_| AppError::Internal)?;
    *slot = Some(pending);
    drop(slot);
    Ok(preview)
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

pub fn prepare_restore_interactive<R: tauri::Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
) -> AppResult<RestorePreview> {
    state.require_real("restore a backup in demo mode")?;
    // A cancelled native picker must not leave an earlier preview applyable.
    cancel_pending_restore(state)?;
    let Some(path) = choose_backup_file(app)? else {
        return Err(AppError::Cancelled);
    };
    prepare_pending_restore(state, path)
}

pub fn cancel_pending_restore(state: &AppState) -> AppResult<()> {
    let mut slot = state
        .pending_restore
        .lock()
        .map_err(|_| AppError::Internal)?;
    *slot = None;
    Ok(())
}

/// Apply a prepared restore using native file/passphrase sheets. The renderer
/// supplies only the scope and non-secret choices.
pub fn apply_restore_interactive<R: tauri::Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    options: RestoreOptions,
) -> AppResult<RestoreResult> {
    state.require_real("restore a backup in demo mode")?;
    // Keep the pending preview available if a native sheet is cancelled or a
    // passphrase is wrong, so the owner can retry without choosing the file
    // again. The slot is cleared after a successful apply or an explicit
    // cancel.
    let pending = {
        let slot = state
            .pending_restore
            .lock()
            .map_err(|_| AppError::Internal)?;
        slot.as_ref().cloned()
    };
    let Some(pending) = pending else {
        return Err(AppError::InvalidInput("no restore is in progress"));
    };
    if pending.non_private && !options.allow_non_private {
        return Err(AppError::PermissionDenied(
            "backup permissions need an acknowledgement",
        ));
    }
    let (unlocked, mode) = {
        let guard = state.session.lock().map_err(|_| AppError::Internal)?;
        (guard.is_unlocked(), guard.mode())
    };
    if mode != SessionMode::Real {
        return Err(AppError::Blocked("restore is unavailable in demo mode"));
    }
    let (target_kind, current_header) = load_current_target(state, unlocked)?;
    if target_kind != pending.target_kind
        || current_header.as_ref().map(|header| header.vault_id) != pending.current_vault_id
        || current_header.as_ref().map(|header| header.generation) != pending.current_generation
    {
        return Err(AppError::Blocked(
            "the restore target changed; choose the backup again",
        ));
    }
    let existing = target_kind != CurrentTargetKind::Fresh;
    if existing && !options.destructive && options.confirmation.as_deref() != Some("replace") {
        return Err(AppError::Blocked("type replace to confirm"));
    }
    if options.destructive && (unlocked || target_kind == CurrentTargetKind::Fresh) {
        return Err(AppError::Blocked(
            "the destructive path requires a locked existing vault",
        ));
    }

    let Some(mut values) = secure_input::ask(app, SecurePurpose::RestoreBackup)? else {
        return Err(AppError::Cancelled);
    };
    let backup_passphrase = values.pop().ok_or(AppError::NativeInput)?;
    drop(values);
    let mut current_passphrase = None;
    if existing && !options.destructive {
        if unlocked {
            // The session already holds the DEK; no current passphrase is needed.
        } else {
            let Some(values) = secure_input::ask(app, SecurePurpose::RestoreCurrentVault)? else {
                return Err(AppError::Cancelled);
            };
            let mut values = values;
            current_passphrase = Some(values.pop().ok_or(AppError::NativeInput)?);
            drop(values);
        }
    }

    let mut result = execute_restore_borrowed(
        state,
        &pending.path,
        &backup_passphrase,
        current_passphrase.as_ref(),
        options.clone(),
        false,
    )?;
    if let Ok(mut slot) = state.pending_restore.lock() {
        *slot = None;
    }

    // Always acknowledge a successful restore. A cancelled or failed
    // passphrase-change sheet does not roll the restored data back.
    let message = if !result.adopted_backup_key {
        "Your current vault passphrase remains in effect. The pre-restore backup uses your current passphrase."
    } else if result.pre_restore_protection == "none" && !existing {
        "The backup passphrase is now your vault passphrase. A fresh install has no pre-restore backup."
    } else if result.pre_restore_protection == "none" {
        "The backup passphrase is now your vault passphrase. No pre-restore backup was made for this destructive restore."
    } else {
        "The backup passphrase is now your vault passphrase. The pre-restore backup still requires your previous vault passphrase."
    };
    let change_now = native_dialog::restore_complete(app, message).unwrap_or(false);
    if change_now {
        // Any prompt or rotation failure leaves the restored data valid with
        // the passphrase already in effect.
        let changed = prompt_and_change_passphrase(
            state,
            app,
            &result,
            backup_passphrase,
            current_passphrase,
        )
        .unwrap_or(false);
        result.vault_key_changed = changed;
    }
    // A failed rotation or an auto-lock during the native prompt must not leave
    // the renderer believing the session is still open.
    result.unlocked = state
        .session
        .lock()
        .map(|guard| guard.is_unlocked())
        .unwrap_or(false);
    Ok(result)
}

fn prompt_and_change_passphrase<R: tauri::Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    result: &RestoreResult,
    backup_passphrase: SecretString,
    current_passphrase: Option<SecretString>,
) -> AppResult<bool> {
    // When the restore adopted the backup key, the backup passphrase is the
    // current passphrase even if the vault ID stayed the same (destructive
    // restore of a same-ID backup).
    let current_is_known = !result.adopted_backup_key;
    let (current, new) = if current_is_known {
        if let Some(current) = current_passphrase {
            let Some(new) = secure_input::ask_new_confirmed(app, SecurePurpose::ChangeNew)? else {
                return Ok(false);
            };
            (current, new)
        } else {
            let Some(mut values) = secure_input::ask(app, SecurePurpose::RestoreChangeCurrentNew)?
            else {
                return Ok(false);
            };
            if values.len() != 3 {
                return Ok(false);
            }
            let new_passphrase = values.pop().ok_or(AppError::NativeInput)?;
            let confirmation = values.pop().ok_or(AppError::NativeInput)?;
            let current = values.pop().ok_or(AppError::NativeInput)?;
            let matches: bool = new_passphrase
                .expose()
                .as_bytes()
                .ct_eq(confirmation.expose().as_bytes())
                .into();
            if !matches {
                return Ok(false);
            }
            (current, new_passphrase)
        }
    } else {
        let Some(new) = secure_input::ask_new_confirmed(app, SecurePurpose::ChangeNew)? else {
            return Ok(false);
        };
        (backup_passphrase, new)
    };
    match crate::services::vault::change_passphrase_after_restore(state, app, current, new) {
        Ok(()) => Ok(true),
        // A cancelled sheet, a wrong current passphrase, or a failed rotation
        // never invalidates the restored data. The restore stays usable with
        // the passphrase that was already in effect.
        Err(_) => Ok(false),
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

fn check_schema_objects(conn: &Connection, require_all_tables: bool) -> AppResult<()> {
    let mut stmt =
        conn.prepare("SELECT type, name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'")?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut tables = std::collections::BTreeSet::new();
    for (kind, name, sql) in rows {
        match kind.as_str() {
            "table" => {
                if !ALLOWED_TABLES.contains(&name.as_str()) {
                    return Err(AppError::IntegrityFailure);
                }
                let sql = sql.ok_or(AppError::IntegrityFailure)?;
                if !sql
                    .trim_start()
                    .to_ascii_uppercase()
                    .starts_with("CREATE TABLE")
                {
                    return Err(AppError::IntegrityFailure);
                }
                tables.insert(name);
            }
            "index" => {
                if sql.is_none() {
                    // sqlite_autoindex names are filtered above; this branch is
                    // defensive for any future implicit index.
                    continue;
                }
                if !ALLOWED_INDEXES.contains(&name.as_str()) {
                    return Err(AppError::IntegrityFailure);
                }
            }
            // The application schema has no triggers or views. Refuse a
            // restored database that introduces schema-executable objects.
            _ => return Err(AppError::IntegrityFailure),
        }
    }
    if require_all_tables {
        for table in ALLOWED_TABLES {
            if !tables.contains(*table) {
                return Err(AppError::IntegrityFailure);
            }
        }
    }
    Ok(())
}

fn check_attached_databases(conn: &Connection) -> AppResult<()> {
    let mut stmt = conn
        .prepare("PRAGMA database_list")
        .map_err(|_| AppError::IntegrityFailure)?;
    let names = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|_| AppError::IntegrityFailure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| AppError::IntegrityFailure)?;
    for name in names {
        if name != "main" && name != "temp" {
            return Err(AppError::IntegrityFailure);
        }
    }
    Ok(())
}

fn check_foreign_keys(conn: &Connection) -> AppResult<()> {
    let mut stmt = conn
        .prepare("PRAGMA foreign_key_check")
        .map_err(|_| AppError::IntegrityFailure)?;
    let mut rows = stmt.query([]).map_err(|_| AppError::IntegrityFailure)?;
    if rows
        .next()
        .map_err(|_| AppError::IntegrityFailure)?
        .is_some()
    {
        return Err(AppError::IntegrityFailure);
    }
    Ok(())
}

fn stage_and_validate(
    state: &AppState,
    path: &Path,
    passphrase: &SecretString,
    allow_non_private: bool,
) -> AppResult<PreparedBackup> {
    let (header, non_private) = read_header_with_policy(path, allow_non_private)?;
    if header.vault_header.schema_version > supported_schema_version() {
        return Err(AppError::SchemaTooNew);
    }
    let dek = header.vault_header.unwrap_dek(passphrase)?;
    header.verify_seal(&dek)?;
    let staged = state
        .app_data
        .join(format!("restore-staged-{}.cmdb", uuid::Uuid::new_v4()));
    stage_database(path, &staged, &header)?;
    let result = (|| -> AppResult<()> {
        let (conn, _cipher) = open_ledger(&staged, &dek, false)?;
        cipher_integrity_check(&conn)?;
        sqlite_integrity_check(&conn)?;
        check_attached_databases(&conn)?;
        check_schema_objects(&conn, false)?;
        check_foreign_keys(&conn)?;
        let meta = vault_meta::load(&conn)?;
        if meta.vault_id != header.vault_header.vault_id_hex() {
            return Err(AppError::IntegrityFailure);
        }
        if meta.schema_version > supported_schema_version() {
            return Err(AppError::SchemaTooNew);
        }
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|_| AppError::Database)?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = std::fs::remove_file(&staged);
        let _ = std::fs::remove_file(format!("{}-wal", staged.display()));
        let _ = std::fs::remove_file(format!("{}-shm", staged.display()));
        return Err(error);
    }
    Ok(PreparedBackup {
        header,
        dek,
        staged_path: staged,
        source_path: path.to_path_buf(),
        non_private,
    })
}

/// Fully validate a backup and stage its database. Kept for lower-level
/// callers/tests; the interactive flow uses [`prepare_pending_restore`].
pub fn prepare_restore(
    state: &AppState,
    backup_path: &Path,
    passphrase: &SecretString,
) -> AppResult<RestorePlan> {
    let prepared = stage_and_validate(state, backup_path, passphrase, false)?;
    let kind = prepared.header.kind.as_str().to_string();
    Ok(RestorePlan {
        header: prepared.header,
        backup_dek: prepared.dek,
        staged_path: prepared.staged_path,
        kind,
        non_private: prepared.non_private,
    })
}

fn mark_restored_snapshots(conn: &Connection) -> AppResult<()> {
    conn.execute(
        "UPDATE balance_snapshots SET restored = 1 WHERE provenance = 'provider'",
        [],
    )?;
    conn.execute(
        "UPDATE payment_snapshots SET restored = 1 WHERE provenance = 'provider'",
        [],
    )?;
    Ok(())
}

fn reset_scope(conn: &Connection, scope: RestoreScope) -> AppResult<()> {
    mark_restored_snapshots(conn)?;
    if !scope.resets_connections() {
        return Ok(());
    }
    // Sync history and account sources disappear with their connections. Run
    // the explicit deletes anyway so the reset semantics do not depend on the
    // candidate's foreign-key mode.
    conn.execute("DELETE FROM sync_steps", [])?;
    conn.execute("DELETE FROM sync_runs", [])?;
    conn.execute("DELETE FROM account_sources", [])?;
    conn.execute("DELETE FROM connections", [])?;
    conn.execute(
        "UPDATE balance_snapshots SET source_connection_id = NULL",
        [],
    )?;
    conn.execute(
        "UPDATE payment_snapshots SET source_connection_id = NULL",
        [],
    )?;
    conn.execute("DELETE FROM security_events", [])?;
    if scope.resets_import_history() {
        conn.execute("DELETE FROM import_rows", [])?;
        conn.execute("DELETE FROM import_batches", [])?;
        conn.execute("UPDATE transactions SET import_batch_id = NULL", [])?;
    }
    if scope.resets_settings() {
        conn.execute("DELETE FROM settings", [])?;
        save_settings(conn, &Settings::default())?;
    }
    Ok(())
}

fn prepare_restored_connections(
    conn: &Connection,
    state: &AppState,
    scope: RestoreScope,
    replace_identity: bool,
    reuse_local_items: bool,
) -> AppResult<(usize, usize)> {
    if scope.resets_connections() {
        return Ok((0, 0));
    }
    let rows = {
        let mut stmt = conn.prepare("SELECT id, provider, credential_ref FROM connections")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    let mut reused = 0usize;
    let mut new_links = 0usize;
    for (id, provider, credential_ref) in rows {
        // Provider tokens live in the Keychain, never in the backup. A token is
        // reusable only when this Mac still has one for the restored reference.
        // A Keychain failure fails closed to "no token" instead of blocking the
        // restore.
        let has_token = provider == "plaid"
            && plaid::load_access_token(state.credentials.as_ref(), &credential_ref)
                .map(|token| token.is_some())
                .unwrap_or(false);
        let reuse = has_token && reuse_local_items;
        if replace_identity && !reuse {
            conn.execute(
                "UPDATE connections SET credential_ref = ?2 WHERE id = ?1",
                rusqlite::params![id, plaid::new_link_credential_ref()],
            )?;
        }
        // A reused connection keeps its Item, reference, and token. It is
        // healthy and needs no interactive step: the cleared cursor forces a
        // full resync, which also proves the token still works. Everything else
        // waits for an interactive reconnect before any live call.
        let status = if reuse {
            reused += 1;
            "healthy"
        } else {
            if provider == "plaid" {
                new_links += 1;
            }
            "reconnect_required"
        };
        // Every restored connection abandons its cursor and does a full
        // transaction resync; provider IDs are upserted, so this is safe.
        conn.execute(
            "UPDATE connections SET status = ?2, cursor = NULL, \
             last_attempt_at = NULL, last_success_at = NULL, sanitized_error = NULL \
             WHERE id = ?1",
            rusqlite::params![id, status],
        )?;
    }
    Ok((reused, new_links))
}

/// Credential reference plus the connection's Plaid environment. Revocation
/// must use that environment's client credentials and host.
fn old_plaid_credential_refs(conn: &Connection) -> Vec<(String, String)> {
    let mut refs = Vec::new();
    if let Ok(mut stmt) =
        conn.prepare("SELECT credential_ref, environment FROM connections WHERE provider = 'plaid'")
    {
        if let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }) {
            for row in rows.flatten() {
                refs.push(row);
            }
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

fn build_candidate(
    state: &AppState,
    prepared: &PreparedBackup,
    target_header: &VaultHeader,
    target_dek: &LockedKey,
    scope: RestoreScope,
    replace_identity: bool,
    reuse_local_items: bool,
) -> AppResult<(usize, usize)> {
    let (conn, _cipher) = open_ledger(&prepared.staged_path, &prepared.dek, false)?;

    // Migrate the candidate first. Older backups may not have the tables or
    // columns that the scope reset and provider gate rely on. The migration
    // snapshot is encrypted with the backup DEK and thrown away with the
    // temporary directory.
    let migration_dir = state
        .app_data
        .join(format!("restore-migrate-{}", uuid::Uuid::new_v4()));
    ensure_private_dir(&migration_dir)?;
    let migrated = run_migrations(&conn, &prepared.dek, &migration_dir);
    let _ = std::fs::remove_dir_all(&migration_dir);
    migrated?;

    reset_scope(&conn, scope)?;
    let provider_counts =
        prepare_restored_connections(&conn, state, scope, replace_identity, reuse_local_items)?;
    rekey_ledger(&conn, target_dek)?;
    let cipher = cipher_summary(&conn)?;
    vault_meta::reconcile_identity(&conn, target_header, &cipher, supported_schema_version())?;
    vault_meta::set_schema_version(&conn, supported_schema_version())?;
    vault_meta::set_integrity_checked(&conn)?;
    let meta = vault_meta::load(&conn)?;
    if meta.vault_id != target_header.vault_id_hex()
        || meta.generation != target_header.generation
        || meta.schema_version != supported_schema_version()
    {
        return Err(AppError::IntegrityFailure);
    }
    check_schema_objects(&conn, true)?;
    check_attached_databases(&conn)?;
    check_foreign_keys(&conn)?;
    cipher_integrity_check(&conn)?;
    sqlite_integrity_check(&conn)?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(|_| AppError::Database)?;
    drop(conn);
    // The candidate is a single file at this point.
    let _ = std::fs::remove_file(format!("{}-wal", prepared.staged_path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", prepared.staged_path.display()));
    Ok(provider_counts)
}

struct LiveLocked {
    header: VaultHeader,
    dek: LockedKey,
    conn: Connection,
}

fn open_current_locked(state: &AppState, passphrase: &SecretString) -> AppResult<LiveLocked> {
    let header = load_header(&state.paths)?.header().clone();
    let dek = header.unwrap_dek(passphrase)?;
    let (conn, _cipher) = open_ledger(&state.paths.database, &dek, false)?;
    cipher_integrity_check(&conn)?;
    sqlite_integrity_check(&conn)?;
    let meta = vault_meta::load(&conn)?;
    if meta.vault_id != header.vault_id_hex() || meta.generation != header.generation {
        return Err(AppError::IntegrityFailure);
    }
    Ok(LiveLocked { header, dek, conn })
}

fn make_pre_restore_backup(
    state: &AppState,
    conn: &Connection,
    header: &VaultHeader,
    dek: &LockedKey,
) -> AppResult<BackupItem> {
    create_backup_inner(
        conn,
        header,
        dek,
        &state.paths.root,
        &backups_dir(state),
        BackupKind::PreRestore,
    )
}

fn remove_db_sidecars(path: &Path) {
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

fn install_keep_identity(
    state: &AppState,
    staged_path: &Path,
    header: &VaultHeader,
    dek: &LockedKey,
    mut guard: Option<&mut crate::state::VaultSession>,
    live_conn: Option<Connection>,
) -> AppResult<(Connection, crate::security::cipher::CipherInfo, Settings)> {
    drop(live_conn);
    let old_path = state.paths.database.with_extension("cmdb.restore-old");
    let _ = std::fs::remove_file(&old_path);
    remove_db_sidecars(&state.paths.database);
    if std::fs::rename(&state.paths.database, &old_path).is_err() {
        return Err(AppError::Io);
    }
    if std::fs::rename(staged_path, &state.paths.database).is_err() {
        let _ = std::fs::rename(&old_path, &state.paths.database);
        return Err(AppError::Io);
    }
    // Rewrite both header copies so a mirror-only recovery is repaired, but
    // never change the identity of a same-vault restore.
    let _ = write_header(&state.paths, header);
    let _ = fsync_dir(&state.paths.root);

    match open_ledger(&state.paths.database, dek, false) {
        Ok((conn, cipher)) => {
            let verified = cipher_integrity_check(&conn)
                .and_then(|_| sqlite_integrity_check(&conn))
                .and_then(|_| load_settings(&conn));
            match verified {
                Ok(settings) => {
                    let _ = std::fs::remove_file(&old_path);
                    if let Some(guard) = guard.as_deref_mut() {
                        guard.cipher = Some(cipher.clone());
                        guard.settings = Some(settings.clone());
                    }
                    Ok((conn, cipher, settings))
                }
                Err(error) => {
                    drop(conn);
                    let _ = std::fs::remove_file(&state.paths.database);
                    let _ = std::fs::rename(&old_path, &state.paths.database);
                    if let Some(guard) = guard {
                        if let Ok((conn, cipher)) = open_ledger(&state.paths.database, dek, false) {
                            let settings = load_settings(&conn).ok();
                            guard.conn = Some(conn);
                            guard.cipher = Some(cipher);
                            guard.settings = settings;
                        }
                    }
                    Err(error)
                }
            }
        }
        Err(error) => {
            let _ = std::fs::remove_file(&state.paths.database);
            let _ = std::fs::rename(&old_path, &state.paths.database);
            if let Some(guard) = guard {
                if let Ok((conn, cipher)) = open_ledger(&state.paths.database, dek, false) {
                    let settings = load_settings(&conn).ok();
                    guard.conn = Some(conn);
                    guard.cipher = Some(cipher);
                    guard.settings = settings;
                }
            }
            Err(error)
        }
    }
}

fn install_replace_identity_in_place(
    state: &AppState,
    staged_path: &Path,
    header: &VaultHeader,
    dek: &LockedKey,
    live_conn: Option<Connection>,
) -> AppResult<(Connection, crate::security::cipher::CipherInfo, Settings)> {
    drop(live_conn);
    let old_path = state.paths.database.with_extension("cmdb.restore-old");
    let old_header = std::fs::read(&state.paths.header).ok();
    let had_old_db = file_exists_no_follow(&state.paths.database).unwrap_or(false);
    let _ = std::fs::remove_file(&old_path);
    remove_db_sidecars(&state.paths.database);
    if had_old_db && std::fs::rename(&state.paths.database, &old_path).is_err() {
        return Err(AppError::Io);
    }
    if std::fs::rename(staged_path, &state.paths.database).is_err() {
        if had_old_db {
            let _ = std::fs::rename(&old_path, &state.paths.database);
        }
        return Err(AppError::Io);
    }
    // Verify the new database under the new key before committing the new
    // header. If verification fails the old files are restored.
    let opened = open_ledger(&state.paths.database, dek, false);
    match opened {
        Ok((conn, cipher)) => {
            let verified = cipher_integrity_check(&conn)
                .and_then(|_| sqlite_integrity_check(&conn))
                .and_then(|_| load_settings(&conn));
            match verified {
                Ok(settings) => {
                    if write_header(&state.paths, header).is_err() {
                        drop(conn);
                        let _ = std::fs::remove_file(&state.paths.database);
                        if had_old_db {
                            let _ = std::fs::rename(&old_path, &state.paths.database);
                        }
                        restore_old_header(state, old_header.as_deref());
                        return Err(AppError::Io);
                    }
                    let _ = fsync_dir(&state.paths.root);
                    if had_old_db {
                        let _ = std::fs::remove_file(&old_path);
                    }
                    Ok((conn, cipher, settings))
                }
                Err(error) => {
                    drop(conn);
                    let _ = std::fs::remove_file(&state.paths.database);
                    if had_old_db {
                        let _ = std::fs::rename(&old_path, &state.paths.database);
                    }
                    restore_old_header(state, old_header.as_deref());
                    Err(error)
                }
            }
        }
        Err(error) => {
            let _ = std::fs::remove_file(&state.paths.database);
            if had_old_db {
                let _ = std::fs::rename(&old_path, &state.paths.database);
            }
            restore_old_header(state, old_header.as_deref());
            Err(error)
        }
    }
}

fn restore_old_header(state: &AppState, bytes: Option<&[u8]>) {
    if let Some(bytes) = bytes {
        let _ = crate::security::files::atomic_write_private(&state.paths.header, bytes);
        let _ = crate::security::files::atomic_write_private(&state.paths.header_mirror, bytes);
    }
}

fn install_fresh(
    state: &AppState,
    staged_path: &Path,
    header: &VaultHeader,
    dek: &LockedKey,
) -> AppResult<(Connection, crate::security::cipher::CipherInfo, Settings)> {
    let install_dir = state
        .app_data
        .join(format!("vault-install-{}", uuid::Uuid::new_v4()));
    ensure_private_dir(&install_dir)?;
    let target_db = install_dir.join("ledger.cmdb");
    let paths = crate::security::vault::VaultPaths {
        root: install_dir.clone(),
        header: install_dir.join("vault.header"),
        header_mirror: install_dir.join("vault.header.bak"),
        journal: install_dir.join("rotation.journal"),
        database: target_db.clone(),
    };
    let mut installed = false;
    let result = (|| -> AppResult<(Connection, crate::security::cipher::CipherInfo, Settings)> {
        std::fs::rename(staged_path, &target_db).map_err(|_| AppError::Io)?;
        write_header(&paths, header)?;
        let (conn, _cipher) = open_ledger(&target_db, dek, false)?;
        cipher_integrity_check(&conn)?;
        sqlite_integrity_check(&conn)?;
        let settings = load_settings(&conn)?;
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|_| AppError::Database)?;
        drop(conn);
        remove_db_sidecars(&target_db);
        fsync_dir(&install_dir)?;
        if state.paths.root.exists() {
            let empty = std::fs::read_dir(&state.paths.root)
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(false);
            if !empty {
                return Err(AppError::VaultExists);
            }
            let _ = std::fs::remove_dir(&state.paths.root);
        }
        std::fs::rename(&install_dir, &state.paths.root).map_err(|_| AppError::Io)?;
        installed = true;
        let _ = fsync_dir(&state.app_data);
        let (conn, cipher) = open_ledger(&state.paths.database, dek, false)?;
        cipher_integrity_check(&conn)?;
        sqlite_integrity_check(&conn)?;
        Ok((conn, cipher, settings))
    })();
    if result.is_err() {
        if installed {
            let _ = std::fs::remove_dir_all(&state.paths.root);
        }
        let _ = std::fs::remove_dir_all(&install_dir);
    }
    result
}

fn record_restore_success(
    conn: &Connection,
    scope: RestoreScope,
    vault_id_changed: bool,
    retired: usize,
) {
    let _ = record_event(
        conn,
        "restore_succeeded",
        "success",
        Some(serde_json::json!({
            "scope": scope.as_str(),
            "vaultIdChanged": vault_id_changed,
            "retiredBackups": retired,
        })),
    );
}

fn apply_plaid_choice(
    state: &AppState,
    scope: RestoreScope,
    replace_identity: bool,
    destructive: bool,
    choice: Option<PlaidTokenChoice>,
    old_refs: &[(String, String)],
) {
    let delete_refs = || {
        for (credential_ref, _) in old_refs {
            let _ = state
                .credentials
                .delete(&access_token_account(credential_ref));
        }
    };
    if destructive {
        if matches!(choice, Some(PlaidTokenChoice::DeleteAll)) {
            let _ = state.credentials.delete_prefix(plaid::ACCESS_TOKEN_PREFIX);
        }
        return;
    }
    if scope.resets_connections() {
        // No connection remains that could reuse the old tokens.
        delete_refs();
        return;
    }
    if !replace_identity {
        // Same-vault full restore keeps the tokens and credential references.
        return;
    }
    match choice {
        Some(PlaidTokenChoice::DeleteAndRevoke) => {
            for (credential_ref, environment) in old_refs {
                revoke_old_item(state, credential_ref, environment);
            }
            delete_refs();
        }
        Some(PlaidTokenChoice::DeleteLocal) => delete_refs(),
        Some(PlaidTokenChoice::Keep | PlaidTokenChoice::ReuseLocal) | None => {}
        _ => {}
    }
}

fn revoke_old_item(state: &AppState, credential_ref: &str, environment: &str) {
    let Some(environment) = plaid::PlaidEnvironment::parse(environment) else {
        return;
    };
    let Ok(Some(credentials)) = plaid::load_credentials(state.credentials.as_ref(), environment)
    else {
        return;
    };
    let Ok(Some(token)) = plaid::load_access_token(state.credentials.as_ref(), credential_ref)
    else {
        return;
    };
    if let Ok(client) = plaid::PlaidClient::new(credentials, Some(token)) {
        let _ = client.item_remove();
    }
}

fn execute_with_prepared(
    state: &AppState,
    prepared: PreparedBackup,
    backup_passphrase: &SecretString,
    current_passphrase: Option<&SecretString>,
    options: RestoreOptions,
    permit_unconfirmed: bool,
) -> AppResult<RestoreResult> {
    let staged_path = prepared.staged_path.clone();
    let result = execute_with_prepared_inner(
        state,
        &prepared,
        backup_passphrase,
        current_passphrase,
        &options,
        permit_unconfirmed,
    );
    if result.is_err() {
        let _ = std::fs::remove_file(&staged_path);
        let _ = std::fs::remove_file(format!("{}-wal", staged_path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", staged_path.display()));
    }
    result
}

fn execute_with_prepared_inner(
    state: &AppState,
    prepared: &PreparedBackup,
    backup_passphrase: &SecretString,
    current_passphrase: Option<&SecretString>,
    options: &RestoreOptions,
    permit_unconfirmed: bool,
) -> AppResult<RestoreResult> {
    state.require_real("restore a backup in demo mode")?;
    if prepared.non_private && !options.allow_non_private {
        return Err(AppError::PermissionDenied(
            "backup permissions need an acknowledgement",
        ));
    }
    let mut guard = state.session.lock().map_err(|_| AppError::Internal)?;
    if guard.mode() != SessionMode::Real {
        return Err(AppError::Blocked("restore is unavailable in demo mode"));
    }
    let was_unlocked = guard.is_unlocked();
    let current_header = if was_unlocked {
        guard.header.clone()
    } else {
        load_header(&state.paths)
            .ok()
            .map(|loaded| loaded.header().clone())
    };
    let present = vault_files_present(state)?;
    let existing = present;
    let unreadable = existing && current_header.is_none();
    let backup_vault_id = prepared.header.vault_header.vault_id;
    let same_vault = current_header
        .as_ref()
        .map(|header| header.vault_id == backup_vault_id)
        .unwrap_or(false);

    if existing && !permit_unconfirmed && options.confirmation.as_deref() != Some("replace") {
        return Err(AppError::Blocked("type replace to confirm"));
    }
    if options.destructive {
        if !existing {
            return Err(AppError::Blocked("no existing vault to replace"));
        }
        if was_unlocked {
            return Err(AppError::Blocked(
                "the destructive path requires a locked existing vault",
            ));
        }
        if options.scope != RestoreScope::Full {
            return Err(AppError::InvalidInput(
                "the destructive path is full-database only",
            ));
        }
        if !matches!(
            options.plaid_choice,
            Some(
                PlaidTokenChoice::DeleteAll
                    | PlaidTokenChoice::LeaveAll
                    | PlaidTokenChoice::ReuseLocal
            )
        ) {
            return Err(AppError::InvalidInput("provider token choice"));
        }
    } else {
        if unreadable {
            return Err(AppError::Blocked(
                "the current vault cannot be opened; use the destructive restore path",
            ));
        }
        if options
            .plaid_choice
            .is_some_and(|choice| choice.is_destructive())
        {
            return Err(AppError::InvalidInput("provider token choice"));
        }
        if options.scope != RestoreScope::Full && options.plaid_choice.is_some() {
            return Err(AppError::InvalidInput("provider token choice"));
        }
        if options.scope == RestoreScope::Full
            && existing
            && !same_vault
            && !matches!(
                options.plaid_choice,
                Some(
                    PlaidTokenChoice::DeleteLocal
                        | PlaidTokenChoice::DeleteAndRevoke
                        | PlaidTokenChoice::Keep
                        | PlaidTokenChoice::ReuseLocal
                )
            )
        {
            return Err(AppError::InvalidInput("provider token choice"));
        }
        if options.scope == RestoreScope::Full
            && (!existing || same_vault)
            && options.plaid_choice.is_some()
        {
            return Err(AppError::InvalidInput("provider token choice"));
        }
    }

    let keep_identity =
        existing && !options.destructive && (options.scope != RestoreScope::Full || same_vault);
    let replace_identity = !keep_identity;
    // Same-vault restores always reuse the local Item. A fresh install has no
    // current tokens to reason about, so it also tries to reuse whatever the
    // Mac's Keychain still holds. Cross-vault restores reuse only when asked.
    let reuse_local_items = !replace_identity
        || matches!(options.plaid_choice, Some(PlaidTokenChoice::ReuseLocal))
        || (!existing && options.plaid_choice.is_none());
    let vault_id_changed = if keep_identity {
        false
    } else if current_header.is_some() {
        !same_vault
    } else {
        unreadable
    };
    let retire = existing && replace_identity && vault_id_changed;
    let pre_restore_protection = if !existing || options.destructive {
        "none"
    } else if keep_identity {
        "current"
    } else {
        "previous"
    };

    if existing && !options.destructive && !was_unlocked && current_passphrase.is_none() {
        return Err(AppError::Locked);
    }
    let mut locked_current = if existing && !options.destructive && !was_unlocked {
        Some(open_current_locked(
            state,
            current_passphrase.ok_or(AppError::Locked)?,
        )?)
    } else {
        None
    };

    // The pre-restore backup must be created and verified before any live file
    // is moved. Any failure here aborts the restore.
    let pre_restore = if existing && !options.destructive {
        if was_unlocked {
            let conn = guard.conn.as_ref().ok_or(AppError::Locked)?;
            let header = current_header.as_ref().ok_or(AppError::Locked)?;
            let dek = guard.dek.as_ref().ok_or(AppError::Locked)?;
            Some(make_pre_restore_backup(state, conn, header, dek)?)
        } else {
            let live = locked_current.as_ref().ok_or(AppError::Locked)?;
            Some(make_pre_restore_backup(
                state,
                &live.conn,
                &live.header,
                &live.dek,
            )?)
        }
    } else {
        None
    };

    let old_refs = if existing && !options.destructive {
        if was_unlocked {
            guard
                .conn
                .as_ref()
                .map(old_plaid_credential_refs)
                .unwrap_or_default()
        } else {
            locked_current
                .as_ref()
                .map(|live| old_plaid_credential_refs(&live.conn))
                .unwrap_or_default()
        }
    } else {
        Vec::new()
    };

    // A replacement identity gets a brand-new header and DEK under the backup
    // passphrase. The backup's own DEK is never adopted.
    let new_identity = if replace_identity {
        crypto::validate_new_passphrase(backup_passphrase)?;
        Some(VaultHeader::create(
            backup_passphrase,
            crypto::calibrate_kdf(),
            backup_vault_id,
            supported_schema_version(),
        )?)
    } else {
        None
    };
    let target_header = match &new_identity {
        Some((header, _)) => header.clone(),
        None => current_header.clone().ok_or(AppError::Locked)?,
    };

    let mut retired = 0usize;
    let mut new_conn: Option<Connection> = None;

    let (reused_connections, new_link_connections) = {
        let target_dek: &LockedKey = match &new_identity {
            Some((_, dek)) => dek,
            None if was_unlocked => guard.dek.as_ref().ok_or(AppError::Locked)?,
            None => &locked_current.as_ref().ok_or(AppError::Locked)?.dek,
        };
        build_candidate(
            state,
            prepared,
            &target_header,
            target_dek,
            options.scope,
            replace_identity,
            reuse_local_items,
        )?
    };

    if keep_identity {
        if was_unlocked {
            let dek = guard.dek.take().ok_or(AppError::Locked)?;
            let live_conn = guard.conn.take();
            match install_keep_identity(
                state,
                &prepared.staged_path,
                &target_header,
                &dek,
                Some(&mut guard),
                live_conn,
            ) {
                Ok((conn, cipher, settings)) => {
                    guard.conn = Some(conn);
                    guard.dek = Some(dek);
                    guard.cipher = Some(cipher);
                    guard.settings = Some(settings);
                }
                Err(error) => {
                    guard.dek = Some(dek);
                    return Err(error);
                }
            }
        } else {
            let live = locked_current.take().ok_or(AppError::Locked)?;
            let dek = live.dek;
            let (conn, _cipher, _settings) = install_keep_identity(
                state,
                &prepared.staged_path,
                &target_header,
                &dek,
                None,
                Some(live.conn),
            )?;
            new_conn = Some(conn);
        }
    } else {
        let (new_header, new_dek) = new_identity.ok_or(AppError::Internal)?;
        if existing {
            let live_conn = if was_unlocked {
                guard.conn.take()
            } else {
                locked_current.take().map(|live| live.conn)
            };
            match install_replace_identity_in_place(
                state,
                &prepared.staged_path,
                &new_header,
                &new_dek,
                live_conn,
            ) {
                Ok((conn, cipher, settings)) => {
                    if was_unlocked {
                        guard.header = Some(new_header);
                        guard.dek = Some(new_dek);
                        guard.conn = Some(conn);
                        guard.cipher = Some(cipher);
                        guard.settings = Some(settings);
                    } else {
                        new_conn = Some(conn);
                    }
                }
                Err(error) => {
                    if was_unlocked {
                        if let Some(dek) = guard.dek.as_ref() {
                            if let Ok((conn, cipher)) =
                                open_ledger(&state.paths.database, dek, false)
                            {
                                let settings = load_settings(&conn).ok();
                                guard.conn = Some(conn);
                                guard.cipher = Some(cipher);
                                guard.settings = settings;
                            }
                        }
                    }
                    return Err(error);
                }
            }
        } else {
            let (conn, _cipher, _settings) =
                install_fresh(state, &prepared.staged_path, &new_header, &new_dek)?;
            new_conn = Some(conn);
        }
    }

    if retire {
        retired = retire_normal_backups_except(state, Some(&backup_vault_id)).unwrap_or(0);
    }
    apply_plaid_choice(
        state,
        options.scope,
        replace_identity,
        options.destructive,
        options.plaid_choice,
        &old_refs,
    );
    if let Some(conn) = new_conn.as_ref() {
        record_restore_success(conn, options.scope, vault_id_changed, retired);
    } else if let Some(conn) = guard.conn.as_ref() {
        record_restore_success(conn, options.scope, vault_id_changed, retired);
    }
    drop(new_conn);

    Ok(RestoreResult {
        restored: true,
        kind: prepared.header.kind.as_str().to_string(),
        pre_restore_backup_id: pre_restore.map(|item| item.id),
        scope: options.scope.as_str().to_string(),
        vault_id_changed,
        adopted_backup_key: replace_identity,
        retired_backups: retired,
        vault_key_changed: false,
        pre_restore_protection: pre_restore_protection.to_string(),
        unlocked: was_unlocked,
        reused_connections,
        new_link_connections,
    })
}

/// Public core entry point used by tests and the native wrapper after all
/// secrets and choices have been collected. The passphrases are borrowed so the
/// native wrapper can still use the backup passphrase for the post-restore
/// change-passphrase prompt.
pub fn execute_restore_borrowed(
    state: &AppState,
    path: &Path,
    backup_passphrase: &SecretString,
    current_passphrase: Option<&SecretString>,
    options: RestoreOptions,
    permit_unconfirmed: bool,
) -> AppResult<RestoreResult> {
    let prepared = stage_and_validate(state, path, backup_passphrase, options.allow_non_private)?;
    execute_with_prepared(
        state,
        prepared,
        backup_passphrase,
        current_passphrase,
        options,
        permit_unconfirmed,
    )
}

/// Owning convenience wrapper for tests and non-native callers.
pub fn execute_restore(state: &AppState, request: RestoreRequest) -> AppResult<RestoreResult> {
    execute_restore_borrowed(
        state,
        &request.path,
        &request.backup_passphrase,
        request.current_passphrase.as_ref(),
        request.options,
        request.permit_unconfirmed,
    )
}

/// Compatibility helper for the old same-vault tests: install a prepared
/// same-vault full restore without a native prompt.
pub fn install_restore_confirmed(state: &AppState, plan: RestorePlan) -> AppResult<RestoreResult> {
    let prepared = PreparedBackup {
        header: plan.header,
        dek: plan.backup_dek,
        staged_path: plan.staged_path,
        source_path: PathBuf::new(),
        non_private: plan.non_private,
    };
    let current_id = crate::services::backups::current_vault_id(state);
    if current_id != Some(prepared.header.vault_header.vault_id) {
        let _ = std::fs::remove_file(&prepared.staged_path);
        return Err(AppError::Blocked(
            "cross-vault restore requires explicit options",
        ));
    }
    let unlocked = state
        .session
        .lock()
        .map(|guard| guard.is_unlocked())
        .unwrap_or(false);
    if !unlocked {
        let _ = std::fs::remove_file(&prepared.staged_path);
        return Err(AppError::Locked);
    }
    let empty = SecretString::new(String::new());
    execute_with_prepared(
        state,
        prepared,
        &empty,
        None,
        RestoreOptions {
            scope: RestoreScope::Full,
            allow_non_private: true,
            confirmation: None,
            plaid_choice: None,
            destructive: false,
        },
        true,
    )
}
