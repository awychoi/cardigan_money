//! Parameterized ledger queries and calendar-month summaries.
//!
//! Identifiers are never interpolated from user input. Sort/filter identifiers
//! come from closed enums, values are positional parameters, and LIKE patterns
//! are escaped. Money is signed integer minor units throughout.

use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};
use serde::{Deserialize, Serialize};

use crate::domain::dates::{parse_date, Month};
use crate::domain::text::{clean_text, escape_like, MAX_SEARCH_CHARS};
use crate::error::{AppError, AppResult};

pub const MAX_PAGE_SIZE: u32 = 200;
pub const MAX_PAGE: u32 = 10_000;
pub const MAX_RANGE_DAYS: i64 = 731;
pub const MAX_ACCOUNT_FILTERS: usize = 20;

pub const EFFECTIVE_TYPE: &str = "COALESCE(o.type_correction, t.tx_type)";
pub const EFFECTIVE_EXCLUDED: &str = "COALESCE(o.excluded, t.excluded)";
pub const EFFECTIVE_MERCHANT: &str =
    "COALESCE(o.merchant_alias, NULLIF(t.rule_merchant, ''), t.merchant_display)";
pub const EFFECTIVE_CATEGORY: &str =
    "COALESCE(NULLIF(o.category, ''), NULLIF(t.rule_category, ''), NULLIF(t.category, ''), 'Uncategorized')";
const SPEND_TYPES: &str = "('purchase','refund','fee','interest')";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerStatus {
    All,
    Pending,
    Posted,
}

impl LedgerStatus {
    fn parse(value: &str) -> AppResult<Self> {
        match value {
            "all" => Ok(Self::All),
            "pending" => Ok(Self::Pending),
            "posted" => Ok(Self::Posted),
            _ => Err(AppError::InvalidInput("status filter")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerSort {
    DateDesc,
    DateAsc,
    AmountDesc,
    AmountAsc,
    MerchantAsc,
}

impl LedgerSort {
    fn parse(value: &str) -> AppResult<Self> {
        match value {
            "date_desc" => Ok(Self::DateDesc),
            "date_asc" => Ok(Self::DateAsc),
            "amount_desc" => Ok(Self::AmountDesc),
            "amount_asc" => Ok(Self::AmountAsc),
            "merchant_asc" => Ok(Self::MerchantAsc),
            _ => Err(AppError::InvalidInput("sort")),
        }
    }

    fn order_by(self) -> &'static str {
        match self {
            LedgerSort::DateDesc => {
                "t.reporting_date DESC, t.created_at DESC, t.id DESC"
            }
            LedgerSort::DateAsc => "t.reporting_date ASC, t.created_at ASC, t.id ASC",
            LedgerSort::AmountDesc => {
                "t.amount_minor DESC, t.reporting_date DESC, t.id DESC"
            }
            LedgerSort::AmountAsc => "t.amount_minor ASC, t.reporting_date ASC, t.id ASC",
            LedgerSort::MerchantAsc => {
                "lower(COALESCE(o.merchant_alias, t.merchant_display)) ASC, t.reporting_date DESC, t.id DESC"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TxTypeFilter {
    Purchase,
    Refund,
    Fee,
    Interest,
    Payment,
    Transfer,
    Other,
}

impl TxTypeFilter {
    fn parse(value: &str) -> AppResult<Self> {
        match value {
            "purchase" => Ok(Self::Purchase),
            "refund" => Ok(Self::Refund),
            "fee" => Ok(Self::Fee),
            "interest" => Ok(Self::Interest),
            "payment" => Ok(Self::Payment),
            "transfer" => Ok(Self::Transfer),
            "other" => Ok(Self::Other),
            _ => Err(AppError::InvalidInput("transaction type filter")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Purchase => "purchase",
            Self::Refund => "refund",
            Self::Fee => "fee",
            Self::Interest => "interest",
            Self::Payment => "payment",
            Self::Transfer => "transfer",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LedgerQuery {
    pub start_date: String,
    pub end_date_exclusive: String,
    #[serde(default)]
    pub account_ids: Vec<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub tx_type: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LedgerRow {
    pub id: String,
    pub reporting_date: String,
    pub merchant: String,
    pub merchant_edited: bool,
    pub raw_description: String,
    pub account_id: String,
    pub account_name: String,
    pub account_last_four: Option<String>,
    pub category: Option<String>,
    pub category_edited: bool,
    pub tx_type: String,
    pub pending: bool,
    pub amount_minor: i64,
    pub currency: String,
    pub source_kind: String,
    pub authorized_date: Option<String>,
    pub posted_date: Option<String>,
    pub reporting_date_source: String,
    pub excluded: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LedgerPage {
    pub rows: Vec<LedgerRow>,
    pub row_count: i64,
    pub page: u32,
    pub page_size: u32,
    pub eligible_subtotal_minor: i64,
    pub pending_subtotal_minor: i64,
}

struct Filters {
    where_sql: String,
    params: Vec<Value>,
}

fn build_filters(query: &LedgerQuery) -> AppResult<Filters> {
    let start = parse_date(&query.start_date).map_err(|_| AppError::InvalidInput("start date"))?;
    let end = parse_date(&query.end_date_exclusive)
        .map_err(|_| AppError::InvalidInput("end date"))?;
    if start >= end {
        return Err(AppError::InvalidInput("date range"));
    }
    if (end - start).num_days() > MAX_RANGE_DAYS {
        return Err(AppError::InvalidInput("date range too large"));
    }
    if query.account_ids.len() > MAX_ACCOUNT_FILTERS {
        return Err(AppError::InvalidInput("too many account filters"));
    }
    for id in &query.account_ids {
        uuid::Uuid::parse_str(id).map_err(|_| AppError::InvalidInput("account id"))?;
    }

    let page_size = query.page_size.unwrap_or(50);
    if page_size == 0 || page_size > MAX_PAGE_SIZE {
        return Err(AppError::InvalidInput("page size"));
    }
    if query.page.unwrap_or(0) > MAX_PAGE {
        return Err(AppError::InvalidInput("page"));
    }

    let mut sql = String::from(
        " FROM transactions t \
         JOIN accounts a ON a.id = t.account_id \
         LEFT JOIN transaction_overrides o ON o.transaction_id = t.id \
         WHERE t.removed = 0 AND t.reporting_date >= ? AND t.reporting_date < ?",
    )
    .replace('?', "?");
    let mut params: Vec<Value> = vec![start.to_string().into(), end.to_string().into()];

    if !query.account_ids.is_empty() {
        let placeholders = vec!["?"; query.account_ids.len()].join(",");
        sql.push_str(&format!(" AND t.account_id IN ({placeholders})"));
        for id in &query.account_ids {
            params.push(id.clone().into());
        }
    }
    if let Some(category) = &query.category {
        let category = clean_text(category, 80, false)
            .map_err(|_| AppError::InvalidInput("category"))?;
        sql.push_str(&format!(" AND {EFFECTIVE_CATEGORY} = ?"));
        params.push(category.into());
    }
    if let Some(status) = &query.status {
        match LedgerStatus::parse(status)? {
            LedgerStatus::All => {}
            LedgerStatus::Pending => sql.push_str(" AND t.pending = 1"),
            LedgerStatus::Posted => sql.push_str(" AND t.pending = 0"),
        }
    }
    if let Some(tx_type) = &query.tx_type {
        let tx_type = TxTypeFilter::parse(tx_type)?;
        sql.push_str(&format!(" AND {EFFECTIVE_TYPE} = ?"));
        params.push(tx_type.as_str().to_string().into());
    }
    if let Some(search) = &query.search {
        let search = clean_text(search, MAX_SEARCH_CHARS, false)
            .map_err(|_| AppError::InvalidInput("search"))?;
        let pattern = format!("%{}%", escape_like(&search.to_lowercase()));
        sql.push_str(&format!(
            " AND (lower({EFFECTIVE_MERCHANT}) LIKE ? ESCAPE '\\' OR lower(t.raw_description) LIKE ? ESCAPE '\\')"
        ));
        params.push(pattern.clone().into());
        params.push(pattern.into());
    }
    Ok(Filters { where_sql: sql, params })
}

/// All matching rows (bounded) for CSV export. Filters are identical to the
/// paginated ledger query; sort identifiers remain closed enums.
pub fn query_ledger_export(
    conn: &Connection,
    query: &LedgerQuery,
    limit: u32,
) -> AppResult<Vec<LedgerRow>> {
    let filters = build_filters(query)?;
    let sort = match &query.sort {
        Some(value) => LedgerSort::parse(value)?,
        None => LedgerSort::DateDesc,
    };
    let sql = format!(
        "SELECT t.id, t.reporting_date, {EFFECTIVE_MERCHANT}, \
            o.merchant_alias IS NOT NULL, t.raw_description, t.account_id, \
            a.display_name, a.last_four, \
            NULLIF({EFFECTIVE_CATEGORY}, 'Uncategorized'), o.category IS NOT NULL OR t.category IS NOT NULL, \
            {EFFECTIVE_TYPE}, t.pending, t.amount_minor, t.currency, t.source_kind, \
            t.authorized_date, t.posted_date, t.reporting_date_source, \
            {EFFECTIVE_EXCLUDED}, o.notes{} ORDER BY {} LIMIT ?",
        filters.where_sql,
        sort.order_by()
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut params = filters.params;
    params.push(i64::from(limit).into());
    let rows = stmt
        .query_map(params_from_iter(params), |row| {
            Ok(LedgerRow {
                id: row.get(0)?,
                reporting_date: row.get(1)?,
                merchant: row.get(2)?,
                merchant_edited: row.get(3)?,
                raw_description: row.get(4)?,
                account_id: row.get(5)?,
                account_name: row.get(6)?,
                account_last_four: row.get(7)?,
                category: row.get(8)?,
                category_edited: row.get(9)?,
                tx_type: row.get(10)?,
                pending: row.get::<_, i64>(11)? != 0,
                amount_minor: row.get(12)?,
                currency: row.get(13)?,
                source_kind: row.get(14)?,
                authorized_date: row.get(15)?,
                posted_date: row.get(16)?,
                reporting_date_source: row.get(17)?,
                excluded: row.get::<_, i64>(18)? != 0,
                notes: row.get(19)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn query_ledger(conn: &Connection, query: &LedgerQuery) -> AppResult<LedgerPage> {
    let filters = build_filters(query)?;
    let page = query.page.unwrap_or(0);
    let page_size = query.page_size.unwrap_or(50);
    let sort = match &query.sort {
        Some(value) => LedgerSort::parse(value)?,
        None => LedgerSort::DateDesc,
    };

    let row_sql = format!(
        "SELECT t.id, t.reporting_date, {EFFECTIVE_MERCHANT}, \
            o.merchant_alias IS NOT NULL, t.raw_description, t.account_id, \
            a.display_name, a.last_four, \
            NULLIF({EFFECTIVE_CATEGORY}, 'Uncategorized'), o.category IS NOT NULL OR t.category IS NOT NULL, \
            {EFFECTIVE_TYPE}, t.pending, t.amount_minor, t.currency, t.source_kind, \
            t.authorized_date, t.posted_date, t.reporting_date_source, \
            {EFFECTIVE_EXCLUDED}, o.notes{} ORDER BY {} LIMIT ? OFFSET ?",
        filters.where_sql,
        sort.order_by()
    );
    let mut stmt = conn.prepare(&row_sql)?;
    let mut params = filters.params.clone();
    params.push(i64::from(page_size).into());
    params.push(i64::from(page.saturating_mul(page_size)).into());
    let rows = stmt
        .query_map(params_from_iter(params), |row| {
            Ok(LedgerRow {
                id: row.get(0)?,
                reporting_date: row.get(1)?,
                merchant: row.get(2)?,
                merchant_edited: row.get(3)?,
                raw_description: row.get(4)?,
                account_id: row.get(5)?,
                account_name: row.get(6)?,
                account_last_four: row.get(7)?,
                category: row.get(8)?,
                category_edited: row.get(9)?,
                tx_type: row.get(10)?,
                pending: row.get::<_, i64>(11)? != 0,
                amount_minor: row.get(12)?,
                currency: row.get(13)?,
                source_kind: row.get(14)?,
                authorized_date: row.get(15)?,
                posted_date: row.get(16)?,
                reporting_date_source: row.get(17)?,
                excluded: row.get::<_, i64>(18)? != 0,
                notes: row.get(19)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let count_sql = format!("SELECT count(*), \
        COALESCE(SUM(CASE WHEN t.pending = 0 AND {EFFECTIVE_EXCLUDED} = 0 AND {EFFECTIVE_TYPE} IN {SPEND_TYPES} THEN t.amount_minor ELSE 0 END), 0), \
        COALESCE(SUM(CASE WHEN t.pending = 1 AND {EFFECTIVE_EXCLUDED} = 0 AND {EFFECTIVE_TYPE} IN {SPEND_TYPES} THEN t.amount_minor ELSE 0 END), 0){}", filters.where_sql);
    let (row_count, eligible_subtotal_minor, pending_subtotal_minor): (i64, i64, i64) = conn.query_row(
        &count_sql,
        params_from_iter(filters.params),
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;

    Ok(LedgerPage {
        rows,
        row_count,
        page,
        page_size,
        eligible_subtotal_minor,
        pending_subtotal_minor,
    })
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountSpend {
    pub account_id: String,
    pub currency: String,
    pub net_minor: i64,
    pub count: i64,
}

/// Posted eligible spending per account and currency for the selected month.
pub fn account_spend(conn: &Connection, month: Month) -> AppResult<Vec<AccountSpend>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT t.account_id, t.currency, COALESCE(SUM(t.amount_minor), 0), COUNT(*) \
         FROM transactions t LEFT JOIN transaction_overrides o ON o.transaction_id = t.id \
         WHERE t.removed = 0 AND t.pending = 0 AND {EFFECTIVE_EXCLUDED} = 0 \
           AND {EFFECTIVE_TYPE} IN {SPEND_TYPES} \
           AND t.reporting_date >= ?1 AND t.reporting_date < ?2 \
         GROUP BY t.account_id, t.currency"
    ))?;
    let rows = stmt
        .query_map(
            rusqlite::params![month.start_string(), month.end_exclusive_string()],
            |row| {
                Ok(AccountSpend {
                    account_id: row.get(0)?,
                    currency: row.get(1)?,
                    net_minor: row.get(2)?,
                    count: row.get(3)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Breakdown {
    pub key: String,
    pub gross_minor: i64,
    pub refunds_minor: i64,
    pub net_minor: i64,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Reconciliation {
    pub eligible_net_minor: i64,
    pub supplier_net_minor: i64,
    pub category_net_minor: i64,
    pub exact: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MonthSummary {
    pub month: String,
    pub start_date: String,
    pub end_date_exclusive: String,
    pub currency: String,
    pub gross_purchases_minor: i64,
    /// Refund amounts are negative, matching their ledger sign.
    pub refunds_minor: i64,
    pub fees_interest_minor: i64,
    pub net_spending_minor: i64,
    pub pending_subtotal_minor: i64,
    pub eligible_count: i64,
    pub pending_count: i64,
    pub row_count: i64,
    pub other_currency_row_count: i64,
    pub suppliers: Vec<Breakdown>,
    pub categories: Vec<Breakdown>,
    pub reconciliation: Reconciliation,
}

fn breakdown_query(
    conn: &Connection,
    month: Month,
    field: &str,
    currency: &str,
) -> AppResult<Vec<Breakdown>> {
    // `field` is a static SQL expression chosen below, never user input.
    let sql = format!(
        "SELECT {field} AS key, \
            COALESCE(SUM(CASE WHEN t.amount_minor > 0 THEN t.amount_minor ELSE 0 END), 0), \
            COALESCE(SUM(CASE WHEN t.amount_minor < 0 THEN t.amount_minor ELSE 0 END), 0), \
            COALESCE(SUM(t.amount_minor), 0), COUNT(*) \
         FROM transactions t LEFT JOIN transaction_overrides o ON o.transaction_id = t.id \
         WHERE t.removed = 0 AND t.pending = 0 \
           AND {EFFECTIVE_EXCLUDED} = 0 \
           AND {EFFECTIVE_TYPE} IN {SPEND_TYPES} \
           AND t.reporting_date >= ?1 AND t.reporting_date < ?2 AND t.currency = ?3 \
         GROUP BY key ORDER BY SUM(t.amount_minor) DESC, key ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(
            rusqlite::params![month.start_string(), month.end_exclusive_string(), currency],
            |row| {
                Ok(Breakdown {
                    key: row.get(0)?,
                    gross_minor: row.get(1)?,
                    refunds_minor: row.get(2)?,
                    net_minor: row.get(3)?,
                    count: row.get(4)?,
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn month_summary(conn: &Connection, month: Month, currency: &str) -> AppResult<MonthSummary> {
    let (start, end) = (month.start_string(), month.end_exclusive_string());
    let (gross, refunds, fees, net, eligible_count): (i64, i64, i64, i64, i64) = conn.query_row(
        &format!(
            "SELECT \
                COALESCE(SUM(CASE WHEN {EFFECTIVE_TYPE} = 'purchase' THEN t.amount_minor ELSE 0 END), 0), \
                COALESCE(SUM(CASE WHEN {EFFECTIVE_TYPE} = 'refund' THEN t.amount_minor ELSE 0 END), 0), \
                COALESCE(SUM(CASE WHEN {EFFECTIVE_TYPE} IN ('fee','interest') THEN t.amount_minor ELSE 0 END), 0), \
                COALESCE(SUM(CASE WHEN {EFFECTIVE_TYPE} IN {SPEND_TYPES} THEN t.amount_minor ELSE 0 END), 0), \
                COALESCE(SUM(CASE WHEN {EFFECTIVE_TYPE} IN {SPEND_TYPES} THEN 1 ELSE 0 END), 0) \
             FROM transactions t LEFT JOIN transaction_overrides o ON o.transaction_id = t.id \
             WHERE t.removed = 0 AND t.pending = 0 AND {EFFECTIVE_EXCLUDED} = 0 \
               AND t.reporting_date >= ?1 AND t.reporting_date < ?2 AND t.currency = ?3"
        ),
        rusqlite::params![start, end, currency],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        },
    )?;

    let pending_subtotal: i64 = conn.query_row(
        &format!(
            "SELECT COALESCE(SUM(t.amount_minor), 0) \
             FROM transactions t LEFT JOIN transaction_overrides o ON o.transaction_id = t.id \
             WHERE t.removed = 0 AND t.pending = 1 AND {EFFECTIVE_EXCLUDED} = 0 \
               AND {EFFECTIVE_TYPE} IN {SPEND_TYPES} \
               AND t.reporting_date >= ?1 AND t.reporting_date < ?2 AND t.currency = ?3"
        ),
        rusqlite::params![start, end, currency],
        |row| row.get(0),
    )?;
    let (pending_count, row_count, other_currency_row_count): (i64, i64, i64) = conn.query_row(
        "SELECT \
            COALESCE(SUM(CASE WHEN t.pending = 1 THEN 1 ELSE 0 END), 0), \
            COUNT(*), \
            COALESCE(SUM(CASE WHEN t.currency <> ?3 THEN 1 ELSE 0 END), 0) \
         FROM transactions t WHERE t.removed = 0 \
           AND t.reporting_date >= ?1 AND t.reporting_date < ?2",
        rusqlite::params![start, end, currency],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;

    let suppliers = breakdown_query(conn, month, EFFECTIVE_MERCHANT, currency)?;
    let categories = breakdown_query(conn, month, EFFECTIVE_CATEGORY, currency)?;
    let supplier_net: i64 = suppliers.iter().map(|item| item.net_minor).sum();
    let category_net: i64 = categories.iter().map(|item| item.net_minor).sum();
    let exact = supplier_net == net && category_net == net;

    Ok(MonthSummary {
        month: month.key(),
        start_date: start,
        end_date_exclusive: end,
        currency: currency.to_string(),
        gross_purchases_minor: gross,
        refunds_minor: refunds,
        fees_interest_minor: fees,
        net_spending_minor: net,
        pending_subtotal_minor: pending_subtotal,
        eligible_count,
        pending_count,
        row_count,
        other_currency_row_count,
        suppliers,
        categories,
        reconciliation: Reconciliation {
            eligible_net_minor: net,
            supplier_net_minor: supplier_net,
            category_net_minor: category_net,
            exact,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_reject_out_of_bounds_and_injection_shaped_sort() {
        let base = |sort: &str| LedgerQuery {
            start_date: "2026-10-01".into(),
            end_date_exclusive: "2026-11-01".into(),
            account_ids: vec![],
            category: None,
            status: None,
            tx_type: None,
            search: None,
            sort: Some(sort.into()),
            page: None,
            page_size: None,
        };
        assert!(build_filters(&base("date_desc")).is_ok());
        assert!(LedgerSort::parse("amount_minor; DROP TABLE accounts").is_err());
        assert!(LedgerSort::parse("date_desc").is_ok());
        let mut q = base("date_desc");
        q.page_size = Some(201);
        assert!(build_filters(&q).is_err());
        q.page_size = Some(50);
        q.start_date = "2020-01-01".into();
        assert!(build_filters(&q).is_err());
        q.start_date = "2026-10-01".into();
        q.account_ids = vec!["not-a-uuid".into()];
        assert!(build_filters(&q).is_err());
    }

    #[test]
    fn sort_order_fragments_are_static() {
        for sort in [
            LedgerSort::DateDesc,
            LedgerSort::DateAsc,
            LedgerSort::AmountDesc,
            LedgerSort::AmountAsc,
            LedgerSort::MerchantAsc,
        ] {
            let fragment = sort.order_by();
            assert!(!fragment.contains(';'));
            assert!(!fragment.contains("--"));
        }
    }
}
