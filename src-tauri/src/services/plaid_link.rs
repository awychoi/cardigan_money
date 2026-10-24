//! Hosted Link orchestration. The Hosted Link URL and raw tokens remain in Rust
//! memory only; the renderer receives a coarse status. Every provider request
//! goes through PlaidClient's allowlisted, bounded transport.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

use crate::error::{AppError, AppResult};
use crate::providers::plaid::{
    load_access_token, load_sandbox_credentials, store_access_token, PlaidClient,
    PlaidEnvironment,
};
use crate::security::crypto::SecretString;
use crate::security::network_url::validate_plaid_hosted_url;
use crate::state::AppState;
use crate::storage::events::record_event;
use crate::storage::time::now_rfc3339;
use uuid::Uuid;

#[derive(Default)]
pub struct PlaidLinkRuntime {
    pub status: String,
    pub message: Option<String>,
    pub connection_id: Option<String>,
    pub expires_at: Option<String>,
    pub active: bool,
    pub cancel: Option<Arc<AtomicBool>>,
    pub mode: String,
    pub reconnect_connection_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaidLinkStatus {
    pub state: String,
    pub message: Option<String>,
    pub connection_id: Option<String>,
    pub expires_at: Option<String>,
}

impl Default for PlaidLinkStatus {
    fn default() -> Self {
        Self { state: "idle".into(), message: None, connection_id: None, expires_at: None }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaidStatus {
    pub configured: bool,
    pub environment: String,
    pub link_available: bool,
    pub message: Option<String>,
}

pub fn status(state: &AppState) -> AppResult<PlaidStatus> {
    state.require_real("configure Plaid in demo mode")?;
    let configured = load_sandbox_credentials(state.credentials.as_ref())?.is_some();
    Ok(PlaidStatus {
        configured,
        environment: "sandbox".into(),
        link_available: configured,
        message: if configured {
            Some("Sandbox credentials are stored in the macOS Keychain.".into())
        } else {
            Some("Add Plaid Sandbox credentials in the native setup sheet.".into())
        },
    })
}

pub fn save_credentials(
    state: &AppState,
    client_id: SecretString,
    secret: SecretString,
) -> AppResult<PlaidStatus> {
    state.require_real("configure Plaid in demo mode")?;
    state.with_db(|_conn, _header, _settings| Ok(()))?;
    crate::providers::plaid::save_sandbox_credentials(
        state.credentials.as_ref(),
        client_id,
        secret,
    )?;
    state.with_db(|conn, _h, _s| {
        let _ = record_event(
            conn,
            "credential_added",
            "success",
            Some(serde_json::json!({ "provider": "plaid", "environment": "sandbox" })),
        );
        Ok(())
    })?;
    status(state)
}

fn link_status(state: &AppState) -> AppResult<PlaidLinkStatus> {
    let runtime = state.plaid_link.lock().map_err(|_| AppError::Internal)?;
    Ok(PlaidLinkStatus {
        state: if runtime.status.is_empty() { "idle".into() } else { runtime.status.clone() },
        message: runtime.message.clone(),
        connection_id: runtime.connection_id.clone(),
        expires_at: runtime.expires_at.clone(),
    })
}

fn set_status(state: &AppState, status: &str, message: Option<String>, connection_id: Option<String>) {
    if let Ok(mut runtime) = state.plaid_link.lock() {
        runtime.status = status.to_string();
        runtime.message = message;
        runtime.connection_id = connection_id;
        if status == "linked" || status == "error" || status == "cancelled" || status == "expired" {
            runtime.active = false;
        }
    }
}

fn provider_app_error(error: crate::providers::ProviderError) -> AppError {
    AppError::Provider(error.safe_message)
}

pub fn start_link<R: tauri::Runtime>(state: &AppState, app: &tauri::AppHandle<R>) -> AppResult<PlaidLinkStatus> {
    state.require_real("link Plaid in demo mode")?;
    let vault_id = state.with_db(|_conn, header, _settings| {
        header
            .map(|header| header.vault_id_hex())
            .ok_or(AppError::Locked)
    })?;
    let credentials = load_sandbox_credentials(state.credentials.as_ref())?
        .ok_or(AppError::ProviderNotConfigured)?;
    {
        let mut runtime = state.plaid_link.lock().map_err(|_| AppError::Internal)?;
        if runtime.active {
            return Err(AppError::Blocked("a Plaid link is already in progress"));
        }
        runtime.status = "starting".into();
        runtime.message = None;
        runtime.connection_id = None;
        runtime.expires_at = None;
        runtime.active = true;
        runtime.cancel = Some(Arc::new(AtomicBool::new(false)));
        runtime.mode = "link".into();
        runtime.reconnect_connection_id = None;
    }

    let client = PlaidClient::new(credentials, None).map_err(|_| AppError::ProviderNotConfigured)?;
    let token = match client.create_hosted_link_token(&vault_id) {
        Ok(token) => token,
        Err(error) => {
            set_status(state, "error", Some(error.safe_message.into()), None);
            return Err(provider_app_error(error));
        }
    };
    let Some(url) = token.hosted_link_url.clone() else {
        set_status(state, "error", Some("Hosted Link is unavailable for this account.".into()), None);
        return Err(AppError::Blocked("Hosted Link is unavailable"));
    };
    if validate_plaid_hosted_url(&url).is_err() {
        set_status(state, "error", Some("Plaid returned an invalid Hosted Link URL.".into()), None);
        return Err(AppError::Blocked("invalid Hosted Link URL"));
    }
    let link_token = SecretString::new(token.link_token);
    let expires_at = token.expiration.clone();
    {
        let mut runtime = state.plaid_link.lock().map_err(|_| AppError::Internal)?;
        runtime.status = "waiting".into();
        runtime.expires_at = Some(expires_at.clone());
        runtime.message = Some("Complete the Plaid Sandbox flow in your browser.".into());
    }
    // The URL is a short-lived bearer: open it once through the opener and do
    // not log, persist, return, or include it in any error.
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|_| {
            set_status(state, "error", Some("Could not open the browser.".into()), None);
            AppError::Provider("Could not open the browser.")
        })?;

    let cancel = state
        .plaid_link
        .lock()
        .ok()
        .and_then(|runtime| runtime.cancel.clone())
        .ok_or(AppError::Internal)?;
    let app_handle = app.clone();
    let expires = DateTime::parse_from_rfc3339(&expires_at)
        .ok()
        .map(|value| value.with_timezone(&Utc));
    std::thread::Builder::new()
        .name("cm-plaid-link".into())
        .spawn(move || poll_link(app_handle, client, link_token, cancel, expires, None))
        .map_err(|_| AppError::Internal)?;
    link_status(state)
}

fn poll_link<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    client: PlaidClient,
    link_token: SecretString,
    cancel: Arc<AtomicBool>,
    expires: Option<DateTime<Utc>>,
    reconnect_connection_id: Option<String>,
) {
    let started = Instant::now();
    let deadline = expires
        .map(|value| value + chrono::Duration::minutes(5))
        .unwrap_or_else(|| Utc::now() + chrono::Duration::minutes(30));
    let mut interval = Duration::from_secs(2);
    loop {
        if cancel.load(Ordering::SeqCst) {
            if let Some(state) = app.try_state::<AppState>() {
                set_status(&state, "cancelled", Some("Plaid link cancelled.".into()), None);
            }
            return;
        }
        if Utc::now() > deadline || started.elapsed() > Duration::from_secs(35 * 60) {
            if let Some(state) = app.try_state::<AppState>() {
                set_status(&state, "expired", Some("The Plaid link session expired. Start again.".into()), None);
            }
            return;
        }
        std::thread::sleep(interval);
        interval = (interval + Duration::from_secs(1)).min(Duration::from_secs(5));
        let response = match client.link_token_get(&link_token) {
            Ok(response) => response,
            Err(error) => {
                if error.kind == crate::providers::ProviderErrorKind::Transient
                    || error.kind == crate::providers::ProviderErrorKind::RateLimited
                {
                    continue;
                }
                if let Some(state) = app.try_state::<AppState>() {
                    set_status(&state, "error", Some(error.safe_message.into()), None);
                }
                return;
            }
        };
        if cancel.load(Ordering::SeqCst) {
            if let Some(state) = app.try_state::<AppState>() {
                set_status(&state, "cancelled", Some("Plaid link cancelled.".into()), None);
            }
            return;
        }
        if let Some(connection_id) = reconnect_connection_id.as_deref() {
            if response.session_finished() {
                finish_reconnect(&app, &client, connection_id);
                return;
            }
            continue;
        }
        if let Some(item) = response.first_linked_item() {
            finish_link(&app, &client, item);
            return;
        }
        if response.session_finished_without_item() {
            if let Some(state) = app.try_state::<AppState>() {
                set_status(
                    &state,
                    "error",
                    Some("The Plaid session finished without adding an Item.".into()),
                    None,
                );
            }
            return;
        }
    }
}

fn finish_link<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    client: &PlaidClient,
    item: crate::providers::plaid::LinkedItem,
) {
    let Some(state) = app.try_state::<AppState>() else { return };
    let exchange = match client.exchange_public_token(&item.public_token) {
        Ok(exchange) => exchange,
        Err(error) => {
            set_status(&state, "error", Some(error.safe_message.into()), None);
            return;
        }
    };
    let credential_ref = Uuid::new_v4().to_string();
    let token = SecretString::new(exchange.access_token);
    if let Err(error) = store_access_token(state.credentials.as_ref(), &credential_ref, token) {
        set_status(&state, "error", Some(AppError::safe_message(&error)), None);
        return;
    }
    let connection_id = Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let inserted = state.with_db(|conn, _h, _s| {
        conn.execute(
            "INSERT INTO connections (id, provider, environment, institution_id, institution_name, \
                credential_ref, status, created_at, updated_at) \
             VALUES (?1, 'plaid', 'sandbox', ?2, ?3, ?4, 'healthy', ?5, ?5)",
            rusqlite::params![
                connection_id,
                item.institution_id,
                item.institution_name,
                credential_ref,
                now
            ],
        )?;
        let _ = record_event(
            conn,
            "credential_added",
            "success",
            Some(serde_json::json!({ "provider": "plaid", "environment": "sandbox" })),
        );
        Ok(())
    });
    if let Err(error) = inserted {
        let _ = state.credentials.delete(&crate::providers::plaid::access_token_account(&credential_ref));
        set_status(&state, "error", Some(AppError::safe_message(&error)), None);
        return;
    }
    set_status(
        &state,
        "linked",
        Some("Sandbox Item linked. Syncing transactions and balances.".into()),
        Some(connection_id.clone()),
    );
    // Initial sync is attempted; a failure is visible in Connections & sync and
    // does not undo the link.
    let _ = crate::services::sync::sync_connection_now(&state, &connection_id, "manual");
}

pub fn start_reconnect<R: tauri::Runtime>(
    state: &AppState,
    app: &tauri::AppHandle<R>,
    connection_id: &str,
) -> AppResult<PlaidLinkStatus> {
    state.require_real("reconnect Plaid in demo mode")?;
    let (credential_ref, environment, status) = state.with_db(|conn, _h, _s| {
        conn.query_row(
            "SELECT credential_ref, environment, status FROM connections WHERE id = ?1 AND provider = 'plaid'",
            [connection_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
        )
        .map_err(|_| AppError::NotFound)
    })?;
    if status == "disconnected" {
        return Err(AppError::Blocked("this connection is disconnected"));
    }
    if environment != "sandbox" {
        return Err(AppError::Blocked("Production Plaid is disabled"));
    }
    let vault_id = state.with_db(|_conn, header, _s| {
        header.map(|header| header.vault_id_hex()).ok_or(AppError::Locked)
    })?;
    let credentials = load_sandbox_credentials(state.credentials.as_ref())?
        .ok_or(AppError::ProviderNotConfigured)?;
    let token = load_access_token(state.credentials.as_ref(), &credential_ref)?
        .ok_or(AppError::ProviderNotConfigured)?;
    {
        let mut runtime = state.plaid_link.lock().map_err(|_| AppError::Internal)?;
        if runtime.active {
            return Err(AppError::Blocked("a Plaid link is already in progress"));
        }
        runtime.status = "starting".into();
        runtime.message = None;
        runtime.connection_id = Some(connection_id.to_string());
        runtime.expires_at = None;
        runtime.active = true;
        runtime.cancel = Some(Arc::new(AtomicBool::new(false)));
        runtime.mode = "reconnect".into();
        runtime.reconnect_connection_id = Some(connection_id.to_string());
    }
    let client = PlaidClient::new(credentials, Some(token))
        .map_err(|_| AppError::ProviderNotConfigured)?;
    let token = match client.create_update_link_token(&vault_id) {
        Ok(token) => token,
        Err(error) => {
            set_status(state, "error", Some(error.safe_message.into()), Some(connection_id.to_string()));
            return Err(provider_app_error(error));
        }
    };
    let Some(url) = token.hosted_link_url else {
        set_status(state, "error", Some("Hosted Link update mode is unavailable.".into()), Some(connection_id.to_string()));
        return Err(AppError::Blocked("Hosted Link update mode unavailable"));
    };
    if validate_plaid_hosted_url(&url).is_err() {
        set_status(state, "error", Some("Plaid returned an invalid Hosted Link URL.".into()), Some(connection_id.to_string()));
        return Err(AppError::Blocked("invalid Hosted Link URL"));
    }
    let expires_at = token.expiration;
    let link_token = SecretString::new(token.link_token);
    {
        let mut runtime = state.plaid_link.lock().map_err(|_| AppError::Internal)?;
        runtime.status = "waiting".into();
        runtime.expires_at = Some(expires_at.clone());
        runtime.message = Some("Reconnect the Sandbox Item in your browser.".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|_| {
            set_status(state, "error", Some("Could not open the browser.".into()), Some(connection_id.to_string()));
            AppError::Provider("Could not open the browser.")
        })?;
    let cancel = state
        .plaid_link
        .lock()
        .ok()
        .and_then(|runtime| runtime.cancel.clone())
        .ok_or(AppError::Internal)?;
    let expires = DateTime::parse_from_rfc3339(&expires_at)
        .ok()
        .map(|value| value.with_timezone(&Utc));
    let app_handle = app.clone();
    let connection = connection_id.to_string();
    std::thread::Builder::new()
        .name("cm-plaid-reconnect".into())
        .spawn(move || poll_link(app_handle, client, link_token, cancel, expires, Some(connection)))
        .map_err(|_| AppError::Internal)?;
    link_status(state)
}

fn finish_reconnect<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    client: &PlaidClient,
    connection_id: &str,
) {
    let Some(state) = app.try_state::<AppState>() else { return };
    // Update mode has no new public token; verify the existing access token
    // actually works before declaring the connection healthy.
    if let Err(error) = client.accounts() {
        set_status(
            &state,
            "error",
            Some(error.safe_message.into()),
            Some(connection_id.to_string()),
        );
        return;
    }
    let now = now_rfc3339();
    let updated = state.with_db(|conn, _h, _s| {
        conn.execute(
            "UPDATE connections SET status = 'healthy', sanitized_error = NULL, \
                last_attempt_at = ?2, updated_at = ?2 WHERE id = ?1",
            rusqlite::params![connection_id, now],
        )?;
        Ok(())
    });
    if let Err(error) = updated {
        set_status(&state, "error", Some(AppError::safe_message(&error)), None);
        return;
    }
    set_status(
        &state,
        "linked",
        Some("Sandbox Item reconnected. Syncing again.".into()),
        Some(connection_id.to_string()),
    );
    let _ = crate::services::sync::sync_connection_now(&state, connection_id, "manual");
}

pub fn cancel(state: &AppState) -> AppResult<PlaidLinkStatus> {
    let runtime = state.plaid_link.lock().map_err(|_| AppError::Internal)?;
    if let Some(cancel) = &runtime.cancel {
        cancel.store(true, Ordering::SeqCst);
    }
    drop(runtime);
    set_status(state, "cancelled", Some("Plaid link cancelled.".into()), None);
    link_status(state)
}

pub fn test_credentials(state: &AppState) -> AppResult<PlaidStatus> {
    state.require_real("test Plaid credentials in demo mode")?;
    let credentials = load_sandbox_credentials(state.credentials.as_ref())?
        .ok_or(AppError::ProviderNotConfigured)?;
    let client = PlaidClient::new(credentials, None).map_err(|_| AppError::ProviderNotConfigured)?;
    client
        .create_hosted_link_token("cardigan-money-credential-test")
        .map_err(provider_app_error)?;
    status(state)
}

pub fn get_status(state: &AppState) -> AppResult<PlaidLinkStatus> {
    state.require_real("link Plaid in demo mode")?;
    link_status(state)
}

pub fn environment() -> PlaidEnvironment {
    PlaidEnvironment::Sandbox
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaidVerificationConnection {
    pub institution_name: Option<String>,
    pub environment: String,
    pub status: String,
    pub account_count: i64,
    pub provider_transaction_count: i64,
    pub pending_transaction_count: i64,
    pub last_success_at: Option<String>,
    pub cursor_present: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaidVerificationReport {
    pub environment: String,
    pub connections: Vec<PlaidVerificationConnection>,
    pub latest_run_status: Option<String>,
    pub latest_liabilities_status: Option<String>,
    pub recent_runs: Vec<crate::services::sync::SyncRunView>,
    pub generated_at: String,
}

/// Aggregate, non-secret evidence for the Phase 9 verification record. No tokens,
/// credential references, account numbers, descriptions, or amounts are included.
pub fn verification_report(state: &AppState) -> AppResult<PlaidVerificationReport> {
    state.require_real("verify Plaid in demo mode")?;
    let connections = state.with_db(|conn, _h, _s| {
        let mut stmt = conn.prepare(
            "SELECT institution_name, environment, status, last_success_at, cursor IS NOT NULL, \
                (SELECT count(*) FROM account_sources s WHERE s.connection_id = c.id AND s.unlinked_at IS NULL), \
                (SELECT count(*) FROM transactions t JOIN account_sources s ON s.account_id = t.account_id \
                    WHERE s.connection_id = c.id AND s.unlinked_at IS NULL AND t.removed = 0), \
                (SELECT count(*) FROM transactions t JOIN account_sources s ON s.account_id = t.account_id \
                    WHERE s.connection_id = c.id AND s.unlinked_at IS NULL AND t.removed = 0 AND t.pending = 1) \
             FROM connections c WHERE c.provider = 'plaid' AND c.status <> 'disconnected' ORDER BY c.created_at",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(PlaidVerificationConnection {
                    institution_name: row.get(0)?,
                    environment: row.get(1)?,
                    status: row.get(2)?,
                    account_count: row.get(5)?,
                    provider_transaction_count: row.get(6)?,
                    pending_transaction_count: row.get(7)?,
                    last_success_at: row.get(3)?,
                    cursor_present: row.get::<_, i64>(4)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })?;
    let recent_runs = crate::services::sync::recent_runs(state, 3)?;
    let latest_run_status = recent_runs.first().map(|run| run.status.clone());
    let latest_liabilities_status = recent_runs
        .first()
        .and_then(|run| run.steps.iter().find(|step| step.stream == "liabilities"))
        .map(|step| step.status.clone());
    Ok(PlaidVerificationReport {
        environment: "sandbox".into(),
        connections,
        latest_run_status,
        latest_liabilities_status,
        recent_runs,
        generated_at: now_rfc3339(),
    })
}
