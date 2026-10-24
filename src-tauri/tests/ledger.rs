//! Financial-domain integration tests against a real encrypted SQLCipher vault.

use chrono::NaiveDate;

use cardigan_money_lib::domain::dates::Month;
use cardigan_money_lib::security::crypto::SecretString;
use cardigan_money_lib::services::vault;
use cardigan_money_lib::state::AppState;
use cardigan_money_lib::storage::accounts::{create_manual, AccountInput, AccountType};
use cardigan_money_lib::storage::ledger::{month_summary, query_ledger, LedgerQuery};
use cardigan_money_lib::storage::overview::overview;

const PASS: &str = "correct horse battery staple ledger";

fn setup() -> (tempfile::TempDir, AppState) {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::new(tmp.path().to_path_buf()).unwrap();
    vault::create(&state, SecretString::new(PASS.to_string())).unwrap();
    (tmp, state)
}

fn account(state: &AppState, name: &str, currency: &str) -> String {
    state
        .with_db(|conn, _header, _settings| {
            create_manual(
                conn,
                &AccountInput {
                    issuer: "Test Bank".into(),
                    display_name: name.into(),
                    last_four: Some("4242".into()),
                    account_type: AccountType::CreditCard,
                    currency: currency.into(),
                    credit_limit_minor: Some(500_000),
                    include_in_overview: true,
                },
            )
            .map(|account| account.id)
        })
        .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn tx(
    state: &AppState,
    account_id: &str,
    reporting_date: &str,
    amount_minor: i64,
    tx_type: &str,
    merchant: &str,
    category: Option<&str>,
    pending: bool,
    excluded: bool,
    currency: &str,
    authorized: Option<&str>,
    posted: Option<&str>,
) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let source = if authorized.is_some() {
        "authorized"
    } else if posted.is_some() {
        "posted"
    } else {
        "manual"
    };
    state
        .with_db(|conn, _header, _settings| {
            conn.execute(
                "INSERT INTO transactions (id, account_id, source_kind, provider_transaction_id, \
                    pending, authorized_date, posted_date, reporting_date, reporting_date_source, \
                    amount_minor, currency, raw_description, merchant_normalized, merchant_display, \
                    category, tx_type, excluded, removed, created_at, updated_at) \
                 VALUES (?1, ?2, 'demo', NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 0, ?16, ?16)",
                rusqlite::params![
                    id,
                    account_id,
                    i64::from(pending),
                    authorized,
                    posted,
                    reporting_date,
                    source,
                    amount_minor,
                    currency,
                    format!("{merchant} raw description"),
                    merchant.to_lowercase(),
                    merchant,
                    category,
                    tx_type,
                    i64::from(excluded),
                    "2026-10-23T12:00:00Z"
                ],
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            Ok(())
        })
        .unwrap();
    id
}

fn balance(state: &AppState, account_id: &str, minor: i64) {
    state
        .with_db(|conn, _header, _settings| {
            conn.execute(
                "INSERT INTO balance_snapshots (id, account_id, current_balance_minor, available_credit_minor, \
                    credit_limit_minor, currency, retrieved_at, provider_as_of, provenance, source_connection_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, 'USD', '2026-10-23T10:00:00Z', NULL, 'manual', NULL)",
                rusqlite::params![uuid::Uuid::new_v4().to_string(), account_id, minor, 300_000i64, 500_000i64],
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            Ok(())
        })
        .unwrap();
}

fn provider_payment(
    state: &AppState,
    account_id: &str,
    due: Option<&str>,
    minimum: Option<i64>,
    statement: Option<i64>,
) {
    state
        .with_db(|conn, _header, _settings| {
            conn.execute(
                "INSERT INTO payment_snapshots (id, account_id, next_due_date, minimum_due_minor, \
                    statement_balance_minor, last_statement_issue_date, next_closing_date, retrieved_at, provenance, source_connection_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, '2026-10-05', NULL, '2026-10-23T10:00:00Z', 'provider', NULL)",
                rusqlite::params![uuid::Uuid::new_v4().to_string(), account_id, due, minimum, statement],
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            Ok(())
        })
        .unwrap();
}

fn manual_payment(
    state: &AppState,
    account_id: &str,
    due: Option<&str>,
    minimum: Option<i64>,
    statement: Option<i64>,
    valid_through: Option<&str>,
) {
    state
        .with_db(|conn, _header, _settings| {
            conn.execute(
                "INSERT INTO manual_payment_metadata (account_id, statement_balance_minor, minimum_due_minor, \
                    next_due_date, valid_through_date, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, '2026-10-23T10:00:00Z', '2026-10-23T10:00:00Z')",
                rusqlite::params![account_id, statement, minimum, due, valid_through],
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            Ok(())
        })
        .unwrap();
}

fn october_fixture(state: &AppState) -> String {
    let account = account(state, "Everyday Card", "USD");
    tx(
        state,
        &account,
        "2026-10-02",
        10_000,
        "purchase",
        "Acme Market",
        Some("Groceries"),
        false,
        false,
        "USD",
        None,
        None,
    );
    tx(
        state,
        &account,
        "2026-10-31",
        5_000,
        "purchase",
        "Bilt",
        Some("Travel"),
        false,
        false,
        "USD",
        Some("2026-10-31"),
        Some("2026-11-02"),
    );
    tx(
        state,
        &account,
        "2026-10-20",
        -2_500,
        "refund",
        "Acme Market",
        Some("Groceries"),
        false,
        false,
        "USD",
        None,
        None,
    );
    tx(
        state,
        &account,
        "2026-10-10",
        500,
        "fee",
        "Acme Market",
        Some("Fees"),
        false,
        false,
        "USD",
        None,
        None,
    );
    tx(
        state,
        &account,
        "2026-10-11",
        200,
        "interest",
        "Citi",
        Some("Interest"),
        false,
        false,
        "USD",
        None,
        None,
    );
    tx(
        state,
        &account,
        "2026-10-05",
        -20_000,
        "payment",
        "Payment Thank You",
        None,
        false,
        false,
        "USD",
        None,
        None,
    );
    tx(
        state,
        &account,
        "2026-10-06",
        7_000,
        "transfer",
        "Transfer",
        None,
        false,
        false,
        "USD",
        None,
        None,
    );
    tx(
        state,
        &account,
        "2026-10-12",
        4_000,
        "purchase",
        "Acme Market",
        Some("Groceries"),
        false,
        true,
        "USD",
        None,
        None,
    );
    tx(
        state,
        &account,
        "2026-10-18",
        7_500,
        "purchase",
        "Pending Shop",
        Some("Shopping"),
        true,
        false,
        "USD",
        None,
        None,
    );
    tx(
        state,
        &account,
        "2026-10-15",
        2_000,
        "purchase",
        "Corner Cafe",
        Some("Dining"),
        false,
        false,
        "USD",
        None,
        None,
    );
    tx(
        state,
        &account,
        "2026-10-15",
        2_000,
        "purchase",
        "Corner Cafe",
        Some("Dining"),
        false,
        false,
        "USD",
        None,
        None,
    );
    tx(
        state,
        &account,
        "2026-10-15",
        5_000,
        "purchase",
        "Cafe Europe",
        Some("Dining"),
        false,
        false,
        "EUR",
        None,
        None,
    );
    tx(
        state,
        &account,
        "2026-11-01",
        9_999,
        "purchase",
        "November Shop",
        Some("Shopping"),
        false,
        false,
        "USD",
        None,
        Some("2026-11-01"),
    );
    account
}

#[test]
fn october_summary_components_reconcile_exactly() {
    let (_tmp, state) = setup();
    let _account = october_fixture(&state);
    let summary = state
        .with_db(|conn, _header, _settings| {
            month_summary(conn, Month::new(2026, 10).unwrap(), "USD")
        })
        .unwrap();

    assert_eq!(summary.gross_purchases_minor, 19_000);
    assert_eq!(summary.refunds_minor, -2_500);
    assert_eq!(summary.fees_interest_minor, 700);
    assert_eq!(summary.net_spending_minor, 17_200);
    assert_eq!(summary.pending_subtotal_minor, 7_500);
    assert_eq!(summary.pending_count, 1);
    assert_eq!(summary.eligible_count, 7);
    assert_eq!(summary.other_currency_row_count, 1);
    // October rows: 13 inserted; November row is outside the month.
    assert_eq!(summary.row_count, 12);
    assert!(summary.reconciliation.exact);
    assert_eq!(summary.reconciliation.eligible_net_minor, 17_200);
    assert_eq!(summary.reconciliation.supplier_net_minor, 17_200);
    assert_eq!(summary.reconciliation.category_net_minor, 17_200);

    let acme = summary
        .suppliers
        .iter()
        .find(|s| s.key == "Acme Market")
        .unwrap();
    assert_eq!(acme.gross_minor, 10_500);
    assert_eq!(acme.refunds_minor, -2_500);
    assert_eq!(acme.net_minor, 8_000);
    assert_eq!(acme.count, 3); // excluded purchase is not eligible
    let cafe = summary
        .suppliers
        .iter()
        .find(|s| s.key == "Corner Cafe")
        .unwrap();
    assert_eq!(
        cafe.count, 2,
        "identical legitimate purchases stay distinct"
    );
    assert_eq!(cafe.net_minor, 4_000);
    assert!(summary.suppliers.iter().all(|s| s.key != "Cafe Europe"));
    assert!(summary
        .suppliers
        .iter()
        .all(|s| s.key != "Payment Thank You"));

    let groceries = summary
        .categories
        .iter()
        .find(|c| c.key == "Groceries")
        .unwrap();
    assert_eq!(groceries.net_minor, 7_500);
}

#[test]
fn month_boundary_uses_authorization_date_and_historical_month_is_independent() {
    let (_tmp, state) = setup();
    let account = october_fixture(&state);
    balance(&state, &account, 123_456);
    let october = state
        .with_db(|conn, _h, _s| month_summary(conn, Month::new(2026, 10).unwrap(), "USD"))
        .unwrap();
    let september = state
        .with_db(|conn, _h, _s| month_summary(conn, Month::new(2026, 9).unwrap(), "USD"))
        .unwrap();
    assert_eq!(september.net_spending_minor, 0);
    assert_eq!(october.net_spending_minor, 17_200);
    let view = state
        .with_db(|conn, _h, _s| overview(conn, NaiveDate::from_ymd_opt(2026, 10, 23).unwrap()))
        .unwrap();
    assert_eq!(view.balances.known_total_minor, 123_456);
    assert!(view.balances.complete);
}

#[test]
fn pending_is_separate_and_posted_replacement_does_not_double_count() {
    let (_tmp, state) = setup();
    let account = account(&state, "Pending Card", "USD");
    let pending_id = tx(
        &state,
        &account,
        "2026-10-18",
        7_500,
        "purchase",
        "Pending Shop",
        Some("Shopping"),
        true,
        false,
        "USD",
        None,
        None,
    );
    let posted_id = tx(
        &state,
        &account,
        "2026-10-19",
        7_500,
        "purchase",
        "Pending Shop",
        Some("Shopping"),
        false,
        false,
        "USD",
        None,
        None,
    );
    state
        .with_db(|conn, _h, _s| {
            conn.execute(
                "UPDATE transactions SET pending_link_id = ?1 WHERE id = ?2",
                rusqlite::params![pending_id, posted_id],
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            Ok(())
        })
        .unwrap();
    let summary = state
        .with_db(|conn, _h, _s| month_summary(conn, Month::new(2026, 10).unwrap(), "USD"))
        .unwrap();
    assert_eq!(summary.net_spending_minor, 7_500);
    assert_eq!(summary.pending_subtotal_minor, 7_500);
    assert_eq!(summary.eligible_count, 1);
}

#[test]
fn overrides_change_eligibility_and_display_without_changing_source_fields() {
    let (_tmp, state) = setup();
    let _account = october_fixture(&state);
    let first_id: String = state
        .with_db(|conn, _h, _s| {
            conn.query_row(
                "SELECT id FROM transactions WHERE merchant_display = 'Acme Market' AND amount_minor = 10000",
                [],
                |row| row.get(0),
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)
        })
        .unwrap();
    state
        .with_db(|conn, _h, _s| {
            conn.execute(
                "INSERT INTO transaction_overrides (transaction_id, merchant_alias, category, type_correction, excluded, notes, edited_at) \
                 VALUES (?1, 'Acme Market & Co', 'Household', NULL, 1, 'reviewed by owner', '2026-10-23T12:00:00Z')",
                [&first_id],
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            Ok(())
        })
        .unwrap();

    let summary = state
        .with_db(|conn, _h, _s| month_summary(conn, Month::new(2026, 10).unwrap(), "USD"))
        .unwrap();
    assert_eq!(summary.gross_purchases_minor, 9_000);
    assert_eq!(summary.net_spending_minor, 7_200);

    let page = state
        .with_db(|conn, _h, _s| {
            query_ledger(
                conn,
                &LedgerQuery {
                    start_date: "2026-10-01".into(),
                    end_date_exclusive: "2026-11-01".into(),
                    account_ids: vec![],
                    category: None,
                    status: None,
                    tx_type: None,
                    search: Some("Acme Market & Co".into()),
                    sort: None,
                    page: None,
                    page_size: Some(50),
                },
            )
        })
        .unwrap();
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0].merchant, "Acme Market & Co");
    assert!(page.rows[0].excluded);
    assert_eq!(page.rows[0].notes.as_deref(), Some("reviewed by owner"));
    assert_eq!(page.rows[0].raw_description, "Acme Market raw description");
}

#[test]
fn ledger_search_filters_sort_and_pagination_are_parameterized() {
    let (_tmp, state) = setup();
    let first_account = october_fixture(&state);
    let other = account(&state, "Other Card", "USD");
    tx(
        &state,
        &other,
        "2026-10-03",
        1_234,
        "purchase",
        "Zeta Shop",
        Some("Other"),
        false,
        false,
        "USD",
        None,
        None,
    );

    let page = state
        .with_db(|conn, _h, _s| {
            query_ledger(
                conn,
                &LedgerQuery {
                    start_date: "2026-10-01".into(),
                    end_date_exclusive: "2026-11-01".into(),
                    account_ids: vec![],
                    category: Some("Groceries".into()),
                    status: None,
                    tx_type: None,
                    search: None,
                    sort: Some("amount_desc".into()),
                    page: None,
                    page_size: Some(2),
                },
            )
        })
        .unwrap();
    assert_eq!(page.row_count, 3);
    assert_eq!(page.page_size, 2);
    assert_eq!(page.rows.len(), 2);
    assert!(page.rows[0].amount_minor >= page.rows[1].amount_minor);
    assert_eq!(page.eligible_subtotal_minor, 7_500);

    // Escaped LIKE: a literal % in the search matches nothing here.
    let percent = state
        .with_db(|conn, _h, _s| {
            query_ledger(
                conn,
                &LedgerQuery {
                    start_date: "2026-10-01".into(),
                    end_date_exclusive: "2026-11-01".into(),
                    account_ids: vec![],
                    category: None,
                    status: None,
                    tx_type: None,
                    search: Some("%".into()),
                    sort: None,
                    page: None,
                    page_size: None,
                },
            )
        })
        .unwrap();
    assert_eq!(percent.rows.len(), 0);

    // Injection-shaped search is a parameter, not SQL.
    let injection = state
        .with_db(|conn, _h, _s| {
            query_ledger(
                conn,
                &LedgerQuery {
                    start_date: "2026-10-01".into(),
                    end_date_exclusive: "2026-11-01".into(),
                    account_ids: vec![],
                    category: None,
                    status: None,
                    tx_type: None,
                    search: Some("'; DROP TABLE accounts; --".into()),
                    sort: None,
                    page: None,
                    page_size: None,
                },
            )
        })
        .unwrap();
    assert_eq!(injection.rows.len(), 0);
    let account_count: i64 = state
        .with_db(|conn, _h, _s| {
            conn.query_row("SELECT count(*) FROM accounts", [], |row| row.get(0))
                .map_err(|_| cardigan_money_lib::error::AppError::Database)
        })
        .unwrap();
    assert_eq!(account_count, 2);

    // A SQL-shaped sort is rejected by the closed enum before query execution.
    let bad_sort = state.with_db(|conn, _h, _s| {
        query_ledger(
            conn,
            &LedgerQuery {
                start_date: "2026-10-01".into(),
                end_date_exclusive: "2026-11-01".into(),
                account_ids: vec![],
                category: None,
                status: None,
                tx_type: None,
                search: None,
                sort: Some("amount_minor DESC; DROP TABLE accounts".into()),
                page: None,
                page_size: None,
            },
        )
    });
    assert!(bad_sort.is_err());

    // Account filter.
    let filtered = state
        .with_db(|conn, _h, _s| {
            query_ledger(
                conn,
                &LedgerQuery {
                    start_date: "2026-10-01".into(),
                    end_date_exclusive: "2026-11-01".into(),
                    account_ids: vec![other.clone()],
                    category: None,
                    status: None,
                    tx_type: None,
                    search: None,
                    sort: None,
                    page: None,
                    page_size: None,
                },
            )
        })
        .unwrap();
    assert_eq!(filtered.rows.len(), 1);
    assert_eq!(filtered.rows[0].account_id, other);
    let _ = first_account;
}

#[test]
fn current_balances_and_payment_metadata_keep_missing_distinct_from_zero() {
    let (_tmp, state) = setup();
    let known = account(&state, "Known Balance", "USD");
    let missing = account(&state, "Missing Balance", "USD");
    let zero_due = account(&state, "Zero Due", "USD");
    balance(&state, &known, 50_000);
    provider_payment(&state, &known, Some("2026-10-30"), None, Some(120_000));
    provider_payment(&state, &zero_due, Some("2026-10-25"), Some(0), Some(80_000));
    // Missing account gets no balance snapshot at all.
    let _ = missing;

    let view = state
        .with_db(|conn, _h, _s| overview(conn, NaiveDate::from_ymd_opt(2026, 10, 23).unwrap()))
        .unwrap();
    assert_eq!(view.balances.known_total_minor, 50_000);
    assert_eq!(view.balances.accounts_with_balance, 1);
    assert_eq!(view.balances.accounts_missing_balance, 2);
    assert!(!view.balances.complete);

    let known_card = view
        .accounts
        .iter()
        .find(|a| a.display_name == "Known Balance")
        .unwrap();
    assert_eq!(
        known_card.payment.next_due_date.as_deref(),
        Some("2026-10-30")
    );
    assert_eq!(known_card.payment.due_amount_minor, None);
    assert_eq!(known_card.payment.due_amount_kind, "not_reported");
    assert_eq!(known_card.payment.statement_balance_minor, Some(120_000));
    assert_ne!(
        known_card.payment.due_amount_minor, known_card.payment.statement_balance_minor,
        "a statement balance must never become the amount due"
    );
    let zero_card = view
        .accounts
        .iter()
        .find(|a| a.display_name == "Zero Due")
        .unwrap();
    assert_eq!(zero_card.payment.due_amount_minor, Some(0));
    assert_eq!(zero_card.payment.due_amount_kind, "reported_zero");
    assert_eq!(view.upcoming.entries.len(), 2);
    // Known balance total excludes the missing accounts; upcoming sums only known amounts.
    assert_eq!(view.upcoming.known_total_minor, 0);
    assert_eq!(view.upcoming.unknown_amount_count, 1);
    assert!(!view.upcoming.complete);
}

#[test]
fn valid_manual_payment_metadata_is_independent_and_can_override_provider() {
    let (_tmp, state) = setup();
    let account = account(&state, "Manual Card", "USD");
    provider_payment(
        &state,
        &account,
        Some("2026-10-30"),
        Some(3_000),
        Some(90_000),
    );
    manual_payment(
        &state,
        &account,
        Some("2026-10-27"),
        Some(2_500),
        Some(88_000),
        Some("2026-10-31"),
    );
    let view = state
        .with_db(|conn, _h, _s| overview(conn, NaiveDate::from_ymd_opt(2026, 10, 23).unwrap()))
        .unwrap();
    let card = &view.accounts[0];
    assert_eq!(card.payment.next_due_date.as_deref(), Some("2026-10-27"));
    assert_eq!(card.payment.due_amount_minor, Some(2_500));
    assert_eq!(card.payment.due_source.as_deref(), Some("manual"));
    assert_eq!(card.payment.statement_balance_minor, Some(88_000));
    assert_eq!(
        card.payment.statement_balance_source.as_deref(),
        Some("manual")
    );

    // Expired manual facts fall back to the provider, with the expiry surfaced.
    state
        .with_db(|conn, _h, _s| {
            conn.execute(
                "UPDATE manual_payment_metadata SET valid_through_date = '2026-10-01' WHERE account_id = ?1",
                [&account],
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            Ok(())
        })
        .unwrap();
    let view = state
        .with_db(|conn, _h, _s| overview(conn, NaiveDate::from_ymd_opt(2026, 10, 23).unwrap()))
        .unwrap();
    let card = &view.accounts[0];
    assert!(card.payment.manual_expired);
    assert_eq!(card.payment.next_due_date.as_deref(), Some("2026-10-30"));
    assert_eq!(card.payment.due_amount_minor, Some(3_000));
    assert_eq!(card.payment.due_source.as_deref(), Some("provider"));
}

#[test]
fn upcoming_window_and_non_usd_balances_are_separated() {
    let (_tmp, state) = setup();
    let within = account(&state, "Due Soon", "USD");
    let outside = account(&state, "Due Later", "USD");
    let eur = account(&state, "Euro Card", "EUR");
    balance(&state, &within, 10_000);
    balance(&state, &outside, 20_000);
    balance(&state, &eur, 30_000);
    provider_payment(
        &state,
        &within,
        Some("2026-10-25"),
        Some(1_000),
        Some(10_000),
    );
    provider_payment(
        &state,
        &outside,
        Some("2026-11-20"),
        Some(2_000),
        Some(20_000),
    );
    let view = state
        .with_db(|conn, _h, _s| overview(conn, NaiveDate::from_ymd_opt(2026, 10, 23).unwrap()))
        .unwrap();
    assert_eq!(view.upcoming.entries.len(), 1);
    assert_eq!(view.upcoming.entries[0].display_name, "Due Soon");
    assert_eq!(view.upcoming.known_total_minor, 1_000);
    assert!(view.upcoming.complete);
    assert_eq!(view.balances.known_total_minor, 30_000);
    assert_eq!(
        view.balances.other_currency,
        vec![("EUR".to_string(), 30_000)]
    );
    // USD accounts missing balances: none; EUR is counted separately, so complete is true.
    assert!(view.balances.complete);
}
