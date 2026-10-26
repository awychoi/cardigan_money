//! Plaid adapter tests that do not require live credentials or Keychain access.

use cardigan_money_lib::providers::plaid::{
    any_credentials_configured, load_access_token, load_credentials, new_link_credential_ref,
    save_credentials, store_access_token, LinkTokenGetResponse, PlaidEnvironment,
};
use cardigan_money_lib::security::crypto::SecretString;
use cardigan_money_lib::security::keychain::MemoryStore;
use cardigan_money_lib::services::{plaid_link, vault};
use cardigan_money_lib::state::AppState;
use std::sync::Arc;

#[test]
fn credentials_are_scoped_per_environment_and_tokens_are_scoped() {
    let store = MemoryStore::default();
    assert!(load_credentials(&store, PlaidEnvironment::Sandbox)
        .unwrap()
        .is_none());
    assert!(load_credentials(&store, PlaidEnvironment::Production)
        .unwrap()
        .is_none());
    assert!(!any_credentials_configured(&store).unwrap());

    save_credentials(
        &store,
        PlaidEnvironment::Sandbox,
        SecretString::new("sandbox_client_id".into()),
        SecretString::new("sandbox-secret-value".into()),
    )
    .unwrap();
    let sandbox = load_credentials(&store, PlaidEnvironment::Sandbox)
        .unwrap()
        .unwrap();
    assert_eq!(sandbox.environment, PlaidEnvironment::Sandbox);
    assert_eq!(sandbox.client_id.expose(), "sandbox_client_id");
    assert!(load_credentials(&store, PlaidEnvironment::Production)
        .unwrap()
        .is_none());

    save_credentials(
        &store,
        PlaidEnvironment::Production,
        SecretString::new("production_client_id".into()),
        SecretString::new("production-secret-value".into()),
    )
    .unwrap();
    let production = load_credentials(&store, PlaidEnvironment::Production)
        .unwrap()
        .unwrap();
    assert_eq!(production.environment, PlaidEnvironment::Production);
    assert_eq!(production.client_id.expose(), "production_client_id");
    assert_eq!(
        load_credentials(&store, PlaidEnvironment::Sandbox)
            .unwrap()
            .unwrap()
            .client_id
            .expose(),
        "sandbox_client_id"
    );
    assert!(any_credentials_configured(&store).unwrap());

    store_access_token(
        &store,
        "credential-ref-1",
        SecretString::new("test-only-not-a-plaid-token-123456".into()),
    )
    .unwrap();
    assert_eq!(
        load_access_token(&store, "credential-ref-1")
            .unwrap()
            .unwrap()
            .expose(),
        "test-only-not-a-plaid-token-123456"
    );
    assert!(load_access_token(&store, "credential-ref-2")
        .unwrap()
        .is_none());
}

#[test]
fn invalid_credentials_are_rejected_before_storage() {
    let store = MemoryStore::default();
    assert!(save_credentials(
        &store,
        PlaidEnvironment::Sandbox,
        SecretString::new("bad id with spaces".into()),
        SecretString::new("sandbox-secret-value".into()),
    )
    .is_err());
    assert!(load_credentials(&store, PlaidEnvironment::Sandbox)
        .unwrap()
        .is_none());
    assert!(save_credentials(
        &store,
        PlaidEnvironment::Production,
        SecretString::new("production_client_id".into()),
        SecretString::new("short".into()),
    )
    .is_err());
    assert!(load_credentials(&store, PlaidEnvironment::Production)
        .unwrap()
        .is_none());
}

#[test]
fn environment_names_round_trip_to_the_database_value() {
    for environment in PlaidEnvironment::ALL {
        assert_eq!(
            PlaidEnvironment::parse(environment.as_str()),
            Some(environment)
        );
    }
    assert_eq!(PlaidEnvironment::parse("trial"), None);
    assert_eq!(PlaidEnvironment::Sandbox.as_str(), "sandbox");
    assert_eq!(PlaidEnvironment::Production.as_str(), "production");
}

#[test]
fn status_reports_each_environment_independently() {
    let tmp = tempfile::tempdir().unwrap();
    let mut state = AppState::new(tmp.path().to_path_buf()).unwrap();
    let store = Arc::new(MemoryStore::default());
    state.credentials = store.clone();

    let initial = plaid_link::status(&state).unwrap();
    assert_eq!(initial.sandbox.environment, PlaidEnvironment::Sandbox);
    assert!(!initial.sandbox.configured);
    assert_eq!(initial.production.environment, PlaidEnvironment::Production);
    assert!(!initial.production.configured);

    save_credentials(
        &*store,
        PlaidEnvironment::Production,
        SecretString::new("production_client_id".into()),
        SecretString::new("production-secret-value".into()),
    )
    .unwrap();
    let saved = plaid_link::status(&state).unwrap();
    assert!(!saved.sandbox.configured);
    assert!(saved.production.configured);
    assert!(saved.production.link_available);
}

#[test]
fn finished_session_with_no_item_is_detected() {
    let response: LinkTokenGetResponse = serde_json::from_str(
        r#"{"link_sessions":[{"link_session_id":"s","finished_at":"2026-10-24T00:00:00Z","results":{"item_add_results":[]}}]}"#,
    )
    .unwrap();
    assert!(response.first_linked_item().is_none());
    assert!(response.session_finished_without_item());
}

const DUPLICATE_PASS: &str = "correct horse battery staple plaid duplicate";

fn state_with_vault() -> (tempfile::TempDir, AppState) {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::new(tmp.path().to_path_buf()).unwrap();
    vault::create(&state, SecretString::new(DUPLICATE_PASS.into())).unwrap();
    (tmp, state)
}

fn insert_connection(
    state: &AppState,
    environment: &str,
    institution_id: Option<&str>,
    institution_name: Option<&str>,
    status: &str,
    credential_ref: &str,
) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    state
        .with_db(|conn, _h, _s| {
            conn.execute(
                "INSERT INTO connections (id, provider, environment, institution_id, institution_name, \
                    credential_ref, status, created_at, updated_at) \
                 VALUES (?1, 'plaid', ?2, ?3, ?4, ?5, ?6, '2026-10-23T12:00:00Z', '2026-10-23T12:00:00Z')",
                rusqlite::params![id, environment, institution_id, institution_name, credential_ref, status],
            )?;
            Ok(())
        })
        .unwrap();
    id
}

#[test]
fn active_duplicate_institution_is_detected_before_exchange() {
    let (_tmp, state) = state_with_vault();
    let token_ref = uuid::Uuid::new_v4().to_string();
    let active = insert_connection(
        &state,
        "sandbox",
        Some("ins_active"),
        Some("Sandbox Bank"),
        "healthy",
        &token_ref,
    );

    let found = plaid_link::active_connection_for_institution(
        &state,
        PlaidEnvironment::Sandbox,
        Some("ins_active"),
        Some("Sandbox Bank"),
    )
    .unwrap();
    assert_eq!(
        found.as_ref().map(|(id, _name)| id.as_str()),
        Some(active.as_str())
    );

    // A different environment is a separate Plaid Item space.
    assert!(plaid_link::active_connection_for_institution(
        &state,
        PlaidEnvironment::Production,
        Some("ins_active"),
        Some("Sandbox Bank"),
    )
    .unwrap()
    .is_none());

    // Name is a fallback when Link omits institution_id.
    let by_name = insert_connection(
        &state,
        "sandbox",
        None,
        Some("Other Bank"),
        "healthy",
        &uuid::Uuid::new_v4().to_string(),
    );
    let found = plaid_link::active_connection_for_institution(
        &state,
        PlaidEnvironment::Sandbox,
        None,
        Some("other bank"),
    )
    .unwrap();
    assert_eq!(
        found.as_ref().map(|(id, _name)| id.as_str()),
        Some(by_name.as_str())
    );

    // Disconnected connections do not block a legitimate re-link.
    insert_connection(
        &state,
        "sandbox",
        Some("ins_disconnected"),
        Some("Old Bank"),
        "disconnected",
        &uuid::Uuid::new_v4().to_string(),
    );
    assert!(plaid_link::active_connection_for_institution(
        &state,
        PlaidEnvironment::Sandbox,
        Some("ins_disconnected"),
        Some("Old Bank"),
    )
    .unwrap()
    .is_none());

    // Restored entries without a local token must be allowed to link again.
    let restored_ref = new_link_credential_ref();
    insert_connection(
        &state,
        "sandbox",
        Some("ins_restored"),
        Some("Restored Bank"),
        "reconnect_required",
        &restored_ref,
    );
    assert!(plaid_link::active_connection_for_institution(
        &state,
        PlaidEnvironment::Sandbox,
        Some("ins_restored"),
        Some("Restored Bank"),
    )
    .unwrap()
    .is_none());
}
