//! Deterministic synthetic demo fixtures. Frozen clock, fixed IDs, no personal
//! data. Covers every scenario required by spec §15.

use chrono::NaiveDate;
use rusqlite::Connection;

use crate::error::{AppError, AppResult};

pub const DEMO_VAULT_ID: &str = "deadbeefcafebabe0011223344556677";
pub const DEMO_TODAY: &str = "2026-10-23";
const NOW: &str = "2026-10-23T12:00:00Z";

pub fn demo_today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 23).expect("valid demo date")
}

pub struct DemoManifest {
    pub accounts: i64,
    pub connections: i64,
    pub transactions: i64,
    pub pending: i64,
    pub records: Vec<String>,
}

fn account_id(index: usize) -> String {
    format!("aaaaaaaa-0000-4000-8000-{index:012}")
}

fn connection_id(index: usize) -> String {
    format!("cccccccc-0000-4000-8000-{index:012}")
}

fn tx_id(index: usize) -> String {
    format!("dddddddd-0000-4000-8000-{index:012}")
}

const CONNECTIONS: &[(&str, &str, &str, &str, &str)] = &[
    // provider, environment, institution id, institution name, status
    ("mock", "demo", "demo-chase", "Chase (demo)", "healthy"),
    (
        "mock",
        "demo",
        "demo-synchrony",
        "Synchrony (demo)",
        "reconnect_required",
    ),
    ("mock", "demo", "demo-bilt", "Bilt (demo)", "healthy"),
    (
        "mock",
        "demo",
        "demo-amex",
        "American Express (demo)",
        "error",
    ),
    ("mock", "demo", "demo-citi", "Citi (demo)", "healthy"),
];

const ACCOUNTS: &[(&str, &str, &str, &str, Option<usize>)] = &[
    // issuer, display name, last four, currency, connection
    ("Chase", "Sapphire Reserve (demo)", "4101", "USD", Some(0)),
    ("Chase", "Freedom Unlimited (demo)", "4102", "USD", Some(0)),
    ("Chase", "Amazon Visa (demo)", "4103", "USD", Some(0)),
    (
        "Synchrony",
        "Amazon Store Card (demo)",
        "5501",
        "USD",
        Some(1),
    ),
    ("Synchrony", "PayPal Credit (demo)", "5502", "USD", Some(1)),
    ("Bilt", "Bilt Mastercard (demo)", "6601", "USD", Some(2)),
    (
        "American Express",
        "Platinum (demo)",
        "7701",
        "USD",
        Some(3),
    ),
    (
        "American Express",
        "Blue Cash Everyday (demo)",
        "7702",
        "USD",
        Some(3),
    ),
    ("Citi", "Double Cash (demo)", "8801", "USD", Some(4)),
    ("Citi", "Custom Cash (demo)", "8802", "USD", Some(4)),
    ("Demo Bank", "Euro Travel Card (demo)", "9901", "EUR", None),
];

const BALANCES: &[(usize, i64, usize)] = &[
    // account index, current balance minor, days before demo today
    (0, 432_100, 0),
    (1, 128_450, 1),
    (2, 76_200, 0),
    (3, 54_300, 0),
    (4, 0, 0),
    (5, 91_750, 0),
    (6, 210_000, 10),
    (7, 65_400, 10),
    (8, 33_200, 0),
    // account 9 (Custom Cash) intentionally has no balance snapshot (unknown)
    (10, 4_500, 0),
];

struct PaymentFixture {
    account: usize,
    due: Option<&'static str>,
    minimum: Option<i64>,
    statement: Option<i64>,
    issue: Option<&'static str>,
    closing: Option<&'static str>,
    provenance: &'static str,
}

const PAYMENTS: &[PaymentFixture] = &[
    PaymentFixture {
        account: 0,
        due: Some("2026-10-28"),
        minimum: Some(35_000),
        statement: Some(432_100),
        issue: Some("2026-10-05"),
        closing: Some("2026-11-05"),
        provenance: "provider",
    },
    PaymentFixture {
        account: 1,
        due: Some("2026-11-03"),
        minimum: Some(25_000),
        statement: Some(128_450),
        issue: Some("2026-10-10"),
        closing: None,
        provenance: "provider",
    },
    PaymentFixture {
        account: 2,
        due: Some("2026-11-12"),
        minimum: Some(15_000),
        statement: Some(76_200),
        issue: Some("2026-10-12"),
        closing: None,
        provenance: "provider",
    },
    // Due date reported, minimum amount NOT reported: must never display $0.
    PaymentFixture {
        account: 3,
        due: Some("2026-10-26"),
        minimum: None,
        statement: Some(54_300),
        issue: Some("2026-10-03"),
        closing: None,
        provenance: "provider",
    },
    // Explicitly reported zero due.
    PaymentFixture {
        account: 4,
        due: Some("2026-10-29"),
        minimum: Some(0),
        statement: Some(0),
        issue: Some("2026-10-02"),
        closing: None,
        provenance: "provider",
    },
    PaymentFixture {
        account: 5,
        due: Some("2026-10-30"),
        minimum: Some(9_000),
        statement: Some(91_750),
        issue: Some("2026-10-06"),
        closing: Some("2026-11-01"),
        provenance: "provider",
    },
    PaymentFixture {
        account: 6,
        due: Some("2026-11-05"),
        minimum: Some(21_000),
        statement: Some(210_000),
        issue: Some("2026-10-08"),
        closing: None,
        provenance: "provider",
    },
    PaymentFixture {
        account: 7,
        due: Some("2026-10-27"),
        minimum: Some(6_500),
        statement: Some(65_400),
        issue: Some("2026-10-04"),
        closing: None,
        provenance: "provider",
    },
    PaymentFixture {
        account: 9,
        due: Some("2026-10-31"),
        minimum: Some(1_200),
        statement: None,
        issue: None,
        closing: None,
        provenance: "provider",
    },
    // Manual, still valid.
    PaymentFixture {
        account: 8,
        due: Some("2026-10-29"),
        minimum: Some(3_300),
        statement: Some(33_200),
        issue: None,
        closing: None,
        provenance: "manual",
    },
    // Manual but expired: must fall back to whatever the provider reports (here: nothing).
    PaymentFixture {
        account: 3,
        due: Some("2026-10-24"),
        minimum: Some(5_000),
        statement: Some(50_000),
        issue: None,
        closing: None,
        provenance: "manual-expired",
    },
];

const MERCHANTS: &[(&str, &str, &str)] = &[
    // raw display, normalized/canonical merchant, category
    ("Whole Foods Market", "Whole Foods Market", "Groceries"),
    ("Blue Bottle Coffee", "Blue Bottle Coffee", "Dining"),
    ("Uber", "Uber", "Transport"),
    ("AMZN Mktp US", "Amazon", "Shopping"),
    ("Netflix", "Netflix", "Subscriptions"),
    ("Shell", "Shell", "Gas"),
    ("CVS Pharmacy", "CVS Pharmacy", "Health"),
    ("Delta Air Lines", "Delta Air Lines", "Travel"),
    ("Trader Joe's", "Trader Joe's", "Groceries"),
    ("Local Hardware", "Local Hardware", "Home"),
];

fn insert_connection(conn: &Connection, index: usize) -> AppResult<()> {
    let (provider, environment, institution_id, institution_name, status) = CONNECTIONS[index];
    let (last_success, error) = match status {
        "healthy" => (Some("2026-10-23T08:00:00Z"), None),
        "reconnect_required" => (Some("2026-09-30T08:00:00Z"), Some("reconnect_required")),
        "error" => (
            Some("2026-10-13T08:00:00Z"),
            Some("provider_temporarily_unavailable"),
        ),
        _ => (None, None),
    };
    conn.execute(
        "INSERT INTO connections (id, provider, environment, institution_id, institution_name, \
            credential_ref, status, last_attempt_at, last_success_at, cursor, sanitized_error, \
            last_remote_revocation_at, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10, NULL, ?11, ?11)",
        rusqlite::params![
            connection_id(index),
            provider,
            environment,
            institution_id,
            institution_name,
            connection_id(index),
            status,
            NOW,
            last_success,
            error,
            NOW
        ],
    )
    .map_err(|_| AppError::Database)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_transaction(
    conn: &Connection,
    index: usize,
    account: usize,
    reporting: &str,
    amount: i64,
    kind: &str,
    merchant: &str,
    category: &str,
    pending: bool,
    excluded: bool,
    currency: &str,
    authorized: Option<&str>,
    posted: Option<&str>,
    provider_id: Option<&str>,
) -> AppResult<String> {
    let source = if authorized.is_some() {
        "authorized"
    } else if posted.is_some() {
        "posted"
    } else {
        "manual"
    };
    let id = tx_id(index);
    conn.execute(
        "INSERT INTO transactions (id, account_id, source_kind, provider_transaction_id, \
            pending, authorized_date, posted_date, reporting_date, reporting_date_source, \
            amount_minor, currency, raw_description, merchant_normalized, merchant_display, \
            category, tx_type, excluded, removed, created_at, updated_at) \
         VALUES (?1, ?2, 'demo', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, 0, ?17, ?17)",
        rusqlite::params![
            id,
            account_id(account),
            provider_id,
            i64::from(pending),
            authorized,
            posted,
            reporting,
            source,
            amount,
            currency,
            format!("{merchant} synthetic demo"),
            merchant.to_lowercase(),
            merchant,
            category,
            kind,
            i64::from(excluded),
            NOW
        ],
    )
    .map_err(|_| AppError::Database)?;
    Ok(id)
}

pub fn seed(conn: &Connection) -> AppResult<DemoManifest> {
    for index in 0..CONNECTIONS.len() {
        insert_connection(conn, index)?;
    }
    for (index, (issuer, display_name, last_four, currency, connection)) in
        ACCOUNTS.iter().enumerate()
    {
        conn.execute(
            "INSERT INTO accounts (id, issuer, display_name, last_four, account_type, currency, \
                credit_limit_minor, include_in_overview, archived, source_kind, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 'credit_card', ?5, 1000000, 1, 0, 'demo', ?6, ?6)",
            rusqlite::params![
                account_id(index),
                issuer,
                display_name,
                last_four,
                currency,
                NOW
            ],
        )
        .map_err(|_| AppError::Database)?;
        if let Some(connection) = connection {
            conn.execute(
                "INSERT INTO account_sources (id, account_id, connection_id, provider_account_id, \
                    provider_mask, linked_at, unlinked_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    account_id(index),
                    connection_id(*connection),
                    format!("demo-provider-account-{index}"),
                    format!("•••• {last_four}"),
                    NOW
                ],
            )
            .map_err(|_| AppError::Database)?;
        }
    }

    for (account, balance, days_ago) in BALANCES {
        let retrieved = format!(
            "2026-10-{}T10:00:00Z",
            demo_today()
                .checked_sub_signed(chrono::Duration::days(*days_ago as i64))
                .map(|d| d.format("%d").to_string())
                .unwrap_or_else(|| "23".into())
        );
        let currency = ACCOUNTS[*account].3;
        conn.execute(
            "INSERT INTO balance_snapshots (id, account_id, current_balance_minor, available_credit_minor, \
                credit_limit_minor, currency, retrieved_at, provider_as_of, provenance, source_connection_id) \
             VALUES (?1, ?2, ?3, 500000, 1000000, ?4, ?5, ?5, 'provider', NULL)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                account_id(*account),
                balance,
                currency,
                retrieved
            ],
        )
        .map_err(|_| AppError::Database)?;
    }

    for payment in PAYMENTS {
        if payment.provenance == "manual" || payment.provenance == "manual-expired" {
            let valid = if payment.provenance == "manual" {
                Some("2026-11-15")
            } else {
                Some("2026-10-20")
            };
            conn.execute(
                "INSERT INTO manual_payment_metadata (account_id, statement_balance_minor, \
                    minimum_due_minor, next_due_date, valid_through_date, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                rusqlite::params![
                    account_id(payment.account),
                    payment.statement,
                    payment.minimum,
                    payment.due,
                    valid,
                    NOW
                ],
            )
            .map_err(|_| AppError::Database)?;
        } else {
            conn.execute(
                "INSERT INTO payment_snapshots (id, account_id, next_due_date, minimum_due_minor, \
                    statement_balance_minor, last_statement_issue_date, next_closing_date, retrieved_at, \
                    provenance, source_connection_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'provider', NULL)",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    account_id(payment.account),
                    payment.due,
                    payment.minimum,
                    payment.statement,
                    payment.issue,
                    payment.closing,
                    NOW
                ],
            )
            .map_err(|_| AppError::Database)?;
        }
    }

    // Bulk deterministic purchases: 10 USD cards × 3 months × 5 rows = 150.
    let mut index = 1usize;
    for card in 0..10usize {
        for month_offset in 0..3usize {
            let month = 8 + month_offset;
            for row in 0..5usize {
                let day = 3 + (card * 3 + month_offset * 5 + row * 7) % 20;
                let day = if month == 10 {
                    day.min(23)
                } else {
                    day.min(28)
                };
                let (merchant, canonical, category) =
                    MERCHANTS[(card * 5 + month_offset * 3 + row) % MERCHANTS.len()];
                let amount = 500
                    + (((card * 37 + month_offset * 311 + row * 97) % 175) as i64) * 100
                    + ((card + row) as i64 % 100);
                let kind = if row == 4 && month_offset == 1 {
                    "refund"
                } else if (card + row) % 13 == 0 {
                    "fee"
                } else if (card + row) % 17 == 0 {
                    "interest"
                } else {
                    "purchase"
                };
                let signed = if kind == "refund" { -amount } else { amount };
                let pending = month_offset == 2 && row == 3 && card < 4;
                let reporting = format!("2026-{month:02}-{day:02}");
                insert_transaction(
                    conn,
                    index,
                    card,
                    &reporting,
                    signed,
                    kind,
                    canonical,
                    category,
                    pending,
                    false,
                    "USD",
                    None,
                    if pending { None } else { Some(&reporting) },
                    Some(&format!("demo-provider-tx-{index}")),
                )?;
                let _ = merchant;
                index += 1;
            }
        }
    }

    // Scenario rows.
    let mut scenario = |account: usize,
                        reporting: &str,
                        amount: i64,
                        kind: &str,
                        merchant: &str,
                        category: &str,
                        pending: bool,
                        excluded: bool,
                        authorized: Option<&str>,
                        posted: Option<&str>|
     -> AppResult<String> {
        let id = insert_transaction(
            conn,
            index,
            account,
            reporting,
            amount,
            kind,
            merchant,
            category,
            pending,
            excluded,
            "USD",
            authorized,
            posted,
            Some(&format!("demo-scenario-tx-{index}")),
        );
        index += 1;
        id
    };

    // Month-boundary authorization/posting pairs.
    scenario(
        0,
        "2026-08-31",
        8_999,
        "purchase",
        "Whole Foods Market",
        "Groceries",
        false,
        false,
        Some("2026-08-31"),
        Some("2026-09-02"),
    )?;
    scenario(
        1,
        "2026-09-30",
        21_540,
        "purchase",
        "Delta Air Lines",
        "Travel",
        false,
        false,
        Some("2026-09-30"),
        Some("2026-10-02"),
    )?;
    // Refund received in a later month.
    scenario(
        0,
        "2026-08-12",
        30_000,
        "purchase",
        "Apple Store",
        "Electronics",
        false,
        false,
        None,
        Some("2026-08-12"),
    )?;
    scenario(
        0,
        "2026-09-08",
        -30_000,
        "refund",
        "Apple Store",
        "Electronics",
        false,
        false,
        None,
        Some("2026-09-08"),
    )?;
    scenario(
        5,
        "2026-09-14",
        4_500,
        "purchase",
        "Spotify",
        "Subscriptions",
        false,
        false,
        None,
        Some("2026-09-14"),
    )?;
    scenario(
        5,
        "2026-10-03",
        -4_500,
        "refund",
        "Spotify",
        "Subscriptions",
        false,
        false,
        None,
        Some("2026-10-03"),
    )?;
    // Pending plus linked posted replacement.
    let pending_id = scenario(
        0,
        "2026-10-22",
        4_250,
        "purchase",
        "Blue Bottle Coffee",
        "Dining",
        true,
        false,
        Some("2026-10-22"),
        None,
    )?;
    let posted_id = scenario(
        0,
        "2026-10-23",
        4_250,
        "purchase",
        "Blue Bottle Coffee",
        "Dining",
        false,
        false,
        None,
        Some("2026-10-23"),
    )?;
    conn.execute(
        "UPDATE transactions SET pending_link_id = ?1 WHERE id = ?2",
        rusqlite::params![pending_id, posted_id],
    )
    .map_err(|_| AppError::Database)?;
    // Two legitimate identical purchases.
    scenario(
        1,
        "2026-10-12",
        1_234,
        "purchase",
        "Blue Bottle Coffee",
        "Dining",
        false,
        false,
        None,
        Some("2026-10-12"),
    )?;
    scenario(
        0,
        "2026-10-12",
        1_234,
        "purchase",
        "Blue Bottle Coffee",
        "Dining",
        false,
        false,
        None,
        Some("2026-10-12"),
    )?;
    // Payment, transfer, fee, interest, exclusion.
    scenario(
        0,
        "2026-10-15",
        -100_000,
        "payment",
        "Payment Thank You",
        "Payments",
        false,
        false,
        None,
        Some("2026-10-15"),
    )?;
    scenario(
        1,
        "2026-10-16",
        50_000,
        "transfer",
        "Balance Transfer",
        "Transfers",
        false,
        false,
        None,
        Some("2026-10-16"),
    )?;
    scenario(
        0,
        "2026-10-01",
        9_500,
        "fee",
        "Chase Annual Fee",
        "Fees",
        false,
        false,
        None,
        Some("2026-10-01"),
    )?;
    scenario(
        8,
        "2026-10-05",
        1_822,
        "interest",
        "Citi Interest Charge",
        "Interest",
        false,
        false,
        None,
        Some("2026-10-05"),
    )?;
    scenario(
        3,
        "2026-10-09",
        7_999,
        "purchase",
        "Amazon",
        "Shopping",
        false,
        true,
        None,
        Some("2026-10-09"),
    )?;
    scenario(
        6,
        "2026-09-20",
        60_000,
        "purchase",
        "Delta Air Lines",
        "Travel",
        false,
        false,
        None,
        Some("2026-09-20"),
    )?;
    scenario(
        7,
        "2026-10-18",
        5_410,
        "purchase",
        "Whole Foods Market",
        "Groceries",
        false,
        false,
        None,
        Some("2026-10-18"),
    )?;
    scenario(
        8,
        "2026-10-21",
        2_875,
        "purchase",
        "CVS Pharmacy",
        "Health",
        false,
        false,
        None,
        Some("2026-10-21"),
    )?;
    scenario(
        9,
        "2026-10-19",
        6_420,
        "purchase",
        "Trader Joe's",
        "Groceries",
        false,
        false,
        None,
        Some("2026-10-19"),
    )?;
    scenario(
        4,
        "2026-10-08",
        -15_000,
        "payment",
        "PayPal Payment",
        "Payments",
        false,
        false,
        None,
        Some("2026-10-08"),
    )?;

    // Non-USD account rows, displayed separately.
    for (offset, (date, amount, merchant, category)) in [
        ("2026-08-18", 6_500, "Café de Flore", "Dining"),
        ("2026-09-09", 4_200, "Trenitalia", "Travel"),
        ("2026-09-27", 12_000, "Hotel Lutetia", "Travel"),
        ("2026-10-04", 3_800, "Monoprix", "Groceries"),
        ("2026-10-14", 7_300, "Café de Flore", "Dining"),
        ("2026-10-20", 2_100, "Métro Paris", "Transport"),
    ]
    .iter()
    .enumerate()
    {
        insert_transaction(
            conn,
            index + offset,
            10,
            date,
            *amount,
            "purchase",
            merchant,
            category,
            false,
            false,
            "EUR",
            None,
            Some(date),
            Some(&format!("demo-eur-tx-{offset}")),
        )?;
    }
    let _ = index;

    // Merchant aliases and a rule set (aliases merge labels, not rows).
    let alias_target: Option<String> = conn
        .query_row(
            "SELECT id FROM transactions WHERE merchant_display = 'Amazon' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();
    if let Some(id) = alias_target {
        conn.execute(
            "INSERT INTO transaction_overrides (transaction_id, merchant_alias, category, type_correction, excluded, notes, edited_at) \
             VALUES (?1, 'Amazon Household', 'Home', NULL, NULL, 'Demo alias across cards', ?2)",
            rusqlite::params![id, NOW],
        )
        .map_err(|_| AppError::Database)?;
    }
    for (match_type, match_text, canonical, category) in [
        ("contains", "amzn", "Amazon", "Shopping"),
        ("contains", "blue bottle", "Blue Bottle Coffee", "Dining"),
    ] {
        conn.execute(
            "INSERT INTO merchant_rules (id, match_type, match_text, canonical_merchant, category, \
                priority, enabled, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 100, 1, ?6, ?6)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), match_type, match_text, canonical, category, NOW],
        )
        .map_err(|_| AppError::Database)?;
    }

    // Partial and failed sync history.
    let run_partial = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO sync_runs (id, connection_id, trigger, started_at, finished_at, status, error_code, skip_reason) \
         VALUES (?1, ?2, 'scheduled', ?3, ?3, 'partial', NULL, NULL)",
        rusqlite::params![run_partial, connection_id(1), NOW],
    )
    .map_err(|_| AppError::Database)?;
    for (stream, status, error) in [
        ("transactions", "success", None),
        ("balances", "success", None),
        ("liabilities", "unavailable", Some("products_not_supported")),
    ] {
        conn.execute(
            "INSERT INTO sync_steps (id, run_id, stream, status, items_added, items_modified, items_removed, \
                provider_as_of, sanitized_error, started_at, finished_at) \
             VALUES (?1, ?2, ?3, ?4, 0, 0, 0, NULL, ?5, ?6, ?6)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), run_partial, stream, status, error, NOW],
        )
        .map_err(|_| AppError::Database)?;
    }
    let run_failed = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO sync_runs (id, connection_id, trigger, started_at, finished_at, status, error_code, skip_reason) \
         VALUES (?1, ?2, 'manual', ?3, ?3, 'failed', 'provider_temporarily_unavailable', NULL)",
        rusqlite::params![run_failed, connection_id(3), NOW],
    )
    .map_err(|_| AppError::Database)?;
    conn.execute(
        "INSERT INTO sync_steps (id, run_id, stream, status, items_added, items_modified, items_removed, \
            provider_as_of, sanitized_error, started_at, finished_at) \
         VALUES (?1, ?2, 'transactions', 'failed', 0, 0, 0, NULL, 'provider_temporarily_unavailable', ?3, ?3)",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), run_failed, NOW],
    )
    .map_err(|_| AppError::Database)?;

    let accounts: i64 = conn
        .query_row("SELECT count(*) FROM accounts", [], |row| row.get(0))
        .map_err(|_| AppError::Database)?;
    let connections: i64 = conn
        .query_row("SELECT count(*) FROM connections", [], |row| row.get(0))
        .map_err(|_| AppError::Database)?;
    let transactions: i64 = conn
        .query_row("SELECT count(*) FROM transactions", [], |row| row.get(0))
        .map_err(|_| AppError::Database)?;
    let pending: i64 = conn
        .query_row(
            "SELECT count(*) FROM transactions WHERE pending = 1",
            [],
            |row| row.get(0),
        )
        .map_err(|_| AppError::Database)?;

    Ok(DemoManifest {
        accounts,
        connections,
        transactions,
        pending,
        records: vec![
            "month-boundary".into(),
            "pending-posted-pair".into(),
            "later-month-refund".into(),
            "identical-purchases".into(),
            "payments-transfers".into(),
            "fees-interest".into(),
            "merchant-aliases".into(),
            "missing-due-vs-zero-due".into(),
            "stale-and-reconnect-accounts".into(),
            "partial-sync".into(),
            "non-usd".into(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_has_enough_transactions_and_scenarios() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../../migrations/0001_initial.sql"))
            .unwrap();
        conn.execute_batch(include_str!("../../migrations/0002_domain.sql"))
            .unwrap();
        let manifest = seed(&conn).unwrap();
        assert!(
            manifest.transactions >= 150,
            "only {} transactions",
            manifest.transactions
        );
        assert_eq!(manifest.accounts, 11);
        assert_eq!(manifest.connections, 5);
        assert!(manifest.pending >= 1);
        assert_eq!(manifest.records.len(), 11);
        let eur: i64 = conn
            .query_row(
                "SELECT count(*) FROM transactions WHERE currency = 'EUR'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(eur >= 6);
    }
}
