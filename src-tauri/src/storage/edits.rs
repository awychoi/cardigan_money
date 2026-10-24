//! Transaction overrides, merchant rules, and dated manual payment metadata.
//! User edits live separately from source fields and survive syncs/imports.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::domain::dates::parse_date;
use crate::domain::money::parse_currency_amount;
use crate::domain::text::{clean_text, normalize_merchant, MAX_CATEGORY_CHARS, MAX_NOTES_CHARS};
use crate::error::{AppError, AppResult};
use crate::storage::time::now_rfc3339;

pub const MAX_MERCHANT_ALIAS: usize = 120;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OverrideInput {
    pub merchant_alias: Option<String>,
    pub category: Option<String>,
    pub type_correction: Option<String>,
    pub excluded: Option<bool>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TransactionDetail {
    pub id: String,
    pub account_id: String,
    pub account_name: String,
    pub account_last_four: Option<String>,
    pub reporting_date: String,
    pub authorized_date: Option<String>,
    pub posted_date: Option<String>,
    pub reporting_date_source: String,
    pub amount_minor: i64,
    pub currency: String,
    pub raw_description: String,
    pub merchant: String,
    pub merchant_alias: Option<String>,
    pub category: Option<String>,
    pub source_category: Option<String>,
    pub tx_type: String,
    pub type_correction: Option<String>,
    pub pending: bool,
    pub excluded: bool,
    pub excluded_override: Option<bool>,
    pub notes: Option<String>,
    pub source_kind: String,
    pub provider_transaction_id: Option<String>,
    pub pending_link_id: Option<String>,
    pub edited_at: Option<String>,
}

fn validate_type(value: &str) -> AppResult<()> {
    match value {
        "purchase" | "refund" | "fee" | "interest" | "payment" | "transfer" | "other" => Ok(()),
        _ => Err(AppError::InvalidInput("transaction type")),
    }
}

pub fn set_override(conn: &Connection, id: &str, input: OverrideInput) -> AppResult<TransactionDetail> {
    uuid::Uuid::parse_str(id).map_err(|_| AppError::InvalidInput("transaction id"))?;
    let exists: i64 = conn
        .query_row("SELECT count(*) FROM transactions WHERE id = ?1", [id], |row| row.get(0))
        .map_err(|_| AppError::Database)?;
    if exists == 0 {
        return Err(AppError::NotFound);
    }
    let merchant_alias = match input.merchant_alias {
        Some(value) => Some(
            clean_text(&value, MAX_MERCHANT_ALIAS, false)
                .map_err(|_| AppError::InvalidInput("merchant alias"))?,
        ),
        None => None,
    };
    let category = match input.category {
        Some(value) if !value.trim().is_empty() => Some(
            clean_text(&value, MAX_CATEGORY_CHARS, false)
                .map_err(|_| AppError::InvalidInput("category"))?,
        ),
        _ => None,
    };
    if let Some(value) = &input.type_correction {
        validate_type(value)?;
    }
    let notes = match input.notes {
        Some(value) if !value.trim().is_empty() => Some(
            clean_text(&value, MAX_NOTES_CHARS, true)
                .map_err(|_| AppError::InvalidInput("notes"))?,
        ),
        _ => None,
    };
    let now = now_rfc3339();
    if merchant_alias.is_none()
        && category.is_none()
        && input.type_correction.is_none()
        && input.excluded.is_none()
        && notes.is_none()
    {
        conn.execute("DELETE FROM transaction_overrides WHERE transaction_id = ?1", [id])
            .map_err(|_| AppError::Database)?;
    } else {
        conn.execute(
            "INSERT INTO transaction_overrides (transaction_id, merchant_alias, category, type_correction, excluded, notes, edited_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(transaction_id) DO UPDATE SET merchant_alias = excluded.merchant_alias, \
                category = excluded.category, type_correction = excluded.type_correction, \
                excluded = excluded.excluded, notes = excluded.notes, edited_at = excluded.edited_at",
            rusqlite::params![
                id,
                merchant_alias,
                category,
                input.type_correction,
                input.excluded.map(i64::from),
                notes,
                now
            ],
        )
        .map_err(|_| AppError::Database)?;
    }
    get_detail(conn, id)
}

pub fn clear_override(conn: &Connection, id: &str) -> AppResult<TransactionDetail> {
    conn.execute("DELETE FROM transaction_overrides WHERE transaction_id = ?1", [id])
        .map_err(|_| AppError::Database)?;
    get_detail(conn, id)
}

pub fn get_detail(conn: &Connection, id: &str) -> AppResult<TransactionDetail> {
    conn.query_row(
        "SELECT t.id, t.account_id, a.display_name, a.last_four, t.reporting_date, \
            t.authorized_date, t.posted_date, t.reporting_date_source, t.amount_minor, t.currency, \
            t.raw_description, COALESCE(o.merchant_alias, NULLIF(t.rule_merchant,''), t.merchant_display), \
            o.merchant_alias, COALESCE(NULLIF(o.category,''), NULLIF(t.rule_category,''), t.category), \
            t.category, COALESCE(o.type_correction, t.tx_type), o.type_correction, t.pending, \
            COALESCE(o.excluded, t.excluded), o.excluded, o.notes, t.source_kind, \
            t.provider_transaction_id, t.pending_link_id, o.edited_at \
         FROM transactions t JOIN accounts a ON a.id = t.account_id \
         LEFT JOIN transaction_overrides o ON o.transaction_id = t.id \
         WHERE t.id = ?1 AND t.removed = 0",
        [id],
        |row| {
            Ok(TransactionDetail {
                id: row.get(0)?,
                account_id: row.get(1)?,
                account_name: row.get(2)?,
                account_last_four: row.get(3)?,
                reporting_date: row.get(4)?,
                authorized_date: row.get(5)?,
                posted_date: row.get(6)?,
                reporting_date_source: row.get(7)?,
                amount_minor: row.get(8)?,
                currency: row.get(9)?,
                raw_description: row.get(10)?,
                merchant: row.get(11)?,
                merchant_alias: row.get(12)?,
                category: row.get(13)?,
                source_category: row.get(14)?,
                tx_type: row.get(15)?,
                type_correction: row.get(16)?,
                pending: row.get::<_, i64>(17)? != 0,
                excluded: row.get::<_, i64>(18)? != 0,
                excluded_override: row.get::<_, Option<i64>>(19)?.map(|value| value != 0),
                notes: row.get(20)?,
                source_kind: row.get(21)?,
                provider_transaction_id: row.get(22)?,
                pending_link_id: row.get(23)?,
                edited_at: row.get(24)?,
            })
        },
    )
    .map_err(|error| match error {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound,
        other => AppError::from(other),
    })
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatchType {
    Exact,
    Contains,
    StartsWith,
}

impl MatchType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Contains => "contains",
            Self::StartsWith => "starts_with",
        }
    }

    pub fn parse(value: &str) -> AppResult<Self> {
        match value {
            "exact" => Ok(Self::Exact),
            "contains" => Ok(Self::Contains),
            "starts_with" => Ok(Self::StartsWith),
            _ => Err(AppError::InvalidInput("rule match type")),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MerchantRuleInput {
    pub match_type: String,
    pub match_text: String,
    pub canonical_merchant: String,
    pub category: Option<String>,
    pub priority: i64,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MerchantRule {
    pub id: String,
    pub match_type: String,
    pub match_text: String,
    pub canonical_merchant: String,
    pub category: Option<String>,
    pub priority: i64,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

pub fn list_rules(conn: &Connection) -> AppResult<Vec<MerchantRule>> {
    let mut stmt = conn.prepare(
        "SELECT id, match_type, match_text, canonical_merchant, category, priority, enabled, created_at, updated_at \
         FROM merchant_rules ORDER BY priority DESC, id",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(MerchantRule {
                id: row.get(0)?,
                match_type: row.get(1)?,
                match_text: row.get(2)?,
                canonical_merchant: row.get(3)?,
                category: row.get(4)?,
                priority: row.get(5)?,
                enabled: row.get::<_, i64>(6)? != 0,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn validate_rule(input: &MerchantRuleInput) -> AppResult<(String, String, Option<String>)> {
    MatchType::parse(&input.match_type)?;
    let match_text = normalize_merchant(&input.match_text)
        .map_err(|_| AppError::InvalidInput("rule match text"))?;
    if match_text.chars().count() > 120 {
        return Err(AppError::InvalidInput("rule match text"));
    }
    let canonical = clean_text(&input.canonical_merchant, 120, false)
        .map_err(|_| AppError::InvalidInput("rule canonical merchant"))?;
    let category = match &input.category {
        Some(value) if !value.trim().is_empty() => Some(
            clean_text(value, MAX_CATEGORY_CHARS, false)
                .map_err(|_| AppError::InvalidInput("rule category"))?,
        ),
        _ => None,
    };
    if !(0..=10_000).contains(&input.priority) {
        return Err(AppError::InvalidInput("rule priority"));
    }
    Ok((match_text, canonical, category))
}

pub fn create_rule(conn: &Connection, input: &MerchantRuleInput) -> AppResult<MerchantRule> {
    let (match_text, canonical, category) = validate_rule(input)?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = now_rfc3339();
    conn.execute(
        "INSERT INTO merchant_rules (id, match_type, match_text, canonical_merchant, category, priority, enabled, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        rusqlite::params![
            id,
            MatchType::parse(&input.match_type)?.as_str(),
            match_text,
            canonical,
            category,
            input.priority,
            i64::from(input.enabled),
            now
        ],
    )
    .map_err(|error| match error {
        rusqlite::Error::SqliteFailure(_, _) => AppError::InvalidInput("rule already exists"),
        other => AppError::from(other),
    })?;
    apply_rules(conn)?;
    get_rule(conn, &id)
}

pub fn update_rule(conn: &Connection, id: &str, input: &MerchantRuleInput) -> AppResult<MerchantRule> {
    uuid::Uuid::parse_str(id).map_err(|_| AppError::InvalidInput("rule id"))?;
    let (match_text, canonical, category) = validate_rule(input)?;
    let changed = conn
        .execute(
            "UPDATE merchant_rules SET match_type = ?2, match_text = ?3, canonical_merchant = ?4, \
                category = ?5, priority = ?6, enabled = ?7, updated_at = ?8 WHERE id = ?1",
            rusqlite::params![
                id,
                MatchType::parse(&input.match_type)?.as_str(),
                match_text,
                canonical,
                category,
                input.priority,
                i64::from(input.enabled),
                now_rfc3339()
            ],
        )
        .map_err(|error| match error {
            rusqlite::Error::SqliteFailure(_, _) => AppError::InvalidInput("rule already exists"),
            other => AppError::from(other),
        })?;
    if changed == 0 {
        return Err(AppError::NotFound);
    }
    apply_rules(conn)?;
    get_rule(conn, id)
}

pub fn delete_rule(conn: &Connection, id: &str) -> AppResult<()> {
    uuid::Uuid::parse_str(id).map_err(|_| AppError::InvalidInput("rule id"))?;
    let changed = conn
        .execute("DELETE FROM merchant_rules WHERE id = ?1", [id])
        .map_err(|_| AppError::Database)?;
    if changed == 0 {
        return Err(AppError::NotFound);
    }
    apply_rules(conn)?;
    Ok(())
}

fn get_rule(conn: &Connection, id: &str) -> AppResult<MerchantRule> {
    conn.query_row(
        "SELECT id, match_type, match_text, canonical_merchant, category, priority, enabled, created_at, updated_at \
         FROM merchant_rules WHERE id = ?1",
        [id],
        |row| {
            Ok(MerchantRule {
                id: row.get(0)?,
                match_type: row.get(1)?,
                match_text: row.get(2)?,
                canonical_merchant: row.get(3)?,
                category: row.get(4)?,
                priority: row.get(5)?,
                enabled: row.get::<_, i64>(6)? != 0,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        },
    )
    .map_err(|error| match error {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound,
        other => AppError::from(other),
    })
}

/// Recompute rule-derived display fields for every transaction. Source fields
/// are never modified; user overrides still take precedence in queries.
pub fn apply_rules(conn: &Connection) -> AppResult<usize> {
    let rules = list_rules(conn)?;
    let mut stmt = conn.prepare("SELECT id, merchant_normalized, raw_description FROM transactions")?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let tx = conn.unchecked_transaction()?;
    let mut matched = 0usize;
    for (id, normalized, raw) in rows {
        let mut chosen: Option<&MerchantRule> = None;
        for rule in rules.iter().filter(|rule| rule.enabled) {
            let needle = rule.match_text.as_str();
            let haystack = normalized.to_lowercase();
            let is_match = match MatchType::parse(&rule.match_type)? {
                MatchType::Exact => haystack == needle,
                MatchType::Contains => haystack.contains(needle),
                MatchType::StartsWith => haystack.starts_with(needle),
            };
            if is_match || raw.to_lowercase().contains(needle) {
                chosen = Some(rule);
                break;
            }
        }
        if let Some(rule) = chosen {
            tx.execute(
                "UPDATE transactions SET rule_merchant = ?2, rule_category = ?3, rule_id = ?4 WHERE id = ?1",
                rusqlite::params![id, rule.canonical_merchant, rule.category, rule.id],
            )
            .map_err(|_| AppError::Database)?;
            matched += 1;
        } else {
            tx.execute(
                "UPDATE transactions SET rule_merchant = NULL, rule_category = NULL, rule_id = NULL WHERE id = ?1",
                [&id],
            )
            .map_err(|_| AppError::Database)?;
        }
    }
    tx.commit().map_err(|_| AppError::Database)?;
    Ok(matched)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManualPaymentInput {
    pub statement_balance: Option<String>,
    pub minimum_due: Option<String>,
    pub next_due_date: Option<String>,
    pub valid_through_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManualPayment {
    pub account_id: String,
    pub statement_balance_minor: Option<i64>,
    pub minimum_due_minor: Option<i64>,
    pub next_due_date: Option<String>,
    pub valid_through_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub fn get_manual_payment(conn: &Connection, account_id: &str) -> AppResult<Option<ManualPayment>> {
    conn.query_row(
        "SELECT account_id, statement_balance_minor, minimum_due_minor, next_due_date, valid_through_date, created_at, updated_at \
         FROM manual_payment_metadata WHERE account_id = ?1",
        [account_id],
        |row| {
            Ok(ManualPayment {
                account_id: row.get(0)?,
                statement_balance_minor: row.get(1)?,
                minimum_due_minor: row.get(2)?,
                next_due_date: row.get(3)?,
                valid_through_date: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        },
    )
    .map(Some)
    .or_else(|error| match error {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(AppError::from(other)),
    })
}

pub fn set_manual_payment(
    conn: &Connection,
    account_id: &str,
    input: &ManualPaymentInput,
) -> AppResult<Option<ManualPayment>> {
    let currency: String = conn
        .query_row("SELECT currency FROM accounts WHERE id = ?1", [account_id], |row| row.get(0))
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound,
            other => AppError::from(other),
        })?;
    let parse_money = |value: &Option<String>| -> AppResult<Option<i64>> {
        match value {
            None => Ok(None),
            Some(value) if value.trim().is_empty() => Ok(None),
            Some(value) => {
                let minor = parse_currency_amount(value, &currency)
                    .map_err(|_| AppError::InvalidInput("payment amount"))?;
                if minor < 0 {
                    return Err(AppError::InvalidInput("payment amount"));
                }
                Ok(Some(minor))
            }
        }
    };
    let statement = parse_money(&input.statement_balance)?;
    let minimum = parse_money(&input.minimum_due)?;
    let next_due = match &input.next_due_date {
        None => None,
        Some(value) if value.trim().is_empty() => None,
        Some(value) => {
            parse_date(value).map_err(|_| AppError::InvalidInput("next due date"))?;
            Some(value.clone())
        }
    };
    let valid_through = match &input.valid_through_date {
        None => None,
        Some(value) if value.trim().is_empty() => None,
        Some(value) => {
            parse_date(value).map_err(|_| AppError::InvalidInput("valid-through date"))?;
            Some(value.clone())
        }
    };
    if statement.is_none() && minimum.is_none() && next_due.is_none() {
        conn.execute("DELETE FROM manual_payment_metadata WHERE account_id = ?1", [account_id])
            .map_err(|_| AppError::Database)?;
        return Ok(None);
    }
    let now = now_rfc3339();
    conn.execute(
        "INSERT INTO manual_payment_metadata (account_id, statement_balance_minor, minimum_due_minor, next_due_date, valid_through_date, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6) \
         ON CONFLICT(account_id) DO UPDATE SET statement_balance_minor = excluded.statement_balance_minor, \
            minimum_due_minor = excluded.minimum_due_minor, next_due_date = excluded.next_due_date, \
            valid_through_date = excluded.valid_through_date, updated_at = excluded.updated_at",
        rusqlite::params![account_id, statement, minimum, next_due, valid_through, now],
    )
    .map_err(|_| AppError::Database)?;
    get_manual_payment(conn, account_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_type_parsing_is_closed() {
        assert_eq!(MatchType::parse("exact").unwrap(), MatchType::Exact);
        assert_eq!(MatchType::parse("contains").unwrap(), MatchType::Contains);
        assert_eq!(MatchType::parse("starts_with").unwrap(), MatchType::StartsWith);
        assert!(MatchType::parse("regex").is_err());
    }

    #[test]
    fn override_input_types_are_validated() {
        let mut input = OverrideInput {
            merchant_alias: None,
            category: None,
            type_correction: Some("purchase".into()),
            excluded: None,
            notes: None,
        };
        assert!(validate_type(&input.type_correction.clone().unwrap()).is_ok());
        input.type_correction = Some("payment".into());
        assert!(validate_type(&input.type_correction.clone().unwrap()).is_ok());
        input.type_correction = Some("delete everything".into());
        assert!(validate_type(&input.type_correction.unwrap()).is_err());
    }
}
