use crate::error::AppResult;
use crate::services::vault::SecuritySummary;
use crate::state::AppState;
use crate::storage::events::{recent_events, SecurityEvent};

pub fn recent(state: &AppState, limit: u32) -> AppResult<Vec<SecurityEvent>> {
    state.with_db(|conn, _header, _settings| recent_events(conn, limit))
}

pub fn summary(state: &AppState) -> AppResult<SecuritySummary> {
    crate::services::vault::security_summary(state)
}
