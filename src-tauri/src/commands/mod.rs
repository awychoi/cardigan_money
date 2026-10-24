use serde::{Deserialize, Serialize};
use tauri::{State, WebviewWindow};

use crate::error::{AppError, AppResult, ErrorPayload};
use crate::ipc::{guard_main_window, GatewayRequest, GatewayResponse, API_VERSION};
use crate::state::AppState;

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
pub struct VaultStatus {
    pub exists: bool,
    pub unlocked: bool,
    pub mode: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    pub ok: bool,
    pub api_version: u16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyPayload {}

/// Execute a validated command. Window/origin authorization happens in the
/// Tauri wrapper before this function so the command logic is unit-testable.
pub fn execute(state: &AppState, request: GatewayRequest) -> AppResult<serde_json::Value> {
    request.validate_envelope()?;
    match request.command.as_str() {
        "app_info" => {
            let _: EmptyPayload = request.decode_payload()?;
            let exists = state.paths.exists().unwrap_or(false);
            GatewayResponse::ok(AppInfo {
                name: "cardigan_money",
                version: env!("CARGO_PKG_VERSION"),
                bundle_identifier: "com.awychoi.cardiganmoney",
                platform: std::env::consts::OS,
                api_version: API_VERSION,
                vault_exists: exists,
                vault_unlocked: false,
                demo_available: true,
            })
        }
        "health_check" => {
            let _: EmptyPayload = request.decode_payload()?;
            GatewayResponse::ok(HealthCheck { ok: true, api_version: API_VERSION })
        }
        "vault_status" => {
            let _: EmptyPayload = request.decode_payload()?;
            GatewayResponse::ok(VaultStatus {
                exists: state.paths.exists().unwrap_or(false),
                unlocked: false,
                mode: "real",
            })
        }
        _ => Err(AppError::InvalidInput("unknown command")),
    }
    .map(|response| response.data)
}

#[tauri::command(async)]
pub fn gateway(
    window: WebviewWindow,
    state: State<'_, AppState>,
    request: GatewayRequest,
) -> Result<GatewayResponse, ErrorPayload> {
    guard_main_window(&window).map_err(ErrorPayload::from)?;
    #[cfg(debug_assertions)]
    eprintln!("cardigan_money dev: gateway request accepted");
    let data = execute(state.inner(), request).map_err(ErrorPayload::from)?;
    Ok(GatewayResponse { api_version: API_VERSION, data })
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
        let state = state();
        let info = execute(&state, req("app_info", json!({}))).unwrap();
        assert_eq!(info["name"], "cardigan_money");
        assert_eq!(info["apiVersion"], 1);
        let err = execute(&state, req("run_sql", json!({}))).unwrap_err();
        assert_eq!(err.code(), "invalid_input");
        let err = execute(&state, req("app_info", json!({"extra": true}))).unwrap_err();
        assert_eq!(err.code(), "invalid_input");
    }

    #[test]
    fn envelope_and_payload_bounds_are_enforced() {
        let state = state();
        let mut request = req("health_check", json!({}));
        request.v = 2;
        assert!(execute(&state, request).is_err());

        let request = req("health_check", json!({"blob": "x".repeat(crate::ipc::MAX_REQUEST_BYTES)}));
        assert!(execute(&state, request).is_err());

        let request = GatewayRequest { v: 1, command: "unknown".into(), payload: json!({}) };
        assert!(execute(&state, request).is_err());
    }

    #[test]
    fn malformed_ipc_corpus_is_rejected() {
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
            match serde_json::from_str::<GatewayRequest>(raw) {
                Ok(request) => {
                    assert!(
                        execute(&state, request).is_err(),
                        "malformed request accepted: {raw}"
                    );
                }
                Err(_) => {}
            }
        }
    }

    #[test]
    fn app_info_never_contains_a_secret_field() {
        let state = state();
        let value = execute(&state, req("app_info", json!({}))).unwrap();
        let text = serde_json::to_string(&value).unwrap().to_lowercase();
        for forbidden in ["passphrase", "secret", "token", "dek", "kek", "password"] {
            assert!(!text.contains(forbidden), "app_info leaked a {forbidden}-shaped field");
        }
    }
}
