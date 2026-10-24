use rusqlite::Connection;
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::storage::time::now_rfc3339;

pub const EVENT_TYPES: &[&str] = &[
    "vault_created",
    "unlock_success",
    "unlock_failure",
    "lock",
    "backup_created",
    "backup_verified",
    "backup_failed",
    "restore_started",
    "restore_succeeded",
    "restore_failed",
    "export",
    "rekey",
    "credential_added",
    "credential_removed",
    "integrity_failure",
    "single_instance_rejected",
    "migration_applied",
    "migration_failed",
    "demo_reset",
    "delete_all",
    "sync_started",
    "sync_finished",
    "settings_changed",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityEvent {
    pub id: String,
    pub event_type: String,
    pub outcome: String,
    pub created_at: String,
    pub metadata: Option<serde_json::Value>,
}

fn sanitize_metadata(metadata: Option<serde_json::Value>) -> AppResult<Option<String>> {
    let Some(value) = metadata else {
        return Ok(None);
    };
    let Some(object) = value.as_object() else {
        return Err(AppError::InvalidInput("event metadata"));
    };
    let mut clean = serde_json::Map::new();
    for (key, value) in object {
        let lower = key.to_ascii_lowercase();
        if [
            "passphrase",
            "password",
            "secret",
            "token",
            "dek",
            "kek",
            "sql",
            "path",
        ]
        .iter()
        .any(|forbidden| lower.contains(forbidden))
        {
            continue;
        }
        if key.len() > 32 {
            continue;
        }
        let allowed = value.is_boolean()
            || value.is_i64()
            || value.is_u64()
            || value.as_str().map(|s| s.len() <= 64).unwrap_or(false);
        if allowed {
            clean.insert(key.clone(), value.clone());
        }
    }
    let encoded = serde_json::to_string(&clean).map_err(|_| AppError::Internal)?;
    if encoded.len() > 1024 {
        return Err(AppError::InvalidInput("event metadata"));
    }
    Ok(Some(encoded))
}

pub fn record_event(
    conn: &Connection,
    event_type: &str,
    outcome: &str,
    metadata: Option<serde_json::Value>,
) -> AppResult<()> {
    if !EVENT_TYPES.contains(&event_type) {
        return Err(AppError::InvalidInput("event type"));
    }
    if !matches!(outcome, "success" | "failure" | "info") {
        return Err(AppError::InvalidInput("event outcome"));
    }
    let metadata = sanitize_metadata(metadata)?;
    conn.execute(
        "INSERT INTO security_events (id, event_type, outcome, created_at, metadata_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            event_type,
            outcome,
            now_rfc3339(),
            metadata
        ],
    )?;
    Ok(())
}

pub fn recent_events(conn: &Connection, limit: u32) -> AppResult<Vec<SecurityEvent>> {
    let limit = limit.clamp(1, 100);
    let mut stmt = conn.prepare(
        "SELECT id, event_type, outcome, created_at, metadata_json
         FROM security_events ORDER BY created_at DESC, rowid DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |row| {
        let metadata: Option<String> = row.get(4)?;
        Ok(SecurityEvent {
            id: row.get(0)?,
            event_type: row.get(1)?,
            outcome: row.get(2)?,
            created_at: row.get(3)?,
            metadata: metadata.and_then(|m| serde_json::from_str(&m).ok()),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_sanitizer_drops_secret_shaped_keys() {
        let encoded = sanitize_metadata(Some(serde_json::json!({
            "reason": "idle",
            "count": 2,
            "passphrase": "sentinel",
            "token": "sentinel",
            "path": "/Users/x",
            "deep": {"nested": true},
            "huge": "x".repeat(500),
        })))
        .unwrap()
        .unwrap();
        assert!(encoded.contains("idle"));
        assert!(encoded.contains("\"count\":2"));
        assert!(!encoded.contains("sentinel"));
        assert!(!encoded.contains("/Users"));
        assert!(!encoded.contains("nested"));
    }
}
