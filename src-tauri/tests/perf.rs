//! Manual 50k-transaction performance measurement. Run with:
//! cargo test --test perf -- --ignored --nocapture

use std::time::{Duration, Instant};

use cardigan_money_lib::domain::dates::Month;
use cardigan_money_lib::security::crypto::SecretString;
use cardigan_money_lib::services::{dashboard, vault};
use cardigan_money_lib::state::AppState;
use cardigan_money_lib::storage::accounts::{create_manual, AccountInput, AccountType};
use cardigan_money_lib::storage::ledger::{month_summary, query_ledger, LedgerQuery};

const PASS: &str = "correct horse battery staple performance";

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

#[test]
#[ignore]
fn fifty_thousand_transactions_meet_targets() {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::new(tmp.path().to_path_buf()).unwrap();
    vault::create(&state, SecretString::new(PASS.into())).unwrap();
    let account = state
        .with_db(|conn, _h, _s| {
            create_manual(
                conn,
                &AccountInput {
                    issuer: "Perf Bank".into(),
                    display_name: "Perf Card".into(),
                    last_four: Some("0001".into()),
                    account_type: AccountType::CreditCard,
                    currency: "USD".into(),
                    credit_limit_minor: Some(1_000_000),
                    include_in_overview: true,
                },
            )
            .map(|account| account.id)
        })
        .unwrap();

    let insert_started = Instant::now();
    state
        .with_db(|conn, _h, _s| {
            let tx = conn.unchecked_transaction()?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO transactions (id, account_id, source_kind, provider_transaction_id, \
                        pending, authorized_date, posted_date, reporting_date, reporting_date_source, \
                        amount_minor, currency, raw_description, merchant_normalized, merchant_display, \
                        category, tx_type, excluded, removed, created_at, updated_at) \
                     VALUES (?1, ?2, 'demo', NULL, 0, ?3, ?3, ?3, 'authorized', ?4, 'USD', ?5, ?6, ?7, ?8, ?9, 0, 0, ?10, ?10)",
                )?;
                for index in 0..50_000i64 {
                    let month = 1 + (index % 24);
                    let year = 2025 + (index / 24) % 2;
                    let day = 1 + (index % 27);
                    let date = format!("{year:04}-{month:02}-{day:02}");
                    let merchant = format!("Perf Merchant {}", index % 500);
                    let id = uuid::Uuid::new_v4().to_string();
                    let amount = 100 + (index % 20_000);
                    let tx_type = if index % 97 == 0 { "refund" } else { "purchase" };
                    let category = match index % 4 {
                        0 => "Dining",
                        1 => "Shopping",
                        2 => "Travel",
                        _ => "Groceries",
                    };
                    let raw = format!("{merchant} raw description");
                    stmt.execute(rusqlite::params![
                        id,
                        account,
                        date,
                        amount,
                        raw,
                        merchant.to_lowercase(),
                        merchant,
                        category,
                        tx_type,
                        "2026-10-23T12:00:00Z"
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
        .unwrap();
    println!("insert 50k: {:?}", insert_started.elapsed());

    let query = LedgerQuery {
        start_date: "2026-10-01".into(),
        end_date_exclusive: "2026-11-01".into(),
        account_ids: vec![],
        category: None,
        status: None,
        tx_type: None,
        search: Some("Perf Merchant 42".into()),
        sort: Some("date_desc".into()),
        page: None,
        page_size: Some(50),
    };
    let mut search_samples = Vec::new();
    for _ in 0..6 {
        let started = Instant::now();
        let page = state
            .with_db(|conn, _h, _s| query_ledger(conn, &query))
            .unwrap();
        assert!(page.row_count > 0);
        search_samples.push(started.elapsed());
    }
    search_samples.remove(0);
    let search = median(search_samples);

    let mut month_samples = Vec::new();
    for _ in 0..6 {
        let started = Instant::now();
        let summary = state
            .with_db(|conn, _h, _s| month_summary(conn, Month::new(2026, 10).unwrap(), "USD"))
            .unwrap();
        assert!(summary.gross_purchases_minor > 0);
        month_samples.push(started.elapsed());
    }
    month_samples.remove(0);
    let month = median(month_samples);

    let mut dashboard_samples = Vec::new();
    for _ in 0..6 {
        let started = Instant::now();
        let dash = dashboard::get(&state, Some("2026-10".into())).unwrap();
        assert!(dash.summary.gross_purchases_minor > 0);
        dashboard_samples.push(started.elapsed());
    }
    dashboard_samples.remove(0);
    let dashboard_time = median(dashboard_samples);

    state.session.lock().unwrap().clear();
    let unlock_started = Instant::now();
    vault::unlock(&state, SecretString::new(PASS.into())).unwrap();
    let unlock = unlock_started.elapsed();
    let startup_started = Instant::now();
    let _ = dashboard::get(&state, Some("2026-10".into())).unwrap();
    let startup = startup_started.elapsed();

    println!("50k warm search: {search:?}");
    println!("50k month change: {month:?}");
    println!("50k dashboard: {dashboard_time:?}");
    println!("unlock (KDF): {unlock:?}");
    println!("cached startup after unlock (dashboard): {startup:?}");
    assert!(search < Duration::from_millis(200), "search target missed: {search:?}");
    assert!(month < Duration::from_millis(200), "month target missed: {month:?}");
}
