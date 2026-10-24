use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State, WebviewWindow};

use crate::error::{AppError, AppResult, ErrorPayload};
use crate::ipc::{guard_main_window, GatewayRequest, GatewayResponse, API_VERSION};
use crate::security::secure_input::{self, SecurePurpose};
use crate::services::{security as security_service, vault as vault_service};
use crate::services::vault::SecuritySummary;
use crate::state::AppState;
use crate::storage::events::SecurityEvent;
use crate::storage::accounts::{self, Account, AccountInput, AccountType};
use crate::storage::settings::{apply_patch, save_settings, Settings, SettingsPatch};
use crate::domain::money::parse_currency_amount;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyPayload {}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EventsPayload {
    #[serde(default)]
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountsListPayload {
    #[serde(default)]
    include_archived: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountPayload {
    issuer: String,
    display_name: String,
    last_four: Option<String>,
    account_type: String,
    currency: String,
    credit_limit: Option<String>,
    include_in_overview: bool,
}

impl AccountPayload {
    fn into_input(self) -> AppResult<AccountInput> {
        let account_type = AccountType::parse(&self.account_type)?;
        let currency = self.currency.trim().to_uppercase();
        let credit_limit_minor = match self.credit_limit {
            None => None,
            Some(value) if value.trim().is_empty() => None,
            Some(value) => {
                let minor = parse_currency_amount(&value, &currency)
                    .map_err(|_| AppError::InvalidInput("credit limit"))?;
                if minor <= 0 || minor > 1_000_000_000_000 {
                    return Err(AppError::InvalidInput("credit limit"));
                }
                Some(minor)
            }
        };
        Ok(AccountInput {
            issuer: self.issuer,
            display_name: self.display_name,
            last_four: self.last_four,
            account_type,
            currency,
            credit_limit_minor,
            include_in_overview: self.include_in_overview,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountUpdatePayload {
    id: String,
    account: AccountPayload,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountArchivePayload {
    id: String,
    archived: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsView {
    pub accounts: Vec<Account>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub bundle_identifier: &'static str,
    pub platform: &'static str,
    pub api_version: u16,
    pub vault_exists: bool,
    pub vault_unlocked: bool,
    pub demo_available: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    pub ok: bool,
    pub api_version: u16,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub settings: Settings,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityView {
    pub security: SecuritySummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsView {
    pub events: Vec<SecurityEvent>,
}

/// Execute a validated command. Window/origin authorization happens in the
/// Tauri wrapper before this function so command logic is unit-testable.
pub fn execute<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    request: GatewayRequest,
) -> AppResult<serde_json::Value> {
    request.validate_envelope()?;
    match request.command.as_str() {
        "app_info" => {
            let _: EmptyPayload = request.decode_payload()?;
            let status = vault_service::status(state);
            GatewayResponse::ok(AppInfo {
                name: "cardigan_money",
                version: env!("CARGO_PKG_VERSION"),
                bundle_identifier: "com.awychoi.cardiganmoney",
                platform: std::env::consts::OS,
                api_version: API_VERSION,
                vault_exists: status.exists,
                vault_unlocked: status.unlocked,
                demo_available: true,
            })
        }
        "health_check" => {
            let _: EmptyPayload = request.decode_payload()?;
            GatewayResponse::ok(HealthCheck { ok: true, api_version: API_VERSION })
        }
        "vault_status" => {
            let _: EmptyPayload = request.decode_payload()?;
            GatewayResponse::ok(vault_service::status(state))
        }
        "vault_create" => {
            let _: EmptyPayload = request.decode_payload()?;
            state.rate_limit("vault_create", 3, 60)?;
            let Some(passphrase) =
                secure_input::ask_new_confirmed(app, SecurePurpose::CreateVault)?
            else {
                return Err(AppError::Cancelled);
            };
            GatewayResponse::ok(vault_service::create(state, passphrase)?)
        }
        "vault_unlock" => {
            let _: EmptyPayload = request.decode_payload()?;
            state.rate_limit("vault_unlock", 10, 60)?;
            let Some(passphrase) = secure_input::ask(app, SecurePurpose::Unlock)? else {
                return Err(AppError::Cancelled);
            };
            let mut values = passphrase;
            let secret = values.pop().ok_or(AppError::NativeInput)?;
            GatewayResponse::ok(vault_service::unlock(state, secret)?)
        }
        "vault_lock" => {
            let _: EmptyPayload = request.decode_payload()?;
            state.lock(app, "manual");
            GatewayResponse::ok(vault_service::status(state))
        }
        "vault_change_passphrase" => {
            let _: EmptyPayload = request.decode_payload()?;
            state.rate_limit("vault_change_passphrase", 2, 60)?;
            let Some(mut current_values) =
                secure_input::ask(app, SecurePurpose::ChangeCurrent)?
            else {
                return Err(AppError::Cancelled);
            };
            let current = current_values.pop().ok_or(AppError::NativeInput)?;
            drop(current_values);
            let Some(new_passphrase) =
                secure_input::ask_new_confirmed(app, SecurePurpose::ChangeNew)?
            else {
                return Err(AppError::Cancelled);
            };
            vault_service::change_passphrase(state, app, current, new_passphrase)?;
            GatewayResponse::ok(vault_service::status(state))
        }
        "settings_get" => {
            let _: EmptyPayload = request.decode_payload()?;
            let settings = state.with_db(|_conn, _header, settings| Ok(settings.clone()))?;
            GatewayResponse::ok(SettingsView { settings })
        }
        "settings_update" => {
            let patch: SettingsPatch = request.decode_payload()?;
            let updated = state.with_db(|conn, _header, current| {
                let next = apply_patch(current, patch)?;
                save_settings(conn, &next)?;
                crate::storage::events::record_event(
                    conn,
                    "settings_changed",
                    "success",
                    None,
                )?;
                Ok(next)
            })?;
            {
                let mut guard = state.session.lock().map_err(|_| AppError::Internal)?;
                guard.settings = Some(updated.clone());
            }
            GatewayResponse::ok(SettingsView { settings: updated })
        }
        "security_status" => {
            let _: EmptyPayload = request.decode_payload()?;
            let summary: SecuritySummary = security_service::summary(state)?;
            GatewayResponse::ok(SecurityView { security: summary })
        }
        "accounts_list" => {
            let payload: AccountsListPayload = request.decode_payload()?;
            let include_archived = payload.include_archived.unwrap_or(false);
            let accounts = state.with_db(|conn, _header, _settings| {
                accounts::list(conn, include_archived)
            })?;
            GatewayResponse::ok(AccountsView { accounts })
        }
        "account_create" => {
            let payload: AccountPayload = request.decode_payload()?;
            let input = payload.into_input()?;
            let created = state.with_db(|conn, _header, _settings| {
                let count: i64 = conn
                    .query_row("SELECT count(*) FROM accounts", [], |row| row.get(0))
                    .map_err(|_| AppError::Database)?;
                if count >= 100 {
                    return Err(AppError::Blocked("account limit reached"));
                }
                accounts::create_manual(conn, &input)
            })?;
            GatewayResponse::ok(created)
        }
        "account_update" => {
            let payload: AccountUpdatePayload = request.decode_payload()?;
            let input = payload.account.into_input()?;
            let updated = state.with_db(|conn, _header, _settings| {
                accounts::update_manual(conn, &payload.id, &input)
            })?;
            GatewayResponse::ok(updated)
        }
        "account_archive" => {
            let payload: AccountArchivePayload = request.decode_payload()?;
            let updated = state.with_db(|conn, _header, _settings| {
                accounts::set_archived(conn, &payload.id, payload.archived)
            })?;
            GatewayResponse::ok(updated)
        }
        "app_quit" => {
            let _: EmptyPayload = request.decode_payload()?;
            state
                .quitting
                .store(true, std::sync::atomic::Ordering::SeqCst);
            state.lock(app, "quit");
            app.exit(0);
            GatewayResponse::ok(serde_json::json!({ "quitting": true }))
        }
        "security_events_list" => {
            let payload: EventsPayload = request.decode_payload()?;
            let limit = payload.limit.unwrap_or(20);
            if limit == 0 || limit > 100 {
                return Err(AppError::InvalidInput("event limit"));
            }
            let events = security_service::recent(state, limit)?;
            GatewayResponse::ok(EventsView { events })
        }
        _ => Err(AppError::InvalidInput("unknown command")),
    }
    .map(|response| response.data)
}

#[tauri::command(async)]
pub fn gateway(
    window: WebviewWindow,
    app: AppHandle,
    state: State<'_, AppState>,
    request: GatewayRequest,
) -> Result<GatewayResponse, ErrorPayload> {
    guard_main_window(&window).map_err(ErrorPayload::from)?;
    #[cfg(debug_assertions)]
    eprintln!("cardigan_money dev: gateway request accepted");
    let data = execute(&app, state.inner(), request).map_err(ErrorPayload::from)?;
    Ok(GatewayResponse { api_version: API_VERSION, data })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn app() -> AppHandle<tauri::test::MockRuntime> {
        crate::test_support::mock_app()
    }

    fn state() -> AppState {
        let tmp = std::env::temp_dir().join(format!("cm-ipc-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        AppState::new(tmp).unwrap()
    }

    fn req(command: &str, payload: serde_json::Value) -> GatewayRequest {
        GatewayRequest { v: 1, command: command.into(), payload }
    }

    #[test]
    fn known_commands_work_and_unknown_commands_are_rejected() {
        let app = app();
        let state = state();
        let info = execute(&app, &state, req("app_info", json!({}))).unwrap();
        assert_eq!(info["name"], "cardigan_money");
        assert_eq!(info["apiVersion"], 1);
        assert_eq!(info["vaultUnlocked"], false);
        let err = execute(&app, &state, req("run_sql", json!({}))).unwrap_err();
        assert_eq!(err.code(), "invalid_input");
        let err = execute(&app, &state, req("app_info", json!({ "extra": true }))).unwrap_err();
        assert_eq!(err.code(), "invalid_input");
    }

    #[test]
    fn envelope_and_payload_bounds_are_enforced() {
        let app = app();
        let state = state();
        let mut request = req("health_check", json!({}));
        request.v = 2;
        assert!(execute(&app, &state, request).is_err());

        let request =
            req("health_check", json!({ "blob": "x".repeat(crate::ipc::MAX_REQUEST_BYTES) }));
        assert!(execute(&app, &state, request).is_err());

        let request = GatewayRequest { v: 1, command: "unknown".into(), payload: json!({}) };
        assert!(execute(&app, &state, request).is_err());
    }

    #[test]
    fn malformed_ipc_corpus_is_rejected() {
        let app = app();
        let state = state();
        let corpus = [
            r#"{"v":1,"command":"app_info","payload":{},"path":"/etc/passwd"}"#,
            r#"{"v":1,"command":"app_info","payload":{"url":"https://example.com"}}"#,
            r#"{"v":1,"command":"app_info","payload":{"sql":"SELECT 1"}}"#,
            r#"{"v":1,"command":"app_info","payload":[]}"#,
            r#"{"v":1,"command":"app_info","payload":null}"#,
            r#"{"v":0,"command":"app_info","payload":{}}"#,
            r#"{"v":1,"command":"APP_INFO","payload":{}}"#,
            r#"{"v":1,"command":"app_info; DROP TABLE settings","payload":{}}"#,
            r#"{"v":1,"command":"","payload":{}}"#,
            r#"{"v":1,"command":"app_info","payload":{"x":"\u0000"}}"#,
        ];
        for raw in corpus {
            if let Ok(request) = serde_json::from_str::<GatewayRequest>(raw) {
                assert!(execute(&app, &state, request).is_err(), "malformed request accepted: {raw}");
            }
        }
    }

    #[test]
    fn locked_commands_fail_with_vault_locked() {
        let app = app();
        let state = state();
        for command in ["settings_get", "security_status"] {
            let err = execute(&app, &state, req(command, json!({}))).unwrap_err();
            assert_eq!(err.code(), "vault_locked", "{command}");
        }
        let err =
            execute(&app, &state, req("security_events_list", json!({"limit": 0}))).unwrap_err();
        assert_eq!(err.code(), "invalid_input");
        let err =
            execute(&app, &state, req("security_events_list", json!({"limit": 5}))).unwrap_err();
        assert_eq!(err.code(), "vault_locked");
    }

    #[test]
    fn app_info_never_contains_a_secret_field() {
        let app = app();
        let state = state();
        let value = execute(&app, &state, req("app_info", json!({}))).unwrap();
        let text = serde_json::to_string(&value).unwrap().to_lowercase();
        for forbidden in ["passphrase", "secret", "token", "dek", "kek", "password"] {
            assert!(!text.contains(forbidden), "app_info leaked a {forbidden}-shaped field");
        }
    }

    #[test]
    fn account_payload_parses_credit_limit_as_decimal_cents() {
        let payload = AccountPayload {
            issuer: "Chase".into(),
            display_name: "Sapphire".into(),
            last_four: Some("1234".into()),
            account_type: "credit_card".into(),
            currency: "usd".into(),
            credit_limit: Some("$12,500.00".into()),
            include_in_overview: true,
        };
        let input = payload.into_input().unwrap();
        assert_eq!(input.currency, "USD");
        assert_eq!(input.credit_limit_minor, Some(1_250_000));
        let bad = AccountPayload {
            issuer: "Chase".into(),
            display_name: "Sapphire".into(),
            last_four: Some("12".into()),
            account_type: "credit_card".into(),
            currency: "USD".into(),
            credit_limit: Some("12.5".into()),
            include_in_overview: true,
        };
        assert!(bad.into_input().is_ok());
        let bad_currency = AccountPayload {
            issuer: "Chase".into(),
            display_name: "Sapphire".into(),
            last_four: None,
            account_type: "credit_card".into(),
            currency: "XYZ".into(),
            credit_limit: None,
            include_in_overview: true,
        };
        assert!(bad_currency.into_input().is_ok());
    }

    #[test]
    fn settings_update_requires_an_unlocked_vault() {
        let app = app();
        let state = state();
        let err = execute(
            &app,
            &state,
            req("settings_update", json!({"theme": "dark"})),
        )
        .unwrap_err();
        assert_eq!(err.code(), "vault_locked");
    }
}
