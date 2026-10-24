//! Encrypted backup container, restore, portable export, retention, and erase.

use std::os::unix::fs::PermissionsExt;

use cardigan_money_lib::security::cipher::open_ledger;
use cardigan_money_lib::security::crypto::{LockedKey, SecretString};
use cardigan_money_lib::services::backups::{
    self, BackupKind, create_backup, create_backup_inner, crypto_erase_confirmed, install_restore_confirmed,
    list_backups, portable_export_to, prepare_restore, prune_backups,
};
use cardigan_money_lib::services::vault;
use cardigan_money_lib::state::AppState;
use cardigan_money_lib::storage::accounts::{create_manual, AccountInput, AccountType};

const PASS: &str = "correct horse battery staple backups";
const EXPORT_PASS: &str = "a separate portable export passphrase";

fn mock_app() -> tauri::AppHandle<tauri::test::MockRuntime> {
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("mock app");
    app.handle().clone()
}

fn setup() -> (tempfile::TempDir, AppState, tauri::AppHandle<tauri::test::MockRuntime>, String) {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::new(tmp.path().to_path_buf()).unwrap();
    vault::create(&state, SecretString::new(PASS.into())).unwrap();
    let account = state
        .with_db(|conn, _h, _s| {
            create_manual(
                conn,
                &AccountInput {
                    issuer: "Backup Bank".into(),
                    display_name: "Backed Up Card".into(),
                    last_four: Some("4321".into()),
                    account_type: AccountType::CreditCard,
                    currency: "USD".into(),
                    credit_limit_minor: Some(500_000),
                    include_in_overview: true,
                },
            )
            .map(|account| account.id)
        })
        .unwrap();
    let app = mock_app();
    (tmp, state, app, account)
}

fn add_marker(state: &AppState, value: &str) {
    state
        .with_db(|conn, _h, _s| {
            conn.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES ('marker', ?1, '2026-10-23T12:00:00Z') \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [format!("\"{value}\"")],
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            Ok(())
        })
        .unwrap();
}

fn marker(state: &AppState) -> Option<String> {
    state
        .with_db(|conn, _h, _s| {
            let value: Option<String> = conn
                .query_row("SELECT value FROM settings WHERE key = 'marker'", [], |row| row.get(0))
                .ok();
            Ok(value)
        })
        .ok()
        .flatten()
}

#[test]
fn backup_is_self_contained_private_and_tamper_evident() {
    let (_tmp, state, app, _account) = setup();
    let result = create_backup(&state, &app, BackupKind::Manual).unwrap();
    let dir = backups::backups_dir(&state);
    let path = dir.join(&result.backup.id);
    assert!(path.exists());
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let list = list_backups(&state).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].kind, "manual");

    // A valid passphrase stages the backup.
    let plan = prepare_restore(&state, &path, &SecretString::new(PASS.into())).unwrap();
    assert!(plan.staged_path.exists());
    std::fs::remove_file(&plan.staged_path).unwrap();

    // Wrong passphrase fails before staging.
    let wrong = prepare_restore(&state, &path, &SecretString::new("wrong passphrase long enough".into()));
    assert!(wrong.is_err());
    assert!(!backups::backups_dir(&state).join("restore-staged").exists());

    // Tampered DB bytes are detected.
    let mut bytes = std::fs::read(&path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    let tampered = dir.join("cm-backup-manual-deadbeef-gen1-20260101T000000Z-0000.cmbak");
    std::fs::write(&tampered, &bytes).unwrap();
    std::fs::set_permissions(&tampered, std::fs::Permissions::from_mode(0o600)).unwrap();
    let result = prepare_restore(&state, &tampered, &SecretString::new(PASS.into()));
    assert!(result.is_err());
    std::fs::remove_file(&tampered).unwrap();

    // Truncation is rejected.
    let truncated = dir.join("cm-backup-manual-deadbeef-gen1-20260101T000001Z-0000.cmbak");
    std::fs::write(&truncated, &bytes[..100]).unwrap();
    std::fs::set_permissions(&truncated, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(prepare_restore(&state, &truncated, &SecretString::new(PASS.into())).is_err());
    std::fs::remove_file(&truncated).unwrap();

    // Owner-only permissions are required; the reader must not chmod a file the
    // owner selected. A symlink to a valid backup is rejected (O_NOFOLLOW).
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(prepare_restore(&state, &path, &SecretString::new(PASS.into())).is_err());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let link = dir.join("cm-backup-manual-deadbeef-gen1-20260101T000002Z-0000.cmbak");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(prepare_restore(&state, &link, &SecretString::new(PASS.into())).is_err());
    std::fs::remove_file(link).unwrap();
}

#[test]
fn restore_replaces_data_non_destructively_and_creates_pre_restore_backup() {
    let (_tmp, state, app, _account) = setup();
    add_marker(&state, "before");
    let backup = create_backup(&state, &app, BackupKind::Manual).unwrap();
    let path = backups::backups_dir(&state).join(&backup.backup.id);
    add_marker(&state, "after");
    assert_eq!(marker(&state), Some("\"after\"".to_string()));

    let plan = prepare_restore(&state, &path, &SecretString::new(PASS.into())).unwrap();
    let result = install_restore_confirmed(&state, plan).unwrap();
    assert!(result.restored);
    assert_eq!(marker(&state), Some("\"before\"".to_string()));
    let list = list_backups(&state).unwrap();
    assert!(list.iter().any(|item| item.kind == "pre-restore"));
    assert!(state.session.lock().unwrap().is_unlocked());
    // The restored ledger still opens with the current passphrase.
    assert!(vault::unlock(&state, SecretString::new(PASS.into())).is_ok());
}

#[test]
fn portable_export_uses_its_own_password_and_can_be_restored() {
    let (tmp, state, app, _account) = setup();
    add_marker(&state, "portable-before");
    let destination = tmp.path().join("portable.cmbak");
    let item = portable_export_to(
        &state,
        &destination,
        SecretString::new(EXPORT_PASS.into()),
    )
    .unwrap();
    assert_eq!(item.kind, "portable");
    assert!(destination.exists());
    assert_eq!(
        std::fs::metadata(&destination).unwrap().permissions().mode() & 0o777,
        0o600
    );
    add_marker(&state, "portable-after");
    // The ledger passphrase does not open the portable export.
    assert!(prepare_restore(&state, &destination, &SecretString::new(PASS.into())).is_err());
    let plan = prepare_restore(
        &state,
        &destination,
        &SecretString::new(EXPORT_PASS.into()),
    )
    .unwrap();
    install_restore_confirmed(&state, plan).unwrap();
    assert_eq!(marker(&state), Some("\"portable-before\"".to_string()));
    let _ = app;
}

#[test]
fn backup_from_a_different_vault_is_rejected() {
    let (_tmp1, state1, app1, _account1) = setup();
    let backup = create_backup(&state1, &app1, BackupKind::Manual).unwrap();
    let path1 = backups::backups_dir(&state1).join(&backup.backup.id);

    let (_tmp2, state2, _app2, _account2) = setup();
    let result = prepare_restore(&state2, &path1, &SecretString::new(PASS.into()));
    assert!(matches!(result, Err(cardigan_money_lib::error::AppError::Blocked(_))));
}

#[test]
fn restore_requires_an_unlocked_vault() {
    let (_tmp, state, app, _account) = setup();
    let backup = create_backup(&state, &app, BackupKind::Manual).unwrap();
    let path = backups::backups_dir(&state).join(&backup.backup.id);
    let plan = prepare_restore(&state, &path, &SecretString::new(PASS.into())).unwrap();
    state.session.lock().unwrap().clear();
    let result = install_restore_confirmed(&state, plan);
    assert!(matches!(result, Err(cardigan_money_lib::error::AppError::Locked)));
    assert!(!state.session.lock().unwrap().is_unlocked());
}

#[test]
fn retention_keeps_seven_daily_backups_and_never_the_last_one() {
    let (_tmp, state, app, _account) = setup();
    for _ in 0..9 {
        create_backup(&state, &app, BackupKind::Daily).unwrap();
    }
    let daily = list_backups(&state)
        .unwrap()
        .into_iter()
        .filter(|item| item.kind == "daily")
        .count();
    assert_eq!(daily, 7);
    // Pruning an otherwise empty directory does nothing.
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(prune_backups(empty.path()).unwrap(), 0);
}

#[test]
fn pre_change_backup_is_containerized_and_new_passphrase_still_works() {
    let (_tmp, state, app, _account) = setup();
    vault::change_passphrase(
        &state,
        &app,
        SecretString::new(PASS.into()),
        SecretString::new("a new correct horse battery staple".into()),
    )
    .unwrap();
    let backups = list_backups(&state).unwrap();
    assert!(backups.iter().any(|item| item.kind == "pre-change"));
    state.session.lock().unwrap().clear();
    assert!(vault::unlock(&state, SecretString::new(PASS.into())).is_err());
    assert!(vault::unlock(
        &state,
        SecretString::new("a new correct horse battery staple".into())
    )
    .is_ok());
}

#[test]
fn crypto_erase_removes_ledger_header_backups_and_demo_data() {
    let (tmp, state, app, _account) = setup();
    create_backup(&state, &app, BackupKind::Manual).unwrap();
    let vault_dir = tmp.path().join("vault");
    let backups_dir = tmp.path().join("backups");
    assert!(vault_dir.exists());
    assert!(backups_dir.exists());
    let status = crypto_erase_confirmed(&state, &app).unwrap();
    assert!(!status.unlocked);
    assert!(!status.exists);
    assert!(!vault_dir.exists());
    assert!(!backups_dir.exists());
    assert!(!tmp.path().join("demo").exists());
    assert!(matches!(
        vault::unlock(&state, SecretString::new(PASS.into())),
        Err(cardigan_money_lib::error::AppError::VaultMissing)
    ));
    // A stale key cannot open a removed database.
    let _ = LockedKey::generate().unwrap();
    let _ = open_ledger(&tmp.path().join("vault/ledger.cmdb"), &LockedKey::generate().unwrap(), false);
}

#[test]
fn restore_rechecks_mode_after_a_native_prompt_gap() {
    let (_tmp, state, app, _account) = setup();
    let backup = create_backup(&state, &app, BackupKind::Manual).unwrap();
    let path = backups::backups_dir(&state).join(&backup.backup.id);
    let plan = prepare_restore(&state, &path, &SecretString::new(PASS.into())).unwrap();
    state.session.lock().unwrap().mode = cardigan_money_lib::state::SessionMode::Demo;
    let result = install_restore_confirmed(&state, plan);
    assert!(matches!(result, Err(cardigan_money_lib::error::AppError::Blocked(_))));
    assert!(!backups::backups_dir(&state).join("restore-staged-").exists());
}
