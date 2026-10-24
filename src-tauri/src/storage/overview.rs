//! Current balances and upcoming reported payments.
//!
//! These facts are independent of the selected spending month. Missing values
//! remain `None`; they are never coerced to zero. A statement balance is never
//! treated as the remaining amount due.

use chrono::NaiveDate;
use rusqlite::Connection;
use serde::Serialize;

use crate::domain::dates::{format_date, parse_date};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PaymentInfo {
    pub next_due_date: Option<String>,
    pub due_amount_minor: Option<i64>,
    /// minimum_due | reported_zero | not_reported | no_due_date
    pub due_amount_kind: String,
    pub due_source: Option<String>,
    pub statement_balance_minor: Option<i64>,
    pub statement_balance_source: Option<String>,
    pub statement_issue_date: Option<String>,
    pub next_closing_date: Option<String>,
    pub valid_through_date: Option<String>,
    pub manual_expired: bool,
    pub retrieved_at: Option<String>,
}

impl Default for PaymentInfo {
    fn default() -> Self {
        Self {
            next_due_date: None,
            due_amount_minor: None,
            due_amount_kind: "no_due_date".to_string(),
            due_source: None,
            statement_balance_minor: None,
            statement_balance_source: None,
            statement_issue_date: None,
            next_closing_date: None,
            valid_through_date: None,
            manual_expired: false,
            retrieved_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountCard {
    pub account_id: String,
    pub issuer: String,
    pub display_name: String,
    pub last_four: Option<String>,
    pub currency: String,
    pub included: bool,
    pub source_kind: String,
    pub connection_status: Option<String>,
    pub current_balance_minor: Option<i64>,
    pub available_credit_minor: Option<i64>,
    pub credit_limit_minor: Option<i64>,
    pub balance_retrieved_at: Option<String>,
    pub balance_provenance: Option<String>,
    pub payment: PaymentInfo,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BalanceTotals {
    pub known_total_minor: i64,
    pub accounts_with_balance: i64,
    pub accounts_missing_balance: i64,
    pub complete: bool,
    pub other_currency: Vec<(String, i64)>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpcomingPayment {
    pub account_id: String,
    pub display_name: String,
    pub last_four: Option<String>,
    pub currency: String,
    pub due_date: String,
    pub amount_minor: Option<i64>,
    pub amount_kind: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Upcoming {
    pub as_of_date: String,
    pub window_days: u32,
    pub entries: Vec<UpcomingPayment>,
    pub known_total_minor: i64,
    pub unknown_amount_count: i64,
    pub non_usd_entry_count: i64,
    pub complete: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub accounts: Vec<AccountCard>,
    pub balances: BalanceTotals,
    pub upcoming: Upcoming,
}

struct AccountRow {
    id: String,
    issuer: String,
    display_name: String,
    last_four: Option<String>,
    currency: String,
    included: bool,
    source_kind: String,
}

fn load_accounts(conn: &Connection, include_archived: bool) -> AppResult<Vec<AccountRow>> {
    let sql = format!(
        "SELECT id, issuer, display_name, last_four, currency, include_in_overview, source_kind \
         FROM accounts {} ORDER BY issuer COLLATE NOCASE, display_name COLLATE NOCASE, id",
        if include_archived { "" } else { "WHERE archived = 0" }
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], |row| {
            Ok(AccountRow {
                id: row.get(0)?,
                issuer: row.get(1)?,
                display_name: row.get(2)?,
                last_four: row.get(3)?,
                currency: row.get(4)?,
                included: row.get::<_, i64>(5)? != 0,
                source_kind: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn connection_status(conn: &Connection, account_id: &str) -> AppResult<Option<String>> {
    conn.query_row(
        "SELECT c.status FROM account_sources s \
         JOIN connections c ON c.id = s.connection_id \
         WHERE s.account_id = ?1 AND s.unlinked_at IS NULL \
         ORDER BY s.linked_at DESC LIMIT 1",
        [account_id],
        |row| row.get::<_, String>(0),
    )
    .map(Some)
    .or_else(|error| match error {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(AppError::from(other)),
    })
}

struct BalanceRow {
    current: i64,
    available: Option<i64>,
    limit: Option<i64>,
    retrieved_at: String,
    provenance: String,
}

fn latest_balance(conn: &Connection, account_id: &str) -> AppResult<Option<BalanceRow>> {
    conn.query_row(
        "SELECT current_balance_minor, available_credit_minor, credit_limit_minor, retrieved_at, provenance \
         FROM balance_snapshots WHERE account_id = ?1 \
         ORDER BY retrieved_at DESC, id DESC LIMIT 1",
        [account_id],
        |row| {
            Ok(BalanceRow {
                current: row.get(0)?,
                available: row.get(1)?,
                limit: row.get(2)?,
                retrieved_at: row.get(3)?,
                provenance: row.get(4)?,
            })
        },
    )
    .map(Some)
    .or_else(|error| match error {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(AppError::from(other)),
    })
}

struct ProviderPayment {
    next_due_date: Option<String>,
    minimum_due: Option<i64>,
    statement_balance: Option<i64>,
    issue_date: Option<String>,
    closing_date: Option<String>,
    retrieved_at: String,
}

fn latest_provider_payment(
    conn: &Connection,
    account_id: &str,
) -> AppResult<Option<ProviderPayment>> {
    conn.query_row(
        "SELECT next_due_date, minimum_due_minor, statement_balance_minor, last_statement_issue_date, \
                next_closing_date, retrieved_at \
         FROM payment_snapshots WHERE account_id = ?1 AND provenance = 'provider' \
         ORDER BY retrieved_at DESC, id DESC LIMIT 1",
        [account_id],
        |row| {
            Ok(ProviderPayment {
                next_due_date: row.get(0)?,
                minimum_due: row.get(1)?,
                statement_balance: row.get(2)?,
                issue_date: row.get(3)?,
                closing_date: row.get(4)?,
                retrieved_at: row.get(5)?,
            })
        },
    )
    .map(Some)
    .or_else(|error| match error {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(AppError::from(other)),
    })
}

struct ManualPayment {
    statement_balance: Option<i64>,
    minimum_due: Option<i64>,
    next_due_date: Option<String>,
    valid_through_date: Option<String>,
    updated_at: String,
}

fn manual_payment(conn: &Connection, account_id: &str) -> AppResult<Option<ManualPayment>> {
    conn.query_row(
        "SELECT statement_balance_minor, minimum_due_minor, next_due_date, valid_through_date, updated_at \
         FROM manual_payment_metadata WHERE account_id = ?1",
        [account_id],
        |row| {
            Ok(ManualPayment {
                statement_balance: row.get(0)?,
                minimum_due: row.get(1)?,
                next_due_date: row.get(2)?,
                valid_through_date: row.get(3)?,
                updated_at: row.get(4)?,
            })
        },
    )
    .map(Some)
    .or_else(|error| match error {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(AppError::from(other)),
    })
}

fn payment_info(
    today: NaiveDate,
    provider: Option<&ProviderPayment>,
    manual: Option<&ManualPayment>,
) -> AppResult<PaymentInfo> {
    let mut info = PaymentInfo::default();
    let manual_valid = manual
        .map(|manual| {
            manual
                .valid_through_date
                .as_deref()
                .and_then(|value| parse_date(value).ok())
                .map(|valid_through| today <= valid_through)
                .unwrap_or(true)
        })
        .unwrap_or(false);
    if manual.is_some() && !manual_valid {
        info.manual_expired = true;
    }

    let manual_has_due = manual_valid
        && manual
            .map(|manual| manual.next_due_date.is_some() || manual.minimum_due.is_some())
            .unwrap_or(false);
    let provider_has_due = provider
        .map(|provider| provider.next_due_date.is_some() || provider.minimum_due.is_some())
        .unwrap_or(false);

    if manual_has_due {
        let manual = manual.expect("checked");
        info.next_due_date = manual.next_due_date.clone();
        info.due_amount_minor = manual.minimum_due;
        info.due_source = Some("manual".to_string());
        info.valid_through_date = manual.valid_through_date.clone();
        info.retrieved_at = Some(manual.updated_at.clone());
    } else if provider_has_due {
        let provider = provider.expect("checked");
        info.next_due_date = provider.next_due_date.clone();
        info.due_amount_minor = provider.minimum_due;
        info.due_source = Some("provider".to_string());
        info.retrieved_at = Some(provider.retrieved_at.clone());
    }

    info.due_amount_kind = match (&info.next_due_date, info.due_amount_minor) {
        (Some(_), Some(0)) => "reported_zero".to_string(),
        (Some(_), Some(_)) => "minimum_due".to_string(),
        (Some(_), None) => "not_reported".to_string(),
        (None, Some(0)) => "reported_zero".to_string(),
        (None, Some(_)) => "minimum_due".to_string(),
        (None, None) => "no_due_date".to_string(),
    };

    if manual_valid {
        if let Some(manual) = manual {
            if manual.statement_balance.is_some() {
                info.statement_balance_minor = manual.statement_balance;
                info.statement_balance_source = Some("manual".to_string());
            }
        }
    }
    if info.statement_balance_minor.is_none() {
        if let Some(provider) = provider {
            info.statement_balance_minor = provider.statement_balance;
            info.statement_balance_source = provider
                .statement_balance
                .map(|_| "provider".to_string());
            info.statement_issue_date = provider.issue_date.clone();
            info.next_closing_date = provider.closing_date.clone();
            if info.retrieved_at.is_none() {
                info.retrieved_at = Some(provider.retrieved_at.clone());
            }
        }
    }
    Ok(info)
}

pub fn overview(conn: &Connection, today: NaiveDate) -> AppResult<Overview> {
    let account_rows = load_accounts(conn, false)?;
    let mut accounts = Vec::with_capacity(account_rows.len());
    let mut known_total = 0i64;
    let mut missing = 0i64;
    let mut with_balance = 0i64;
    let mut other_currency: std::collections::BTreeMap<String, i64> =
        std::collections::BTreeMap::new();
    let mut upcoming_entries = Vec::new();
    let window_end = today + chrono::Duration::days(14);

    for row in account_rows {
        let balance = latest_balance(conn, &row.id)?;
        let provider = latest_provider_payment(conn, &row.id)?;
        let manual = manual_payment(conn, &row.id)?;
        let payment = payment_info(today, provider.as_ref(), manual.as_ref())?;
        let status = connection_status(conn, &row.id)?;

        if row.included {
            if let Some(balance) = &balance {
                with_balance += 1;
                if row.currency == "USD" {
                    known_total += balance.current;
                } else {
                    *other_currency.entry(row.currency.clone()).or_insert(0) += balance.current;
                }
            } else {
                missing += 1;
            }
        }

        let card = AccountCard {
            account_id: row.id.clone(),
            issuer: row.issuer.clone(),
            display_name: row.display_name.clone(),
            last_four: row.last_four.clone(),
            currency: row.currency.clone(),
            included: row.included,
            source_kind: row.source_kind.clone(),
            connection_status: status,
            current_balance_minor: balance.as_ref().map(|b| b.current),
            available_credit_minor: balance.as_ref().and_then(|b| b.available),
            credit_limit_minor: balance.as_ref().and_then(|b| b.limit),
            balance_retrieved_at: balance.as_ref().map(|b| b.retrieved_at.clone()),
            balance_provenance: balance.as_ref().map(|b| b.provenance.clone()),
            payment: payment.clone(),
        };

        if row.included {
            if let Some(due_date) = &payment.next_due_date {
                if let Ok(date) = parse_date(due_date) {
                    if date >= today && date < window_end {
                        upcoming_entries.push(UpcomingPayment {
                            account_id: row.id.clone(),
                            display_name: row.display_name.clone(),
                            last_four: row.last_four.clone(),
                            currency: row.currency.clone(),
                            due_date: due_date.clone(),
                            amount_minor: payment.due_amount_minor,
                            amount_kind: payment.due_amount_kind.clone(),
                            source: payment.due_source.clone().unwrap_or_default(),
                        });
                    }
                }
            }
        }
        accounts.push(card);
    }

    upcoming_entries.sort_by(|a, b| a.due_date.cmp(&b.due_date).then(a.display_name.cmp(&b.display_name)));
    let known_upcoming: i64 = upcoming_entries
        .iter()
        .filter(|e| e.currency == "USD")
        .filter_map(|e| e.amount_minor)
        .sum();
    let unknown_amount_count = upcoming_entries
        .iter()
        .filter(|e| e.amount_minor.is_none())
        .count() as i64;
    let non_usd_entry_count = upcoming_entries
        .iter()
        .filter(|e| e.currency != "USD")
        .count() as i64;
    let upcoming = Upcoming {
        as_of_date: format_date(today),
        window_days: 14,
        complete: unknown_amount_count == 0,
        entries: upcoming_entries,
        known_total_minor: known_upcoming,
        unknown_amount_count,
        non_usd_entry_count,
    };

    let complete = missing == 0 && with_balance > 0;
    Ok(Overview {
        accounts,
        balances: BalanceTotals {
            known_total_minor: known_total,
            accounts_with_balance: with_balance,
            accounts_missing_balance: missing,
            complete,
            other_currency: other_currency.into_iter().collect(),
        },
        upcoming,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_kind_never_turns_unknown_into_zero() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 23).unwrap();
        let provider = ProviderPayment {
            next_due_date: Some("2026-10-30".into()),
            minimum_due: None,
            statement_balance: Some(123_456),
            issue_date: Some("2026-10-01".into()),
            closing_date: None,
            retrieved_at: "2026-10-23T00:00:00Z".into(),
        };
        let info = payment_info(today, Some(&provider), None).unwrap();
        assert_eq!(info.next_due_date.as_deref(), Some("2026-10-30"));
        assert_eq!(info.due_amount_minor, None);
        assert_eq!(info.due_amount_kind, "not_reported");
        assert_eq!(info.statement_balance_minor, Some(123_456));

        let zero = ProviderPayment { minimum_due: Some(0), ..provider };
        let info = payment_info(today, Some(&zero), None).unwrap();
        assert_eq!(info.due_amount_minor, Some(0));
        assert_eq!(info.due_amount_kind, "reported_zero");
    }

    #[test]
    fn expired_manual_metadata_is_not_used() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 23).unwrap();
        let manual = ManualPayment {
            statement_balance: Some(50_000),
            minimum_due: Some(2_500),
            next_due_date: Some("2026-10-25".into()),
            valid_through_date: Some("2026-10-20".into()),
            updated_at: "2026-10-19T00:00:00Z".into(),
        };
        let info = payment_info(today, None, Some(&manual)).unwrap();
        assert!(info.manual_expired);
        assert_eq!(info.next_due_date, None);
        assert_eq!(info.due_amount_minor, None);
        assert_eq!(info.statement_balance_minor, None);
    }
}
