//! CSV import/export, duplicate review, rollback, overrides, rules, and manual
//! payment metadata against a real encrypted vault.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;

use cardigan_money_lib::domain::dates::Month;
use cardigan_money_lib::providers::csv::{file_info, CsvMapping};
use cardigan_money_lib::security::crypto::SecretString;
use cardigan_money_lib::services::exports::export_filtered;
use cardigan_money_lib::services::imports::{self, store_job};
use cardigan_money_lib::services::vault;
use cardigan_money_lib::state::AppState;
use cardigan_money_lib::storage::accounts::{create_manual, AccountInput, AccountType};
use cardigan_money_lib::storage::edits::{
    self, ManualPaymentInput, MerchantRuleInput, OverrideInput,
};
use cardigan_money_lib::storage::ledger::{month_summary, query_ledger, LedgerQuery};

const PASS: &str = "correct horse battery staple imports";

fn setup() -> (tempfile::TempDir, AppState, String) {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::new(tmp.path().to_path_buf()).unwrap();
    vault::create(&state, SecretString::new(PASS.to_string())).unwrap();
    let account = state
        .with_db(|conn, _h, _s| {
            create_manual(
                conn,
                &AccountInput {
                    issuer: "CSV Bank".into(),
                    display_name: "Imported Card".into(),
                    last_four: Some("9999".into()),
                    account_type: AccountType::CreditCard,
                    currency: "USD".into(),
                    credit_limit_minor: Some(500_000),
                    include_in_overview: true,
                },
            )
            .map(|account| account.id)
        })
        .unwrap();
    (tmp, state, account)
}

fn mapping() -> CsvMapping {
    CsvMapping {
        date_column: 0,
        description_column: 1,
        amount_column: Some(2),
        debit_column: None,
        credit_column: None,
        category_column: Some(3),
        source_id_column: Some(4),
        date_format: "iso".into(),
        sign_convention: "signed_amount".into(),
        credit_type: "refund".into(),
    }
}

fn csv_file(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    path
}

fn create_job(state: &AppState, account: &str, path: &std::path::Path) -> String {
    let info = file_info(path).unwrap();
    store_job(state, account, path, info).unwrap().job_id
}

#[test]
fn imports_csv_with_bom_quotes_and_parentheses_then_is_idempotent() {
    let (tmp, state, account) = setup();
    let csv = csv_file(
        tmp.path(),
        "history.csv",
        "\u{feff}Date,Description,Amount,Category,Source ID\n2026-10-02,\"Coffee, Inc\",(12.34),Dining,c1\n2026-10-03,Refund example,-5.00,Dining,c2\n2026-10-04,Paycheck,100.00,Income,c3\n",
    );
    let job = create_job(&state, &account, &csv);
    let preview = imports::validate(&state, &job, &mapping()).unwrap();
    assert_eq!(preview.insert_count, 3);
    assert_eq!(preview.duplicate_count, 0);
    assert_eq!(preview.review_count, 0);
    assert_eq!(preview.rejected_count, 0);
    assert!(!preview.no_op);

    let result = imports::commit(&state, &job, &mapping(), false).unwrap();
    assert_eq!(result.inserted, 3);
    assert!(!result.no_op);

    let summary = state
        .with_db(|conn, _h, _s| month_summary(conn, Month::new(2026, 10).unwrap(), "USD"))
        .unwrap();
    assert_eq!(summary.gross_purchases_minor, 10_000);
    assert_eq!(summary.refunds_minor, -1_734);
    // "Paycheck" positive is a purchase by the generic mapper; it is not
    // reclassified automatically. The owner can correct the type.
    assert_eq!(summary.net_spending_minor, 8_266);

    // Reimporting the identical file is a no-op.
    let job2 = create_job(&state, &account, &csv);
    let preview2 = imports::validate(&state, &job2, &mapping()).unwrap();
    assert!(preview2.no_op);
    let result2 = imports::commit(&state, &job2, &mapping(), false).unwrap();
    assert!(result2.no_op);
    let count: i64 = state
        .with_db(|conn, _h, _s| {
            conn.query_row("SELECT count(*) FROM transactions", [], |row| row.get(0))
                .map_err(|_| cardigan_money_lib::error::AppError::Database)
        })
        .unwrap();
    assert_eq!(count, 3);

    // Export is plaintext CSV, 0600, and escapes a formula-shaped merchant.
    let query = LedgerQuery {
        start_date: "2026-10-01".into(),
        end_date_exclusive: "2026-11-01".into(),
        account_ids: vec![],
        category: None,
        status: None,
        tx_type: None,
        search: None,
        sort: None,
        page: None,
        page_size: None,
    };
    let export_path = tmp.path().join("export.csv");
    let exported = state
        .with_db(|conn, _h, _s| export_filtered(conn, &query, &export_path))
        .unwrap();
    assert_eq!(exported.rows, 3);
    assert_eq!(
        std::fs::metadata(&export_path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn duplicate_review_preserves_legitimate_identical_rows() {
    let (tmp, state, account) = setup();
    let first = csv_file(
        tmp.path(),
        "one.csv",
        "Date,Description,Amount,Category,Source ID\n2026-10-10,Coffee Shop,12.34,Dining,\n",
    );
    let job = create_job(&state, &account, &first);
    imports::commit(&state, &job, &mapping(), false).unwrap();

    // A different file with the same date/amount/merchant but no source id is a
    // possible overlap, not an automatic delete.
    let second = csv_file(
        tmp.path(),
        "two.csv",
        "Date,Description,Amount,Category,Source ID\n2026-10-10,Coffee Shop,12.34,Dining,\n2026-10-11,Tea Shop,5.00,Dining,\n",
    );
    let job2 = create_job(&state, &account, &second);
    let preview = imports::validate(&state, &job2, &mapping()).unwrap();
    assert_eq!(preview.insert_count, 1);
    assert_eq!(preview.review_count, 1);
    let result = imports::commit(&state, &job2, &mapping(), false).unwrap();
    assert_eq!(result.inserted, 1);
    assert_eq!(result.review, 1);
    let count: i64 = state
        .with_db(|conn, _h, _s| {
            conn.query_row("SELECT count(*) FROM transactions", [], |row| row.get(0))
                .map_err(|_| cardigan_money_lib::error::AppError::Database)
        })
        .unwrap();
    assert_eq!(count, 2, "the review row is not silently inserted or merged");

    // A third, explicitly accepted review inserts the legitimate duplicate.
    let third = csv_file(
        tmp.path(),
        "three.csv",
        "Date,Description,Amount,Category,Source ID\n2026-10-10,Coffee Shop,12.34,Dining,\n2026-10-12,New Shop,6.00,Dining,\n",
    );
    let job3 = create_job(&state, &account, &third);
    let result = imports::commit(&state, &job3, &mapping(), true).unwrap();
    assert_eq!(result.inserted, 2);
    let count: i64 = state
        .with_db(|conn, _h, _s| {
            conn.query_row("SELECT count(*) FROM transactions", [], |row| row.get(0))
                .map_err(|_| cardigan_money_lib::error::AppError::Database)
        })
        .unwrap();
    assert_eq!(count, 4);
}

#[test]
fn rollback_removes_only_that_batchs_unique_rows() {
    let (tmp, state, account) = setup();
    let one = csv_file(
        tmp.path(),
        "one.csv",
        "Date,Description,Amount,Category,Source ID\n2026-10-01,Alpha,10.00,Test,a1\n2026-10-02,Beta,20.00,Test,a2\n",
    );
    let job1 = create_job(&state, &account, &one);
    let result1 = imports::commit(&state, &job1, &mapping(), false).unwrap();

    let two = csv_file(
        tmp.path(),
        "two.csv",
        "Date,Description,Amount,Category,Source ID\n2026-10-03,Gamma,30.00,Test,b1\n",
    );
    let job2 = create_job(&state, &account, &two);
    imports::commit(&state, &job2, &mapping(), false).unwrap();

    let batch = imports::rollback(&state, &result1.batch_id).unwrap();
    assert_eq!(batch.status, "rolled_back");
    let count: i64 = state
        .with_db(|conn, _h, _s| {
            conn.query_row("SELECT count(*) FROM transactions", [], |row| row.get(0))
                .map_err(|_| cardigan_money_lib::error::AppError::Database)
        })
        .unwrap();
    assert_eq!(count, 1, "only Gamma from the untouched batch remains");
    let merchant: String = state
        .with_db(|conn, _h, _s| {
            conn.query_row("SELECT merchant_display FROM transactions", [], |row| row.get(0))
                .map_err(|_| cardigan_money_lib::error::AppError::Database)
        })
        .unwrap();
    assert_eq!(merchant, "Gamma");
}

#[test]
fn malformed_and_oversized_files_fail_safely() {
    let (tmp, state, account) = setup();
    let binary = tmp.path().join("binary.csv");
    std::fs::write(&binary, [0xff, 0xfe, 0x00, 0x01]).unwrap();
    assert!(file_info(&binary).is_err());

    let huge = tmp.path().join("huge.csv");
    let file = std::fs::File::create(&huge).unwrap();
    file.set_len(21 * 1024 * 1024).unwrap();
    assert!(file_info(&huge).is_err());

    let link = tmp.path().join("link.csv");
    std::os::unix::fs::symlink(&binary, &link).unwrap();
    assert!(file_info(&link).is_err());

    // Bad mapping columns are rejected.
    let good = csv_file(
        tmp.path(),
        "good.csv",
        "Date,Description,Amount\n2026-10-01,Alpha,10.00\n",
    );
    let job = create_job(&state, &account, &good);
    let mut bad = mapping();
    bad.date_column = 9;
    assert!(imports::validate(&state, &job, &bad).is_err());
}

#[test]
fn overrides_rules_and_manual_metadata_are_independent_and_survive_reload() {
    let (tmp, state, account) = setup();
    let csv = csv_file(
        tmp.path(),
        "edit.csv",
        "Date,Description,Amount,Category,Source ID\n2026-10-12,AMZN Mktp US,42.00,Shopping,e1\n2026-10-13,Coffee,7.50,Dining,e2\n",
    );
    let job = create_job(&state, &account, &csv);
    imports::commit(&state, &job, &mapping(), false).unwrap();

    let (tx_id, tx2_id): (String, String) = state
        .with_db(|conn, _h, _s| {
            let mut stmt = conn
                .prepare("SELECT id FROM transactions ORDER BY merchant_display")
                .map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            let ids = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|_| cardigan_money_lib::error::AppError::Database)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            Ok((ids[0].clone(), ids[1].clone()))
        })
        .unwrap();

    // Rule-derived merchant + category.
    state
        .with_db(|conn, _h, _s| {
            edits::create_rule(
                conn,
                &MerchantRuleInput {
                    match_type: "contains".into(),
                    match_text: "amzn".into(),
                    canonical_merchant: "Amazon".into(),
                    category: Some("Shopping".into()),
                    priority: 100,
                    enabled: true,
                },
            )
        })
        .unwrap();
    let detail = state
        .with_db(|conn, _h, _s| edits::get_detail(conn, &tx_id))
        .unwrap();
    assert_eq!(detail.merchant, "Amazon");

    // User override beats the rule.
    state
        .with_db(|conn, _h, _s| {
            edits::set_override(
                conn,
                &tx_id,
                OverrideInput {
                    merchant_alias: Some("Amazon Household".into()),
                    category: Some("Home".into()),
                    type_correction: None,
                    excluded: Some(true),
                    notes: Some("owner note".into()),
                },
            )
        })
        .unwrap();
    let detail = state
        .with_db(|conn, _h, _s| edits::get_detail(conn, &tx_id))
        .unwrap();
    assert_eq!(detail.merchant, "Amazon Household");
    assert_eq!(detail.category.as_deref(), Some("Home"));
    assert!(detail.excluded);
    assert_eq!(detail.notes.as_deref(), Some("owner note"));
    assert_eq!(detail.raw_description, "AMZN Mktp US");

    let summary = state
        .with_db(|conn, _h, _s| month_summary(conn, Month::new(2026, 10).unwrap(), "USD"))
        .unwrap();
    assert_eq!(summary.net_spending_minor, 750);

    // Deleting the rule reverts the rule-derived display but not the override.
    let rules = state.with_db(|conn, _h, _s| edits::list_rules(conn)).unwrap();
    state
        .with_db(|conn, _h, _s| edits::delete_rule(conn, &rules[0].id))
        .unwrap();
    let detail = state
        .with_db(|conn, _h, _s| edits::get_detail(conn, &tx_id))
        .unwrap();
    assert_eq!(detail.merchant, "Amazon Household");
    let other = state
        .with_db(|conn, _h, _s| edits::get_detail(conn, &tx2_id))
        .unwrap();
    assert_eq!(other.merchant, "Coffee");

    // Manual payment metadata is independent of provider snapshots and survives lock/unlock.
    state
        .with_db(|conn, _h, _s| {
            edits::set_manual_payment(
                conn,
                &account,
                &ManualPaymentInput {
                    statement_balance: Some("$120.00".into()),
                    minimum_due: Some("$25.00".into()),
                    next_due_date: Some("2026-10-29".into()),
                    valid_through_date: Some("2026-11-15".into()),
                },
            )
        })
        .unwrap();
    state.session.lock().unwrap().clear();
    vault::unlock(&state, SecretString::new(PASS.to_string())).unwrap();
    let payment = state
        .with_db(|conn, _h, _s| edits::get_manual_payment(conn, &account))
        .unwrap()
        .unwrap();
    assert_eq!(payment.minimum_due_minor, Some(2_500));
    assert_eq!(payment.statement_balance_minor, Some(12_000));

    // Clearing removes the dated manual facts.
    state
        .with_db(|conn, _h, _s| {
            edits::set_manual_payment(
                conn,
                &account,
                &ManualPaymentInput {
                    statement_balance: None,
                    minimum_due: None,
                    next_due_date: None,
                    valid_through_date: None,
                },
            )
        })
        .unwrap();
    assert!(state
        .with_db(|conn, _h, _s| edits::get_manual_payment(conn, &account))
        .unwrap()
        .is_none());

    // The filtered ledger reflects the override.
    let page = state
        .with_db(|conn, _h, _s| {
            query_ledger(
                conn,
                &LedgerQuery {
                    start_date: "2026-10-01".into(),
                    end_date_exclusive: "2026-11-01".into(),
                    account_ids: vec![],
                    category: Some("Home".into()),
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
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0].merchant, "Amazon Household");
    let _ = tmp;
}

#[test]
fn export_escapes_formula_cells_and_sets_0600() {
    let (tmp, state, account) = setup();
    let csv = csv_file(
        tmp.path(),
        "danger2.csv",
        "Date,Description,Amount,Category,Source ID\n2026-10-01,\"=SUM(A1:A9)\",1.00,\"=cmd\",d1\n2026-10-02,\" +cmd\",2.00,\"@x\",d2\n2026-10-03,Normal,3.00,Test,d3\n",
    );
    let job = create_job(&state, &account, &csv);
    imports::commit(&state, &job, &mapping(), false).unwrap();
    let query = LedgerQuery {
        start_date: "2026-10-01".into(),
        end_date_exclusive: "2026-11-01".into(),
        account_ids: vec![],
        category: None,
        status: None,
        tx_type: None,
        search: None,
        sort: None,
        page: None,
        page_size: None,
    };
    let path = tmp.path().join("danger-export.csv");
    state
        .with_db(|conn, _h, _s| export_filtered(conn, &query, &path))
        .unwrap();
    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.contains("'=SUM(A1:A9)"), "formula not escaped: {contents}");
    assert!(contents.contains("'+cmd"));
    assert!(contents.contains("'@x"));
    assert!(contents.contains("'=cmd"));
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(!contents.contains("/Users/"), "export must not leak a path");

    // A symlink destination is rejected without writing through it.
    let outside = tmp.path().join("outside.csv");
    std::fs::write(&outside, b"original").unwrap();
    let link = tmp.path().join("link-export.csv");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    let err = state
        .with_db(|conn, _h, _s| export_filtered(conn, &query, &link))
        .unwrap_err();
    assert!(matches!(err, cardigan_money_lib::error::AppError::PermissionDenied(_)));
    assert_eq!(std::fs::read(&outside).unwrap(), b"original");
}
