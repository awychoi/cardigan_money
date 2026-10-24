//! Bounded, streaming CSV parsing for the generic importer. Every field is
//! untrusted text. The renderer never receives a file path.

use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::domain::dates::{format_date, parse_date, Month};
use crate::domain::money::parse_currency_amount;
use crate::domain::text::{clean_text, display_safe, normalize_merchant, MAX_CATEGORY_CHARS};
use crate::error::{AppError, AppResult};

pub const MAX_FILE_BYTES: u64 = 20 * 1024 * 1024;
pub const MAX_ROWS: usize = 100_000;
pub const MAX_COLUMNS: usize = 40;
pub const MAX_FIELD_CHARS: usize = 4096;
pub const MAX_PARSE_TIME: Duration = Duration::from_secs(30);
pub const SAMPLE_ROWS: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DateFormat {
    Iso,
    Us,
    Eu,
}

impl DateFormat {
    pub fn parse(value: &str) -> AppResult<Self> {
        match value {
            "iso" => Ok(Self::Iso),
            "us" => Ok(Self::Us),
            "eu" => Ok(Self::Eu),
            _ => Err(AppError::InvalidInput("date format")),
        }
    }

    fn parse_date(self, value: &str) -> Option<NaiveDate> {
        match self {
            Self::Iso => parse_date(value).ok(),
            Self::Us => NaiveDate::parse_from_str(value, "%m/%d/%Y").ok(),
            Self::Eu => NaiveDate::parse_from_str(value, "%d/%m/%Y").ok(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignConvention {
    SignedAmount,
    DebitCredit,
}

impl SignConvention {
    pub fn parse(value: &str) -> AppResult<Self> {
        match value {
            "signed_amount" => Ok(Self::SignedAmount),
            "debit_credit" => Ok(Self::DebitCredit),
            _ => Err(AppError::InvalidInput("sign convention")),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CsvMapping {
    pub date_column: usize,
    pub description_column: usize,
    pub amount_column: Option<usize>,
    pub debit_column: Option<usize>,
    pub credit_column: Option<usize>,
    pub category_column: Option<usize>,
    pub source_id_column: Option<usize>,
    pub date_format: String,
    pub sign_convention: String,
    pub credit_type: String,
}

impl CsvMapping {
    pub fn validated(&self, column_count: usize) -> AppResult<(DateFormat, SignConvention)> {
        let max = [
            Some(self.date_column),
            Some(self.description_column),
            self.amount_column,
            self.debit_column,
            self.credit_column,
            self.category_column,
            self.source_id_column,
        ]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(0);
        if max >= column_count {
            return Err(AppError::InvalidInput("mapping column"));
        }
        let date_format = DateFormat::parse(&self.date_format)?;
        let sign = SignConvention::parse(&self.sign_convention)?;
        match sign {
            SignConvention::SignedAmount => {
                if self.amount_column.is_none() {
                    return Err(AppError::InvalidInput("amount column required"));
                }
            }
            SignConvention::DebitCredit => {
                if self.debit_column.is_none() && self.credit_column.is_none() {
                    return Err(AppError::InvalidInput("debit/credit columns required"));
                }
            }
        }
        if !matches!(self.credit_type.as_str(), "refund" | "payment" | "other") {
            return Err(AppError::InvalidInput("credit type"));
        }
        Ok((date_format, sign))
    }
}

#[derive(Debug, Clone)]
pub struct ParsedRow {
    pub row_number: usize,
    pub date: NaiveDate,
    pub amount_minor: i64,
    pub tx_type: String,
    pub raw_description: String,
    pub merchant_normalized: String,
    pub category: Option<String>,
    pub source_id: Option<String>,
    pub fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct RejectedRow {
    pub row_number: usize,
    pub reason: &'static str,
}

pub struct ParsedFile {
    pub rows: Vec<ParsedRow>,
    pub rejected: Vec<RejectedRow>,
    pub total_rows: usize,
}

pub struct FileInfo {
    pub sha256: String,
    pub size: u64,
    pub headers: Vec<String>,
    pub total_rows: usize,
}

pub fn read_bounded(path: &Path) -> AppResult<Vec<u8>> {
    let meta = std::fs::symlink_metadata(path).map_err(|_| AppError::Io)?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(AppError::PermissionDenied("import file type"));
    }
    if meta.len() == 0 || meta.len() > MAX_FILE_BYTES {
        return Err(AppError::Blocked("file size is outside the import limit"));
    }
    let file = std::fs::File::open(path).map_err(|_| AppError::Io)?;
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AppError::Io)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(AppError::Blocked("file size is outside the import limit"));
    }
    // Strip a UTF-8 BOM before validation.
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        bytes.drain(0..3);
    }
    if bytes.contains(&0) {
        return Err(AppError::InvalidInput("file contains NUL bytes"));
    }
    std::str::from_utf8(&bytes).map_err(|_| AppError::InvalidInput("file is not valid UTF-8"))?;
    Ok(bytes)
}

pub fn file_info(path: &Path) -> AppResult<FileInfo> {
    let bytes = read_bounded(path)?;
    let sha256 = hex::encode(Sha256::digest(&bytes));
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes.as_slice());
    let headers = reader
        .headers()
        .map_err(|_| AppError::InvalidInput("CSV header is malformed"))?
        .iter()
        .map(|value| display_safe(value, 80))
        .collect::<Vec<_>>();
    if headers.is_empty() || headers.len() > MAX_COLUMNS {
        return Err(AppError::InvalidInput("CSV column count"));
    }
    let mut total_rows = 0usize;
    for record in reader.records() {
        let record = record.map_err(|_| AppError::InvalidInput("CSV row is malformed"))?;
        if record.len() > MAX_COLUMNS {
            return Err(AppError::InvalidInput("CSV column count"));
        }
        total_rows += 1;
        if total_rows > MAX_ROWS {
            return Err(AppError::Blocked("CSV row limit exceeded"));
        }
    }
    Ok(FileInfo {
        sha256,
        size: bytes.len() as u64,
        headers,
        total_rows,
    })
}

pub fn parse(
    path: &Path,
    mapping: &CsvMapping,
    account_id: &str,
    currency: &str,
) -> AppResult<ParsedFile> {
    let bytes = read_bounded(path)?;
    let (date_format, sign) = mapping.validated(MAX_COLUMNS)?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(false)
        .from_reader(bytes.as_slice());
    let headers = reader
        .headers()
        .map_err(|_| AppError::InvalidInput("CSV header is malformed"))?
        .len();
    let (_date_format, _sign) = mapping.validated(headers)?;
    let started = Instant::now();
    let mut rows = Vec::new();
    let mut rejected = Vec::new();
    let mut total_rows = 0usize;
    for (index, record) in reader.records().enumerate() {
        total_rows += 1;
        if total_rows > MAX_ROWS {
            return Err(AppError::Blocked("CSV row limit exceeded"));
        }
        if index % 512 == 0 && started.elapsed() > MAX_PARSE_TIME {
            return Err(AppError::Blocked("CSV parse time limit exceeded"));
        }
        let record = match record {
            Ok(record) => record,
            Err(_) => {
                rejected.push(RejectedRow {
                    row_number: total_rows,
                    reason: "malformed row",
                });
                continue;
            }
        };
        if record.len() > MAX_COLUMNS
            || record
                .iter()
                .any(|field| field.chars().count() > MAX_FIELD_CHARS)
        {
            rejected.push(RejectedRow {
                row_number: total_rows,
                reason: "field limit",
            });
            continue;
        }
        match parse_row(
            total_rows,
            &record,
            mapping,
            date_format,
            sign,
            account_id,
            currency,
        ) {
            Ok(row) => rows.push(row),
            Err(reason) => rejected.push(RejectedRow {
                row_number: total_rows,
                reason,
            }),
        }
    }
    Ok(ParsedFile {
        rows,
        rejected,
        total_rows,
    })
}

fn field(record: &csv::StringRecord, index: usize) -> &str {
    record.get(index).unwrap_or("")
}

fn parse_row(
    row_number: usize,
    record: &csv::StringRecord,
    mapping: &CsvMapping,
    date_format: DateFormat,
    sign: SignConvention,
    account_id: &str,
    currency: &str,
) -> Result<ParsedRow, &'static str> {
    let date_text = field(record, mapping.date_column).trim();
    let date = date_format.parse_date(date_text).ok_or("invalid date")?;
    let description = clean_text(
        field(record, mapping.description_column),
        crate::domain::text::MAX_DESCRIPTION_CHARS,
        false,
    )
    .map_err(|_| "invalid description")?;
    let merchant_normalized = normalize_merchant(&description).map_err(|_| "invalid merchant")?;
    let (amount_minor, tx_type) = match sign {
        SignConvention::SignedAmount => {
            let raw = field(
                record,
                mapping.amount_column.ok_or("missing amount column")?,
            )
            .trim();
            let amount = parse_currency_amount(raw, currency).map_err(|_| "invalid amount")?;
            if amount < 0 {
                (amount, mapping.credit_type.clone())
            } else {
                (amount, "purchase".to_string())
            }
        }
        SignConvention::DebitCredit => {
            let debit = mapping
                .debit_column
                .map(|column| field(record, column).trim())
                .filter(|value| !value.is_empty());
            let credit = mapping
                .credit_column
                .map(|column| field(record, column).trim())
                .filter(|value| !value.is_empty());
            match (debit, credit) {
                (Some(debit), _) => {
                    let amount =
                        parse_currency_amount(debit, currency).map_err(|_| "invalid debit")?;
                    if amount < 0 {
                        return Err("negative debit");
                    }
                    (amount, "purchase".to_string())
                }
                (None, Some(credit)) => {
                    let amount =
                        parse_currency_amount(credit, currency).map_err(|_| "invalid credit")?;
                    if amount < 0 {
                        return Err("negative credit");
                    }
                    (-amount, mapping.credit_type.clone())
                }
                (None, None) => return Err("missing amount"),
            }
        }
    };
    let category = mapping.category_column.and_then(|column| {
        let value = field(record, column).trim();
        if value.is_empty() {
            None
        } else {
            clean_text(value, MAX_CATEGORY_CHARS, false).ok()
        }
    });
    let source_id = mapping.source_id_column.and_then(|column| {
        let value = field(record, column).trim();
        if value.is_empty() {
            None
        } else {
            clean_text(value, 128, false).ok()
        }
    });
    let fingerprint = fingerprint(
        account_id,
        &format_date(date),
        amount_minor,
        &merchant_normalized,
        source_id.as_deref(),
    );
    Ok(ParsedRow {
        row_number,
        date,
        amount_minor,
        tx_type,
        raw_description: description,
        merchant_normalized,
        category,
        source_id,
        fingerprint,
    })
}

pub fn fingerprint(
    account_id: &str,
    date: &str,
    amount_minor: i64,
    merchant: &str,
    source_id: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(account_id.as_bytes());
    hasher.update(b"|");
    hasher.update(date.as_bytes());
    hasher.update(b"|");
    hasher.update(amount_minor.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(merchant.as_bytes());
    hasher.update(b"|");
    hasher.update(source_id.unwrap_or("").as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MonthPreview {
    pub month: String,
    pub purchases_minor: i64,
    pub refunds_minor: i64,
    pub net_minor: i64,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRow {
    pub row_number: usize,
    pub date: String,
    pub merchant: String,
    pub amount_minor: i64,
    pub category: Option<String>,
    pub status: String,
    pub reason: Option<String>,
}

pub fn month_totals(rows: &[ParsedRow]) -> Vec<MonthPreview> {
    let mut map: std::collections::BTreeMap<String, MonthPreview> =
        std::collections::BTreeMap::new();
    for row in rows {
        let month = Month::from_date(row.date).key();
        let entry = map.entry(month.clone()).or_insert_with(|| MonthPreview {
            month,
            purchases_minor: 0,
            refunds_minor: 0,
            net_minor: 0,
            count: 0,
        });
        if row.amount_minor > 0 {
            entry.purchases_minor += row.amount_minor;
        } else {
            entry.refunds_minor += row.amount_minor;
        }
        entry.net_minor += row.amount_minor;
        entry.count += 1;
    }
    map.into_values().collect()
}

pub fn preview_row(row: &ParsedRow, status: &str, reason: Option<&str>) -> PreviewRow {
    PreviewRow {
        row_number: row.row_number,
        date: format_date(row.date),
        merchant: display_safe(&row.raw_description, 120),
        amount_minor: row.amount_minor,
        category: row.category.clone(),
        status: status.to_string(),
        reason: reason.map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_csv(content: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.csv");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        (dir, path)
    }

    #[test]
    fn parses_bom_quotes_parentheses_and_debit_credit() {
        let (_dir, path) = temp_csv(
            "\u{feff}Date,Description,Amount\n2026-10-01,\"Coffee, Inc\",(12.34)\n2026-10-02,Refund,-5.00\n",
        );
        let mapping = CsvMapping {
            date_column: 0,
            description_column: 1,
            amount_column: Some(2),
            debit_column: None,
            credit_column: None,
            category_column: None,
            source_id_column: None,
            date_format: "iso".into(),
            sign_convention: "signed_amount".into(),
            credit_type: "refund".into(),
        };
        let parsed = parse(&path, &mapping, "acct", "USD").unwrap();
        assert_eq!(parsed.rows.len(), 2);
        assert_eq!(parsed.rows[0].amount_minor, -1234);
        assert_eq!(parsed.rows[0].tx_type, "refund");
        assert_eq!(parsed.rows[0].raw_description, "Coffee, Inc");
        assert_eq!(parsed.rows[1].amount_minor, -500);

        let (_dir2, path2) = temp_csv(
            "Date,Description,Debit,Credit\n10/01/2026,Coffee,12.34,\n10/02/2026,Payment,,50.00\n",
        );
        let mapping2 = CsvMapping {
            date_column: 0,
            description_column: 1,
            amount_column: None,
            debit_column: Some(2),
            credit_column: Some(3),
            category_column: None,
            source_id_column: None,
            date_format: "us".into(),
            sign_convention: "debit_credit".into(),
            credit_type: "refund".into(),
        };
        let parsed = parse(&path2, &mapping2, "acct", "USD").unwrap();
        assert_eq!(parsed.rows[0].amount_minor, 1234);
        assert_eq!(parsed.rows[0].tx_type, "purchase");
        assert_eq!(parsed.rows[1].amount_minor, -5000);
        assert_eq!(parsed.rows[1].tx_type, "refund");
    }

    #[test]
    fn rejects_ambiguous_dates_until_format_is_explicit() {
        let (_dir, path) = temp_csv("Date,Description,Amount\n03/04/2026,Coffee,1.00\n");
        let us = CsvMapping {
            date_column: 0,
            description_column: 1,
            amount_column: Some(2),
            debit_column: None,
            credit_column: None,
            category_column: None,
            source_id_column: None,
            date_format: "us".into(),
            sign_convention: "signed_amount".into(),
            credit_type: "refund".into(),
        };
        let eu = CsvMapping {
            date_format: "eu".into(),
            ..us.clone()
        };
        assert_eq!(
            parse(&path, &us, "a", "USD").unwrap().rows[0]
                .date
                .to_string(),
            "2026-03-04"
        );
        assert_eq!(
            parse(&path, &eu, "a", "USD").unwrap().rows[0]
                .date
                .to_string(),
            "2026-04-03"
        );
        let bad = CsvMapping {
            date_format: "guess".into(),
            ..us
        };
        assert!(parse(&path, &bad, "a", "USD").is_err());
    }

    #[test]
    fn rejects_nul_invalid_utf8_oversized_and_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let nul = dir.path().join("nul.csv");
        std::fs::write(&nul, b"a,b\n1,\0\n").unwrap();
        assert!(read_bounded(&nul).is_err());
        let invalid = dir.path().join("invalid.csv");
        std::fs::write(&invalid, [0xff, 0xfe, 0x00]).unwrap();
        assert!(read_bounded(&invalid).is_err());
        let link = dir.path().join("link.csv");
        std::os::unix::fs::symlink(&nul, &link).unwrap();
        assert!(read_bounded(&link).is_err());
        let empty = dir.path().join("empty.csv");
        std::fs::write(&empty, b"").unwrap();
        assert!(read_bounded(&empty).is_err());
    }

    #[test]
    fn malformed_rows_are_rejected_not_fatal() {
        let (_dir, path) = temp_csv(
            "Date,Description,Amount\n2026-10-01,Coffee,1.00\nnot-a-date,Broken,2.00\n2026-10-03,NoAmount,\n",
        );
        let mapping = CsvMapping {
            date_column: 0,
            description_column: 1,
            amount_column: Some(2),
            debit_column: None,
            credit_column: None,
            category_column: None,
            source_id_column: None,
            date_format: "iso".into(),
            sign_convention: "signed_amount".into(),
            credit_type: "refund".into(),
        };
        let parsed = parse(&path, &mapping, "a", "USD").unwrap();
        assert_eq!(parsed.rows.len(), 1);
        assert_eq!(parsed.rejected.len(), 2);
        assert_eq!(parsed.total_rows, 3);
    }

    #[test]
    fn fingerprint_is_stable_and_source_sensitive() {
        let a = fingerprint("acct", "2026-10-01", 1234, "coffee", None);
        let b = fingerprint("acct", "2026-10-01", 1234, "coffee", None);
        let c = fingerprint("acct", "2026-10-01", 1234, "coffee", Some("id-1"));
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
