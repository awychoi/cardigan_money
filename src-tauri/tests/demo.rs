//! Demo isolation, determinism, and read-only guarantees.

use cardigan_money_lib::demo;
use cardigan_money_lib::demo::fixtures as demo_fixtures;
use cardigan_money_lib::security::cipher::open_ledger;
use cardigan_money_lib::security::crypto::{LockedKey, SecretString};
use cardigan_money_lib::services::dashboard;
use cardigan_money_lib::services::vault;
use cardigan_money_lib::state::{AppState, SessionMode};
use cardigan_money_lib::storage::settings::Theme;

const PASS: &str = "correct horse battery staple demo";

fn mock_app() -> tauri::AppHandle<tauri::test::MockRuntime> {
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("mock app");
    app.handle().clone()
}

fn state() -> (tempfile::TempDir, AppState) {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::new(tmp.path().to_path_buf()).unwrap();
    (tmp, state)
}

#[test]
fn demo_open_seeds_at_least_150_transactions_and_all_scenarios() {
    let (tmp, state) = state();
    let app = mock_app();
    let view = demo::open(&state, &app).unwrap();
    assert!(view.unlocked);
    assert_eq!(view.mode, "demo");
    assert!(view.settings.as_ref().unwrap().onboarding_completed);
    let settings = view.settings.unwrap();
    assert_eq!(settings.theme, Theme::System);

    let counts = state
        .with_db(|conn, _header, _settings| {
            let transactions: i64 = conn.query_row("SELECT count(*) FROM transactions", [], |r| r.get(0)).map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            let accounts: i64 = conn.query_row("SELECT count(*) FROM accounts", [], |r| r.get(0)).map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            let connections: i64 = conn.query_row("SELECT count(*) FROM connections", [], |r| r.get(0)).map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            let pending: i64 = conn.query_row("SELECT count(*) FROM transactions WHERE pending = 1", [], |r| r.get(0)).map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            let eur: i64 = conn.query_row("SELECT count(*) FROM transactions WHERE currency = 'EUR'", [], |r| r.get(0)).map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            let linked: i64 = conn.query_row("SELECT count(*) FROM transactions WHERE pending_link_id IS NOT NULL", [], |r| r.get(0)).map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            let refunds: i64 = conn.query_row("SELECT count(*) FROM transactions WHERE tx_type = 'refund'", [], |r| r.get(0)).map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            let identical: i64 = conn.query_row("SELECT count(*) FROM transactions WHERE merchant_display = 'Blue Bottle Coffee' AND amount_minor = 1234", [], |r| r.get(0)).map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            let aliases: i64 = conn.query_row("SELECT count(*) FROM transaction_overrides WHERE merchant_alias IS NOT NULL", [], |r| r.get(0)).map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            let partial: i64 = conn.query_row("SELECT count(*) FROM sync_runs WHERE status = 'partial'", [], |r| r.get(0)).map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            Ok((transactions, accounts, connections, pending, eur, linked, refunds, identical, aliases, partial))
        })
        .unwrap();
    let (
        transactions,
        accounts,
        connections,
        pending,
        eur,
        linked,
        refunds,
        identical,
        aliases,
        partial,
    ) = counts;
    assert!(
        transactions >= 150,
        "demo has only {transactions} transactions"
    );
    assert_eq!(accounts, 11);
    assert_eq!(connections, 5);
    assert!(pending > 0);
    assert!(eur >= 6);
    assert!(linked > 0);
    assert!(refunds > 0);
    assert_eq!(
        identical, 2,
        "legitimate identical purchases must both exist"
    );
    assert!(aliases > 0);
    assert!(partial > 0);
    assert_eq!(state.mode(), SessionMode::Demo);

    // Demo database is separate from the real path.
    let real_db = state.paths.database.clone();
    assert!(!real_db.exists());
    assert!(tmp.path().join("demo/demo.cmdb").exists());
}

#[test]
fn demo_cannot_be_used_with_the_real_vault_or_mutate_ledger_data() {
    let (tmp, state) = state();
    let app = mock_app();
    // Real vault exists with a marker.
    vault::create(&state, SecretString::new(PASS.to_string())).unwrap();
    state
        .with_db(|conn, _h, _s| {
            conn.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES ('real-marker', '\"yes\"', '2026-10-23T12:00:00Z')",
                [],
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            Ok(())
        })
        .unwrap();
    let real_db = state.paths.database.clone();

    // Demo mode closes the real session and must never replace the real DB;
    // the real ledger is reopened with its own passphrase below.
    demo::open(&state, &app).unwrap();
    assert_eq!(state.mode(), SessionMode::Demo);
    assert!(real_db.exists());
    assert!(state.require_real("mutation").is_err());
    assert!(vault::change_passphrase(
        &state,
        &app,
        SecretString::new(PASS.into()),
        SecretString::new("new correct horse battery staple".into())
    )
    .is_err());

    // The fixed demo key cannot open the real encrypted ledger.
    let demo_key = LockedKey::from_slice(
        &hex::decode("a2ceb32f7ebfe63ff306dcb0eb26326ca99c31998730f72c1446aafb2697164d").unwrap(),
    )
    .unwrap();
    assert!(open_ledger(&real_db, &demo_key, false).is_err());

    // Demo reset recreates only the demo DB; the real ledger still opens with
    // its own passphrase and still contains its marker.
    demo::reset(&state, &app).unwrap();
    let demo_db = tmp.path().join("demo/demo.cmdb");
    assert!(std::fs::metadata(&demo_db).unwrap().len() > 0);

    // Closing demo returns to the real locked gate.
    let closed = demo::close(&state, &app);
    assert!(!closed.unlocked);
    assert_eq!(closed.mode, "real");
    assert!(state.mode() == SessionMode::Real);
    let reopened = vault::unlock(&state, SecretString::new(PASS.to_string())).unwrap();
    assert!(reopened.unlocked);
    assert_eq!(reopened.mode, "real");
    let marker: i64 = state
        .with_db(|conn, _h, _s| {
            conn.query_row(
                "SELECT count(*) FROM settings WHERE key = 'real-marker'",
                [],
                |row| row.get(0),
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)
        })
        .unwrap();
    assert_eq!(marker, 1);
}

#[test]
fn demo_dashboard_is_deterministic_and_month_navigation_is_backend_computed() {
    let (_tmp, state) = state();
    let app = mock_app();
    demo::open(&state, &app).unwrap();
    let first = dashboard::get(&state, Some("2026-10".into())).unwrap();
    assert_eq!(first.month, "2026-10");
    assert_eq!(first.current_month, "2026-10");
    assert_eq!(first.previous_month, "2026-09");
    assert_eq!(first.next_month, "2026-11");
    assert!(first.summary.net_spending_minor != 0);
    assert!(first.summary.reconciliation.exact);
    assert_eq!(first.overview.accounts.len(), 11);
    assert_eq!(first.overview.accounts.len(), 11);
    assert!(first.overview.balances.known_total_minor > 0);
    assert!(first.overview.balances.accounts_missing_balance >= 1);
    assert!(first.account_spend.iter().any(|row| row.currency == "EUR"));

    // Pending-only and other months work.
    let september = dashboard::get(&state, Some("2026-09".into())).unwrap();
    assert_eq!(september.month, "2026-09");
    assert!(september.summary.net_spending_minor > 0);
    let august = dashboard::get(&state, Some("2026-08".into())).unwrap();
    assert!(august.summary.net_spending_minor > 0);
    // Invalid month rejected.
    assert!(dashboard::get(&state, Some("2026-13".into())).is_err());

    // Reopening/resetting yields the same deterministic totals.
    let first_total = first.summary.net_spending_minor;
    demo::close(&state, &app);
    demo::open(&state, &app).unwrap();
    let reopened = dashboard::get(&state, Some("2026-10".into())).unwrap();
    assert_eq!(reopened.summary.net_spending_minor, first_total);
    demo::reset(&state, &app).unwrap();
    let reset = dashboard::get(&state, Some("2026-10".into())).unwrap();
    assert_eq!(reset.summary.net_spending_minor, first_total);
}

#[test]
fn demo_overview_exposes_unknown_zero_stale_and_reconnect_states() {
    let (_tmp, state) = state();
    let app = mock_app();
    demo::open(&state, &app).unwrap();
    let dash = dashboard::get(&state, Some("2026-10".into())).unwrap();
    let kinds: Vec<String> = dash
        .overview
        .accounts
        .iter()
        .map(|account| account.payment.due_amount_kind.clone())
        .collect();
    assert!(kinds.iter().any(|kind| kind == "reported_zero"));
    assert!(kinds.iter().any(|kind| kind == "not_reported"));
    assert!(kinds.iter().any(|kind| kind == "minimum_due"));
    assert!(kinds.iter().any(|kind| kind == "no_due_date"));
    assert!(dash.overview.balances.accounts_missing_balance >= 1);
    assert!(dash
        .overview
        .accounts
        .iter()
        .any(|account| account.balance_stale));
    assert!(dash
        .overview
        .accounts
        .iter()
        .any(|account| account.connection_status.as_deref() == Some("reconnect_required")));
    assert!(dash
        .overview
        .accounts
        .iter()
        .any(|account| account.connection_status.as_deref() == Some("error")));
    let manual = dash
        .overview
        .accounts
        .iter()
        .find(|account| account.display_name == "Double Cash (demo)")
        .unwrap();
    assert_eq!(manual.payment.due_source.as_deref(), Some("manual"));
    // Upcoming only next 14 days; unknown amounts are not summed.
    assert!(!dash.overview.upcoming.entries.is_empty());
    assert!(dash.overview.upcoming.unknown_amount_count >= 1);
    assert!(!dash.overview.upcoming.complete);
}

#[test]
fn demo_fixtures_are_available_for_ui_expectation_tests() {
    assert_eq!(demo_fixtures::DEMO_VAULT_ID.len(), 32);
    assert_eq!(demo::demo_today().to_string(), "2026-10-23");
}
