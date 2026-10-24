//! Plaid adapter tests that do not require live credentials or Keychain access.

use cardigan_money_lib::providers::plaid::{
    load_access_token, load_sandbox_credentials, save_sandbox_credentials, store_access_token,
    LinkTokenGetResponse, PlaidEnvironment,
};
use cardigan_money_lib::security::crypto::SecretString;
use cardigan_money_lib::security::keychain::MemoryStore;

#[test]
fn credentials_round_trip_in_memory_and_tokens_are_scoped() {
    let store = MemoryStore::default();
    assert!(load_sandbox_credentials(&store).unwrap().is_none());
    save_sandbox_credentials(
        &store,
        SecretString::new("sandbox_client_id".into()),
        SecretString::new("sandbox-secret-value".into()),
    )
    .unwrap();
    let credentials = load_sandbox_credentials(&store).unwrap().unwrap();
    assert_eq!(credentials.environment, PlaidEnvironment::Sandbox);
    assert_eq!(credentials.client_id.expose(), "sandbox_client_id");
    store_access_token(
        &store,
        "credential-ref-1",
        SecretString::new("access-sandbox-token-value-123456".into()),
    )
    .unwrap();
    assert_eq!(
        load_access_token(&store, "credential-ref-1")
            .unwrap()
            .unwrap()
            .expose(),
        "access-sandbox-token-value-123456"
    );
    assert!(load_access_token(&store, "credential-ref-2")
        .unwrap()
        .is_none());
}

#[test]
fn invalid_credentials_are_rejected_before_storage() {
    let store = MemoryStore::default();
    assert!(save_sandbox_credentials(
        &store,
        SecretString::new("bad id with spaces".into()),
        SecretString::new("sandbox-secret-value".into()),
    )
    .is_err());
    assert!(load_sandbox_credentials(&store).unwrap().is_none());
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
