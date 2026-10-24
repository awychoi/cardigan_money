//! Plaintext CSV export of the filtered ledger. Formula-injection escaping,
//! owner-only file permissions, atomic replace, and no auto-open.

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;
use tauri::AppHandle;

use crate::error::{AppError, AppResult};
use crate::storage::ledger::{query_ledger_export, LedgerQuery};
use crate::storage::time::now_rfc3339;

pub const MAX_EXPORT_ROWS: u32 = 100_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub rows: usize,
    pub written: bool,
}

fn escape_field(value: &str) -> String {
    let trimmed = value.trim_start();
    if trimmed.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_string()
    }
}

fn amount_text(minor: i64) -> String {
    let sign = if minor < 0 { "-" } else { "" };
    let abs = minor.unsigned_abs();
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
}

fn write_atomic_0600(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let parent = path.parent().ok_or(AppError::Io)?;
    if !parent.is_dir() {
        return Err(AppError::Io);
    }
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err(AppError::PermissionDenied("symlink export destination"));
        }
        if !meta.is_file() {
            return Err(AppError::PermissionDenied("export destination type"));
        }
    }
    let suffix = uuid::Uuid::new_v4().to_string();
    let temp = parent.join(format!(".cm-export-{suffix}.tmp"));
    let result = (|| -> AppResult<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)?;
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

pub fn export_filtered(
    conn: &Connection,
    query: &LedgerQuery,
    destination: &Path,
) -> AppResult<ExportResult> {
    let rows = query_ledger_export(conn, query, MAX_EXPORT_ROWS)?;
    let mut writer = csv::WriterBuilder::new().from_writer(Vec::new());
    writer
        .write_record([
            "reporting_date",
            "merchant",
            "raw_description",
            "card",
            "last_four",
            "category",
            "type",
            "status",
            "amount",
            "currency",
            "notes",
        ])
        .map_err(|_| AppError::Internal)?;
    for row in &rows {
        let status = if row.excluded {
            "excluded"
        } else if row.pending {
            "pending"
        } else {
            "posted"
        };
        writer
            .write_record([
                row.reporting_date.clone(),
                escape_field(&row.merchant),
                escape_field(&row.raw_description),
                escape_field(&row.account_name),
                row.account_last_four.clone().unwrap_or_default(),
                escape_field(row.category.as_deref().unwrap_or("Uncategorized")),
                row.tx_type.clone(),
                status.to_string(),
                amount_text(row.amount_minor),
                row.currency.clone(),
                escape_field(row.notes.as_deref().unwrap_or("")),
            ])
            .map_err(|_| AppError::Internal)?;
    }
    let bytes = writer.into_inner().map_err(|_| AppError::Internal)?;
    write_atomic_0600(destination, &bytes)?;
    Ok(ExportResult { rows: rows.len(), written: true })
}

pub fn choose_destination<R: tauri::Runtime>(app: &AppHandle<R>) -> AppResult<Option<std::path::PathBuf>> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let picked = rfd::FileDialog::new()
            .add_filter("CSV", &["csv"])
            .set_file_name(format!("cardigan-money-ledger-{date}.csv"))
            .set_title("Export filtered ledger as plaintext CSV")
            .save_file();
        let _ = tx.send(picked);
    })
    .map_err(|_| AppError::NativeInput)?;
    rx.recv_timeout(std::time::Duration::from_secs(300))
        .map_err(|_| AppError::NativeInput)
}

pub fn default_filename() -> String {
    format!("cardigan-money-ledger-{}.csv", now_rfc3339().get(0..10).unwrap_or("export"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formula_injection_is_escaped_after_leading_whitespace() {
        for value in ["=cmd", " +sum", "-1+2", "@foo", "\t=1", "\r=1", "   =hidden"] {
            assert!(escape_field(value).starts_with('\''), "not escaped: {value:?}");
        }
        assert_eq!(escape_field("Coffee"), "Coffee");
        assert_eq!(escape_field("2026-10-01"), "2026-10-01");
    }

    #[test]
    fn amount_format_has_no_float_rounding() {
        assert_eq!(amount_text(1234), "12.34");
        assert_eq!(amount_text(-5), "-0.05");
        assert_eq!(amount_text(0), "0.00");
    }
}
