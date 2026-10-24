//! Vault lifecycle: create, unlock (with rotation-journal recovery), lock, and
//! passphrase rotation. All key material stays in Rust memory.

use std::sync::atomic::Ordering;

use rusqlite::Connection;
use serde::Serialize;
use subtle::ConstantTimeEq;
use tauri::AppHandle;

use crate::error::{AppError, AppResult};
use crate::security::cipher::{
    cipher_integrity_check, cipher_summary, open_ledger, rekey_ledger, sqlite_integrity_check,
    CipherInfo,
};
use crate::security::crypto::{calibrate_kdf, random_vault_id, validate_new_passphrase, LockedKey, SecretString};
use crate::security::files::{
    atomic_write_private, file_exists_no_follow, remove_file_if_exists,
};
use crate::security::vault::{
    load_header, load_journal, remove_header, remove_journal, write_header, write_journal,
    RotationJournal, VaultHeader, VaultPaths,
};
use crate::state::AppState;
use crate::storage::events::{recent_events, record_event, SecurityEvent};
use crate::storage::migrations::{run_migrations, supported_schema_version};
use crate::storage::settings::{load_settings, save_settings, Settings};
use crate::storage::snapshot::create_encrypted_snapshot;
use crate::storage::time::now_rfc3339;
use crate::storage::vault_meta::{
    insert_initial, load as load_meta, set_generation, set_integrity_checked, set_schema_version,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecuritySummary {
    pub cipher_version: String,
    pub cipher_provider: String,
    pub kdf_summary: String,
    pub generation: u32,
    pub schema_version: u32,
    pub last_integrity_check_at: Option<String>,
    pub vault_id_prefix: String,
    pub cipher_status_supported: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultState {
    pub exists: bool,
    pub unlocked: bool,
    pub mode: &'static str,
    pub settings: Option<Settings>,
    pub security: Option<SecuritySummary>,
}

fn summary(conn: &Connection, header: &VaultHeader, cipher: &CipherInfo) -> AppResult<SecuritySummary> {
    let meta = load_meta(conn)?;
    Ok(SecuritySummary {
        cipher_version: cipher.version.clone(),
        cipher_provider: cipher.provider.clone(),
        kdf_summary: header.params.human_summary(),
        generation: header.generation,
        schema_version: meta.schema_version,
        last_integrity_check_at: meta.last_integrity_check_at,
        vault_id_prefix: header.vault_id_hex().chars().take(8).collect(),
        cipher_status_supported: cipher.cipher_status_supported,
    })
}

pub fn status(state: &AppState) -> VaultState {
    let exists = state.paths.exists().unwrap_or(false);
    let guard = match state.session.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return VaultState { exists, unlocked: false, mode: "real", settings: None, security: None }
        }
    };
    if let (Some(conn), Some(header), Some(cipher), Some(settings)) = (
        guard.conn.as_ref(),
        guard.header.as_ref(),
        guard.cipher.as_ref(),
        guard.settings.as_ref(),
    ) {
        VaultState {
            exists,
            unlocked: true,
            mode: "real",
            settings: Some(settings.clone()),
            security: summary(conn, header, cipher).ok(),
        }
    } else {
        VaultState { exists, unlocked: false, mode: "real", settings: None, security: None }
    }
}

fn session_state(state: &AppState) -> AppResult<VaultState> {
    let guard = state.session.lock().map_err(|_| AppError::Internal)?;
    let (Some(conn), Some(header), Some(cipher), Some(settings)) = (
        guard.conn.as_ref(),
        guard.header.as_ref(),
        guard.cipher.as_ref(),
        guard.settings.as_ref(),
    ) else {
        return Err(AppError::Locked);
    };
    Ok(VaultState {
        exists: true,
        unlocked: true,
        mode: "real",
        settings: Some(settings.clone()),
        security: Some(summary(conn, header, cipher)?),
    })
}

fn cleanup_failed_create(paths: &VaultPaths) {
    let _ = remove_header(paths);
    let _ = remove_file_if_exists(&paths.database);
    let _ = remove_file_if_exists(std::path::Path::new(&format!("{}-wal", paths.database.display())));
    let _ = remove_file_if_exists(std::path::Path::new(&format!("{}-shm", paths.database.display())));
    let _ = remove_journal(paths);
}

pub fn create(state: &AppState, passphrase: SecretString) -> AppResult<VaultState> {
    validate_new_passphrase(&passphrase)?;
    let mut guard = state.session.lock().map_err(|_| AppError::Internal)?;
    if guard.is_unlocked() {
        return Err(AppError::Blocked("vault is already unlocked"));
    }
    state.paths.ensure()?;
    if state.paths.exists()? || file_exists_no_follow(&state.paths.database)? {
        return Err(AppError::VaultExists);
    }

    let params = calibrate_kdf();
    let vault_id = random_vault_id()?;
    let (header, dek) = VaultHeader::create(
        &passphrase,
        params,
        vault_id,
        supported_schema_version(),
    )?;
    // Passphrase is no longer needed after the KEK derivation inside create().
    drop(passphrase);

    let (conn, cipher) = match open_ledger(&state.paths.database, &dek, true) {
        Ok(opened) => opened,
        Err(error) => {
            cleanup_failed_create(&state.paths);
            return Err(error);
        }
    };

    let initialized = (|| -> AppResult<Settings> {
        let schema = run_migrations(&conn, &dek, &state.paths.root)?;
        insert_initial(&conn, &header, &cipher, schema)?;
        let settings = Settings::default();
        save_settings(&conn, &settings)?;
        cipher_integrity_check(&conn)?;
        sqlite_integrity_check(&conn)?;
        record_event(&conn, "vault_created", "success", None)?;
        write_header(&state.paths, &header)?;
        Ok(settings)
    })();

    match initialized {
        Ok(settings) => {
            guard.header = Some(header);
            guard.conn = Some(conn);
            guard.cipher = Some(cipher);
            guard.dek = Some(dek);
            guard.settings = Some(settings);
            guard.unlocked_at = Some(std::time::Instant::now());
            drop(guard);
            state.unlock_failures.store(0, Ordering::SeqCst);
            session_state(state)
        }
        Err(error) => {
            drop(conn);
            cleanup_failed_create(&state.paths);
            Err(error)
        }
    }
}

enum CandidateError {
    WrongKey,
    BadDatabase,
    Fatal(AppError),
}

struct Opened {
    header: VaultHeader,
    dek: LockedKey,
    conn: Connection,
    cipher: CipherInfo,
}

fn try_candidate(
    header: &VaultHeader,
    passphrase: &SecretString,
    path: &std::path::Path,
) -> Result<Opened, CandidateError> {
    let dek = match header.unwrap_dek(passphrase) {
        Ok(dek) => dek,
        Err(AppError::WrongPassphrase) => return Err(CandidateError::WrongKey),
        Err(error) => return Err(CandidateError::Fatal(error)),
    };
    let (conn, cipher) = match open_ledger(path, &dek, false) {
        Ok(opened) => opened,
        Err(_) => return Err(CandidateError::BadDatabase),
    };
    if cipher_integrity_check(&conn).is_err() || sqlite_integrity_check(&conn).is_err() {
        return Err(CandidateError::BadDatabase);
    }
    let meta = match load_meta(&conn) {
        Ok(meta) => meta,
        Err(_) => return Err(CandidateError::BadDatabase),
    };
    if meta.vault_id != header.vault_id_hex() {
        return Err(CandidateError::Fatal(AppError::IntegrityFailure));
    }
    if !meta.cipher_version.starts_with("4.") || meta.cipher_provider != cipher.provider {
        return Err(CandidateError::Fatal(AppError::IntegrityFailure));
    }
    Ok(Opened { header: header.clone(), dek, conn, cipher })
}

/// Unlock with a passphrase already collected by the native sheet. Non-destructive:
/// a wrong passphrase never creates, renames, resets, or overwrites anything.
pub fn unlock(state: &AppState, passphrase: SecretString) -> AppResult<VaultState> {
    {
        let guard = state.session.lock().map_err(|_| AppError::Internal)?;
        if guard.is_unlocked() {
            drop(guard);
            return session_state(state);
        }
    }

    // A corrupt or stale journal must not prevent unlocking a healthy vault.
    // A missing header may still be recoverable from a valid rotation journal.
    let journal = load_journal(&state.paths).ok().flatten();
    let loaded = load_header(&state.paths);
    let (primary, loaded_from_mirror) = match &loaded {
        Ok(loaded) => (Some(loaded.header().clone()), loaded.from_mirror()),
        Err(error) => {
            if journal.is_some() {
                (None, false)
            } else {
                return Err(match error {
                    AppError::VaultMissing => AppError::VaultMissing,
                    AppError::HeaderInvalid(_) => AppError::HeaderInvalid("no valid header"),
                    _ => AppError::HeaderInvalid("no valid header"),
                });
            }
        }
    };

    let mut candidates = Vec::new();
    if let Some(primary) = &primary {
        candidates.push(primary.clone());
    }
    if let Some(journal) = &journal {
        if primary
            .as_ref()
            .map(|primary| journal.new_header.generation != primary.generation)
            .unwrap_or(true)
        {
            candidates.push(journal.new_header.clone());
        }
    }
    candidates.sort_by_key(|header| std::cmp::Reverse(header.generation));

    let mut opened: Option<Opened> = None;
    let mut saw_wrong_key = false;
    for candidate in &candidates {
        match try_candidate(candidate, &passphrase, &state.paths.database) {
            Ok(open) => {
                opened = Some(open);
                break;
            }
            Err(CandidateError::WrongKey) => saw_wrong_key = true,
            // A rekey may have completed before the header was replaced; the
            // journal candidate is tried next.
            Err(CandidateError::BadDatabase) => {}
            Err(CandidateError::Fatal(error)) => return Err(error),
        }
    }
    drop(passphrase);

    let Some(opened) = opened else {
        if saw_wrong_key {
            let failures = state.unlock_failures.fetch_add(1, Ordering::SeqCst) + 1;
            // Exponential delay after repeated failures, capped; no permanent
            // lockout (denial-of-service / data-loss avoidance).
            if failures >= 3 {
                let delay_ms = (250u64 << (failures - 3).min(4)).min(4_000);
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            }
            return Err(AppError::WrongPassphrase);
        }
        return Err(AppError::IntegrityFailure);
    };

    let meta = load_meta(&opened.conn)?;
    let is_journal_header = journal
        .as_ref()
        .map(|journal| {
            journal.new_header.generation == opened.header.generation
                && journal.new_header.wrapped_dek == opened.header.wrapped_dek
        })
        .unwrap_or(false);

    if meta.generation != opened.header.generation {
        let recoverable = is_journal_header
            && journal
                .as_ref()
                .map(|journal| meta.generation == journal.previous_generation)
                .unwrap_or(false);
        if !recoverable {
            return Err(AppError::IntegrityFailure);
        }
        // The rekey committed but the header/metadata update was interrupted.
        set_generation(&opened.conn, opened.header.generation, true)?;
    }

    let schema = run_migrations(&opened.conn, &opened.dek, &state.paths.root)?;
    set_schema_version(&opened.conn, schema)?;
    set_integrity_checked(&opened.conn)?;

    // Finalize header/journal state only after the database checks passed.
    if loaded_from_mirror || is_journal_header || primary.is_none() {
        write_header(&state.paths, &opened.header)?;
    }
    // Any journal that remains after a confirmed open is stale (crash before
    // rekey) or has just been finalized. Remove it unconditionally.
    if state.paths.journal.exists() {
        remove_journal(&state.paths)?;
    }

    let pending_failures = state.unlock_failures.swap(0, Ordering::SeqCst);
    if pending_failures > 0 {
        let _ = record_event(
            &opened.conn,
            "unlock_failure",
            "failure",
            Some(serde_json::json!({ "count": pending_failures })),
        );
    }
    record_event(&opened.conn, "unlock_success", "success", None)?;
    let settings = load_settings(&opened.conn)?;

    {
        let mut guard = state.session.lock().map_err(|_| AppError::Internal)?;
        guard.clear();
        guard.header = Some(opened.header);
        guard.conn = Some(opened.conn);
        guard.cipher = Some(opened.cipher);
        guard.dek = Some(opened.dek);
        guard.settings = Some(settings);
        guard.unlocked_at = Some(std::time::Instant::now());
    }
    session_state(state)
}

/// Verify the current passphrase against the live DEK in constant time.
fn verify_current(
    header: &VaultHeader,
    dek: &LockedKey,
    passphrase: &SecretString,
) -> AppResult<()> {
    let candidate = header.unwrap_dek(passphrase).map_err(|_| AppError::WrongPassphrase)?;
    let matches: bool = candidate.expose().ct_eq(dek.expose()).into();
    if matches {
        Ok(())
    } else {
        Err(AppError::WrongPassphrase)
    }
}

/// Rotate the passphrase and DEK with a journal and pre-change snapshots.
/// On any failure after the rekey starts, the session locks rather than keeping
/// a stale key; the journal makes the new passphrase recoverable.
pub fn change_passphrase<R: tauri::Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    current: SecretString,
    new: SecretString,
) -> AppResult<()> {
    validate_new_passphrase(&new)?;
    let mut guard = state.session.lock().map_err(|_| AppError::Internal)?;
    let conn = guard.conn.as_ref().ok_or(AppError::Locked)?;
    let header = guard.header.as_ref().ok_or(AppError::Locked)?.clone();
    let dek = guard.dek.as_ref().ok_or(AppError::Locked)?;

    verify_current(&header, dek, &current)?;
    drop(current);

    let stamp = now_rfc3339().replace([':', '-'], "");
    let snapshot = state.paths.root.join(format!("pre-rekey-{stamp}.cmdb"));
    create_encrypted_snapshot(conn, &snapshot, dek)?;
    let header_copy = state.paths.root.join(format!("pre-rekey-{stamp}.vault-header"));
    atomic_write_private(&header_copy, &header.encode())?;

    let (new_header, new_dek) = header.rotate(&new, header.params)?;
    drop(new);
    let journal = RotationJournal::new(header.generation, new_header.clone());
    write_journal(&state.paths, &journal)?;

    // From here on, an error must not leave the session with the old DEK.
    let rekey_result = (|| -> AppResult<()> {
        rekey_ledger(conn, &new_dek)?;
        set_generation(conn, new_header.generation, true)?;
        cipher_integrity_check(conn)?;
        write_header(&state.paths, &new_header)?;
        remove_journal(&state.paths)?;
        record_event(
            conn,
            "rekey",
            "success",
            Some(serde_json::json!({ "generation": new_header.generation })),
        )?;
        Ok(())
    })();

    match rekey_result {
        Ok(()) => {
            let cipher = cipher_summary(conn)?;
            guard.header = Some(new_header);
            guard.dek = Some(new_dek);
            guard.cipher = Some(cipher);
            Ok(())
        }
        Err(error) => {
            guard.clear();
            drop(guard);
            state.lock(app, "rekey_failure");
            Err(error)
        }
    }
}

pub fn security_events(state: &AppState, limit: u32) -> AppResult<Vec<SecurityEvent>> {
    state.with_db(|conn, _header, _settings| recent_events(conn, limit))
}

pub fn security_summary(state: &AppState) -> AppResult<SecuritySummary> {
    state.with_db(|conn, header, _settings| {
        let cipher = cipher_summary(conn)?;
        summary(conn, header, &cipher)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> (tempfile::TempDir, AppState) {
        let tmp = tempfile::tempdir().unwrap();
        let state = AppState::new(tmp.path().to_path_buf()).unwrap();
        (tmp, state)
    }

    fn pass(value: &str) -> SecretString {
        SecretString::new(value.to_string())
    }

    #[test]
    fn create_unlock_lock_cycle() {
        let (_tmp, state) = state();
        let created = create(&state, pass("correct horse battery staple test")).unwrap();
        assert!(created.unlocked);
        assert!(created.settings.is_some());
        assert_eq!(created.security.as_ref().unwrap().cipher_version.split(' ').next(), Some("4.6.1"));

        let settings = created.settings.unwrap();
        assert_eq!(settings.timezone, "America/New_York");

        let mut guard = state.session.lock().unwrap();
        guard.clear();
        drop(guard);
        assert!(!status(&state).unlocked);

        let unlocked = unlock(&state, pass("correct horse battery staple test")).unwrap();
        assert!(unlocked.unlocked);
        assert_eq!(unlocked.security.unwrap().vault_id_prefix.len(), 8);
    }

    #[test]
    fn wrong_passphrase_is_non_destructive() {
        let (_tmp, state) = state();
        create(&state, pass("correct horse battery staple test")).unwrap();
        {
            let mut guard = state.session.lock().unwrap();
            guard.clear();
        }
        let db = state.paths.database.clone();
        let before = std::fs::read(&db).unwrap();
        let err = unlock(&state, pass("wrong passphrase but long enough")).unwrap_err();
        assert_eq!(err.code(), "wrong_passphrase");
        let after = std::fs::read(&db).unwrap();
        assert_eq!(before, after);
        // Original passphrase still works.
        assert!(unlock(&state, pass("correct horse battery staple test")).is_ok());
    }

    #[test]
    fn missing_vault_does_not_create_one() {
        let (_tmp, state) = state();
        let err = unlock(&state, pass("correct horse battery staple test")).unwrap_err();
        assert_eq!(err.code(), "vault_missing");
        assert!(!state.paths.database.exists());
    }

    #[test]
    fn create_twice_is_rejected() {
        let (_tmp, state) = state();
        create(&state, pass("correct horse battery staple test")).unwrap();
        // While unlocked, creation is blocked without touching the files.
        let err = create(&state, pass("another correct horse battery staple")).unwrap_err();
        assert_eq!(err.code(), "blocked");
        {
            let mut guard = state.session.lock().unwrap();
            guard.clear();
        }
        let err = create(&state, pass("another correct horse battery staple")).unwrap_err();
        assert_eq!(err.code(), "vault_exists");
    }

    #[test]
    fn wrong_passphrase_policy_is_enforced_before_creation() {
        let (_tmp, state) = state();
        let err = create(&state, pass("short")).unwrap_err();
        assert_eq!(err.code(), "invalid_input");
        assert!(!state.paths.database.exists());
    }

    #[test]
    fn settings_patch_roundtrip() {
        let (_tmp, state) = state();
        create(&state, pass("correct horse battery staple test")).unwrap();
        let s = status(&state).settings.unwrap();
        let mut next = s.clone();
        next.theme = crate::storage::settings::Theme::Dark;
        next.auto_lock_minutes = 30;
        state
            .with_db(|conn, _h, _s| save_settings(conn, &next))
            .unwrap();
        let loaded = state.with_db(|conn, _h, _s| load_settings(conn)).unwrap();
        assert_eq!(loaded.theme, crate::storage::settings::Theme::Dark);
        assert_eq!(loaded.auto_lock_minutes, 30);
    }

    #[test]
    fn security_events_record_without_secrets() {
        let (_tmp, state) = state();
        create(&state, pass("correct horse battery staple test")).unwrap();
        let events = security_events(&state, 10).unwrap();
        assert!(events.iter().any(|event| event.event_type == "vault_created"));
        let text = serde_json::to_string(&events).unwrap().to_lowercase();
        assert!(!text.contains("passphrase"));
        assert!(!text.contains("correct horse"));
    }
}
