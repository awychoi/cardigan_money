//! Rust-owned CSV import jobs: native file selection, bounded streaming parse,
//! duplicate review, transactionally committed batches, and rollback of unique
//! contributions only. The renderer receives an opaque job id, counts, preview
//! rows, and sanitized errors — never a filesystem path.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rusqlite::Connection;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use crate::domain::dates::format_date;
use crate::error::{AppError, AppResult};
use crate::providers::csv::{self, CsvMapping, MonthPreview, ParsedRow, PreviewRow};
use crate::state::AppState;
use crate::storage::time::now_rfc3339;

const JOB_TTL: Duration = Duration::from_secs(15 * 60);
const MAX_JOBS: usize = 4;

pub struct PendingImport {
    pub id: String,
    pub account_id: String,
    pub path: PathBuf,
    pub sha256: String,
    pub size: u64,
    pub headers: Vec<String>,
    pub total_rows: usize,
    pub created: Instant,
}

pub type ImportJobs = Mutex<HashMap<String, PendingImport>>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFileView {
    pub job_id: String,
    pub headers: Vec<String>,
    pub size_bytes: u64,
    pub row_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationPreview {
    pub job_id: String,
    pub row_count: usize,
    pub insert_count: usize,
    pub duplicate_count: usize,
    pub review_count: usize,
    pub rejected_count: usize,
    pub sample_rows: Vec<PreviewRow>,
    pub months: Vec<MonthPreview>,
    pub review_months: Vec<MonthPreview>,
    pub no_op: bool,
    pub existing_batch_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub batch_id: String,
    pub inserted: usize,
    pub duplicates: usize,
    pub review: usize,
    pub rejected: usize,
    pub months: Vec<MonthPreview>,
    pub no_op: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportBatchView {
    pub id: String,
    pub account_id: String,
    pub account_name: String,
    pub file_sha256: String,
    pub row_count: i64,
    pub inserted_count: i64,
    pub duplicate_count: i64,
    pub review_count: i64,
    pub rejected_count: i64,
    pub status: String,
    pub created_at: String,
    pub imported_at: Option<String>,
    pub rolled_back_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowClass {
    Insert,
    Duplicate,
    Review,
}

struct Classified {
    row: ParsedRow,
    class: RowClass,
}

fn prune_jobs(jobs: &mut HashMap<String, PendingImport>) {
    let now = Instant::now();
    jobs.retain(|_, job| now.duration_since(job.created) < JOB_TTL);
    if jobs.len() >= MAX_JOBS {
        if let Some(oldest) = jobs
            .iter()
            .min_by_key(|(_, job)| job.created)
            .map(|(id, _)| id.clone())
        {
            jobs.remove(&oldest);
        }
    }
}

pub fn store_job(
    state: &AppState,
    account_id: &str,
    path: &Path,
    info: csv::FileInfo,
) -> AppResult<ImportFileView> {
    // Account must exist; demo mode never imports real data.
    state.require_real("import CSV in demo mode")?;
    let account_currency: String = state.with_db(|conn, _header, _settings| {
        conn.query_row(
            "SELECT currency FROM accounts WHERE id = ?1 AND archived = 0",
            [account_id],
            |row| row.get(0),
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound,
            other => AppError::from(other),
        })
    })?;
    let _ = account_currency;
    let id = uuid::Uuid::new_v4().to_string();
    let job = PendingImport {
        id: id.clone(),
        account_id: account_id.to_string(),
        path: path.to_path_buf(),
        sha256: info.sha256,
        size: info.size,
        headers: info.headers.clone(),
        total_rows: info.total_rows,
        created: Instant::now(),
    };
    {
        let mut jobs = state.imports.lock().map_err(|_| AppError::Internal)?;
        prune_jobs(&mut jobs);
        jobs.insert(id.clone(), job);
    }
    Ok(ImportFileView {
        job_id: id,
        headers: info.headers,
        size_bytes: info.size,
        row_count: info.total_rows,
    })
}

fn get_job(state: &AppState, job_id: &str) -> AppResult<(String, PathBuf, String, String, usize)> {
    let jobs = state.imports.lock().map_err(|_| AppError::Internal)?;
    let job = jobs.get(job_id).ok_or(AppError::NotFound)?;
    if Instant::now().duration_since(job.created) > JOB_TTL {
        return Err(AppError::Blocked("import preview expired"));
    }
    Ok((
        job.account_id.clone(),
        job.path.clone(),
        job.sha256.clone(),
        job.id.clone(),
        job.total_rows,
    ))
}

fn verify_hash(path: &Path, expected: &str) -> AppResult<()> {
    let bytes = csv::read_bounded(path)?;
    let hash = hex::encode(Sha256::digest(&bytes));
    if hash != expected {
        return Err(AppError::Blocked("import file changed since preview"));
    }
    Ok(())
}

fn existing_exact_batch(conn: &Connection, account_id: &str, sha: &str) -> AppResult<Option<String>> {
    conn.query_row(
        "SELECT id FROM import_batches WHERE account_id = ?1 AND file_sha256 = ?2 AND status = 'imported' LIMIT 1",
        rusqlite::params![account_id, sha],
        |row| row.get::<_, String>(0),
    )
    .map(Some)
    .or_else(|error| match error {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(AppError::from(other)),
    })
}

fn classify(conn: &Connection, account_id: &str, parsed: &csv::ParsedFile) -> AppResult<Vec<Classified>> {
    let mut stmt = conn.prepare(
        "SELECT source_row_ref, import_fingerprint FROM transactions WHERE account_id = ?1",
    )?;
    let mut rows = stmt.query([account_id])?;
    let mut source_ids = HashSet::new();
    let mut fingerprints = HashSet::new();
    while let Some(row) = rows.next()? {
        if let Ok(Some(value)) = row.get::<_, Option<String>>(0) {
            source_ids.insert(value);
        }
        if let Ok(Some(value)) = row.get::<_, Option<String>>(1) {
            fingerprints.insert(value);
        }
    }
    let mut seen_sources = HashSet::new();
    let mut seen_fingerprints = HashSet::new();
    let mut out = Vec::with_capacity(parsed.rows.len());
    for row in &parsed.rows {
        let class = if let Some(source) = &row.source_id {
            if source_ids.contains(source) || seen_sources.contains(source) {
                RowClass::Duplicate
            } else if fingerprints.contains(&row.fingerprint) || seen_fingerprints.contains(&row.fingerprint) {
                RowClass::Review
            } else {
                RowClass::Insert
            }
        } else if fingerprints.contains(&row.fingerprint) || seen_fingerprints.contains(&row.fingerprint) {
            RowClass::Review
        } else {
            RowClass::Insert
        };
        if let Some(source) = &row.source_id {
            seen_sources.insert(source.clone());
        }
        seen_fingerprints.insert(row.fingerprint.clone());
        out.push(Classified { row: row.clone(), class });
    }
    Ok(out)
}

pub fn validate(
    state: &AppState,
    job_id: &str,
    mapping: &CsvMapping,
) -> AppResult<ValidationPreview> {
    state.require_real("import CSV in demo mode")?;
    let (account_id, path, sha, _, _) = get_job(state, job_id)?;
    verify_hash(&path, &sha)?;
    let currency = state.with_db(|conn, _h, _s| {
        conn.query_row("SELECT currency FROM accounts WHERE id = ?1", [&account_id], |row| row.get::<_, String>(0))
            .map_err(|_| AppError::NotFound)
    })?;
    let parsed = csv::parse(&path, mapping, &account_id, &currency)?;
    let classified = state.with_db(|conn, _h, _s| classify(conn, &account_id, &parsed))?;
    let insert: Vec<&ParsedRow> = classified
        .iter()
        .filter(|item| item.class == RowClass::Insert)
        .map(|item| &item.row)
        .collect();
    let review: Vec<&ParsedRow> = classified
        .iter()
        .filter(|item| item.class == RowClass::Review)
        .map(|item| &item.row)
        .collect();
    let insert_rows: Vec<ParsedRow> = insert.iter().map(|row| (*row).clone()).collect();
    let review_rows: Vec<ParsedRow> = review.iter().map(|row| (*row).clone()).collect();
    let mut sample_rows: Vec<PreviewRow> = Vec::new();
    for item in classified.iter().take(csv::SAMPLE_ROWS) {
        let (status, reason) = match item.class {
            RowClass::Insert => ("insert", None),
            RowClass::Duplicate => ("duplicate", Some("source id already imported")),
            RowClass::Review => ("review", Some("possible duplicate; review before importing")),
        };
        sample_rows.push(csv::preview_row(&item.row, status, reason));
    }
    for rejected in parsed.rejected.iter().take(csv::SAMPLE_ROWS.saturating_sub(sample_rows.len())) {
        sample_rows.push(PreviewRow {
            row_number: rejected.row_number,
            date: String::new(),
            merchant: String::new(),
            amount_minor: 0,
            category: None,
            status: "rejected".into(),
            reason: Some(rejected.reason.into()),
        });
    }
    let no_op_batch = state.with_db(|conn, _h, _s| existing_exact_batch(conn, &account_id, &sha))?;
    let no_op = no_op_batch.is_some();
    Ok(ValidationPreview {
        job_id: job_id.to_string(),
        row_count: parsed.total_rows,
        insert_count: if no_op { 0 } else { insert.len() },
        duplicate_count: if no_op { parsed.total_rows } else { parsed.rows.len() - insert.len() - review.len() },
        review_count: if no_op { 0 } else { review.len() },
        rejected_count: parsed.rejected.len(),
        sample_rows,
        months: csv::month_totals(&insert_rows),
        review_months: csv::month_totals(&review_rows),
        no_op,
        existing_batch_id: no_op_batch,
    })
}

pub fn commit(
    state: &AppState,
    job_id: &str,
    mapping: &CsvMapping,
    accept_review: bool,
) -> AppResult<ImportResult> {
    state.require_real("import CSV in demo mode")?;
    let (account_id, path, sha, _, _) = get_job(state, job_id)?;
    verify_hash(&path, &sha)?;
    let currency = state.with_db(|conn, _h, _s| {
        conn.query_row("SELECT currency FROM accounts WHERE id = ?1", [&account_id], |row| row.get::<_, String>(0))
            .map_err(|_| AppError::NotFound)
    })?;
    let parsed = csv::parse(&path, mapping, &account_id, &currency)?;
    verify_hash(&path, &sha)?;
    let batch_id = state.with_db(|conn, _h, _s| {
        if let Some(existing) = existing_exact_batch(conn, &account_id, &sha)? {
            return Ok((existing, true));
        }
        let classified = classify(conn, &account_id, &parsed)?;
        let insert_count = classified.iter().filter(|item| item.class == RowClass::Insert).count();
        let duplicate_count = classified.iter().filter(|item| item.class == RowClass::Duplicate).count();
        let review_count = classified.iter().filter(|item| item.class == RowClass::Review).count();
        let inserted_total = insert_count + if accept_review { review_count } else { 0 };
        let batch = uuid::Uuid::new_v4().to_string();
        let now = now_rfc3339();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO import_batches (id, account_id, file_sha256, file_size_bytes, mapping_json, \
                row_count, inserted_count, duplicate_count, review_count, rejected_count, status, created_at, imported_at, rolled_back_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'imported', ?11, ?11, NULL)",
            rusqlite::params![
                batch,
                account_id,
                sha,
                parsed.total_rows as i64,
                serde_json::to_string(mapping).map_err(|_| AppError::Internal)?,
                parsed.total_rows as i64,
                inserted_total as i64,
                duplicate_count as i64,
                review_count as i64,
                parsed.rejected.len() as i64,
                now,
            ],
        )?;
        let mut inserted = 0usize;
        for item in &classified {
            let will_insert = item.class == RowClass::Insert
                || (accept_review && item.class == RowClass::Review);
            let status = match item.class {
                RowClass::Insert => "inserted",
                RowClass::Duplicate => "duplicate",
                RowClass::Review => {
                    if accept_review {
                        "inserted"
                    } else {
                        "review"
                    }
                }
            };
            let mut transaction_id: Option<String> = None;
            if will_insert {
                let id = uuid::Uuid::new_v4().to_string();
                let merchant_display = item.row.raw_description.chars().take(200).collect::<String>();
                tx.execute(
                    "INSERT INTO transactions (id, account_id, source_kind, provider_transaction_id, \
                        import_batch_id, source_row_ref, pending, authorized_date, posted_date, reporting_date, \
                        reporting_date_source, amount_minor, currency, raw_description, merchant_normalized, \
                        merchant_display, category, tx_type, excluded, removed, import_fingerprint, created_at, updated_at) \
                     VALUES (?1, ?2, 'csv', NULL, ?3, ?4, 0, NULL, ?5, ?5, 'posted', ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, 0, ?13, ?14, ?14)",
                    rusqlite::params![
                        id,
                        account_id,
                        batch,
                        item.row.source_id,
                        format_date(item.row.date),
                        item.row.amount_minor,
                        currency,
                        item.row.raw_description,
                        item.row.merchant_normalized,
                        merchant_display,
                        item.row.category,
                        item.row.tx_type,
                        item.row.fingerprint,
                        now,
                    ],
                )?;
                transaction_id = Some(id);
                inserted += 1;
            }
            let row_hash = item.row.fingerprint.clone();
            tx.execute(
                "INSERT INTO import_rows (id, batch_id, transaction_id, row_number, source_row_hash, fingerprint, status, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    batch,
                    transaction_id,
                    item.row.row_number as i64,
                    row_hash,
                    item.row.fingerprint,
                    status,
                    now,
                ],
            )?;
        }
        for rejected in &parsed.rejected {
            let synthetic = hex::encode(Sha256::digest(
                format!("{batch}|{}", rejected.row_number).as_bytes(),
            ));
            tx.execute(
                "INSERT INTO import_rows (id, batch_id, transaction_id, row_number, source_row_hash, fingerprint, status, created_at) \
                 VALUES (?1, ?2, NULL, ?3, ?4, NULL, 'rejected', ?5)",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    batch,
                    rejected.row_number as i64,
                    synthetic,
                    now
                ],
            )?;
        }
        tx.execute(
            "UPDATE import_batches SET inserted_count = ?2, duplicate_count = ?3, review_count = ?4 WHERE id = ?1",
            rusqlite::params![
                batch,
                inserted as i64,
                duplicate_count as i64,
                if accept_review { 0i64 } else { review_count as i64 }
            ],
        )?;
        tx.commit()?;
        Ok((batch, false))
    })?;
    let (batch_id, no_op) = batch_id;
    if !no_op {
        state.with_db(|conn, _h, _s| crate::storage::edits::apply_rules(conn))?;
    }
    let view = state.with_db(|conn, _h, _s| get_batch(conn, &batch_id))?;
    let classified = state.with_db(|conn, _h, _s| classify(conn, &account_id, &parsed))?;
    let inserted_rows: Vec<ParsedRow> = classified
        .iter()
        .filter(|item| {
            item.class == RowClass::Insert || (accept_review && item.class == RowClass::Review)
        })
        .map(|item| item.row.clone())
        .collect();
    if !no_op {
        let mut jobs = state.imports.lock().map_err(|_| AppError::Internal)?;
        jobs.remove(job_id);
    }
    Ok(ImportResult {
        batch_id,
        inserted: view.inserted_count as usize,
        duplicates: view.duplicate_count as usize,
        review: view.review_count as usize,
        rejected: view.rejected_count as usize,
        months: if no_op { Vec::new() } else { csv::month_totals(&inserted_rows) },
        no_op,
    })
}

pub fn cancel(state: &AppState, job_id: &str) -> AppResult<()> {
    let mut jobs = state.imports.lock().map_err(|_| AppError::Internal)?;
    if jobs.remove(job_id).is_none() {
        return Err(AppError::NotFound);
    }
    Ok(())
}

fn get_batch(conn: &Connection, batch_id: &str) -> AppResult<ImportBatchView> {
    conn.query_row(
        "SELECT b.id, b.account_id, a.display_name, b.file_sha256, b.row_count, b.inserted_count, \
            b.duplicate_count, b.review_count, b.rejected_count, b.status, b.created_at, b.imported_at, b.rolled_back_at \
         FROM import_batches b JOIN accounts a ON a.id = b.account_id WHERE b.id = ?1",
        [batch_id],
        |row| {
            Ok(ImportBatchView {
                id: row.get(0)?,
                account_id: row.get(1)?,
                account_name: row.get(2)?,
                file_sha256: row.get(3)?,
                row_count: row.get(4)?,
                inserted_count: row.get(5)?,
                duplicate_count: row.get(6)?,
                review_count: row.get(7)?,
                rejected_count: row.get(8)?,
                status: row.get(9)?,
                created_at: row.get(10)?,
                imported_at: row.get(11)?,
                rolled_back_at: row.get(12)?,
            })
        },
    )
    .map_err(|error| match error {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound,
        other => AppError::from(other),
    })
}

pub fn list_batches(conn: &Connection, limit: u32) -> AppResult<Vec<ImportBatchView>> {
    let limit = limit.clamp(1, 50);
    let mut stmt = conn.prepare(
        "SELECT b.id, b.account_id, a.display_name, b.file_sha256, b.row_count, b.inserted_count, \
            b.duplicate_count, b.review_count, b.rejected_count, b.status, b.created_at, b.imported_at, b.rolled_back_at \
         FROM import_batches b JOIN accounts a ON a.id = b.account_id \
         ORDER BY b.created_at DESC, b.id DESC LIMIT ?1",
    )?;
    let rows = stmt
        .query_map([i64::from(limit)], |row| {
            Ok(ImportBatchView {
                id: row.get(0)?,
                account_id: row.get(1)?,
                account_name: row.get(2)?,
                file_sha256: row.get(3)?,
                row_count: row.get(4)?,
                inserted_count: row.get(5)?,
                duplicate_count: row.get(6)?,
                review_count: row.get(7)?,
                rejected_count: row.get(8)?,
                status: row.get(9)?,
                created_at: row.get(10)?,
                imported_at: row.get(11)?,
                rolled_back_at: row.get(12)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Remove only the rows uniquely contributed by this batch.
pub fn rollback(state: &AppState, batch_id: &str) -> AppResult<ImportBatchView> {
    state.require_real("roll back an import in demo mode")?;
    state.with_db(|conn, _h, _s| {
        let status: String = conn
            .query_row("SELECT status FROM import_batches WHERE id = ?1", [batch_id], |row| row.get(0))
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => AppError::NotFound,
                other => AppError::from(other),
            })?;
        if status != "imported" {
            return Err(AppError::Blocked("batch is not an imported batch"));
        }
        let tx = conn.unchecked_transaction()?;
        let mut stmt = tx.prepare(
            "SELECT ir.transaction_id FROM import_rows ir \
             WHERE ir.batch_id = ?1 AND ir.transaction_id IS NOT NULL",
        )?;
        let ids = stmt
            .query_map([batch_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        let mut removed = 0usize;
        for id in ids {
            let others: i64 = tx.query_row(
                "SELECT count(*) FROM import_rows WHERE transaction_id = ?1 AND batch_id <> ?2",
                rusqlite::params![id, batch_id],
                |row| row.get(0),
            )?;
            if others == 0 {
                tx.execute("DELETE FROM transactions WHERE id = ?1", [&id])?;
                removed += 1;
            }
        }
        let now = now_rfc3339();
        tx.execute(
            "UPDATE import_rows SET status = 'rolled_back', transaction_id = NULL WHERE batch_id = ?1",
            [batch_id],
        )?;
        tx.execute(
            "UPDATE import_batches SET status = 'rolled_back', rolled_back_at = ?2 WHERE id = ?1",
            rusqlite::params![batch_id, now],
        )?;
        tx.commit()?;
        let _ = removed;
        get_batch(conn, batch_id)
    })
}

pub fn choose_file<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    account_id: &str,
) -> AppResult<ImportFileView> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let picked = rfd::FileDialog::new()
            .add_filter("CSV", &["csv"])
            .set_title("Choose a CSV file to import")
            .pick_file();
        let _ = tx.send(picked);
    })
    .map_err(|_| AppError::NativeInput)?;
    let picked = rx
        .recv_timeout(Duration::from_secs(300))
        .map_err(|_| AppError::NativeInput)?;
    let Some(path) = picked else {
        return Err(AppError::Cancelled);
    };
    let info = csv::file_info(&path)?;
    store_job(state, account_id, &path, info)
}
