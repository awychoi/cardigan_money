//! Demo mode: a separate SQLCipher database with a fixed synthetic key, seeded
//! deterministically. It cannot open the real ledger, read the Keychain, import
//! real data, create backups, or export real data.

pub mod fixtures;

use std::path::{Path, PathBuf};

use tauri::Manager;

use crate::error::{AppError, AppResult};
use crate::security::cipher::{
    cipher_integrity_check, cipher_summary, open_ledger, sqlite_integrity_check, CipherInfo,
};
use crate::security::crypto::LockedKey;
use crate::security::files::{ensure_private_dir, file_exists_no_follow};
use crate::services::vault::{SecuritySummary, VaultState};
use crate::state::{AppState, SessionMode};
use crate::storage::events::record_event;
use crate::storage::migrations::{run_migrations, supported_schema_version};
use crate::storage::settings::{load_settings, save_settings, Settings};
use crate::storage::time::now_rfc3339;
use crate::storage::vault_meta::{self, insert_demo_metadata};

/// Fixed app-managed key: SHA-256("cardigan_money demo synthetic v1").
/// This is public by design and protects nothing real; it exists only so demo
/// data at rest is still a SQLCipher database rather than plaintext SQLite.
const DEMO_KEY_HEX: &str =
    "a2ceb32f7ebfe63ff306dcb0eb26326ca99c31998730f72c1446aafb2697164d";

pub struct DemoPaths {
    pub root: PathBuf,
    pub database: PathBuf,
}

impl DemoPaths {
    pub fn new(app_data: &Path) -> Self {
        let root = app_data.join("demo");
        Self { database: root.join("demo.cmdb"), root }
    }

    fn ensure(&self) -> AppResult<()> {
        ensure_private_dir(&self.root)
    }

    fn database_exists(&self) -> AppResult<bool> {
        file_exists_no_follow(&self.database)
    }
}

fn demo_key() -> AppResult<LockedKey> {
    let bytes = hex::decode(DEMO_KEY_HEX).map_err(|_| AppError::Internal)?;
    LockedKey::from_slice(&bytes)
}

fn demo_settings() -> Settings {
    let mut settings = Settings::default();
    settings.onboarding_completed = true;
    settings.theme = crate::storage::settings::Theme::System;
    settings.note_demo();
    settings
}

fn demo_security(conn: &rusqlite::Connection, cipher: &CipherInfo) -> AppResult<SecuritySummary> {
    let meta = vault_meta::load(conn)?;
    Ok(SecuritySummary {
        cipher_version: cipher.version.clone(),
        cipher_provider: cipher.provider.clone(),
        kdf_summary: "Demo mode: fixed synthetic key; no owner passphrase".to_string(),
        generation: meta.generation,
        schema_version: meta.schema_version,
        last_integrity_check_at: meta.last_integrity_check_at,
        vault_id_prefix: meta.vault_id.chars().take(8).collect(),
        cipher_status_supported: cipher.cipher_status_supported,
    })
}

fn screenshot_title<R: tauri::Runtime>(app: &tauri::AppHandle<R>, demo: bool) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_title(if demo {
            "cardigan_money — Demo (synthetic data)"
        } else {
            "cardigan_money"
        });
        let _ = window.set_content_protected(true);
    }
}

fn cleanup_demo_files(paths: &DemoPaths) {
    let _ = std::fs::remove_file(&paths.database);
    let _ = std::fs::remove_file(format!("{}-wal", paths.database.display()));
    let _ = std::fs::remove_file(format!("{}-shm", paths.database.display()));
    if let Ok(entries) = std::fs::read_dir(&paths.root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("pre-migration-") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

fn seed_fresh(conn: &rusqlite::Connection, key: &LockedKey, root: &Path) -> AppResult<CipherInfo> {
    let cipher = cipher_summary(conn)?;
    let schema = run_migrations(conn, key, root)?;
    insert_demo_metadata(conn, &cipher, schema)?;
    save_settings(conn, &demo_settings())?;
    crate::demo::fixtures::seed(conn)?;
    record_event(conn, "demo_reset", "success", None)?;
    vault_meta::set_integrity_checked(conn)?;
    cipher_integrity_check(conn)?;
    sqlite_integrity_check(conn)?;
    Ok(cipher)
}

fn open_existing(
    conn: &rusqlite::Connection,
    key: &LockedKey,
    root: &Path,
) -> AppResult<CipherInfo> {
    cipher_integrity_check(conn)?;
    sqlite_integrity_check(conn)?;
    let meta = vault_meta::load(conn)?;
    if meta.vault_id != fixtures::DEMO_VAULT_ID {
        // Never treat a real (or foreign) database as demo data.
        return Err(AppError::IntegrityFailure);
    }
    let schema = run_migrations(conn, key, root)?;
    vault_meta::set_schema_version(conn, schema)?;
    vault_meta::set_integrity_checked(conn)?;
    cipher_summary(conn)
}

fn finish_open<R: tauri::Runtime>(
    state: &AppState,
    app: &tauri::AppHandle<R>,
    demo_paths: &DemoPaths,
    key: LockedKey,
    conn: rusqlite::Connection,
    cipher: CipherInfo,
    settings: Settings,
) -> AppResult<VaultState> {
    let security = demo_security(&conn, &cipher)?;
    {
        let mut session = state.session.lock().map_err(|_| AppError::Internal)?;
        session.clear();
        session.mode = SessionMode::Demo;
        session.conn = Some(conn);
        session.dek = Some(key);
        session.cipher = Some(cipher);
        session.settings = Some(settings.clone());
        session.unlocked_at = Some(std::time::Instant::now());
    }
    let _ = demo_paths;
    screenshot_title(app, true);
    Ok(VaultState {
        exists: true,
        unlocked: true,
        mode: "demo",
        settings: Some(settings),
        security: Some(security),
    })
}

pub fn open<R: tauri::Runtime>(state: &AppState, app: &tauri::AppHandle<R>) -> AppResult<VaultState> {
    {
        let guard = state.session.lock().map_err(|_| AppError::Internal)?;
        if guard.is_unlocked() && guard.mode == SessionMode::Demo {
            drop(guard);
            return current_state(state);
        }
    }
    // Opening demo closes any real session; the real vault stays untouched.
    state.lock(app, "demo_switch");

    let paths = DemoPaths::new(&state.app_data);
    paths.ensure()?;
    let key = demo_key()?;
    let exists = paths.database_exists()?;
    let (conn, _initial_cipher) = match open_ledger(&paths.database, &key, !exists) {
        Ok(opened) => opened,
        Err(error) => return Err(error),
    };

    let initialized = (|| -> AppResult<(CipherInfo, Settings)> {
        let cipher = if exists {
            open_existing(&conn, &key, &paths.root)?
        } else {
            seed_fresh(&conn, &key, &paths.root)?
        };
        let settings = load_settings(&conn)?;
        Ok((cipher, settings))
    })();

    match initialized {
        Ok((cipher, settings)) => finish_open(state, app, &paths, key, conn, cipher, settings),
        Err(error) => {
            drop(conn);
            if !exists {
                cleanup_demo_files(&paths);
            }
            Err(error)
        }
    }
}

fn current_state(state: &AppState) -> AppResult<VaultState> {
    let guard = state.session.lock().map_err(|_| AppError::Internal)?;
    let (Some(conn), Some(cipher), Some(settings)) = (
        guard.conn.as_ref(),
        guard.cipher.as_ref(),
        guard.settings.as_ref(),
    ) else {
        return Err(AppError::Locked);
    };
    Ok(VaultState {
        exists: true,
        unlocked: true,
        mode: "demo",
        settings: Some(settings.clone()),
        security: Some(demo_security(conn, cipher)?),
    })
}

pub fn close<R: tauri::Runtime>(state: &AppState, app: &tauri::AppHandle<R>) -> VaultState {
    state.lock(app, "demo_close");
    screenshot_title(app, false);
    crate::services::vault::status(state)
}

/// Delete and recreate only the demo database. The real vault is never touched.
pub fn reset<R: tauri::Runtime>(
    state: &AppState,
    app: &tauri::AppHandle<R>,
) -> AppResult<VaultState> {
    {
        let mut session = state.session.lock().map_err(|_| AppError::Internal)?;
        if session.mode == SessionMode::Demo {
            session.clear();
        }
    }
    let paths = DemoPaths::new(&state.app_data);
    paths.ensure()?;
    cleanup_demo_files(&paths);
    open(state, app)
}

pub fn demo_today() -> chrono::NaiveDate {
    fixtures::demo_today()
}

pub fn default_schema_version() -> u32 {
    supported_schema_version()
}

pub fn touch_title<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    screenshot_title(app, true);
}

pub fn now_timestamp() -> String {
    now_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_key_is_fixed_and_32_bytes() {
        let key = demo_key().unwrap();
        assert_eq!(key.expose().len(), 32);
        assert_eq!(hex::encode(key.expose()), DEMO_KEY_HEX);
        assert_ne!(DEMO_KEY_HEX, "0".repeat(64));
    }

    #[test]
    fn demo_paths_are_distinct_from_the_real_vault() {
        let root = Path::new("/tmp/cardigan-test");
        let demo = DemoPaths::new(root);
        let real = crate::security::vault::VaultPaths::new(root);
        assert_ne!(demo.database, real.database);
        assert!(demo.database.starts_with(root.join("demo")));
        assert!(real.database.starts_with(root.join("vault")));
    }
}
