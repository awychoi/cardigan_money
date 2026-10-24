//! The owner-facing verification report must contain aggregates only.

use cardigan_money_lib::security::crypto::SecretString;
use cardigan_money_lib::services::plaid_link::verification_report;
use cardigan_money_lib::services::vault;
use cardigan_money_lib::state::AppState;
use cardigan_money_lib::storage::accounts::{create_manual, AccountInput, AccountType};

const PASS: &str = "correct horse battery staple plaid report";

#[test]
fn verification_report_exposes_only_sanitized_counts() {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::new(tmp.path().to_path_buf()).unwrap();
    vault::create(&state, SecretString::new(PASS.into())).unwrap();
    let (connection_id, account_id, credential_ref) = state
        .with_db(|conn, _h, _s| {
            let account = create_manual(
                conn,
                &AccountInput {
                    issuer: "Sandbox Bank".into(),
                    display_name: "Sandbox Card".into(),
                    last_four: Some("0000".into()),
                    account_type: AccountType::CreditCard,
                    currency: "USD".into(),
                    credit_limit_minor: Some(100_000),
                    include_in_overview: true,
                },
            )?;
            let connection_id = uuid::Uuid::new_v4().to_string();
            let credential_ref = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO connections (id, provider, environment, institution_name, credential_ref, status, cursor, created_at, updated_at) \
                 VALUES (?1, 'plaid', 'sandbox', 'Sandbox Bank', ?2, 'healthy', 'cursor-sentinel', '2026-10-24T00:00:00Z', '2026-10-24T00:00:00Z')",
                rusqlite::params![connection_id, credential_ref],
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            conn.execute(
                "INSERT INTO account_sources (id, account_id, connection_id, provider_account_id, provider_mask, linked_at) \
                 VALUES (?1, ?2, ?3, 'provider-account-sentinel', '0000', '2026-10-24T00:00:00Z')",
                rusqlite::params![uuid::Uuid::new_v4().to_string(), account.id, connection_id],
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            conn.execute(
                "INSERT INTO transactions (id, account_id, source_kind, provider_transaction_id, pending, authorized_date, posted_date, reporting_date, reporting_date_source, amount_minor, currency, raw_description, merchant_normalized, merchant_display, category, tx_type, excluded, removed, created_at, updated_at) \
                 VALUES (?1, ?2, 'provider', 'provider-transaction-sentinel', 1, '2026-10-23', NULL, '2026-10-23', 'authorized', 12345, 'USD', 'Description sentinel', 'merchant sentinel', 'Merchant Sentinel', 'Test', 'purchase', 0, 0, '2026-10-24T00:00:00Z', '2026-10-24T00:00:00Z')",
                rusqlite::params![uuid::Uuid::new_v4().to_string(), account.id],
            )
            .map_err(|_| cardigan_money_lib::error::AppError::Database)?;
            Ok((connection_id, account.id, credential_ref))
        })
        .unwrap();
    let report = verification_report(&state).unwrap();
    assert_eq!(report.environment, "sandbox");
    assert_eq!(report.connections.len(), 1);
    assert_eq!(report.connections[0].account_count, 1);
    assert_eq!(report.connections[0].provider_transaction_count, 1);
    assert_eq!(report.connections[0].pending_transaction_count, 1);
    assert!(report.connections[0].cursor_present);
    let json = serde_json::to_string(&report).unwrap();
    for forbidden in [
        credential_ref.as_str(),
        connection_id.as_str(),
        account_id.as_str(),
        "cursor-sentinel",
        "provider-account-sentinel",
        "provider-transaction-sentinel",
        "Description sentinel",
        "merchant sentinel",
        "12345",
    ] {
        assert!(!json.contains(forbidden), "report leaked {forbidden}");
    }
}
