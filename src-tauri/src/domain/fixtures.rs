//! Deterministic domain fixtures used by financial tests (and later by the demo
//! generator). All IDs, amounts, and dates are fixed; no clock or randomness.

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

pub const ACCOUNT_EVERYDAY: &str = "11111111-1111-4111-8111-111111111111";
pub const ACCOUNT_TRAVEL: &str = "22222222-2222-4222-8222-222222222222";
pub const ACCOUNT_EURO: &str = "33333333-3333-4333-8333-333333333333";

const NOW: &str = "2026-10-23T12:00:00Z";

fn tx_id(index: u32) -> String {
    format!("00000000-0000-4000-8000-{index:012}")
}

struct FixtureTx {
    account: &'static str,
    reporting: &'static str,
    amount: i64,
    kind: &'static str,
    merchant: &'static str,
    category: Option<&'static str>,
    pending: bool,
    excluded: bool,
    currency: &'static str,
    authorized: Option<&'static str>,
    posted: Option<&'static str>,
}

const FIXTURE_TX: &[FixtureTx] = &[
    // Leap day.
    FixtureTx {
        account: ACCOUNT_EVERYDAY,
        reporting: "2024-02-29",
        amount: 1_111,
        kind: "purchase",
        merchant: "Leap Shop",
        category: Some("Leap"),
        pending: false,
        excluded: false,
        currency: "USD",
        authorized: None,
        posted: Some("2024-02-29"),
    },
    // New York DST spring-forward and fall-back calendar days.
    FixtureTx {
        account: ACCOUNT_EVERYDAY,
        reporting: "2026-03-08",
        amount: 2_222,
        kind: "purchase",
        merchant: "DST Spring",
        category: Some("DST"),
        pending: false,
        excluded: false,
        currency: "USD",
        authorized: None,
        posted: Some("2026-03-08"),
    },
    FixtureTx {
        account: ACCOUNT_EVERYDAY,
        reporting: "2026-11-01",
        amount: 3_333,
        kind: "purchase",
        merchant: "DST Fall",
        category: Some("DST"),
        pending: false,
        excluded: false,
        currency: "USD",
        authorized: None,
        posted: Some("2026-11-01"),
    },
    // Year boundary: authorized Dec 31, posted Jan 2.
    FixtureTx {
        account: ACCOUNT_EVERYDAY,
        reporting: "2026-12-31",
        amount: 4_444,
        kind: "purchase",
        merchant: "Year End",
        category: Some("Holiday"),
        pending: false,
        excluded: false,
        currency: "USD",
        authorized: Some("2026-12-31"),
        posted: Some("2027-01-02"),
    },
    // Refund received in a later month than the purchase.
    FixtureTx {
        account: ACCOUNT_TRAVEL,
        reporting: "2026-09-20",
        amount: 9_000,
        kind: "purchase",
        merchant: "Refundable Co",
        category: Some("Travel"),
        pending: false,
        excluded: false,
        currency: "USD",
        authorized: None,
        posted: Some("2026-09-20"),
    },
    FixtureTx {
        account: ACCOUNT_TRAVEL,
        reporting: "2026-10-05",
        amount: -2_500,
        kind: "refund",
        merchant: "Refundable Co",
        category: Some("Travel"),
        pending: false,
        excluded: false,
        currency: "USD",
        authorized: None,
        posted: Some("2026-10-05"),
    },
    // Pending plus posted replacement; only the posted row contributes.
    FixtureTx {
        account: ACCOUNT_EVERYDAY,
        reporting: "2026-10-18",
        amount: 7_500,
        kind: "purchase",
        merchant: "Pending Pair",
        category: Some("Shopping"),
        pending: true,
        excluded: false,
        currency: "USD",
        authorized: Some("2026-10-18"),
        posted: None,
    },
    FixtureTx {
        account: ACCOUNT_EVERYDAY,
        reporting: "2026-10-19",
        amount: 7_500,
        kind: "purchase",
        merchant: "Pending Pair",
        category: Some("Shopping"),
        pending: false,
        excluded: false,
        currency: "USD",
        authorized: None,
        posted: None,
    },
    // Fees and interest are spending; payments and transfers are not.
    FixtureTx {
        account: ACCOUNT_EVERYDAY,
        reporting: "2026-10-10",
        amount: 500,
        kind: "fee",
        merchant: "Annual Fee",
        category: Some("Fees"),
        pending: false,
        excluded: false,
        currency: "USD",
        authorized: None,
        posted: None,
    },
    FixtureTx {
        account: ACCOUNT_EVERYDAY,
        reporting: "2026-10-11",
        amount: 200,
        kind: "interest",
        merchant: "Interest Charge",
        category: Some("Interest"),
        pending: false,
        excluded: false,
        currency: "USD",
        authorized: None,
        posted: None,
    },
    FixtureTx {
        account: ACCOUNT_EVERYDAY,
        reporting: "2026-10-05",
        amount: -5_000,
        kind: "payment",
        merchant: "Payment Thank You",
        category: None,
        pending: false,
        excluded: false,
        currency: "USD",
        authorized: None,
        posted: None,
    },
    FixtureTx {
        account: ACCOUNT_EVERYDAY,
        reporting: "2026-10-06",
        amount: 1_000,
        kind: "transfer",
        merchant: "Balance Transfer",
        category: None,
        pending: false,
        excluded: false,
        currency: "USD",
        authorized: None,
        posted: None,
    },
    // Explicit exclusion.
    FixtureTx {
        account: ACCOUNT_EVERYDAY,
        reporting: "2026-10-12",
        amount: 4_000,
        kind: "purchase",
        merchant: "Excluded Shop",
        category: Some("Other"),
        pending: false,
        excluded: true,
        currency: "USD",
        authorized: None,
        posted: None,
    },
    // Two legitimate identical purchases.
    FixtureTx {
        account: ACCOUNT_EVERYDAY,
        reporting: "2026-10-15",
        amount: 2_000,
        kind: "purchase",
        merchant: "Corner Cafe",
        category: Some("Dining"),
        pending: false,
        excluded: false,
        currency: "USD",
        authorized: None,
        posted: None,
    },
    FixtureTx {
        account: ACCOUNT_EVERYDAY,
        reporting: "2026-10-15",
        amount: 2_000,
        kind: "purchase",
        merchant: "Corner Cafe",
        category: Some("Dining"),
        pending: false,
        excluded: false,
        currency: "USD",
        authorized: None,
        posted: None,
    },
    // Synthetic non-USD row, kept separate.
    FixtureTx {
        account: ACCOUNT_EURO,
        reporting: "2026-10-15",
        amount: 5_000,
        kind: "purchase",
        merchant: "Euro Shop",
        category: Some("Dining"),
        pending: false,
        excluded: false,
        currency: "EUR",
        authorized: None,
        posted: None,
    },
];

pub fn seed(conn: &Connection) -> AppResult<()> {
    for (id, name, last_four, currency) in [
        (ACCOUNT_EVERYDAY, "Fixture Everyday", "1111", "USD"),
        (ACCOUNT_TRAVEL, "Fixture Travel", "2222", "USD"),
        (ACCOUNT_EURO, "Fixture Euro", "3333", "EUR"),
    ] {
        conn.execute(
            "INSERT INTO accounts (id, issuer, display_name, last_four, account_type, currency, \
                credit_limit_minor, include_in_overview, archived, source_kind, created_at, updated_at) \
             VALUES (?1, 'Fixture Bank', ?2, ?3, 'credit_card', ?4, 500000, 1, 0, 'demo', ?5, ?5)",
            rusqlite::params![id, name, last_four, currency, NOW],
        )
        .map_err(|_| AppError::Database)?;
    }

    for (index, tx) in FIXTURE_TX.iter().enumerate() {
        let source = if tx.authorized.is_some() {
            "authorized"
        } else if tx.posted.is_some() {
            "posted"
        } else {
            "manual"
        };
        conn.execute(
            "INSERT INTO transactions (id, account_id, source_kind, provider_transaction_id, \
                pending, authorized_date, posted_date, reporting_date, reporting_date_source, \
                amount_minor, currency, raw_description, merchant_normalized, merchant_display, \
                category, tx_type, excluded, removed, created_at, updated_at) \
             VALUES (?1, ?2, 'demo', NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 0, ?16, ?16)",
            rusqlite::params![
                tx_id(index as u32 + 1),
                tx.account,
                i64::from(tx.pending),
                tx.authorized,
                tx.posted,
                tx.reporting,
                source,
                tx.amount,
                tx.currency,
                format!("{} raw description", tx.merchant),
                tx.merchant.to_lowercase(),
                tx.merchant,
                tx.category,
                tx.kind,
                i64::from(tx.excluded),
                NOW
            ],
        )
        .map_err(|_| AppError::Database)?;
    }
    // Link the pending row to its posted replacement.
    conn.execute(
        "UPDATE transactions SET pending_link_id = ?1 WHERE id = ?2",
        rusqlite::params![tx_id(8), tx_id(7)],
    )
    .map_err(|_| AppError::Database)?;

    // One known balance, one missing balance (unknown is not zero).
    conn.execute(
        "INSERT INTO balance_snapshots (id, account_id, current_balance_minor, available_credit_minor, \
            credit_limit_minor, currency, retrieved_at, provider_as_of, provenance, source_connection_id) \
         VALUES ('44444444-4444-4444-8444-444444444444', ?1, 50000, 200000, 500000, 'USD', ?2, NULL, 'provider', NULL)",
        rusqlite::params![ACCOUNT_EVERYDAY, NOW],
    )
    .map_err(|_| AppError::Database)?;
    conn.execute(
        "INSERT INTO payment_snapshots (id, account_id, next_due_date, minimum_due_minor, \
            statement_balance_minor, last_statement_issue_date, next_closing_date, retrieved_at, provenance, source_connection_id) \
         VALUES ('55555555-5555-4555-8555-555555555555', ?1, '2026-10-30', NULL, 120000, '2026-10-05', NULL, ?2, 'provider', NULL)",
        rusqlite::params![ACCOUNT_EVERYDAY, NOW],
    )
    .map_err(|_| AppError::Database)?;
    conn.execute(
        "INSERT INTO manual_payment_metadata (account_id, statement_balance_minor, minimum_due_minor, \
            next_due_date, valid_through_date, created_at, updated_at) \
         VALUES (?1, 80000, 0, '2026-10-27', '2026-10-31', ?2, ?2)",
        rusqlite::params![ACCOUNT_TRAVEL, NOW],
    )
    .map_err(|_| AppError::Database)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded() -> Connection {
        // An in-memory plain SQLite connection is not used here: this unit test
        // only checks the fixture table shape. The encrypted integration test
        // exercises summary reconciliation.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE accounts (id TEXT); CREATE TABLE transactions (id TEXT);")
            .unwrap();
        conn
    }

    #[test]
    fn fixture_covers_required_shapes() {
        assert!(FIXTURE_TX.iter().any(|tx| tx.reporting == "2024-02-29"));
        assert!(FIXTURE_TX.iter().any(|tx| tx.reporting == "2026-03-08"));
        assert!(FIXTURE_TX.iter().any(|tx| tx.reporting == "2026-11-01"));
        assert!(FIXTURE_TX.iter().any(|tx| tx.pending));
        assert!(FIXTURE_TX.iter().any(|tx| tx.excluded));
        assert!(FIXTURE_TX
            .iter()
            .any(|tx| tx.kind == "refund" && tx.reporting.starts_with("2026-10")));
        assert!(
            FIXTURE_TX
                .iter()
                .filter(|tx| tx.merchant == "Corner Cafe")
                .count()
                == 2
        );
        assert!(FIXTURE_TX.iter().any(|tx| tx.currency == "EUR"));
        assert_eq!(ACCOUNT_EVERYDAY.len(), 36);
        let _ = seeded();
    }
}
