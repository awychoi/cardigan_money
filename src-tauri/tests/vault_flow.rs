//! End-to-end vault lifecycle tests on real encrypted databases.
//! These use the service layer directly (native prompts are covered by the
//! native smoke checklist); passphrases are passed as zeroizing values.

use std::os::unix::fs::PermissionsExt;

use cardigan_money_lib::error::AppError;
use cardigan_money_lib::security::cipher::rekey_ledger;
use cardigan_money_lib::security::crypto::SecretString;
use cardigan_money_lib::security::vault::{
    load_header, load_journal, write_journal, RotationJournal,
};
use cardigan_money_lib::services::vault::{change_passphrase, create, status, unlock};
use cardigan_money_lib::state::AppState;

const PASS: &str = "correct horse battery staple test";
const NEW_PASS: &str = "a new correct horse battery staple";

fn pass(value: &str) -> SecretString {
    SecretString::new(value.to_string())
}

fn mock_app() -> tauri::AppHandle<tauri::test::MockRuntime> {
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("mock app");
    app.handle().clone()
}

fn setup() -> (
    tempfile::TempDir,
    AppState,
    tauri::AppHandle<tauri::test::MockRuntime>,
) {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::new(tmp.path().to_path_buf()).unwrap();
    let app = mock_app();
    (tmp, state, app)
}

fn lock(state: &AppState) {
    state.session.lock().unwrap().clear();
}

#[test]
fn created_vault_is_private_and_reopens_after_lock() {
    let (tmp, state, _app) = setup();
    let view = create(&state, pass(PASS)).unwrap();
    assert!(view.unlocked);
    assert!(view.settings.is_some());
    assert!(view.security.is_some());

    let vault_dir = tmp.path().join("vault");
    assert_eq!(
        std::fs::metadata(&vault_dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(vault_dir.join("vault.header"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(vault_dir.join("ledger.cmdb"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    for sidecar in ["-wal", "-shm"] {
        let path = vault_dir.join(format!("ledger.cmdb{sidecar}"));
        if path.exists() {
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600,
                "{sidecar} must be owner-only"
            );
        }
    }

    lock(&state);
    assert!(!status(&state).unlocked);
    let reopened = unlock(&state, pass(PASS)).unwrap();
    assert!(reopened.unlocked);
    assert!(reopened.security.unwrap().generation >= 1);
}

#[test]
fn wrong_passphrase_leaves_database_and_header_intact() {
    let (tmp, state, _app) = setup();
    create(&state, pass(PASS)).unwrap();
    lock(&state);
    let db = tmp.path().join("vault/ledger.cmdb");
    let header = tmp.path().join("vault/vault.header");
    let before_db = std::fs::read(&db).unwrap();
    let before_header = std::fs::read(&header).unwrap();
    let error = unlock(&state, pass("this passphrase is definitely wrong")).unwrap_err();
    assert!(matches!(error, AppError::WrongPassphrase));
    assert_eq!(std::fs::read(&db).unwrap(), before_db);
    assert_eq!(std::fs::read(&header).unwrap(), before_header);
    assert!(unlock(&state, pass(PASS)).is_ok());
}

#[test]
fn mirrored_header_recovers_a_damaged_primary() {
    let (tmp, state, _app) = setup();
    create(&state, pass(PASS)).unwrap();
    lock(&state);
    let primary = tmp.path().join("vault/vault.header");
    std::fs::write(&primary, b"not a vault header").unwrap();
    let view = unlock(&state, pass(PASS)).unwrap();
    assert!(view.unlocked);
    // The primary is repaired from the mirror after a successful unlock.
    let bytes = std::fs::read(&primary).unwrap();
    assert_eq!(bytes.len(), 152);
    assert!(
        load_header(&cardigan_money_lib::security::vault::VaultPaths::new(
            tmp.path()
        ))
        .is_ok()
    );
}

#[test]
fn tampered_or_missing_headers_fail_closed_without_touching_the_db() {
    let (tmp, state, _app) = setup();
    create(&state, pass(PASS)).unwrap();
    lock(&state);
    let paths = cardigan_money_lib::security::vault::VaultPaths::new(tmp.path());
    let db = std::fs::read(&paths.database).unwrap();
    std::fs::write(&paths.header, [0u8; 152]).unwrap();
    std::fs::write(&paths.header_mirror, [1u8; 152]).unwrap();
    let error = unlock(&state, pass(PASS)).unwrap_err();
    assert!(matches!(
        error,
        AppError::HeaderInvalid(_) | AppError::WrongPassphrase
    ));
    assert_eq!(std::fs::read(&paths.database).unwrap(), db);

    std::fs::remove_file(&paths.header).unwrap();
    std::fs::remove_file(&paths.header_mirror).unwrap();
    assert!(matches!(
        unlock(&state, pass(PASS)),
        Err(AppError::VaultMissing)
    ));
    assert_eq!(std::fs::read(&paths.database).unwrap(), db);
}

#[test]
fn passphrase_rotation_changes_the_dek_and_rejects_the_old_one() {
    let (tmp, state, app) = setup();
    create(&state, pass(PASS)).unwrap();
    let before = status(&state).security.unwrap();
    change_passphrase(&state, &app, pass(PASS), pass(NEW_PASS)).unwrap();
    let after = status(&state).security.unwrap();
    assert_eq!(after.generation, before.generation + 1);

    lock(&state);
    assert!(matches!(
        unlock(&state, pass(PASS)),
        Err(AppError::WrongPassphrase)
    ));
    let view = unlock(&state, pass(NEW_PASS)).unwrap();
    assert!(view.unlocked);
    assert_eq!(view.security.unwrap().generation, after.generation);

    // A pre-rekey snapshot and old header copy exist for recovery.
    let names: Vec<String> = std::fs::read_dir(tmp.path().join("vault"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert!(names
        .iter()
        .any(|n| n.starts_with("pre-rekey-") && n.ends_with(".cmdb")));
    assert!(names
        .iter()
        .any(|n| n.starts_with("pre-rekey-") && n.ends_with(".vault-header")));
    assert!(!names.iter().any(|n| n == "rotation.journal"));
}

#[test]
fn rotation_journal_recovers_a_crash_after_rekey() {
    let (tmp, state, _app) = setup();
    create(&state, pass(PASS)).unwrap();
    {
        let guard = state.session.lock().unwrap();
        let header = guard.header.clone().unwrap();
        let conn = guard.conn.as_ref().unwrap();
        // Simulate: pre-change backup done, journal written, DB rekeyed, then
        // crash before the header/metadata update.
        let (new_header, new_dek) = header.rotate(&pass(NEW_PASS), header.params).unwrap();
        let paths = cardigan_money_lib::security::vault::VaultPaths::new(tmp.path());
        write_journal(&paths, &RotationJournal::new(header.generation, new_header)).unwrap();
        rekey_ledger(conn, &new_dek).unwrap();
        // Do not update metadata or the header; this is the crash window.
    }
    lock(&state);
    assert!(
        load_journal(&cardigan_money_lib::security::vault::VaultPaths::new(
            tmp.path()
        ))
        .unwrap()
        .is_some()
    );

    // The old passphrase cannot open the rekeyed DB; the journal header uses
    // the new passphrase.
    assert!(unlock(&state, pass(PASS)).is_err());
    let view = unlock(&state, pass(NEW_PASS)).unwrap();
    assert!(view.unlocked);
    let paths = cardigan_money_lib::security::vault::VaultPaths::new(tmp.path());
    assert_eq!(load_header(&paths).unwrap().header().generation, 2);
    assert!(load_journal(&paths).unwrap().is_none());

    lock(&state);
    assert!(unlock(&state, pass(NEW_PASS)).is_ok());
}

#[test]
fn migration_checksum_mismatch_fails_closed() {
    let (tmp, state, _app) = setup();
    create(&state, pass(PASS)).unwrap();
    state
        .with_db(|conn, _header, _settings| {
            conn.execute("UPDATE schema_migrations SET checksum = 'tampered'", [])
                .map_err(|_| AppError::Database)?;
            Ok(())
        })
        .unwrap();
    lock(&state);
    let error = unlock(&state, pass(PASS)).unwrap_err();
    assert!(matches!(error, AppError::Database));
    assert!(!status(&state).unlocked);
    // The encrypted database was not deleted or replaced.
    assert!(tmp.path().join("vault/ledger.cmdb").exists());
}

#[test]
fn lock_closes_connections_zeroizes_session_and_records_the_event() {
    let (_tmp, state, app) = setup();
    create(&state, pass(PASS)).unwrap();
    assert!(status(&state).unlocked);
    assert!(state.lock(&app, "manual"));
    {
        let guard = state.session.lock().unwrap();
        assert!(guard.conn.is_none(), "connection must be closed");
        assert!(guard.dek.is_none(), "DEK must be dropped");
        assert!(guard.header.is_none());
        assert!(guard.cipher.is_none());
        assert!(guard.settings.is_none());
    }
    assert!(!status(&state).unlocked);
    // Locking twice is a no-op.
    assert!(!state.lock(&app, "manual"));
    let view = unlock(&state, pass(PASS)).unwrap();
    assert!(view.unlocked);
    let events = cardigan_money_lib::services::vault::security_events(&state, 20).unwrap();
    assert!(events.iter().any(|event| event.event_type == "lock"));
    assert!(events
        .iter()
        .any(|event| event.event_type == "unlock_success"));
}

#[test]
fn initial_schema_version_is_recorded_and_migrations_are_checksummed() {
    let (_tmp, state, _app) = setup();
    let view = create(&state, pass(PASS)).unwrap();
    assert_eq!(view.security.as_ref().unwrap().schema_version, 4);
    state
        .with_db(|conn, _header, _settings| {
            let (version, name, checksum): (u32, String, String) = conn
                .query_row(
                    "SELECT version, name, checksum FROM schema_migrations ORDER BY version DESC LIMIT 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(|_| AppError::Database)?;
            assert_eq!(version, 4);
            assert!(!name.is_empty());
            assert_eq!(checksum.len(), 64);
            Ok(())
        })
        .unwrap();
}

#[test]
fn settings_survive_lock_and_unlock() {
    let (_tmp, state, _app) = setup();
    create(&state, pass(PASS)).unwrap();
    state
        .with_db(|conn, _header, current| {
            let mut next = current.clone();
            next.theme = cardigan_money_lib::storage::settings::Theme::Dark;
            next.auto_lock_minutes = 30;
            next.close_behavior = cardigan_money_lib::storage::settings::CloseBehavior::Hide;
            next.onboarding_completed = true;
            cardigan_money_lib::storage::settings::save_settings(conn, &next)
        })
        .unwrap();
    lock(&state);
    let view = unlock(&state, pass(PASS)).unwrap();
    let settings = view.settings.unwrap();
    assert_eq!(
        settings.theme,
        cardigan_money_lib::storage::settings::Theme::Dark
    );
    assert_eq!(settings.auto_lock_minutes, 30);
    assert_eq!(
        settings.close_behavior,
        cardigan_money_lib::storage::settings::CloseBehavior::Hide
    );
    assert!(settings.onboarding_completed);
}

#[test]
fn updated_settings_survive_relock_and_are_validated() {
    let (_tmp, state, _app) = setup();
    create(&state, pass(PASS)).unwrap();
    let invalid = state.with_db(|conn, _header, current| {
        let mut next = current.clone();
        next.auto_lock_minutes = 0;
        cardigan_money_lib::storage::settings::save_settings(conn, &next)
    });
    assert!(invalid.is_err(), "invalid auto-lock must not be persisted");
    let settings = unlock(&state, pass(PASS)).unwrap().settings.unwrap();
    assert_eq!(settings.auto_lock_minutes, 15);
}
