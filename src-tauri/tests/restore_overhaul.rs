//! Restore matrix: identity adoption, scopes, locked/sessionless restore,
//! destructive recovery, provider gating, and non-private acknowledgement.

use std::sync::Arc;

use cardigan_money_lib::error::AppError;
use cardigan_money_lib::security::crypto::SecretString;
use cardigan_money_lib::security::keychain::MemoryStore;
use cardigan_money_lib::services::backups::{
    self, create_backup, create_manual_backup_to, list_backups, list_retired_backups,
    prune_backups_for_vault, BackupKind,
};
use cardigan_money_lib::services::restore::{
    execute_restore_borrowed, inspect_restore, PlaidTokenChoice, RestoreOptions, RestoreScope,
};
use cardigan_money_lib::services::vault;
use cardigan_money_lib::state::AppState;
use cardigan_money_lib::storage::accounts::{create_manual, AccountInput, AccountType};
use cardigan_money_lib::storage::overview::overview;

const PASS: &str = "correct horse battery staple restore";
const OTHER_PASS: &str = "other correct horse battery staple restore";
const BACKUP_PASS: &str = "separate backup passphrase for restore";

fn mock_app() -> tauri::AppHandle<tauri::test::MockRuntime> {
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("mock app");
    app.handle().clone()
}

fn pass(value: &str) -> SecretString {
    SecretString::new(value.to_string())
}

fn setup(vault_pass: &str) -> (tempfile::TempDir, AppState) {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = AppState::new(tmp.path().to_path_buf()).unwrap();
    state.credentials = Arc::new(MemoryStore::default());
    vault::create(&state, pass(vault_pass)).unwrap();
    (tmp, state)
}

fn add_account(state: &AppState, name: &str) -> String {
    state
        .with_db(|conn, _h, _s| {
            create_manual(
                conn,
                &AccountInput {
                    issuer: "Restore Bank".into(),
                    display_name: name.into(),
                    last_four: Some("4242".into()),
                    account_type: AccountType::CreditCard,
                    currency: "USD".into(),
                    credit_limit_minor: Some(500_000),
                    include_in_overview: true,
                },
            )
            .map(|account| account.id)
        })
        .unwrap()
}

fn add_marker(state: &AppState, value: &str) {
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

fn count(state: &AppState, sql: &str) -> i64 {
    state
        .with_db(|conn, _h, _s| Ok(conn.query_row(sql, [], |row| row.get::<_, i64>(0))?))
        .unwrap()
}

fn make_backup(
    state: &AppState,
    app: &tauri::AppHandle<tauri::test::MockRuntime>,
) -> std::path::PathBuf {
    let result = create_backup(state, app, BackupKind::Manual).unwrap();
    backups::backups_dir(state).join(result.backup.id)
}

fn options(scope: RestoreScope) -> RestoreOptions {
    RestoreOptions {
        scope,
        allow_non_private: false,
        confirmation: Some("replace".into()),
        plaid_choice: None,
        destructive: false,
    }
}

fn options_with_plaid(scope: RestoreScope, choice: PlaidTokenChoice) -> RestoreOptions {
    RestoreOptions {
        plaid_choice: Some(choice),
        ..options(scope)
    }
}

fn add_plaid_connection(state: &AppState, credential_ref: &str) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    state
        .with_db(|conn, _h, _s| {
            conn.execute(
                "INSERT INTO connections (id, provider, environment, institution_id, institution_name, \
                    credential_ref, status, cursor, last_attempt_at, last_success_at, created_at, updated_at) \
                 VALUES (?1, 'plaid', 'sandbox', 'ins_1', 'Restore Bank', ?2, 'healthy', 'cursor-1', \
                    '2026-10-23T12:00:00Z', '2026-10-23T12:00:00Z', '2026-10-23T12:00:00Z', '2026-10-23T12:00:00Z')",
                rusqlite::params![id, credential_ref],
            )
            .map_err(|_| AppError::Database)?;
            Ok(())
        })
        .unwrap();
    id
}

fn add_provider_snapshot(state: &AppState, account_id: &str, connection_id: &str, balance: i64) {
    state
        .with_db(|conn, _h, _s| {
            conn.execute(
                "INSERT INTO balance_snapshots (id, account_id, current_balance_minor, available_credit_minor, \
                    credit_limit_minor, currency, retrieved_at, provenance, source_connection_id) \
                 VALUES (?1, ?2, ?3, 400000, 500000, 'USD', '2026-10-23T12:00:00Z', 'provider', ?4)",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    account_id,
                    balance,
                    connection_id
                ],
            )
            .map_err(|_| AppError::Database)?;
            Ok(())
        })
        .unwrap();
}

fn restore(
    state: &AppState,
    path: &std::path::Path,
    backup_pass: &str,
    current_pass: Option<&str>,
    options: RestoreOptions,
) -> Result<backups::RestoreResult, AppError> {
    let backup_pass = pass(backup_pass);
    let current = current_pass.map(pass);
    execute_restore_borrowed(state, path, &backup_pass, current.as_ref(), options, false)
}

fn lock(state: &AppState) {
    state.session.lock().unwrap().clear();
}

#[test]
fn same_vault_full_restore_keeps_identity_and_creates_pre_restore_backup() {
    let (_tmp, state) = setup(PASS);
    let app = mock_app();
    add_marker(&state, "before");
    let backup = make_backup(&state, &app);
    add_marker(&state, "after");
    let old_header = state.paths.header.clone();
    let old_header_bytes = std::fs::read(&old_header).unwrap();
    let vault_id = backups::current_vault_id(&state);

    let result = restore(&state, &backup, PASS, None, options(RestoreScope::Full)).unwrap();
    assert!(result.restored);
    assert!(!result.vault_id_changed);
    assert_eq!(result.pre_restore_protection, "current");
    assert!(result.pre_restore_backup_id.is_some());
    assert!(result.unlocked);
    assert_eq!(marker(&state), Some("\"before\"".to_string()));
    assert_eq!(backups::current_vault_id(&state), vault_id);
    assert_eq!(std::fs::read(&old_header).unwrap(), old_header_bytes);
    assert!(list_backups(&state)
        .unwrap()
        .iter()
        .any(|item| item.kind == "pre-restore"));
    // The current passphrase is still the only vault passphrase.
    lock(&state);
    assert!(vault::unlock(&state, pass(PASS)).is_ok());
}

#[test]
fn cross_vault_full_restore_adopts_identity_and_retires_old_backups() {
    let (_tmp_a, state_a) = setup(PASS);
    let app_a = mock_app();
    add_marker(&state_a, "source");
    let source_account = add_account(&state_a, "Source Card");
    let source_credential_ref = uuid::Uuid::new_v4().to_string();
    let source_connection = add_plaid_connection(&state_a, &source_credential_ref);
    add_provider_snapshot(&state_a, &source_account, &source_connection, 123_456);
    let backup = make_backup(&state_a, &app_a);
    let backup_vault_id = backups::current_vault_id(&state_a).unwrap();

    let (_tmp_b, state_b) = setup(OTHER_PASS);
    let app_b = mock_app();
    add_marker(&state_b, "target");
    let target_backup = make_backup(&state_b, &app_b);
    assert!(target_backup.exists());
    // Tokens for the previous vault. The token is associated with a connection
    // so the restore can discover its credential reference.
    let target_credential_ref = uuid::Uuid::new_v4().to_string();
    add_plaid_connection(&state_b, &target_credential_ref);
    let token_account =
        cardigan_money_lib::providers::plaid::access_token_account(&target_credential_ref);
    state_b
        .credentials
        .set(&token_account, &pass("old-access-token-value-123"))
        .unwrap();

    let result = restore(
        &state_b,
        &backup,
        PASS,
        Some(OTHER_PASS),
        options_with_plaid(RestoreScope::Full, PlaidTokenChoice::DeleteLocal),
    )
    .unwrap();
    assert!(result.vault_id_changed);
    assert_eq!(result.pre_restore_protection, "previous");
    assert!(result.retired_backups >= 1);
    assert!(result.unlocked);
    assert_eq!(marker(&state_b), Some("\"source\"".to_string()));
    assert_eq!(backups::current_vault_id(&state_b), Some(backup_vault_id));
    // The previous vault's normal containers are retired and excluded from the
    // current list and from pruning.
    assert!(list_backups(&state_b).unwrap().is_empty());
    let retired = list_retired_backups(&state_b).unwrap();
    assert!(!retired.is_empty());
    let dir = backups::backups_dir(&state_b);
    assert_eq!(
        prune_backups_for_vault(&dir, Some(&backup_vault_id)).unwrap(),
        0
    );
    assert!(!list_retired_backups(&state_b).unwrap().is_empty());
    // Old tokens followed the choice.
    assert!(state_b.credentials.get(&token_account).unwrap().is_none());
    // Restored connection starts reconnect_required with fresh credential refs,
    // and its provider snapshot is hidden from the overview.
    let (status, credential_ref, last_success): (String, String, Option<String>) = state_b
        .with_db(|conn, _h, _s| {
            Ok(conn.query_row(
                "SELECT status, credential_ref, last_success_at FROM connections WHERE institution_name = 'Restore Bank'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?)
        })
        .unwrap();
    assert_eq!(status, "reconnect_required");
    assert_ne!(credential_ref, source_credential_ref);
    assert!(last_success.is_none());
    let connections = cardigan_money_lib::services::sync::list_connections(&state_b).unwrap();
    assert_eq!(connections.len(), 1);
    assert!(!connections[0].has_token);
    assert_eq!(
        count(
            &state_b,
            "SELECT count(*) FROM balance_snapshots WHERE restored = 1"
        ),
        1
    );
    let overview = state_b
        .with_db(|conn, _h, _s| {
            overview(conn, chrono::NaiveDate::from_ymd_opt(2026, 10, 24).unwrap())
        })
        .unwrap();
    assert!(overview
        .accounts
        .iter()
        .all(|account| account.current_balance_minor.is_none()));
    // The previous passphrase no longer opens the vault; the backup passphrase
    // is now the vault passphrase.
    lock(&state_b);
    assert!(vault::unlock(&state_b, pass(OTHER_PASS)).is_err());
    assert!(vault::unlock(&state_b, pass(PASS)).is_ok());
}

#[test]
fn fresh_install_full_restore_creates_a_locked_vault_with_the_backup_identity() {
    let (_tmp_a, state_a) = setup(PASS);
    let app_a = mock_app();
    add_marker(&state_a, "source");
    let backup = make_backup(&state_a, &app_a);
    let backup_vault_id = backups::current_vault_id(&state_a).unwrap();

    let tmp = tempfile::tempdir().unwrap();
    let mut state = AppState::new(tmp.path().to_path_buf()).unwrap();
    state.credentials = Arc::new(MemoryStore::default());
    assert!(backups::current_vault_id(&state).is_none());

    let preview = inspect_restore(&state, &backup).unwrap();
    assert!(preview.fresh_install);
    assert!(!preview.same_vault);
    assert!(!preview.requires_typed_confirmation);

    let result = restore(&state, &backup, PASS, None, options(RestoreScope::Full)).unwrap();
    assert!(result.restored);
    assert!(!result.unlocked);
    assert!(result.pre_restore_backup_id.is_none());
    assert_eq!(result.pre_restore_protection, "none");
    assert_eq!(backups::current_vault_id(&state), Some(backup_vault_id));
    assert!(state.paths.root.exists());
    assert!(!state.session.lock().unwrap().is_unlocked());
    assert!(vault::unlock(&state, pass(PASS)).is_ok());
    assert_eq!(marker(&state), Some("\"source\"".to_string()));
}

#[test]
fn custom_backup_passphrase_only_decrypts_a_same_vault_backup() {
    let (_tmp, state) = setup(PASS);
    let app = mock_app();
    add_marker(&state, "before");
    let dir = backups::backups_dir(&state);
    let destination = dir.join("cm-backup-manual-deadbeef-gen1-20261024T120000Z-0000.cmbak");
    let created = create_manual_backup_to(&state, &destination, pass(BACKUP_PASS), None).unwrap();
    assert!(created.listed);
    add_marker(&state, "after");
    // The vault passphrase cannot open the custom backup.
    assert!(restore(
        &state,
        &destination,
        PASS,
        None,
        options(RestoreScope::Full)
    )
    .is_err());
    let result = restore(
        &state,
        &destination,
        BACKUP_PASS,
        None,
        options(RestoreScope::Full),
    )
    .unwrap();
    assert!(!result.vault_id_changed);
    assert_eq!(marker(&state), Some("\"before\"".to_string()));
    // The current vault passphrase still opens the restored vault.
    lock(&state);
    assert!(vault::unlock(&state, pass(PASS)).is_ok());
    lock(&state);
    assert!(vault::unlock(&state, pass(BACKUP_PASS)).is_err());
    let _ = app;
}

fn add_financial_fixture(state: &AppState) -> (String, String, String, String) {
    let account = add_account(state, "Fixture Card");
    let rule = uuid::Uuid::new_v4().to_string();
    let batch = uuid::Uuid::new_v4().to_string();
    let transaction = uuid::Uuid::new_v4().to_string();
    let connection = uuid::Uuid::new_v4().to_string();
    state
        .with_db(|conn, _h, _s| {
            conn.execute(
                "INSERT INTO merchant_rules (id, match_type, match_text, canonical_merchant, category, priority, enabled, created_at, updated_at) \
                 VALUES (?1, 'contains', 'fixture', 'Fixture Merchant', 'Shopping', 100, 1, '2026-10-23T12:00:00Z', '2026-10-23T12:00:00Z')",
                [&rule],
            )?;
            conn.execute(
                "INSERT INTO import_batches (id, account_id, file_sha256, file_size_bytes, mapping_json, row_count, inserted_count, duplicate_count, review_count, rejected_count, status, created_at, imported_at, rolled_back_at) \
                 VALUES (?1, ?2, ?3, 10, '{}', 1, 1, 0, 0, 0, 'imported', '2026-10-23T12:00:00Z', '2026-10-23T12:00:00Z', NULL)",
                rusqlite::params![batch, account, "a".repeat(64)],
            )?;
            conn.execute(
                "INSERT INTO transactions (id, account_id, source_kind, provider_transaction_id, import_batch_id, source_row_ref, pending, pending_link_id, authorized_date, posted_date, reporting_date, reporting_date_source, amount_minor, currency, raw_description, merchant_normalized, merchant_display, category, tx_type, excluded, removed, source_updated_at, created_at, updated_at) \
                 VALUES (?1, ?2, 'csv', NULL, ?3, 'row-1', 0, NULL, NULL, '2026-10-10', '2026-10-10', 'posted', 1234, 'USD', 'Fixture raw', 'fixture merchant', 'Fixture Merchant', 'Shopping', 'purchase', 0, 0, NULL, '2026-10-23T12:00:00Z', '2026-10-23T12:00:00Z')",
                rusqlite::params![transaction, account, batch],
            )?;
            conn.execute(
                "INSERT INTO import_rows (id, batch_id, transaction_id, row_number, source_row_hash, fingerprint, status, created_at) \
                 VALUES (?1, ?2, ?3, 1, ?4, ?5, 'inserted', '2026-10-23T12:00:00Z')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    batch,
                    transaction,
                    "b".repeat(64),
                    "c".repeat(64)
                ],
            )?;
            conn.execute(
                "INSERT INTO security_events (id, event_type, outcome, created_at, metadata_json) \
                 VALUES (?1, 'unlock_success', 'success', '2026-10-23T12:00:00Z', NULL)",
                [uuid::Uuid::new_v4().to_string()],
            )?;
            conn.execute(
                "INSERT INTO connections (id, provider, environment, institution_name, credential_ref, status, cursor, last_attempt_at, last_success_at, created_at, updated_at) \
                 VALUES (?1, 'plaid', 'sandbox', 'Fixture Bank', ?1, 'healthy', 'c', '2026-10-23T12:00:00Z', '2026-10-23T12:00:00Z', '2026-10-23T12:00:00Z', '2026-10-23T12:00:00Z')",
                [&connection],
            )?;
            Ok(())
        })
        .unwrap();
    add_provider_snapshot(state, &account, &connection, 777_000);
    (account, transaction, rule, batch)
}

#[test]
fn locked_same_vault_restore_requires_the_current_passphrase_and_stays_locked() {
    let (_tmp, state) = setup(PASS);
    let app = mock_app();
    add_marker(&state, "before");
    let backup = make_backup(&state, &app);
    add_marker(&state, "after");
    let header_before = std::fs::read(&state.paths.header).unwrap();
    lock(&state);

    let err = restore(&state, &backup, PASS, None, options(RestoreScope::Full)).unwrap_err();
    assert_eq!(err.code(), "vault_locked");
    assert!(!state.session.lock().unwrap().is_unlocked());
    assert_eq!(std::fs::read(&state.paths.header).unwrap(), header_before);

    let result = restore(
        &state,
        &backup,
        PASS,
        Some(PASS),
        options(RestoreScope::Full),
    )
    .unwrap();
    assert!(result.restored);
    assert!(!result.unlocked);
    assert!(!state.session.lock().unwrap().is_unlocked());
    assert_eq!(std::fs::read(&state.paths.header).unwrap(), header_before);
    assert!(vault::unlock(&state, pass(PASS)).is_ok());
    assert_eq!(marker(&state), Some("\"before\"".to_string()));
}

#[test]
fn cross_vault_locked_restore_adopts_the_backup_passphrase_and_stays_locked() {
    let (_tmp_a, state_a) = setup(PASS);
    let app_a = mock_app();
    add_marker(&state_a, "source");
    let backup = make_backup(&state_a, &app_a);

    let (_tmp_b, state_b) = setup(OTHER_PASS);
    let app_b = mock_app();
    add_marker(&state_b, "target");
    make_backup(&state_b, &app_b);
    lock(&state_b);

    let result = restore(
        &state_b,
        &backup,
        PASS,
        Some(OTHER_PASS),
        options_with_plaid(RestoreScope::Full, PlaidTokenChoice::Keep),
    )
    .unwrap();
    assert!(result.vault_id_changed);
    assert_eq!(result.pre_restore_protection, "previous");
    assert!(!result.unlocked);
    assert!(!state_b.session.lock().unwrap().is_unlocked());
    assert!(vault::unlock(&state_b, pass(OTHER_PASS)).is_err());
    assert!(vault::unlock(&state_b, pass(PASS)).is_ok());
    assert_eq!(marker(&state_b), Some("\"source\"".to_string()));
}

#[test]
fn destructive_locked_restore_skips_the_pre_restore_backup_and_honours_token_choice() {
    let (_tmp_a, state_a) = setup(PASS);
    let app_a = mock_app();
    add_marker(&state_a, "source");
    let backup = make_backup(&state_a, &app_a);

    let (_tmp_b, state_b) = setup(OTHER_PASS);
    let app_b = mock_app();
    add_marker(&state_b, "target");
    make_backup(&state_b, &app_b);
    let orphan = format!(
        "{}{}",
        cardigan_money_lib::providers::plaid::ACCESS_TOKEN_PREFIX,
        uuid::Uuid::new_v4()
    );
    state_b
        .credentials
        .set(&orphan, &pass("orphan-access-token-value-123"))
        .unwrap();
    lock(&state_b);

    let result = restore(
        &state_b,
        &backup,
        PASS,
        None,
        RestoreOptions {
            scope: RestoreScope::Full,
            allow_non_private: false,
            confirmation: Some("replace".into()),
            plaid_choice: Some(PlaidTokenChoice::DeleteAll),
            destructive: true,
        },
    )
    .unwrap();
    assert!(result.restored);
    assert_eq!(result.pre_restore_protection, "none");
    assert!(result.pre_restore_backup_id.is_none());
    assert!(result.vault_id_changed);
    assert!(!result.unlocked);
    assert!(state_b.credentials.get(&orphan).unwrap().is_none());
    assert!(vault::unlock(&state_b, pass(PASS)).is_ok());
    assert_eq!(marker(&state_b), Some("\"source\"".to_string()));

    // A destructive restore refuses a partial scope and an invalid token choice.
    lock(&state_b);
    let partial = restore(
        &state_b,
        &backup,
        PASS,
        None,
        RestoreOptions {
            scope: RestoreScope::FinancialOnly,
            allow_non_private: false,
            confirmation: Some("replace".into()),
            plaid_choice: Some(PlaidTokenChoice::DeleteAll),
            destructive: true,
        },
    );
    assert!(partial.is_err());
    let no_choice = restore(
        &state_b,
        &backup,
        PASS,
        None,
        RestoreOptions {
            scope: RestoreScope::Full,
            allow_non_private: false,
            confirmation: Some("replace".into()),
            plaid_choice: None,
            destructive: true,
        },
    );
    assert!(no_choice.is_err());
}

#[test]
fn financial_settings_scope_resets_connections_events_and_sync_but_keeps_imports() {
    let (_tmp_a, state_a) = setup(PASS);
    let app_a = mock_app();
    add_marker(&state_a, "source-settings");
    let (account, transaction, _rule, batch) = add_financial_fixture(&state_a);
    let backup = make_backup(&state_a, &app_a);

    let (_tmp_b, state_b) = setup(OTHER_PASS);
    let app_b = mock_app();
    add_marker(&state_b, "target-settings");
    make_backup(&state_b, &app_b);
    let vault_id = backups::current_vault_id(&state_b);

    let result = restore(
        &state_b,
        &backup,
        PASS,
        Some(OTHER_PASS),
        options(RestoreScope::FinancialSettings),
    )
    .unwrap();
    assert!(!result.vault_id_changed);
    assert_eq!(result.pre_restore_protection, "current");
    assert_eq!(backups::current_vault_id(&state_b), vault_id);
    assert_eq!(marker(&state_b), Some("\"source-settings\"".to_string()));
    assert_eq!(count(&state_b, "SELECT count(*) FROM connections"), 0);
    assert_eq!(count(&state_b, "SELECT count(*) FROM account_sources"), 0);
    assert_eq!(count(&state_b, "SELECT count(*) FROM sync_runs"), 0);
    assert_eq!(count(&state_b, "SELECT count(*) FROM security_events"), 1);
    assert_eq!(count(&state_b, "SELECT count(*) FROM import_batches"), 1);
    assert_eq!(count(&state_b, "SELECT count(*) FROM import_rows"), 1);
    let (tx_batch, snapshot_source): (Option<String>, Option<String>) = state_b
        .with_db(|conn, _h, _s| {
            Ok((
                conn.query_row(
                    "SELECT import_batch_id FROM transactions WHERE id = ?1",
                    [&transaction],
                    |row| row.get(0),
                )?,
                conn.query_row(
                    "SELECT source_connection_id FROM balance_snapshots WHERE account_id = ?1",
                    [&account],
                    |row| row.get(0),
                )?,
            ))
        })
        .unwrap();
    assert_eq!(tx_batch.as_deref(), Some(batch.as_str()));
    assert!(snapshot_source.is_none());
    // The current passphrase and identity remain in effect.
    lock(&state_b);
    assert!(vault::unlock(&state_b, pass(OTHER_PASS)).is_ok());
}

#[test]
fn financial_only_scope_resets_settings_and_import_history() {
    let (_tmp_a, state_a) = setup(PASS);
    let app_a = mock_app();
    add_marker(&state_a, "source-settings");
    let (account, transaction, rule, _batch) = add_financial_fixture(&state_a);
    let backup = make_backup(&state_a, &app_a);

    let (_tmp_b, state_b) = setup(OTHER_PASS);
    let app_b = mock_app();
    add_marker(&state_b, "target-settings");
    make_backup(&state_b, &app_b);

    let result = restore(
        &state_b,
        &backup,
        PASS,
        Some(OTHER_PASS),
        options(RestoreScope::FinancialOnly),
    )
    .unwrap();
    assert!(!result.vault_id_changed);
    // Settings were reset to defaults, so the fixture marker is gone.
    assert_eq!(marker(&state_b), None);
    assert_eq!(count(&state_b, "SELECT count(*) FROM import_batches"), 0);
    assert_eq!(count(&state_b, "SELECT count(*) FROM import_rows"), 0);
    assert_eq!(count(&state_b, "SELECT count(*) FROM connections"), 0);
    assert_eq!(count(&state_b, "SELECT count(*) FROM merchant_rules"), 1);
    assert_eq!(count(&state_b, "SELECT count(*) FROM accounts"), 1);
    let (tx_batch, rule_id): (Option<String>, String) = state_b
        .with_db(|conn, _h, _s| {
            Ok((
                conn.query_row(
                    "SELECT import_batch_id FROM transactions WHERE id = ?1",
                    [&transaction],
                    |row| row.get(0),
                )?,
                conn.query_row("SELECT id FROM merchant_rules", [], |row| row.get(0))?,
            ))
        })
        .unwrap();
    assert!(tx_batch.is_none());
    assert_eq!(rule_id, rule);
    // Financial tables are present; provider snapshots are hidden.
    let snapshot_restored: i64 = state_b
        .with_db(|conn, _h, _s| {
            Ok(conn.query_row(
                "SELECT restored FROM balance_snapshots WHERE account_id = ?1",
                [&account],
                |row| row.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(snapshot_restored, 1);
}

#[test]
fn non_private_backup_requires_acknowledgement_but_other_checks_still_run() {
    let (_tmp, state) = setup(PASS);
    let app = mock_app();
    add_marker(&state, "before");
    let backup = make_backup(&state, &app);
    add_marker(&state, "after");
    std::fs::set_permissions(&backup, std::os::unix::fs::PermissionsExt::from_mode(0o644)).unwrap();

    let preview = inspect_restore(&state, &backup).unwrap();
    assert!(preview.non_private);
    let err = restore(&state, &backup, PASS, None, options(RestoreScope::Full)).unwrap_err();
    assert_eq!(err.code(), "permission_denied");
    assert_eq!(marker(&state), Some("\"after\"".to_string()));

    let mut acknowledged = options(RestoreScope::Full);
    acknowledged.allow_non_private = true;
    let result = restore(&state, &backup, PASS, None, acknowledged).unwrap();
    assert!(result.restored);
    assert_eq!(marker(&state), Some("\"before\"".to_string()));

    // A symlink is rejected even with the acknowledgement.
    std::fs::set_permissions(&backup, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
    let link = _tmp.path().join("link.cmbak");
    std::os::unix::fs::symlink(&backup, &link).unwrap();
    let mut acknowledged = options(RestoreScope::Full);
    acknowledged.allow_non_private = true;
    let err = restore(&state, &link, PASS, None, acknowledged).unwrap_err();
    assert_eq!(err.code(), "permission_denied");
    // Truncation is rejected regardless of the mode acknowledgement.
    let truncated = _tmp.path().join("truncated.cmbak");
    std::fs::write(&truncated, &std::fs::read(&backup).unwrap()[..100]).unwrap();
    std::fs::set_permissions(
        &truncated,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    )
    .unwrap();
    let mut acknowledged = options(RestoreScope::Full);
    acknowledged.allow_non_private = true;
    assert!(restore(&state, &truncated, PASS, None, acknowledged).is_err());
}

#[test]
fn a_failed_pre_restore_backup_aborts_before_the_live_vault_changes() {
    let (_tmp, state) = setup(PASS);
    let app = mock_app();
    add_marker(&state, "before");
    let backup = make_backup(&state, &app);
    // Move the source backup outside the app directory, then make the backup
    // directory unusable so the pre-restore backup must fail.
    let outside = _tmp.path().join("outside.cmbak");
    std::fs::copy(&backup, &outside).unwrap();
    std::fs::set_permissions(
        &outside,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    )
    .unwrap();
    let dir = backups::backups_dir(&state);
    std::fs::remove_dir_all(&dir).unwrap();
    std::fs::write(&dir, b"not a directory").unwrap();
    add_marker(&state, "after");

    let err = restore(&state, &outside, PASS, None, options(RestoreScope::Full)).unwrap_err();
    assert_eq!(err.code(), "permission_denied");
    assert_eq!(marker(&state), Some("\"after\"".to_string()));
    assert!(state.session.lock().unwrap().is_unlocked());
    // No staged candidate is left behind.
    let staged = std::fs::read_dir(&state.app_data)
        .unwrap()
        .flatten()
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("restore-staged-")
        });
    assert!(!staged);
}

#[test]
fn restored_connections_are_blocked_from_sync_until_reconnected() {
    let (_tmp, state) = setup(PASS);
    let app = mock_app();
    let account = add_account(&state, "Gate Card");
    let credential_ref = uuid::Uuid::new_v4().to_string();
    let connection = add_plaid_connection(&state, &credential_ref);
    add_provider_snapshot(&state, &account, &connection, 55_000);
    let backup = make_backup(&state, &app);
    // Make a fresh restore target with the same data, then restore.
    let result = restore(&state, &backup, PASS, None, options(RestoreScope::Full)).unwrap();
    assert!(result.restored);

    let summary = cardigan_money_lib::services::sync::sync_now(&state, "manual").unwrap();
    assert!(summary.accepted);
    assert_eq!(summary.results.len(), 1);
    assert_eq!(
        summary.results[0].error_code.as_deref(),
        Some("reconnect_required")
    );
    let connection_id = connection.clone();
    let blocked =
        cardigan_money_lib::services::sync::sync_connection_now(&state, &connection_id, "manual");
    assert!(blocked.is_err());
    let preview = state
        .with_db(|conn, _h, _s| {
            overview(conn, chrono::NaiveDate::from_ymd_opt(2026, 10, 24).unwrap())
        })
        .unwrap();
    assert!(preview
        .accounts
        .iter()
        .all(|account| account.current_balance_minor.is_none()));
}

#[test]
fn post_restore_passphrase_change_works_while_locked() {
    let (_tmp, state) = setup(PASS);
    let app = mock_app();
    add_marker(&state, "kept");
    let backup = make_backup(&state, &app);
    restore(&state, &backup, PASS, None, options(RestoreScope::Full)).unwrap();
    lock(&state);
    const NEW_PASS: &str = "a brand new correct horse battery staple";
    vault::change_passphrase_after_restore(&state, &app, pass(PASS), pass(NEW_PASS)).unwrap();
    assert!(!state.session.lock().unwrap().is_unlocked());
    assert!(vault::unlock(&state, pass(PASS)).is_err());
    assert!(vault::unlock(&state, pass(NEW_PASS)).is_ok());
    assert_eq!(marker(&state), Some("\"kept\"".to_string()));
    assert!(list_backups(&state)
        .unwrap()
        .iter()
        .any(|item| item.kind == "pre-change"));
}

#[test]
fn post_restore_passphrase_change_works_while_unlocked() {
    let (_tmp, state) = setup(PASS);
    let app = mock_app();
    const NEW_PASS: &str = "a brand new correct horse battery staple";
    vault::change_passphrase_after_restore(&state, &app, pass(PASS), pass(NEW_PASS)).unwrap();
    assert!(state.session.lock().unwrap().is_unlocked());
    lock(&state);
    assert!(vault::unlock(&state, pass(PASS)).is_err());
    assert!(vault::unlock(&state, pass(NEW_PASS)).is_ok());
}

#[test]
fn restore_requires_a_typed_replace_confirmation() {
    let (_tmp, state) = setup(PASS);
    let app = mock_app();
    add_marker(&state, "after");
    let backup = make_backup(&state, &app);
    let mut missing = options(RestoreScope::Full);
    missing.confirmation = None;
    let err = restore(&state, &backup, PASS, None, missing).unwrap_err();
    assert_eq!(err.code(), "blocked");
    assert!(state.session.lock().unwrap().is_unlocked());
    let mut wrong = options(RestoreScope::Full);
    wrong.confirmation = Some("yes".into());
    assert!(restore(&state, &backup, PASS, None, wrong).is_err());
    assert_eq!(marker(&state), Some("\"after\"".to_string()));
}

#[test]
fn pending_restore_preview_never_exposes_the_path_or_passphrase() {
    let (_tmp, state) = setup(PASS);
    let app = mock_app();
    let backup = make_backup(&state, &app);
    let preview = inspect_restore(&state, &backup).unwrap();
    let text = serde_json::to_string(&preview).unwrap().to_lowercase();
    assert!(!text.contains("passphrase"));
    assert!(!text.contains(_tmp.path().to_string_lossy().as_ref()));
    assert!(!text.contains('/'));
}

#[test]
fn same_vault_full_restore_keeps_keychain_tokens_and_credential_references() {
    let (_tmp, state) = setup(PASS);
    let app = mock_app();
    let account = add_account(&state, "Token Card");
    let credential_ref = uuid::Uuid::new_v4().to_string();
    let connection = add_plaid_connection(&state, &credential_ref);
    add_provider_snapshot(&state, &account, &connection, 10_000);
    let token_account = cardigan_money_lib::providers::plaid::access_token_account(&credential_ref);
    state
        .credentials
        .set(&token_account, &pass("same-vault-access-token-123"))
        .unwrap();
    let backup = make_backup(&state, &app);

    let result = restore(&state, &backup, PASS, None, options(RestoreScope::Full)).unwrap();
    assert!(result.restored);
    // The same-vault full restore does not delete the token or rewrite the ref.
    assert!(state.credentials.get(&token_account).unwrap().is_some());
    let restored_ref: String = state
        .with_db(|conn, _h, _s| {
            Ok(
                conn.query_row("SELECT credential_ref FROM connections", [], |row| {
                    row.get(0)
                })?,
            )
        })
        .unwrap();
    assert_eq!(restored_ref, credential_ref);
    let status: String = state
        .with_db(|conn, _h, _s| {
            Ok(conn.query_row("SELECT status FROM connections", [], |row| row.get(0))?)
        })
        .unwrap();
    assert_eq!(status, "reconnect_required");
    let connections = cardigan_money_lib::services::sync::list_connections(&state).unwrap();
    assert_eq!(connections.len(), 1);
    assert!(connections[0].has_token);
}

#[test]
fn a_cross_vault_source_backup_in_the_default_directory_stays_listed() {
    let (_tmp_a, state_a) = setup(PASS);
    let app_a = mock_app();
    add_marker(&state_a, "source");
    let source = make_backup(&state_a, &app_a);

    let (_tmp_b, state_b) = setup(OTHER_PASS);
    let app_b = mock_app();
    add_marker(&state_b, "target");
    let target_backup = make_backup(&state_b, &app_b);
    // Place the source container in the target vault's default directory.
    let source_name = source.file_name().unwrap().to_string_lossy().to_string();
    let copied = backups::backups_dir(&state_b).join(&source_name);
    std::fs::copy(&source, &copied).unwrap();
    std::fs::set_permissions(&copied, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();

    let result = restore(
        &state_b,
        &copied,
        PASS,
        Some(OTHER_PASS),
        options_with_plaid(RestoreScope::Full, PlaidTokenChoice::Keep),
    )
    .unwrap();
    assert!(result.vault_id_changed);
    // The restored vault's own source container remains normal and listed.
    assert!(list_backups(&state_b)
        .unwrap()
        .iter()
        .any(|item| item.id == source_name));
    // The previous vault's own container is retired.
    let target_name = target_backup
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    assert!(list_retired_backups(&state_b)
        .unwrap()
        .iter()
        .any(|item| item.id.ends_with(&target_name)));
}

#[test]
fn destructive_same_vault_restore_adopts_a_custom_backup_passphrase() {
    let (_tmp, state) = setup(PASS);
    add_marker(&state, "source");
    let dir = backups::backups_dir(&state);
    let backup = dir.join("cm-backup-manual-deadbeef-gen1-20261024T120000Z-0001.cmbak");
    create_manual_backup_to(&state, &backup, pass(BACKUP_PASS), None).unwrap();
    lock(&state);
    let result = restore(
        &state,
        &backup,
        BACKUP_PASS,
        None,
        RestoreOptions {
            scope: RestoreScope::Full,
            allow_non_private: false,
            confirmation: Some("replace".into()),
            plaid_choice: Some(PlaidTokenChoice::LeaveAll),
            destructive: true,
        },
    )
    .unwrap();
    // The vault ID is unchanged, but the header/DEK were replaced under the
    // backup passphrase.
    assert!(!result.vault_id_changed);
    assert!(result.adopted_backup_key);
    assert_eq!(result.pre_restore_protection, "none");
    assert!(vault::unlock(&state, pass(PASS)).is_err());
    assert!(vault::unlock(&state, pass(BACKUP_PASS)).is_ok());
    assert_eq!(marker(&state), Some("\"source\"".to_string()));
}

#[test]
fn a_schema_three_backup_is_migrated_before_scope_reset() {
    let (_tmp, state) = setup(PASS);
    let app = mock_app();
    let account = add_account(&state, "Legacy Card");
    let credential_ref = uuid::Uuid::new_v4().to_string();
    let connection = add_plaid_connection(&state, &credential_ref);
    add_provider_snapshot(&state, &account, &connection, 42_000);
    // Shape the live database like the schema-3 version this backup came from.
    state
        .with_db(|conn, _h, _s| {
            conn.execute_batch(
                "DROP INDEX balance_snapshots_live_idx;\
                 DROP INDEX payment_snapshots_live_idx;\
                 ALTER TABLE balance_snapshots DROP COLUMN restored;\
                 ALTER TABLE payment_snapshots DROP COLUMN restored;\
                 DELETE FROM schema_migrations WHERE version = 4;\
                 UPDATE vault_metadata SET schema_version = 3;",
            )?;
            Ok(())
        })
        .unwrap();
    add_marker(&state, "legacy");
    let backup = make_backup(&state, &app);

    let result = restore(
        &state,
        &backup,
        PASS,
        None,
        options(RestoreScope::FinancialSettings),
    )
    .unwrap();
    assert!(result.restored);
    assert_eq!(marker(&state), Some("\"legacy\"".to_string()));
    let (schema, has_restored): (u32, i64) = state
        .with_db(|conn, _h, _s| {
            Ok((
                conn.query_row("SELECT schema_version FROM vault_metadata", [], |row| row.get(0))?,
                conn.query_row(
                    "SELECT count(*) FROM pragma_table_info('balance_snapshots') WHERE name = 'restored'",
                    [],
                    |row| row.get(0),
                )?,
            ))
        })
        .unwrap();
    assert_eq!(schema, 4);
    assert_eq!(has_restored, 1);
    // The connection reset ran after migration, so the table/columns exist.
    assert_eq!(count(&state, "SELECT count(*) FROM connections"), 0);
}

#[test]
fn destructive_restore_recovers_a_header_only_vault() {
    let (_tmp_a, state_a) = setup(PASS);
    let app_a = mock_app();
    add_marker(&state_a, "source");
    let backup = make_backup(&state_a, &app_a);

    let (_tmp_b, state_b) = setup(OTHER_PASS);
    let app_b = mock_app();
    add_marker(&state_b, "target");
    make_backup(&state_b, &app_b);
    // Simulate a vault whose database file was lost but whose header survives.
    state_b.session.lock().unwrap().clear();
    std::fs::remove_file(&state_b.paths.database).unwrap();
    for suffix in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", state_b.paths.database.display()));
    }
    let result = restore(
        &state_b,
        &backup,
        PASS,
        None,
        RestoreOptions {
            scope: RestoreScope::Full,
            allow_non_private: false,
            confirmation: Some("replace".into()),
            plaid_choice: Some(PlaidTokenChoice::LeaveAll),
            destructive: true,
        },
    )
    .unwrap();
    assert!(result.restored);
    assert!(result.vault_id_changed);
    assert!(!result.unlocked);
    assert!(vault::unlock(&state_b, pass(PASS)).is_ok());
    assert_eq!(marker(&state_b), Some("\"source\"".to_string()));
}

#[test]
fn fresh_install_financial_only_scope_creates_defaults_without_leaving_a_session() {
    let (_tmp_a, state_a) = setup(PASS);
    let app_a = mock_app();
    add_marker(&state_a, "source-settings");
    let (account, transaction, rule, _batch) = add_financial_fixture(&state_a);
    let backup = make_backup(&state_a, &app_a);
    let backup_vault_id = backups::current_vault_id(&state_a).unwrap();

    let tmp = tempfile::tempdir().unwrap();
    let mut state = AppState::new(tmp.path().to_path_buf()).unwrap();
    state.credentials = Arc::new(MemoryStore::default());
    let result = restore(
        &state,
        &backup,
        PASS,
        None,
        options(RestoreScope::FinancialOnly),
    )
    .unwrap();
    assert!(result.restored);
    assert!(result.adopted_backup_key);
    assert!(!result.unlocked);
    assert_eq!(backups::current_vault_id(&state), Some(backup_vault_id));
    assert!(vault::unlock(&state, pass(PASS)).is_ok());
    assert_eq!(marker(&state), None);
    assert_eq!(count(&state, "SELECT count(*) FROM import_batches"), 0);
    assert_eq!(count(&state, "SELECT count(*) FROM connections"), 0);
    assert_eq!(count(&state, "SELECT count(*) FROM accounts"), 1);
    assert_eq!(count(&state, "SELECT count(*) FROM transactions"), 1);
    let rule_id: String = state
        .with_db(|conn, _h, _s| {
            Ok(conn.query_row("SELECT id FROM merchant_rules", [], |row| row.get(0))?)
        })
        .unwrap();
    assert_eq!(rule_id, rule);
    let tx_batch: Option<String> = state
        .with_db(|conn, _h, _s| {
            Ok(conn.query_row(
                "SELECT import_batch_id FROM transactions WHERE id = ?1",
                [&transaction],
                |row| row.get(0),
            )?)
        })
        .unwrap();
    assert!(tx_batch.is_none());
    let _ = account;
}

#[test]
fn a_newer_schema_metadata_is_rejected_before_install() {
    let (_tmp, state) = setup(PASS);
    let app = mock_app();
    add_marker(&state, "before");
    state
        .with_db(|conn, _h, _s| {
            conn.execute("UPDATE vault_metadata SET schema_version = 5", [])?;
            Ok(())
        })
        .unwrap();
    let backup = make_backup(&state, &app);
    add_marker(&state, "after");
    let err = restore(&state, &backup, PASS, None, options(RestoreScope::Full)).unwrap_err();
    assert_eq!(err.code(), "schema_too_new");
    assert_eq!(marker(&state), Some("\"after\"".to_string()));
    let staged = std::fs::read_dir(&state.app_data)
        .unwrap()
        .flatten()
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("restore-staged-")
        });
    assert!(!staged);
}

#[test]
fn a_failed_fresh_restore_leaves_no_partial_vault() {
    let (_tmp_a, state_a) = setup(PASS);
    let app_a = mock_app();
    let backup = make_backup(&state_a, &app_a);
    let mut bytes = std::fs::read(&backup).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    let tampered = _tmp_a.path().join("tampered.cmbak");
    std::fs::write(&tampered, bytes).unwrap();
    std::fs::set_permissions(
        &tampered,
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    )
    .unwrap();

    let tmp = tempfile::tempdir().unwrap();
    let mut state = AppState::new(tmp.path().to_path_buf()).unwrap();
    state.credentials = Arc::new(MemoryStore::default());
    let result = restore(&state, &tampered, PASS, None, options(RestoreScope::Full));
    assert!(result.is_err());
    assert!(!state.paths.root.exists());
    let install_dirs = std::fs::read_dir(&state.app_data)
        .unwrap()
        .flatten()
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("vault-install-")
        });
    assert!(!install_dirs);
}
