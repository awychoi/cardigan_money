//! Unified "Back up now": explicit destination, optional passphrase, locked
//! backup without a UI session, listing rules, and retention.

use std::sync::Arc;

use cardigan_money_lib::error::AppError;
use cardigan_money_lib::security::crypto::SecretString;
use cardigan_money_lib::security::keychain::MemoryStore;
use cardigan_money_lib::services::backups::{
    self, create_manual_backup_to, list_backups, prune_backups_for_vault,
};
use cardigan_money_lib::services::restore::{
    execute_restore_borrowed, RestoreOptions, RestoreScope,
};
use cardigan_money_lib::services::vault;
use cardigan_money_lib::state::AppState;

const PASS: &str = "correct horse battery staple backup now";
const CUSTOM: &str = "a separate backup passphrase for this file";

fn pass(value: &str) -> SecretString {
    SecretString::new(value.to_string())
}

fn setup() -> (tempfile::TempDir, AppState) {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = AppState::new(tmp.path().to_path_buf()).unwrap();
    state.credentials = Arc::new(MemoryStore::default());
    vault::create(&state, pass(PASS)).unwrap();
    (tmp, state)
}

fn set_marker(state: &AppState, value: &str) {
    state
        .with_db(|conn, _h, _s| {
            conn.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES ('marker', ?1, '2026-10-23T12:00:00Z') \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [format!("\"{value}\"")],
            )
            .map_err(|_| AppError::Database)?;
            Ok(())
        })
        .unwrap();
}

fn marker(state: &AppState) -> Option<String> {
    state
        .with_db(|conn, _h, _s| {
            let value: Option<String> = conn
                .query_row(
                    "SELECT value FROM settings WHERE key = 'marker'",
                    [],
                    |row| row.get(0),
                )
                .ok();
            Ok(value)
        })
        .ok()
        .flatten()
}

fn destination(state: &AppState, nonce: usize) -> std::path::PathBuf {
    backups::backups_dir(state).join(format!(
        "cm-backup-manual-deadbeef-gen1-20261024T1200{:02}Z-{:04}.cmbak",
        nonce % 60,
        nonce
    ))
}

fn restore(state: &AppState, path: &std::path::Path, backup_pass: &str) -> bool {
    let options = RestoreOptions {
        scope: RestoreScope::Full,
        allow_non_private: false,
        confirmation: Some("replace".into()),
        plaid_choice: None,
        destructive: false,
    };
    execute_restore_borrowed(state, path, &pass(backup_pass), None, options, false)
        .map(|result| result.restored)
        .unwrap_or(false)
}

#[test]
fn unlocked_blank_and_custom_passphrase_backups_are_restorable() {
    let (_tmp, state) = setup();
    set_marker(&state, "blank");
    let blank = create_manual_backup_to(&state, &destination(&state, 1), pass(""), None).unwrap();
    assert!(blank.listed);
    set_marker(&state, "after-blank");
    assert!(restore(&state, &destination(&state, 1), PASS));
    assert_eq!(marker(&state), Some("\"blank\"".to_string()));

    set_marker(&state, "custom");
    let custom =
        create_manual_backup_to(&state, &destination(&state, 2), pass(CUSTOM), None).unwrap();
    assert!(custom.listed);
    set_marker(&state, "after-custom");
    // The vault passphrase cannot decrypt the custom backup.
    assert!(!restore(&state, &destination(&state, 2), PASS));
    assert!(restore(&state, &destination(&state, 2), CUSTOM));
    assert_eq!(marker(&state), Some("\"custom\"".to_string()));
    // The current vault passphrase is unchanged.
    state.session.lock().unwrap().clear();
    assert!(vault::unlock(&state, pass(PASS)).is_ok());
}

#[test]
fn locked_backup_requires_the_current_passphrase_and_never_unlocks() {
    let (_tmp, state) = setup();
    set_marker(&state, "before");
    state.session.lock().unwrap().clear();
    let locked = destination(&state, 3);
    // No vault passphrase: not allowed while locked.
    assert_eq!(
        create_manual_backup_to(&state, &locked, pass(""), None)
            .unwrap_err()
            .code(),
        "vault_locked"
    );
    assert!(!locked.exists());
    // Wrong vault passphrase: rejected before any file is written.
    assert_eq!(
        create_manual_backup_to(
            &state,
            &locked,
            pass(""),
            Some(pass("wrong passphrase long enough")),
        )
        .unwrap_err()
        .code(),
        "wrong_passphrase"
    );
    assert!(!locked.exists());
    // Correct vault passphrase: the backup is created while the app stays locked.
    let created = create_manual_backup_to(&state, &locked, pass(""), Some(pass(PASS))).unwrap();
    assert!(created.listed);
    assert!(!state.session.lock().unwrap().is_unlocked());
    assert!(vault::unlock(&state, pass(PASS)).is_ok());
    assert_eq!(marker(&state), Some("\"before\"".to_string()));
    // A locked custom-passphrase backup is also possible without unlocking.
    let locked_custom = destination(&state, 4);
    state.session.lock().unwrap().clear();
    assert!(
        create_manual_backup_to(&state, &locked_custom, pass(CUSTOM), Some(pass(PASS))).is_ok()
    );
    assert!(!state.session.lock().unwrap().is_unlocked());
    assert!(vault::unlock(&state, pass(PASS)).is_ok());
}

#[test]
fn custom_destination_is_not_listed_or_pruned_and_no_temp_litter_remains() {
    let (_tmp, state) = setup();
    let outside = _tmp.path().join("outside.cmbak");
    let created = create_manual_backup_to(&state, &outside, pass(""), None).unwrap();
    assert!(!created.listed);
    assert!(outside.exists());
    assert!(list_backups(&state).unwrap().is_empty());
    assert_eq!(
        prune_backups_for_vault(&backups::backups_dir(&state), None).unwrap(),
        0
    );
    assert!(outside.exists());
    // No snapshot temp file is left behind.
    let snapshot_leftovers = std::fs::read_dir(&state.paths.root)
        .unwrap()
        .flatten()
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".backup-snapshot-")
        });
    assert!(!snapshot_leftovers);
}

#[test]
fn default_manual_retention_keeps_ten_and_never_the_last_one() {
    let (_tmp, state) = setup();
    for nonce in 0..12 {
        create_manual_backup_to(&state, &destination(&state, nonce), pass(""), None).unwrap();
    }
    let manual = list_backups(&state)
        .unwrap()
        .into_iter()
        .filter(|item| item.kind == "manual")
        .count();
    assert_eq!(manual, 10);
}
