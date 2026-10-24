//! Typed IPC envelope and gateway guards.
//!
//! The renderer can call exactly one custom command (`gateway`). It carries a
//! versioned command name and a JSON payload. Each command implementation
//! deserializes its own `deny_unknown_fields` request struct and validates
//! bounds. There is no generic SQL/file/URL/shell/HTTP command.

use serde::{Deserialize, Serialize};
use tauri::WebviewWindow;

use crate::error::{AppError, AppResult};

pub const API_VERSION: u16 = 1;
pub const MAX_REQUEST_BYTES: usize = 256 * 1024;
pub const MAX_COMMAND_LEN: usize = 64;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayRequest {
    pub v: u16,
    pub command: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

impl GatewayRequest {
    pub fn validate_envelope(&self) -> AppResult<()> {
        if self.v != API_VERSION {
            return Err(AppError::InvalidInput("unsupported API version"));
        }
        if self.command.is_empty() || self.command.len() > MAX_COMMAND_LEN {
            return Err(AppError::InvalidInput("command name length"));
        }
        if !self
            .command
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(AppError::InvalidInput("command name charset"));
        }
        let encoded = serde_json::to_vec(&self.payload).map_err(|_| AppError::InvalidInput("payload"))?;
        if encoded.len() > MAX_REQUEST_BYTES {
            return Err(AppError::InvalidInput("payload too large"));
        }
        Ok(())
    }

    pub fn decode_payload<T: serde::de::DeserializeOwned>(&self) -> AppResult<T> {
        // Every command takes a JSON object. Reject arrays, strings, numbers,
        // and null even when serde could coerce them into an empty struct.
        if !self.payload.is_object() {
            return Err(AppError::InvalidInput("payload must be an object"));
        }
        serde_json::from_value(self.payload.clone())
            .map_err(|_| AppError::InvalidInput("payload shape"))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayResponse {
    pub api_version: u16,
    pub data: serde_json::Value,
}

impl GatewayResponse {
    pub fn ok<T: Serialize>(data: T) -> AppResult<Self> {
        Ok(Self {
            api_version: API_VERSION,
            data: serde_json::to_value(data).map_err(|_| AppError::Internal)?,
        })
    }
}

/// Only the single `main` window at the app origin may invoke commands.
/// Dev origins are accepted only in debug builds.
pub fn guard_main_window(window: &WebviewWindow) -> AppResult<()> {
    if window.label() != "main" {
        return Err(AppError::PermissionDenied("window label"));
    }
    let url = window.url().map_err(|_| AppError::PermissionDenied("window origin"))?;
    let allowed = match url.scheme() {
        "tauri" => url.host_str() == Some("localhost"),
        "http" => {
            cfg!(debug_assertions)
                && (url.host_str() == Some("localhost") || url.host_str() == Some("127.0.0.1"))
                && url.port() == Some(1420)
        }
        _ => false,
    };
    if !allowed {
        return Err(AppError::PermissionDenied("window origin"));
    }
    Ok(())
}

/// Navigation allowlist for the webview. Exactly the app origin (plus the local
/// dev server in debug builds). Everything else, including file: and https:, is
/// blocked before load.
pub fn is_allowed_navigation(url: &tauri::Url) -> bool {
    match url.scheme() {
        "tauri" => url.host_str() == Some("localhost"),
        "http" => {
            cfg!(debug_assertions)
                && (url.host_str() == Some("localhost") || url.host_str() == Some("127.0.0.1"))
                && url.port() == Some(1420)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_rejects_unknown_fields_and_bad_versions() {
        let json = r#"{"v":1,"command":"health_check","payload":{},"extra":1}"#;
        assert!(serde_json::from_str::<GatewayRequest>(json).is_err());
        let req = GatewayRequest { v: 2, command: "health_check".into(), payload: serde_json::json!({}) };
        assert!(req.validate_envelope().is_err());
        let req = GatewayRequest { v: 1, command: "Bad Command".into(), payload: serde_json::json!({}) };
        assert!(req.validate_envelope().is_err());
        let req = GatewayRequest { v: 1, command: "health_check".into(), payload: serde_json::json!({"blob":"x".repeat(MAX_REQUEST_BYTES)}) };
        assert!(req.validate_envelope().is_err());
    }

    #[test]
    fn navigation_allowlist_blocks_remote_and_file() {
        assert!(is_allowed_navigation(&tauri::Url::parse("tauri://localhost/index.html").unwrap()));
        assert!(!is_allowed_navigation(&tauri::Url::parse("https://example.com/").unwrap()));
        assert!(!is_allowed_navigation(&tauri::Url::parse("http://example.com/").unwrap()));
        assert!(!is_allowed_navigation(&tauri::Url::parse("file:///etc/passwd").unwrap()));
        #[cfg(not(debug_assertions))]
        assert!(!is_allowed_navigation(&tauri::Url::parse("http://localhost:1420/").unwrap()));
    }
}
