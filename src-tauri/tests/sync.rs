//! Sync engine: pagination/idempotency, cursor safety, pending replacement,
//! removals, partial streams, lock-during-sync, and connection isolation.

use cardigan_money_lib::providers::mock::MockProvider;
use cardigan_money_lib::providers::{
    Capability, LiabilitiesResult, ProviderAccount, ProviderBalance, ProviderCapabilities,
    ProviderClient, ProviderResult, ProviderTransaction, TransactionsPage,
};
use cardigan_money_lib::security::crypto::SecretString;
use cardigan_money_lib::services::sync::{self, sync_connection_by_id_with_client};
use cardigan_money_lib::services::vault;
use cardigan_money_lib::state::AppState;
use chrono::NaiveDate;

const PASS: &str = "correct horse battery staple sync";

fn setup() -> (tempfile::TempDir, AppState, String) {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::new(tmp.path().to_path_buf()).unwrap();
    vault::create(&state, SecretString::new(PASS.into())).unwrap();
    let connection = state
        .with_db(|conn, _h, _s| {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO connections (id, provider, environment, institution_name, credential_ref, status, created_at, updated_at) \
                 VALUES (?1, 'mock', 'sandbox', 'Synthetic Bank', ?2, 'healthy', '2026-10-23T12:00:00Z', '2026-10-23T12:00:00Z')",
                rusqlite::params![id, uuid::Uuid::new_v4().to_string()],
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            Ok(id)
        })
        .unwrap();
    (tmp, state, connection)
}

fn count(state: &AppState, sql: &str) -> i64 {
    state
        .with_db(|conn, _h, _s| {
            conn.query_row(sql, [], |row| row.get(0))
                .map_err(|_| cardigan_money_lib::error::AppError::Database)
        })
        .unwrap()
}

#[test]
fn mock_sync_pages_accounts_transactions_balances_and_is_idempotent() {
    let (_tmp, state, connection) = setup();
    let provider = MockProvider::new(NaiveDate::from_ymd_opt(2026, 10, 23).unwrap());
    let result =
        sync_connection_by_id_with_client(&state, &connection, &provider, "manual").unwrap();
    assert_eq!(result.transactions_added, 3);
    assert_eq!(result.transactions_removed, 0);
    assert_eq!(result.balances_written, 2);
    assert_eq!(result.status, "partial", "liabilities are unsupported");

    assert_eq!(
        count(
            &state,
            "SELECT count(*) FROM accounts WHERE source_kind = 'provider'"
        ),
        2
    );
    assert_eq!(count(&state, "SELECT count(*) FROM transactions"), 3);
    assert_eq!(count(&state, "SELECT count(*) FROM balance_snapshots"), 2);
    let cursor: String = state
        .with_db(|conn, _h, _s| {
            conn.query_row(
                "SELECT cursor FROM connections WHERE id = ?1",
                [&connection],
                |row| row.get(0),
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)
        })
        .unwrap();
    assert_eq!(cursor, "done");

    // Replaying from the saved cursor is a no-op.
    let again =
        sync_connection_by_id_with_client(&state, &connection, &provider, "manual").unwrap();
    assert_eq!(again.transactions_added, 0);
    assert_eq!(count(&state, "SELECT count(*) FROM transactions"), 3);

    // Provider balance changes are append-only snapshots.
    let updated = MockProviderWithBalance { inner: provider };
    let third = sync_connection_by_id_with_client(&state, &connection, &updated, "manual").unwrap();
    assert_eq!(third.balances_written, 2);
    assert_eq!(count(&state, "SELECT count(*) FROM balance_snapshots"), 6);
}

struct MockProviderWithBalance {
    inner: MockProvider,
}

impl ProviderClient for MockProviderWithBalance {
    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }
    fn accounts(&self) -> ProviderResult<Vec<ProviderAccount>> {
        self.inner.accounts()
    }
    fn transactions_page(&self, cursor: Option<&str>) -> ProviderResult<TransactionsPage> {
        self.inner.transactions_page(cursor)
    }
    fn balances(&self) -> ProviderResult<Vec<ProviderBalance>> {
        let mut balances = self.inner.balances()?;
        balances[0].current_balance_minor += 100;
        Ok(balances)
    }
    fn liabilities(&self) -> ProviderResult<LiabilitiesResult> {
        self.inner.liabilities()
    }
}

#[test]
fn provider_failure_does_not_advance_past_the_committed_page() {
    let (_tmp, state, connection) = setup();
    let failing =
        MockProvider::with_failure(NaiveDate::from_ymd_opt(2026, 10, 23).unwrap(), "page-2");
    let result =
        sync_connection_by_id_with_client(&state, &connection, &failing, "manual").unwrap();
    assert_eq!(result.status, "failed");
    assert_eq!(result.transactions_added, 1);
    let cursor: String = state
        .with_db(|conn, _h, _s| {
            conn.query_row(
                "SELECT cursor FROM connections WHERE id = ?1",
                [&connection],
                |row| row.get(0),
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)
        })
        .unwrap();
    assert_eq!(cursor, "page-2", "only the committed page advanced");
    assert_eq!(count(&state, "SELECT count(*) FROM transactions"), 1);

    // A healthy retry continues from the committed cursor without duplicates.
    let healthy = MockProvider::new(NaiveDate::from_ymd_opt(2026, 10, 23).unwrap());
    sync_connection_by_id_with_client(&state, &connection, &healthy, "manual").unwrap();
    assert_eq!(count(&state, "SELECT count(*) FROM transactions"), 3);
}

struct LockingProvider<'a> {
    state: &'a AppState,
    inner: MockProvider,
    calls: std::cell::Cell<usize>,
}

impl<'a> ProviderClient for LockingProvider<'a> {
    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }
    fn accounts(&self) -> ProviderResult<Vec<ProviderAccount>> {
        self.inner.accounts()
    }
    fn transactions_page(&self, cursor: Option<&str>) -> ProviderResult<TransactionsPage> {
        let calls = self.calls.get() + 1;
        self.calls.set(calls);
        if calls == 2 {
            self.state.session.lock().unwrap().clear();
        }
        self.inner.transactions_page(cursor)
    }
    fn balances(&self) -> ProviderResult<Vec<ProviderBalance>> {
        self.inner.balances()
    }
    fn liabilities(&self) -> ProviderResult<LiabilitiesResult> {
        self.inner.liabilities()
    }
}

#[test]
fn vault_lock_mid_sync_stops_without_advancing_and_can_resume() {
    let (_tmp, state, connection) = setup();
    let locking = LockingProvider {
        state: &state,
        inner: MockProvider::new(NaiveDate::from_ymd_opt(2026, 10, 23).unwrap()),
        calls: std::cell::Cell::new(0),
    };
    let result = sync_connection_by_id_with_client(&state, &connection, &locking, "manual");
    assert!(result.is_err());
    vault::unlock(&state, SecretString::new(PASS.into())).unwrap();
    let cursor: String = state
        .with_db(|conn, _h, _s| {
            conn.query_row(
                "SELECT cursor FROM connections WHERE id = ?1",
                [&connection],
                |row| row.get(0),
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)
        })
        .unwrap();
    assert_eq!(cursor, "page-2");
    assert_eq!(count(&state, "SELECT count(*) FROM transactions"), 1);

    let healthy = MockProvider::new(NaiveDate::from_ymd_opt(2026, 10, 23).unwrap());
    sync_connection_by_id_with_client(&state, &connection, &healthy, "manual").unwrap();
    assert_eq!(count(&state, "SELECT count(*) FROM transactions"), 3);
}

#[derive(Clone)]
struct PendingThenPosted {
    calls: std::cell::Cell<usize>,
}

impl ProviderClient for PendingThenPosted {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            transactions: Capability::Supported,
            balances: Capability::Unsupported,
            liabilities: Capability::Unsupported,
        }
    }
    fn accounts(&self) -> ProviderResult<Vec<ProviderAccount>> {
        Ok(vec![ProviderAccount {
            provider_account_id: "acct-1".into(),
            name: "Pending Card".into(),
            mask: "0042".into(),
            currency: "USD".into(),
            account_type: "credit_card".into(),
        }])
    }
    fn transactions_page(&self, cursor: Option<&str>) -> ProviderResult<TransactionsPage> {
        let calls = self.calls.get() + 1;
        self.calls.set(calls);
        match cursor {
            None => Ok(TransactionsPage {
                changes: vec![tx("pending-1", 500, true, None)],
                next_cursor: Some("posted".into()),
                has_more: true,
            }),
            Some("posted") => Ok(TransactionsPage {
                changes: vec![tx("posted-1", 500, false, Some("pending-1"))],
                next_cursor: Some("done".into()),
                has_more: false,
            }),
            _ => Ok(TransactionsPage {
                changes: vec![],
                next_cursor: Some("done".into()),
                has_more: false,
            }),
        }
    }
    fn balances(&self) -> ProviderResult<Vec<ProviderBalance>> {
        Ok(vec![])
    }
    fn liabilities(&self) -> ProviderResult<LiabilitiesResult> {
        Ok(LiabilitiesResult {
            capability: Capability::Unsupported,
            rows: vec![],
        })
    }
}

fn tx(id: &str, amount: i64, pending: bool, pending_id: Option<&str>) -> ProviderTransaction {
    ProviderTransaction {
        provider_transaction_id: id.into(),
        provider_account_id: "acct-1".into(),
        amount_minor: amount,
        currency: "USD".into(),
        description: "Synthetic Pending".into(),
        authorized_date: Some("2026-10-20".into()),
        posted_date: if pending {
            None
        } else {
            Some("2026-10-22".into())
        },
        pending,
        removed: false,
        pending_transaction_id: pending_id.map(str::to_string),
        tx_type: "purchase".into(),
        category: Some("Test".into()),
    }
}

#[test]
fn pending_to_posted_does_not_double_count() {
    let (_tmp, state, connection) = setup();
    let provider = PendingThenPosted {
        calls: std::cell::Cell::new(0),
    };
    sync_connection_by_id_with_client(&state, &connection, &provider, "manual").unwrap();
    sync_connection_by_id_with_client(&state, &connection, &provider, "manual").unwrap();
    assert_eq!(count(&state, "SELECT count(*) FROM transactions"), 2);
    assert_eq!(
        count(
            &state,
            "SELECT count(*) FROM transactions WHERE removed = 1"
        ),
        1
    );
    assert_eq!(
        count(&state, "SELECT count(*) FROM transactions WHERE provider_transaction_id = 'posted-1' AND pending_link_id IS NOT NULL"),
        1
    );
    // Ledger excludes the retired pending row from totals.
    let summary = state
        .with_db(|conn, _h, _s| {
            cardigan_money_lib::storage::ledger::month_summary(
                conn,
                cardigan_money_lib::domain::dates::Month::new(2026, 10).unwrap(),
                "USD",
            )
        })
        .unwrap();
    assert_eq!(summary.gross_purchases_minor, 500);
}

#[test]
fn liabilities_available_but_null_preserves_previous_values() {
    let (_tmp, state, connection) = setup();
    let provider = MockProvider::with_liabilities(
        NaiveDate::from_ymd_opt(2026, 10, 23).unwrap(),
        Capability::AvailableButNull,
    );
    // Seed a previous provider liability snapshot for the account created by sync.
    sync_connection_by_id_with_client(&state, &connection, &provider, "manual").unwrap();
    let account_id: String = state
        .with_db(|conn, _h, _s| {
            conn.query_row("SELECT id FROM accounts LIMIT 1", [], |row| row.get(0))
                .map_err(|_| cardigan_money_lib::error::AppError::Database)
        })
        .unwrap();
    state
        .with_db(|conn, _h, _s| {
            conn.execute(
                "INSERT INTO payment_snapshots (id, account_id, next_due_date, minimum_due_minor, statement_balance_minor, retrieved_at, provenance, source_connection_id) \
                 VALUES (?1, ?2, '2026-10-30', 3000, 42000, '2026-10-20T12:00:00Z', 'provider', ?3)",
                rusqlite::params![uuid::Uuid::new_v4().to_string(), account_id, connection],
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            Ok(())
        })
        .unwrap();
    let result =
        sync_connection_by_id_with_client(&state, &connection, &provider, "manual").unwrap();
    assert_eq!(result.liabilities_status, "available_but_null");
    assert_eq!(
        count(&state, "SELECT count(*) FROM payment_snapshots"),
        1,
        "prior value preserved"
    );
}

#[test]
fn concurrent_trigger_is_skipped_without_network_or_writes() {
    let (_tmp, state, _connection) = setup();
    let guard = state.sync_gate.lock().unwrap();
    let summary = sync::sync_now(&state, "manual").unwrap();
    assert!(!summary.accepted);
    assert_eq!(summary.skip_reason.as_deref(), Some("already_running"));
    drop(guard);
}

#[test]
fn locked_sync_is_skipped() {
    let (_tmp, state, _connection) = setup();
    state.session.lock().unwrap().clear();
    let summary = sync::sync_now(&state, "scheduled").unwrap();
    assert!(!summary.accepted);
    assert_eq!(summary.skip_reason.as_deref(), Some("vault_locked"));
}

#[test]
fn one_failed_connection_does_not_block_another() {
    let (_tmp, state, mock_connection) = setup();
    let plaid_connection = state
        .with_db(|conn, _h, _s| {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO connections (id, provider, environment, institution_name, credential_ref, status, created_at, updated_at) \
                 VALUES (?1, 'plaid', 'sandbox', 'Live Bank', ?2, 'healthy', '2026-10-23T12:00:00Z', '2026-10-23T12:00:00Z')",
                rusqlite::params![id, uuid::Uuid::new_v4().to_string()],
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            Ok(id)
        })
        .unwrap();
    let summary = sync::sync_now(&state, "manual").unwrap();
    assert!(summary.accepted);
    assert_eq!(summary.results.len(), 2);
    assert!(summary
        .results
        .iter()
        .any(|r| r.connection_id == mock_connection && r.transactions_added == 3));
    assert!(summary
        .results
        .iter()
        .any(|r| r.connection_id == plaid_connection && r.status == "failed"));
}
