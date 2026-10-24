//! Deterministic financial fixture assertions from spec §16.

use cardigan_money_lib::domain::dates::Month;
use cardigan_money_lib::domain::fixtures;
use cardigan_money_lib::security::crypto::SecretString;
use cardigan_money_lib::services::vault;
use cardigan_money_lib::state::AppState;
use cardigan_money_lib::storage::ledger::month_summary;
use cardigan_money_lib::storage::overview::overview;
use chrono::NaiveDate;

fn seeded() -> (tempfile::TempDir, AppState) {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::new(tmp.path().to_path_buf()).unwrap();
    vault::create(
        &state,
        SecretString::new("correct horse battery staple fixtures".to_string()),
    )
    .unwrap();
    state.with_db(|conn, _h, _s| fixtures::seed(conn)).unwrap();
    (tmp, state)
}

fn summary(
    state: &AppState,
    year: i32,
    month: u32,
) -> cardigan_money_lib::storage::ledger::MonthSummary {
    state
        .with_db(|conn, _h, _s| month_summary(conn, Month::new(year, month).unwrap(), "USD"))
        .unwrap()
}

#[test]
fn fixture_month_totals_cover_leap_dst_boundary_refund_and_pending() {
    let (_tmp, state) = seeded();
    let leap = summary(&state, 2024, 2);
    assert_eq!(leap.net_spending_minor, 1_111);

    assert_eq!(summary(&state, 2026, 3).net_spending_minor, 2_222);
    assert_eq!(summary(&state, 2026, 11).net_spending_minor, 3_333);

    let september = summary(&state, 2026, 9);
    assert_eq!(september.net_spending_minor, 9_000);
    let october = summary(&state, 2026, 10);
    assert_eq!(october.refunds_minor, -2_500);
    assert_eq!(october.pending_subtotal_minor, 7_500);
    assert_eq!(october.gross_purchases_minor, 11_500);
    assert_eq!(october.fees_interest_minor, 700);
    assert_eq!(october.net_spending_minor, 9_700);
    assert_eq!(october.pending_count, 1);
    assert_eq!(october.eligible_count, 6); // posted pair, refund, fee, interest, two cafe purchases
    assert!(october.reconciliation.exact);
    assert_eq!(october.other_currency_row_count, 1);

    let december = summary(&state, 2026, 12);
    assert_eq!(december.net_spending_minor, 4_444);
    assert_eq!(summary(&state, 2027, 1).net_spending_minor, 0);

    let cafe = october
        .suppliers
        .iter()
        .find(|s| s.key == "Corner Cafe")
        .unwrap();
    assert_eq!(cafe.count, 2);
    assert!(october.suppliers.iter().all(|s| s.key != "Euro Shop"));
    assert!(october
        .suppliers
        .iter()
        .all(|s| s.key != "Payment Thank You"));
    assert!(october
        .suppliers
        .iter()
        .all(|s| s.key != "Balance Transfer"));
    assert!(october.suppliers.iter().all(|s| s.key != "Excluded Shop"));
}

#[test]
fn fixture_metadata_keeps_unknown_and_reported_zero_distinct() {
    let (_tmp, state) = seeded();
    let view = state
        .with_db(|conn, _h, _s| overview(conn, NaiveDate::from_ymd_opt(2026, 10, 23).unwrap()))
        .unwrap();
    assert_eq!(view.balances.known_total_minor, 50_000);
    assert_eq!(view.balances.accounts_missing_balance, 2);
    assert!(!view.balances.complete);

    let everyday = view
        .accounts
        .iter()
        .find(|a| a.display_name == "Fixture Everyday")
        .unwrap();
    assert_eq!(everyday.payment.due_amount_minor, None);
    assert_eq!(everyday.payment.due_amount_kind, "not_reported");
    assert_eq!(everyday.payment.statement_balance_minor, Some(120_000));

    let travel = view
        .accounts
        .iter()
        .find(|a| a.display_name == "Fixture Travel")
        .unwrap();
    assert_eq!(travel.payment.due_amount_kind, "reported_zero");
    assert_eq!(travel.payment.due_amount_minor, Some(0));

    assert_eq!(view.upcoming.entries.len(), 2);
    assert_eq!(view.upcoming.known_total_minor, 0);
    assert_eq!(view.upcoming.unknown_amount_count, 1);
    assert!(!view.upcoming.complete);
}
