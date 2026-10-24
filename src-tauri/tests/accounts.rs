use cardigan_money_lib::error::AppError;
use cardigan_money_lib::security::crypto::SecretString;
use cardigan_money_lib::services::vault;
use cardigan_money_lib::state::AppState;
use cardigan_money_lib::storage::accounts::{
    create_manual, get, list, set_archived, update_manual, AccountInput, AccountType,
};

fn setup() -> (tempfile::TempDir, AppState) {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::new(tmp.path().to_path_buf()).unwrap();
    vault::create(&state, SecretString::new("correct horse battery staple accounts".into())).unwrap();
    (tmp, state)
}

fn input(name: &str) -> AccountInput {
    AccountInput {
        issuer: "Chase".into(),
        display_name: name.into(),
        last_four: Some("1234".into()),
        account_type: AccountType::CreditCard,
        currency: "USD".into(),
        credit_limit_minor: Some(500_000),
        include_in_overview: true,
    }
}

#[test]
fn manual_accounts_create_update_archive_and_validate() {
    let (_tmp, state) = setup();
    let created = state
        .with_db(|conn, _h, _s| create_manual(conn, &input("Sapphire Reserve")))
        .unwrap();
    assert_eq!(created.issuer, "Chase");
    assert_eq!(created.source_kind, "manual");
    assert!(!created.archived);

    let listed = state.with_db(|conn, _h, _s| list(conn, false)).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);

    let mut changed = input("Sapphire Preferred");
    changed.last_four = Some("9876".into());
    changed.credit_limit_minor = Some(1_000_000);
    let updated = state
        .with_db(|conn, _h, _s| update_manual(conn, &created.id, &changed))
        .unwrap();
    assert_eq!(updated.display_name, "Sapphire Preferred");
    assert_eq!(updated.last_four.as_deref(), Some("9876"));
    assert_eq!(updated.credit_limit_minor, Some(1_000_000));

    let archived = state
        .with_db(|conn, _h, _s| set_archived(conn, &created.id, true))
        .unwrap();
    assert!(archived.archived);
    assert_eq!(state.with_db(|conn, _h, _s| list(conn, false)).unwrap().len(), 0);
    assert_eq!(state.with_db(|conn, _h, _s| list(conn, true)).unwrap().len(), 1);

    // Archived accounts cannot be edited until restored.
    let err = state
        .with_db(|conn, _h, _s| update_manual(conn, &created.id, &changed))
        .unwrap_err();
    assert!(matches!(err, AppError::NotFound));
    state
        .with_db(|conn, _h, _s| set_archived(conn, &created.id, false))
        .unwrap();
    assert!(state.with_db(|conn, _h, _s| get(conn, &created.id)).is_ok());
}

#[test]
fn account_validation_rejects_bad_fields() {
    let (_tmp, state) = setup();
    let cases = [
        AccountInput { issuer: "".into(), ..input("X") },
        AccountInput { display_name: "".into(), ..input("X") },
        AccountInput { last_four: Some("12".into()), ..input("X") },
        AccountInput { last_four: Some("abcd".into()), ..input("X") },
        AccountInput { currency: "US".into(), ..input("X") },
        AccountInput { currency: "XYZ".into(), ..input("X") },
        AccountInput { credit_limit_minor: Some(0), ..input("X") },
        AccountInput { issuer: "a".repeat(81), ..input("X") },
    ];
    for case in cases {
        let result = state.with_db(|conn, _h, _s| create_manual(conn, &case));
        assert!(result.is_err(), "accepted invalid account: {case:?}");
    }
    let _ = input("valid");
}

#[test]
fn account_text_is_cleaned_and_bounded() {
    let (_tmp, state) = setup();
    let mut with_controls = input("Card\u{0007}Name");
    with_controls.display_name = "Card\u{0007}Name".into();
    assert!(state
        .with_db(|conn, _h, _s| create_manual(conn, &with_controls))
        .is_err());
    let mut with_bidi = input("Normal");
    with_bidi.display_name = "Card\u{202E}Name".into();
    assert!(state
        .with_db(|conn, _h, _s| create_manual(conn, &with_bidi))
        .is_err());
    let trimmed = input("  Spaced Card  ");
    let created = state
        .with_db(|conn, _h, _s| create_manual(conn, &trimmed))
        .unwrap();
    assert_eq!(created.display_name, "Spaced Card");
}
