//! Provider sync engine. Cursor advancement is transactional with page writes;
//! provider errors and vault locks stop without inventing or deleting data.

use std::time::Instant;

use rusqlite::Connection;
use serde::Serialize;
use uuid::Uuid;

use crate::domain::dates::{choose_reporting_date, format_date, parse_date, DateSource};
use crate::domain::text::{clean_text, normalize_merchant, MAX_CATEGORY_CHARS, MAX_DESCRIPTION_CHARS};
use crate::error::{AppError, AppResult};
use crate::providers::mock::MockProvider;
use crate::providers::plaid::{
    load_access_token, load_sandbox_credentials, PlaidClient, PlaidProvider,
};
use crate::providers::{
    Capability, ProviderAccount, ProviderBalance, ProviderClient, ProviderErrorKind,
    ProviderLiability, ProviderTransaction,
};
use crate::state::AppState;
use crate::storage::events::record_event;
use crate::storage::ledger::LedgerQuery;
use crate::storage::time::now_rfc3339;

const MAX_PAGES: usize = 100;

#[derive(Debug, Clone)]
pub struct ConnectionRow {
    id: String,
    provider: String,
    environment: String,
    institution_name: Option<String>,
    status: String,
    cursor: Option<String>,
    #[allow(dead_code)]
    credential_ref: String,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRuntimeStatus {
    pub running: bool,
    pub last_result: Option<String>,
    pub last_skip_reason: Option<String>,
    pub last_attempt_at: Option<String>,
    pub last_finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSyncResult {
    pub connection_id: String,
    pub institution: String,
    pub status: String,
    pub transactions_added: i64,
    pub transactions_modified: i64,
    pub transactions_removed: i64,
    pub balances_written: i64,
    pub liabilities_status: String,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSummary {
    pub accepted: bool,
    pub skip_reason: Option<String>,
    pub results: Vec<ConnectionSyncResult>,
    pub started_at: String,
    pub finished_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionView {
    pub id: String,
    pub provider: String,
    pub environment: String,
    pub institution_name: Option<String>,
    pub status: String,
    pub last_attempt_at: Option<String>,
    pub last_success_at: Option<String>,
    pub sanitized_error: Option<String>,
    pub has_cursor: bool,
    pub account_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStepView {
    pub stream: String,
    pub status: String,
    pub items_added: i64,
    pub items_modified: i64,
    pub items_removed: i64,
    pub sanitized_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRunView {
    pub id: String,
    pub connection_id: String,
    pub institution_name: Option<String>,
    pub trigger: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub error_code: Option<String>,
    pub steps: Vec<SyncStepView>,
}

pub fn sync_now(state: &AppState, trigger: &str) -> AppResult<SyncSummary> {
    let started_at = now_rfc3339();
    let _guard = match state.sync_gate.try_lock() {
        Ok(guard) => guard,
        Err(_) => {
            let mut runtime = state.sync_runtime.lock().map_err(|_| AppError::Internal)?;
            runtime.last_skip_reason = Some("already_running".into());
            return Ok(SyncSummary {
                accepted: false,
                skip_reason: Some("already_running".into()),
                results: vec![],
                started_at: started_at.clone(),
                finished_at: started_at,
            });
        }
    };
    {
        let mut runtime = state.sync_runtime.lock().map_err(|_| AppError::Internal)?;
        runtime.running = true;
        runtime.last_attempt_at = Some(now_rfc3339());
        runtime.last_skip_reason = None;
    }
    let result = sync_all_inner(state, trigger);
    {
        let mut runtime = state.sync_runtime.lock().map_err(|_| AppError::Internal)?;
        runtime.running = false;
        runtime.last_finished_at = Some(now_rfc3339());
        match &result {
            Ok(summary) => {
                runtime.last_result = Some(if summary.accepted { "completed" } else { "skipped" }.into());
                if let Some(reason) = &summary.skip_reason {
                    runtime.last_skip_reason = Some(reason.clone());
                }
            }
            Err(_) => runtime.last_result = Some("failed".into()),
        }
    }
    result
}

fn sync_all_inner(state: &AppState, trigger: &str) -> AppResult<SyncSummary> {
    let started_at = now_rfc3339();
    let connections = match state.with_db(|conn, _h, _s| load_connections(conn)) {
        Ok(connections) => connections,
        Err(AppError::Locked) => {
            return Ok(SyncSummary {
                accepted: false,
                skip_reason: Some("vault_locked".into()),
                results: vec![],
                started_at: started_at.clone(),
                finished_at: now_rfc3339(),
            });
        }
        Err(error) => return Err(error),
    };
    if connections.is_empty() {
        return Ok(SyncSummary {
            accepted: true,
            skip_reason: Some("no_connections".into()),
            results: vec![],
            started_at: started_at.clone(),
            finished_at: now_rfc3339(),
        });
    }
    let mut results = Vec::new();
    for connection in connections {
        match sync_connection(state, &connection, trigger) {
            Ok(result) => results.push(result),
            Err(AppError::Locked) => {
                return Ok(SyncSummary {
                    accepted: false,
                    skip_reason: Some("vault_locked".into()),
                    results,
                    started_at: started_at.clone(),
                    finished_at: now_rfc3339(),
                });
            }
            Err(_) => {
                results.push(ConnectionSyncResult {
                    connection_id: connection.id,
                    institution: connection.institution_name.unwrap_or_else(|| "Provider".into()),
                    status: "failed".into(),
                    transactions_added: 0,
                    transactions_modified: 0,
                    transactions_removed: 0,
                    balances_written: 0,
                    liabilities_status: "skipped".into(),
                    error_code: Some("sync_failed".into()),
                });
            }
        }
    }
    Ok(SyncSummary {
        accepted: true,
        skip_reason: None,
        results,
        started_at,
        finished_at: now_rfc3339(),
    })
}

fn load_connections(conn: &Connection) -> AppResult<Vec<ConnectionRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, provider, environment, institution_name, status, cursor, credential_ref \
         FROM connections WHERE status <> 'disconnected' ORDER BY institution_name, id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ConnectionRow {
                id: row.get(0)?,
                provider: row.get(1)?,
                environment: row.get(2)?,
                institution_name: row.get(3)?,
                status: row.get(4)?,
                cursor: row.get(5)?,
                credential_ref: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn provider_for(state: &AppState, connection: &ConnectionRow) -> AppResult<Box<dyn ProviderClient>> {
    match connection.provider.as_str() {
        "mock" => Ok(Box::new(MockProvider::default())),
        "plaid" => {
            if connection.environment != "sandbox" {
                return Err(AppError::Blocked("Production Plaid is disabled until Sandbox verification"));
            }
            let credentials = load_sandbox_credentials(state.credentials.as_ref())?
                .ok_or(AppError::ProviderNotConfigured)?;
            let token = load_access_token(state.credentials.as_ref(), &connection.credential_ref)?
                .ok_or(AppError::ProviderNotConfigured)?;
            let client = PlaidClient::new(credentials, Some(token))
                .map_err(|_| AppError::ProviderNotConfigured)?;
            Ok(Box::new(PlaidProvider::new(client)))
        }
        _ => Err(AppError::Blocked("unsupported provider")),
    }
}

fn sync_connection(state: &AppState, connection: &ConnectionRow, trigger: &str) -> AppResult<ConnectionSyncResult> {
    let client = provider_for(state, connection)?;
    let result = sync_connection_with_client(state, connection, client.as_ref(), trigger);
    result
}

pub fn sync_connection_now(
    state: &AppState,
    connection_id: &str,
    trigger: &str,
) -> AppResult<ConnectionSyncResult> {
    let _guard = state
        .sync_gate
        .try_lock()
        .map_err(|_| AppError::Blocked("a sync is already running"))?;
    let connection = connection_by_id(state, connection_id)?;
    sync_connection(state, &connection, trigger)
}

struct RunTracker {
    run_id: String,
    steps: Vec<(String, String, i64, i64, i64, Option<String>)>,
}

fn start_run(state: &AppState, connection: &ConnectionRow, trigger: &str) -> AppResult<RunTracker> {
    state.with_db(|conn, _h, _s| {
        let run_id = Uuid::new_v4().to_string();
        let now = now_rfc3339();
        conn.execute(
            "INSERT INTO sync_runs (id, connection_id, trigger, started_at, finished_at, status, error_code, skip_reason) \
             VALUES (?1, ?2, ?3, ?4, NULL, 'running', NULL, NULL)",
            rusqlite::params![run_id, connection.id, trigger, now],
        )?;
        conn.execute(
            "UPDATE sync_runs SET status = 'failed', finished_at = ?1, error_code = 'interrupted' \
             WHERE connection_id = ?2 AND status = 'running' AND id <> ?3",
            rusqlite::params![now, connection.id, run_id],
        )?;
        conn.execute(
            "UPDATE connections SET last_attempt_at = ?2, updated_at = ?2 WHERE id = ?1",
            rusqlite::params![connection.id, now],
        )?;
        for stream in ["transactions", "balances", "liabilities"] {
            conn.execute(
                "INSERT OR IGNORE INTO sync_steps (id, run_id, stream, status, items_added, items_modified, items_removed, provider_as_of, sanitized_error, started_at, finished_at) \
                 VALUES (?1, ?2, ?3, 'pending', 0, 0, 0, NULL, NULL, ?4, NULL)",
                rusqlite::params![Uuid::new_v4().to_string(), run_id, stream, now],
            )?;
        }
        let _ = record_event(
            conn,
            "sync_started",
            "info",
            Some(serde_json::json!({ "trigger": trigger })),
        );
        Ok(RunTracker { run_id, steps: Vec::new() })
    })
}

fn finish_step(
    state: &AppState,
    run: &mut RunTracker,
    stream: &str,
    status: &str,
    added: i64,
    modified: i64,
    removed: i64,
    error: Option<&str>,
) -> AppResult<()> {
    state.with_db(|conn, _h, _s| {
        conn.execute(
            "UPDATE sync_steps SET status = ?2, items_added = ?3, items_modified = ?4, \
                items_removed = ?5, sanitized_error = ?6, finished_at = ?7 \
             WHERE run_id = ?1 AND stream = ?8",
            rusqlite::params![
                run.run_id,
                status,
                added,
                modified,
                removed,
                error,
                now_rfc3339(),
                stream
            ],
        )?;
        Ok(())
    })?;
    run.steps.push((stream.to_string(), status.to_string(), added, modified, removed, error.map(str::to_string)));
    Ok(())
}

fn ensure_accounts(
    conn: &Connection,
    connection: &ConnectionRow,
    accounts: &[ProviderAccount],
) -> AppResult<std::collections::HashMap<String, String>> {
    let now = now_rfc3339();
    let mut map = std::collections::HashMap::new();
    for account in accounts {
        let existing: Option<String> = conn
            .query_row(
                "SELECT account_id FROM account_sources WHERE connection_id = ?1 AND provider_account_id = ?2 AND unlinked_at IS NULL",
                rusqlite::params![connection.id, account.provider_account_id],
                |row| row.get(0),
            )
            .ok();
        let id = if let Some(id) = existing {
            id
        } else {
            let id = Uuid::new_v4().to_string();
            let issuer = connection
                .institution_name
                .clone()
                .unwrap_or_else(|| "Provider".to_string());
            let last_four = account.mask.chars().filter(|c| c.is_ascii_digit()).collect::<String>();
            let last_four = if last_four.len() >= 4 {
                Some(last_four[last_four.len() - 4..].to_string())
            } else {
                None
            };
            let account_type = match account.account_type.as_str() {
                "credit_card" | "charge_card" | "line_of_credit" => account.account_type.as_str(),
                _ => "other",
            };
            conn.execute(
                "INSERT INTO accounts (id, issuer, display_name, last_four, account_type, currency, \
                    credit_limit_minor, include_in_overview, archived, source_kind, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, 1, 0, 'provider', ?7, ?7)",
                rusqlite::params![
                    id,
                    issuer,
                    clean_text(&account.name, 120, false).unwrap_or_else(|_| "Provider account".into()),
                    last_four,
                    account_type,
                    account.currency,
                    now
                ],
            )?;
            conn.execute(
                "INSERT INTO account_sources (id, account_id, connection_id, provider_account_id, provider_mask, linked_at, unlinked_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
                rusqlite::params![
                    Uuid::new_v4().to_string(),
                    id,
                    connection.id,
                    account.provider_account_id,
                    account.mask,
                    now
                ],
            )?;
            id
        };
        map.insert(account.provider_account_id.clone(), id);
    }
    Ok(map)
}

fn apply_page(
    conn: &Connection,
    connection: &ConnectionRow,
    accounts: &std::collections::HashMap<String, String>,
    changes: &[ProviderTransaction],
    next_cursor: Option<&str>,
) -> AppResult<(i64, i64, i64)> {
    let now = now_rfc3339();
    let tx = conn.unchecked_transaction()?;
    let mut added = 0;
    let mut modified = 0;
    let mut removed = 0;
    for change in changes {
        if change.removed {
            if let Ok(id) = tx.query_row(
                "SELECT id FROM transactions WHERE provider_transaction_id = ?1 AND removed = 0",
                [&change.provider_transaction_id],
                |row| row.get::<_, String>(0),
            ) {
                tx.execute(
                    "UPDATE transactions SET removed = 1, updated_at = ?2 WHERE id = ?1",
                    rusqlite::params![id, now],
                )?;
                removed += 1;
            }
            continue;
        }
        let Some(account_id) = accounts.get(&change.provider_account_id) else {
            continue;
        };
        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM transactions WHERE account_id = ?1 AND provider_transaction_id = ?2",
                rusqlite::params![account_id, change.provider_transaction_id],
                |row| row.get(0),
            )
            .ok();
        let authorized = change.authorized_date.as_deref().and_then(|v| parse_date(v).ok());
        let posted = change.posted_date.as_deref().and_then(|v| parse_date(v).ok());
        let (reporting, source) = choose_reporting_date(authorized, posted, None)
            .map_err(|_| AppError::InvalidInput("provider transaction date"))?;
        let tx_type = if matches!(
            change.tx_type.as_str(),
            "purchase" | "refund" | "fee" | "interest" | "payment" | "transfer" | "other"
        ) {
            change.tx_type.clone()
        } else if change.amount_minor < 0 {
            "refund".to_string()
        } else {
            "purchase".to_string()
        };
        let description = clean_text(&change.description, MAX_DESCRIPTION_CHARS, false)
            .unwrap_or_else(|_| "Provider transaction".to_string());
        let normalized = normalize_merchant(&description)
            .unwrap_or_else(|_| "provider transaction".to_string());
        let category = change.category.as_deref().and_then(|value| {
            clean_text(value, MAX_CATEGORY_CHARS, false).ok()
        });
        if let Some(id) = existing {
            tx.execute(
                "UPDATE transactions SET amount_minor = ?2, currency = ?3, authorized_date = ?4, \
                    posted_date = ?5, reporting_date = ?6, reporting_date_source = ?7, pending = ?8, \
                    raw_description = ?9, merchant_normalized = ?10, merchant_display = ?11, \
                    category = ?12, tx_type = ?13, removed = 0, source_updated_at = ?14, updated_at = ?14 \
                 WHERE id = ?1",
                rusqlite::params![
                    id,
                    change.amount_minor,
                    change.currency,
                    change.authorized_date,
                    change.posted_date,
                    format_date(reporting),
                    source.as_str(),
                    i64::from(change.pending),
                    description,
                    normalized,
                    description.chars().take(200).collect::<String>(),
                    category,
                    tx_type,
                    now
                ],
            )?;
            modified += 1;
            if !change.pending {
                link_pending(&tx, account_id, &id, change.pending_transaction_id.as_deref(), &now)?;
            }
        } else {
            let id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO transactions (id, account_id, source_kind, provider_transaction_id, \
                    pending, authorized_date, posted_date, reporting_date, reporting_date_source, \
                    amount_minor, currency, raw_description, merchant_normalized, merchant_display, \
                    category, tx_type, excluded, removed, source_updated_at, created_at, updated_at) \
                 VALUES (?1, ?2, 'provider', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 0, 0, ?16, ?16, ?16)",
                rusqlite::params![
                    id,
                    account_id,
                    change.provider_transaction_id,
                    i64::from(change.pending),
                    change.authorized_date,
                    change.posted_date,
                    format_date(reporting),
                    source.as_str(),
                    change.amount_minor,
                    change.currency,
                    description,
                    normalized,
                    description.chars().take(200).collect::<String>(),
                    category,
                    tx_type,
                    now
                ],
            )?;
            added += 1;
            if !change.pending {
                link_pending(&tx, account_id, &id, change.pending_transaction_id.as_deref(), &now)?;
            }
        }
    }
    tx.execute(
        "UPDATE connections SET status = 'healthy', last_success_at = ?2, sanitized_error = NULL,             cursor = COALESCE(?3, cursor), updated_at = ?2 WHERE id = ?1",
        rusqlite::params![connection.id, now, next_cursor],
    )?;
    tx.commit()?;
    Ok((added, modified, removed))
}

fn link_pending(
    tx: &rusqlite::Transaction<'_>,
    account_id: &str,
    posted_id: &str,
    pending_provider_id: Option<&str>,
    now: &str,
) -> AppResult<()> {
    let Some(pending_provider_id) = pending_provider_id else {
        return Ok(());
    };
    let existing: Option<String> = tx
        .query_row(
            "SELECT id FROM transactions WHERE account_id = ?1 AND provider_transaction_id = ?2 AND pending = 1",
            rusqlite::params![account_id, pending_provider_id],
            |row| row.get(0),
        )
        .ok();
    if let Some(pending_id) = existing {
        tx.execute(
            "UPDATE transactions SET pending_link_id = ?2 WHERE id = ?1",
            rusqlite::params![posted_id, pending_id],
        )?;
        tx.execute(
            "UPDATE transactions SET removed = 1, updated_at = ?2 WHERE id = ?1",
            rusqlite::params![pending_id, now],
        )?;
    }
    Ok(())
}

fn apply_balances(
    conn: &Connection,
    connection: &ConnectionRow,
    accounts: &std::collections::HashMap<String, String>,
    balances: &[ProviderBalance],
) -> AppResult<i64> {
    let now = now_rfc3339();
    let mut written = 0;
    for balance in balances {
        let Some(account_id) = accounts.get(&balance.provider_account_id) else {
            continue;
        };
        if let Some(limit) = balance.credit_limit_minor {
            conn.execute(
                "UPDATE accounts SET credit_limit_minor = ?2, updated_at = ?3 WHERE id = ?1",
                rusqlite::params![account_id, limit, now],
            )?;
        }
        conn.execute(
            "INSERT INTO balance_snapshots (id, account_id, current_balance_minor, available_credit_minor, \
                credit_limit_minor, currency, retrieved_at, provider_as_of, provenance, source_connection_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'provider', ?9)",
            rusqlite::params![
                Uuid::new_v4().to_string(),
                account_id,
                balance.current_balance_minor,
                balance.available_credit_minor,
                balance.credit_limit_minor,
                balance.currency,
                now,
                balance.as_of,
                connection.id
            ],
        )?;
        written += 1;
    }
    Ok(written)
}

fn apply_liabilities(
    conn: &Connection,
    connection: &ConnectionRow,
    accounts: &std::collections::HashMap<String, String>,
    rows: &[ProviderLiability],
) -> AppResult<i64> {
    let now = now_rfc3339();
    let mut written = 0;
    for row in rows {
        let Some(account_id) = accounts.get(&row.provider_account_id) else {
            continue;
        };
        conn.execute(
            "INSERT INTO payment_snapshots (id, account_id, next_due_date, minimum_due_minor, \
                statement_balance_minor, last_statement_issue_date, next_closing_date, retrieved_at, provenance, source_connection_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'provider', ?9)",
            rusqlite::params![
                Uuid::new_v4().to_string(),
                account_id,
                row.next_due_date,
                row.minimum_due_minor,
                row.statement_balance_minor,
                row.last_statement_issue_date,
                row.next_closing_date,
                now,
                connection.id
            ],
        )?;
        written += 1;
    }
    Ok(written)
}

fn finish_run(
    state: &AppState,
    run: &RunTracker,
    _connection: &ConnectionRow,
    status: &str,
    error_code: Option<&str>,
    results: &ConnectionSyncResult,
    trigger: &str,
) -> AppResult<()> {
    state.with_db(|conn, _h, _s| {
        conn.execute(
            "UPDATE sync_runs SET status = ?2, finished_at = ?3, error_code = ?4 WHERE id = ?1",
            rusqlite::params![run.run_id, status, now_rfc3339(), error_code],
        )?;
        let _ = record_event(
            conn,
            "sync_finished",
            if status == "success" { "success" } else { "failure" },
            Some(serde_json::json!({
                "trigger": trigger,
                "added": results.transactions_added,
                "modified": results.transactions_modified,
                "removed": results.transactions_removed,
            })),
        );
        Ok(())
    })
}

fn mark_connection_error(state: &AppState, connection: &ConnectionRow, kind: ProviderErrorKind, message: &str) -> AppResult<()> {
    state.with_db(|conn, _h, _s| {
        let status = match kind {
            ProviderErrorKind::ReauthRequired => "reconnect_required",
            _ => "error",
        };
        conn.execute(
            "UPDATE connections SET status = ?2, sanitized_error = ?3, last_attempt_at = ?4, updated_at = ?4 WHERE id = ?1",
            rusqlite::params![connection.id, status, message, now_rfc3339()],
        )?;
        Ok(())
    })
}

pub fn sync_connection_with_client(
    state: &AppState,
    connection: &ConnectionRow,
    client: &dyn ProviderClient,
    trigger: &str,
) -> AppResult<ConnectionSyncResult> {
    let mut run = start_run(state, connection, trigger)?;
    let mut result = ConnectionSyncResult {
        connection_id: connection.id.clone(),
        institution: connection.institution_name.clone().unwrap_or_else(|| "Provider".into()),
        status: "success".into(),
        transactions_added: 0,
        transactions_modified: 0,
        transactions_removed: 0,
        balances_written: 0,
        liabilities_status: "skipped".into(),
        error_code: None,
    };
    let accounts = match client.accounts() {
        Ok(accounts) => accounts,
        Err(error) => {
            mark_connection_error(state, connection, error.kind, error.safe_message)?;
            finish_step(state, &mut run, "transactions", "failed", 0, 0, 0, Some(error.safe_message))?;
            finish_run(state, &run, connection, "failed", Some("provider_accounts"), &result, trigger)?;
            return Ok(ConnectionSyncResult { status: "failed".into(), error_code: Some("provider_accounts".into()), ..result });
        }
    };
    let account_map = state.with_db(|conn, _h, _s| ensure_accounts(conn, connection, &accounts))?;
    let capabilities = client.capabilities();

    if capabilities.transactions == Capability::Supported {
        let mut cursor = connection.cursor.clone();
        let mut page_count = 0usize;
        loop {
            page_count += 1;
            if page_count > MAX_PAGES {
                result.status = "partial".into();
                finish_step(state, &mut run, "transactions", "partial", result.transactions_added, result.transactions_modified, result.transactions_removed, Some("page limit"))?;
                break;
            }
            let page = match client.transactions_page(cursor.as_deref()) {
                Ok(page) => page,
                Err(error) => {
                    mark_connection_error(state, connection, error.kind, error.safe_message)?;
                    finish_step(state, &mut run, "transactions", "failed", result.transactions_added, result.transactions_modified, result.transactions_removed, Some(error.safe_message))?;
                    finish_run(state, &run, connection, "failed", Some("transactions"), &result, trigger)?;
                    return Ok(ConnectionSyncResult { status: "failed".into(), error_code: Some("transactions".into()), ..result });
                }
            };
            let page_counts = match state.with_db(|conn, _h, _s| {
                apply_page(conn, connection, &account_map, &page.changes, page.next_cursor.as_deref())
            }) {
                Ok(counts) => counts,
                Err(AppError::Locked) => {
                    // Cannot write the run/step while the database is locked;
                    // the next sync marks the running run interrupted.
                    return Err(AppError::Locked);
                }
                Err(error) => {
                    finish_step(state, &mut run, "transactions", "failed", result.transactions_added, result.transactions_modified, result.transactions_removed, Some("database write failed"))?;
                    finish_run(state, &run, connection, "failed", Some("database"), &result, trigger)?;
                    return Err(error);
                }
            };
            result.transactions_added += page_counts.0;
            result.transactions_modified += page_counts.1;
            result.transactions_removed += page_counts.2;
            if let Some(next) = page.next_cursor {
                cursor = Some(next);
            }
            if !page.has_more {
                break;
            }
        }
        finish_step(state, &mut run, "transactions", "success", result.transactions_added, result.transactions_modified, result.transactions_removed, None)?;
        state.with_db(|conn, _h, _s| crate::storage::edits::apply_rules(conn))?;
    } else {
        finish_step(state, &mut run, "transactions", "unavailable", 0, 0, 0, Some("unsupported"))?;
        result.status = "partial".into();
    }

    // Balances
    match client.balances() {
        Ok(balances) if capabilities.balances == Capability::Supported => {
            match state.with_db(|conn, _h, _s| apply_balances(conn, connection, &account_map, &balances)) {
                Ok(count) => {
                    result.balances_written = count;
                    finish_step(state, &mut run, "balances", "success", count, 0, 0, None)?;
                }
                Err(AppError::Locked) => return Err(AppError::Locked),
                Err(_) => {
                    finish_step(state, &mut run, "balances", "failed", 0, 0, 0, Some("database write failed"))?;
                    result.status = "partial".into();
                }
            }
        }
        Ok(_) => {
            finish_step(state, &mut run, "balances", "unavailable", 0, 0, 0, Some("unsupported"))?;
            result.status = "partial".into();
        }
        Err(error) => {
            mark_connection_error(state, connection, error.kind, error.safe_message)?;
            finish_step(state, &mut run, "balances", "failed", 0, 0, 0, Some(error.safe_message))?;
            result.status = "partial".into();
        }
    }

    // Liabilities: never delete previously reported values on a partial failure.
    match client.liabilities() {
        Ok(liabilities) => match liabilities.capability {
            Capability::Supported => match state.with_db(|conn, _h, _s| {
                apply_liabilities(conn, connection, &account_map, &liabilities.rows)
            }) {
                Ok(count) => {
                    result.liabilities_status = "success".into();
                    finish_step(state, &mut run, "liabilities", "success", count, 0, 0, None)?;
                }
                Err(AppError::Locked) => return Err(AppError::Locked),
                Err(_) => {
                    result.liabilities_status = "failed".into();
                    finish_step(state, &mut run, "liabilities", "failed", 0, 0, 0, Some("database write failed"))?;
                    result.status = "partial".into();
                }
            },
            other => {
                result.liabilities_status = match other {
                    Capability::AvailableButNull => "available_but_null",
                    Capability::TemporarilyUnavailable => "temporarily_unavailable",
                    _ => "unsupported",
                }
                .into();
                let step_status = if other == Capability::Unsupported { "unavailable" } else { "partial" };
                finish_step(state, &mut run, "liabilities", step_status, 0, 0, 0, Some(result.liabilities_status.as_str()))?;
                result.status = "partial".into();
            }
        },
        Err(error) => {
            result.liabilities_status = "unavailable".into();
            finish_step(state, &mut run, "liabilities", "failed", 0, 0, 0, Some(error.safe_message))?;
            result.status = "partial".into();
        }
    }

    finish_run(state, &run, connection, &result.status, None, &result, trigger)?;
    Ok(result)
}

pub fn sync_connection_by_id_with_client(
    state: &AppState,
    connection_id: &str,
    client: &dyn ProviderClient,
    trigger: &str,
) -> AppResult<ConnectionSyncResult> {
    let connection = connection_by_id(state, connection_id)?;
    sync_connection_with_client(state, &connection, client, trigger)
}

pub fn list_connections(state: &AppState) -> AppResult<Vec<ConnectionView>> {
    state.with_db(|conn, _h, _s| {
        let mut stmt = conn.prepare(
            "SELECT c.id, c.provider, c.environment, c.institution_name, c.status, c.last_attempt_at, \
                c.last_success_at, c.sanitized_error, c.cursor IS NOT NULL, \
                (SELECT count(*) FROM account_sources s WHERE s.connection_id = c.id AND s.unlinked_at IS NULL) \
             FROM connections c WHERE c.status <> 'disconnected' ORDER BY c.institution_name, c.id",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ConnectionView {
                    id: row.get(0)?,
                    provider: row.get(1)?,
                    environment: row.get(2)?,
                    institution_name: row.get(3)?,
                    status: row.get(4)?,
                    last_attempt_at: row.get(5)?,
                    last_success_at: row.get(6)?,
                    sanitized_error: row.get(7)?,
                    has_cursor: row.get::<_, i64>(8)? != 0,
                    account_count: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

pub fn recent_runs(state: &AppState, limit: u32) -> AppResult<Vec<SyncRunView>> {
    state.with_db(|conn, _h, _s| {
        let limit = limit.clamp(1, 50);
        let mut stmt = conn.prepare(
            "SELECT r.id, r.connection_id, c.institution_name, r.trigger, r.status, r.started_at, \
                r.finished_at, r.error_code \
             FROM sync_runs r JOIN connections c ON c.id = r.connection_id \
             ORDER BY r.started_at DESC, r.id DESC LIMIT ?1",
        )?;
        let base = stmt
            .query_map([i64::from(limit)], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut runs = Vec::new();
        for (id, connection_id, institution, trigger, status, started, finished, error) in base {
            let mut steps_stmt = conn.prepare(
                "SELECT stream, status, items_added, items_modified, items_removed, sanitized_error \
                 FROM sync_steps WHERE run_id = ?1 ORDER BY stream",
            )?;
            let steps = steps_stmt
                .query_map([&id], |row| {
                    Ok(SyncStepView {
                        stream: row.get(0)?,
                        status: row.get(1)?,
                        items_added: row.get(2)?,
                        items_modified: row.get(3)?,
                        items_removed: row.get(4)?,
                        sanitized_error: row.get(5)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            runs.push(SyncRunView {
                id,
                connection_id,
                institution_name: institution,
                trigger,
                status,
                started_at: started,
                finished_at: finished,
                error_code: error,
                steps,
            });
        }
        Ok(runs)
    })
}

pub fn runtime_status(state: &AppState) -> AppResult<SyncRuntimeStatus> {
    let mut runtime = state.sync_runtime.lock().map_err(|_| AppError::Internal)?;
    runtime.running = state.sync_gate.try_lock().is_err();
    Ok(runtime.clone())
}

pub fn connection_by_id(state: &AppState, id: &str) -> AppResult<ConnectionRow> {
    state.with_db(|conn, _h, _s| {
        conn.query_row(
            "SELECT id, provider, environment, institution_name, status, cursor, credential_ref \
             FROM connections WHERE id = ?1",
            [id],
            |row| {
                Ok(ConnectionRow {
                    id: row.get(0)?,
                    provider: row.get(1)?,
                    environment: row.get(2)?,
                    institution_name: row.get(3)?,
                    status: row.get(4)?,
                    cursor: row.get(5)?,
                    credential_ref: row.get(6)?,
                })
            },
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound,
            other => AppError::from(other),
        })
    })
}

pub fn disconnect(state: &AppState, connection_id: &str) -> AppResult<()> {
    let connection = connection_by_id(state, connection_id)?;
    if connection.provider == "plaid" {
        if let Ok(client) = provider_for(state, &connection) {
            // Best-effort remote revoke; local disconnect always proceeds.
            let _ = client.revoke();
        }
        let _ = state.credentials.delete(&crate::providers::plaid::access_token_account(
            &connection.credential_ref,
        ));
    }
    state.with_db(|conn, _h, _s| {
        conn.execute(
            "UPDATE connections SET status = 'disconnected', cursor = NULL, updated_at = ?2 WHERE id = ?1",
            rusqlite::params![connection_id, now_rfc3339()],
        )?;
        let _ = record_event(
            conn,
            "credential_removed",
            "success",
            Some(serde_json::json!({ "provider": connection.provider })),
        );
        Ok(())
    })
}

#[allow(dead_code)]
fn _unused(_: Instant, _: &LedgerQuery, _: &DateSource) {}
