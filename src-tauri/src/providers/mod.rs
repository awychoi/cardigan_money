pub mod csv;
pub mod mock;
pub mod network;
pub mod plaid;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Supported,
    TemporarilyUnavailable,
    AvailableButNull,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub transactions: Capability,
    pub balances: Capability,
    pub liabilities: Capability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderAccount {
    pub provider_account_id: String,
    pub name: String,
    pub mask: String,
    pub currency: String,
    pub account_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderTransaction {
    pub provider_transaction_id: String,
    pub provider_account_id: String,
    pub amount_minor: i64,
    pub currency: String,
    pub description: String,
    pub authorized_date: Option<String>,
    pub posted_date: Option<String>,
    pub pending: bool,
    pub removed: bool,
    pub pending_transaction_id: Option<String>,
    pub tx_type: String,
    pub category: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransactionsPage {
    pub changes: Vec<ProviderTransaction>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderBalance {
    pub provider_account_id: String,
    pub current_balance_minor: i64,
    pub available_credit_minor: Option<i64>,
    pub credit_limit_minor: Option<i64>,
    pub currency: String,
    pub as_of: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderLiability {
    pub provider_account_id: String,
    pub next_due_date: Option<String>,
    pub minimum_due_minor: Option<i64>,
    pub statement_balance_minor: Option<i64>,
    pub last_statement_issue_date: Option<String>,
    pub next_closing_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiabilitiesResult {
    pub capability: Capability,
    pub rows: Vec<ProviderLiability>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorKind {
    ReauthRequired,
    Transient,
    Permanent,
    RateLimited,
    Unsupported,
    Network,
}

#[derive(Debug, Clone)]
pub struct ProviderError {
    pub kind: ProviderErrorKind,
    pub safe_message: &'static str,
}

impl ProviderError {
    pub fn new(kind: ProviderErrorKind, safe_message: &'static str) -> Self {
        Self { kind, safe_message }
    }
}

pub type ProviderResult<T> = Result<T, ProviderError>;

pub trait ProviderClient {
    fn capabilities(&self) -> ProviderCapabilities;
    fn accounts(&self) -> ProviderResult<Vec<ProviderAccount>>;
    fn transactions_page(&self, cursor: Option<&str>) -> ProviderResult<TransactionsPage>;
    fn balances(&self) -> ProviderResult<Vec<ProviderBalance>>;
    fn liabilities(&self) -> ProviderResult<LiabilitiesResult>;
    /// Best-effort remote revoke. Never required for local disconnect.
    fn revoke(&self) -> ProviderResult<()> {
        Ok(())
    }
}
