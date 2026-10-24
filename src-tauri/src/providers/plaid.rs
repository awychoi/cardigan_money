//! Plaid Sandbox/Production HTTP adapter. The renderer never sees tokens; all
//! requests run in Rust through the allowlisted, bounded network policy.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::providers::network::{read_bounded, NetworkPolicy, MAX_RESPONSE_BYTES};
use crate::providers::{
    Capability, LiabilitiesResult, ProviderAccount, ProviderBalance, ProviderCapabilities,
    ProviderClient, ProviderError, ProviderErrorKind, ProviderLiability, ProviderResult,
    ProviderTransaction, TransactionsPage,
};
use crate::security::crypto::SecretString;
use crate::security::keychain::{CredentialStore, ACCOUNT_CLIENT_ID, ACCOUNT_SECRET};

pub const ACCESS_TOKEN_PREFIX: &str = "plaid-access-token-";
pub const SANDBOX_BASE: &str = "https://sandbox.plaid.com";
pub const PRODUCTION_BASE: &str = "https://production.plaid.com";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaidEnvironment {
    Sandbox,
    Production,
}

impl PlaidEnvironment {
    pub fn base_url(self) -> &'static str {
        match self {
            Self::Sandbox => SANDBOX_BASE,
            Self::Production => PRODUCTION_BASE,
        }
    }
}

pub struct PlaidCredentials {
    pub client_id: SecretString,
    pub secret: SecretString,
    pub environment: PlaidEnvironment,
}

pub fn load_sandbox_credentials(
    store: &dyn CredentialStore,
) -> AppResult<Option<PlaidCredentials>> {
    let Some(client_id) = store.get(ACCOUNT_CLIENT_ID)? else {
        return Ok(None);
    };
    let Some(secret) = store.get(ACCOUNT_SECRET)? else {
        return Ok(None);
    };
    Ok(Some(PlaidCredentials {
        client_id,
        secret,
        environment: PlaidEnvironment::Sandbox,
    }))
}

pub fn save_sandbox_credentials(
    store: &dyn CredentialStore,
    client_id: SecretString,
    secret: SecretString,
) -> AppResult<()> {
    validate_client_id(&client_id)?;
    if secret.expose().len() < 8 || secret.expose().len() > 256 {
        return Err(AppError::InvalidInput("Plaid Sandbox secret length"));
    }
    store.set(ACCOUNT_CLIENT_ID, &client_id)?;
    store.set(ACCOUNT_SECRET, &secret)?;
    Ok(())
}

fn validate_client_id(client_id: &SecretString) -> AppResult<()> {
    let value = client_id.expose();
    if value.len() < 8
        || value.len() > 128
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(AppError::InvalidInput("Plaid client ID"));
    }
    Ok(())
}

pub fn access_token_account(credential_ref: &str) -> String {
    format!("{ACCESS_TOKEN_PREFIX}{credential_ref}")
}

pub fn store_access_token(
    store: &dyn CredentialStore,
    credential_ref: &str,
    token: SecretString,
) -> AppResult<()> {
    if token.expose().len() < 20 {
        return Err(AppError::InvalidInput("Plaid access token"));
    }
    store.set(&access_token_account(credential_ref), &token)
}

pub fn load_access_token(
    store: &dyn CredentialStore,
    credential_ref: &str,
) -> AppResult<Option<SecretString>> {
    store.get(&access_token_account(credential_ref))
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinkTokenResponse {
    pub link_token: String,
    pub expiration: String,
    pub hosted_link_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinkTokenGetResponse {
    #[serde(default)]
    pub link_sessions: Vec<LinkSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinkSession {
    pub link_session_id: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub results: Option<LinkResults>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinkResults {
    #[serde(default)]
    pub item_add_results: Vec<ItemAddResult>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItemAddResult {
    pub public_token: String,
    pub institution: Option<Institution>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Institution {
    pub institution_id: Option<String>,
    pub name: Option<String>,
}

pub struct LinkedItem {
    pub public_token: SecretString,
    pub institution_id: Option<String>,
    pub institution_name: Option<String>,
}

impl LinkTokenGetResponse {
    pub fn first_linked_item(&self) -> Option<LinkedItem> {
        for session in &self.link_sessions {
            let Some(results) = session.results.as_ref() else {
                continue;
            };
            for item in results.item_add_results.iter() {
                if !item.public_token.is_empty() {
                    return Some(LinkedItem {
                        public_token: SecretString::new(item.public_token.clone()),
                        institution_id: item
                            .institution
                            .as_ref()
                            .and_then(|value| value.institution_id.clone()),
                        institution_name: item
                            .institution
                            .as_ref()
                            .and_then(|value| value.name.clone()),
                    });
                }
            }
        }
        None
    }

    pub fn session_finished(&self) -> bool {
        self.link_sessions
            .iter()
            .any(|session| session.finished_at.is_some())
    }

    pub fn session_finished_without_item(&self) -> bool {
        self.link_sessions.iter().any(|session| {
            session.finished_at.is_some()
                && session
                    .results
                    .as_ref()
                    .map(|results| results.item_add_results.is_empty())
                    .unwrap_or(true)
        })
    }
}

pub struct PlaidClient {
    client: reqwest::blocking::Client,
    policy: NetworkPolicy,
    base_url: &'static str,
    client_id: SecretString,
    secret: SecretString,
    access_token: Option<SecretString>,
}

impl PlaidClient {
    pub fn new(
        credentials: PlaidCredentials,
        access_token: Option<SecretString>,
    ) -> AppResult<Self> {
        let policy = NetworkPolicy::default();
        let client = policy.client()?;
        Ok(Self {
            client,
            policy,
            base_url: credentials.environment.base_url(),
            client_id: credentials.client_id,
            secret: credentials.secret,
            access_token,
        })
    }

    fn post<T: for<'de> Deserialize<'de>>(&self, path: &str, mut body: Value) -> ProviderResult<T> {
        let url = self
            .policy
            .validate_url(&format!("{}{}", self.base_url, path))
            .map_err(|_| {
                ProviderError::new(ProviderErrorKind::Permanent, "provider URL rejected")
            })?;
        let object = body
            .as_object_mut()
            .ok_or_else(|| ProviderError::new(ProviderErrorKind::Permanent, "provider body"))?;
        object.insert(
            "client_id".into(),
            Value::String(self.client_id.expose().into()),
        );
        object.insert("secret".into(), Value::String(self.secret.expose().into()));
        if let Some(token) = &self.access_token {
            object.insert("access_token".into(), Value::String(token.expose().into()));
        }
        let response = self.client.post(url).json(&body).send().map_err(|error| {
            let kind = if error.is_timeout() {
                ProviderErrorKind::Transient
            } else {
                ProviderErrorKind::Network
            };
            ProviderError::new(kind, "Plaid could not be reached.")
        })?;
        let status = response.status();
        let declared = response.content_length().unwrap_or(0);
        if declared > MAX_RESPONSE_BYTES {
            return Err(ProviderError::new(
                ProviderErrorKind::Permanent,
                "Plaid returned an oversized response.",
            ));
        }
        let mut response = response;
        let bytes = read_bounded(&mut response, MAX_RESPONSE_BYTES).map_err(|_| {
            ProviderError::new(ProviderErrorKind::Network, "Plaid response failed.")
        })?;
        if !status.is_success() {
            let code = serde_json::from_slice::<PlaidErrorResponse>(&bytes)
                .ok()
                .map(|error| error.error_code);
            return Err(map_plaid_error(code.as_deref()));
        }
        serde_json::from_slice(&bytes).map_err(|_| {
            ProviderError::new(
                ProviderErrorKind::Permanent,
                "Plaid returned an invalid response.",
            )
        })
    }

    pub fn create_hosted_link_token(
        &self,
        client_user_id: &str,
    ) -> ProviderResult<LinkTokenResponse> {
        self.post(
            "/link/token/create",
            json!({
                "client_name": "cardigan_money",
                "language": "en",
                "country_codes": ["US"],
                "user": { "client_user_id": client_user_id },
                "products": ["transactions"],
                "optional_products": ["liabilities"],
                "hosted_link": {}
            }),
        )
    }

    pub fn create_update_link_token(
        &self,
        client_user_id: &str,
    ) -> ProviderResult<LinkTokenResponse> {
        self.post(
            "/link/token/create",
            json!({
                "client_name": "cardigan_money",
                "language": "en",
                "country_codes": ["US"],
                "user": { "client_user_id": client_user_id },
                "hosted_link": {}
            }),
        )
    }

    pub fn link_token_get(
        &self,
        link_token: &SecretString,
    ) -> ProviderResult<LinkTokenGetResponse> {
        self.post(
            "/link/token/get",
            json!({ "link_token": link_token.expose() }),
        )
    }

    pub fn exchange_public_token(
        &self,
        public_token: &SecretString,
    ) -> ProviderResult<ExchangeResponse> {
        self.post(
            "/item/public_token/exchange",
            json!({ "public_token": public_token.expose() }),
        )
    }

    pub fn accounts(&self) -> ProviderResult<AccountsResponse> {
        self.post("/accounts/get", json!({}))
    }

    pub fn balances(&self) -> ProviderResult<AccountsResponse> {
        // `/accounts/get` returns cached balances and does not require a separate
        // Balance product beyond Transactions. `/accounts/balance/get` is the
        // real-time Balance product and is intentionally not required here.
        self.post("/accounts/get", json!({}))
    }

    pub fn transactions_sync(
        &self,
        cursor: Option<&str>,
    ) -> ProviderResult<TransactionsSyncResponse> {
        let mut body = json!({ "count": 500 });
        if let Some(cursor) = cursor {
            body["cursor"] = Value::String(cursor.to_string());
        }
        self.post("/transactions/sync", body)
    }

    pub fn liabilities(&self) -> ProviderResult<LiabilitiesResponse> {
        self.post("/liabilities/get", json!({}))
    }

    pub fn item_get(&self) -> ProviderResult<ItemResponse> {
        self.post("/item/get", json!({}))
    }

    pub fn item_remove(&self) -> ProviderResult<()> {
        let _: Value = self.post("/item/remove", json!({}))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeResponse {
    pub access_token: String,
    pub item_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountsResponse {
    pub accounts: Vec<PlaidAccount>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaidAccount {
    pub account_id: String,
    pub name: Option<String>,
    pub official_name: Option<String>,
    pub mask: Option<String>,
    #[serde(rename = "type")]
    pub account_type: Option<String>,
    pub subtype: Option<String>,
    pub balances: PlaidBalances,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaidBalances {
    pub current: Option<f64>,
    pub available: Option<f64>,
    pub limit: Option<f64>,
    pub iso_currency_code: Option<String>,
    pub unofficial_currency_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransactionsSyncResponse {
    #[serde(default)]
    pub added: Vec<PlaidTransaction>,
    #[serde(default)]
    pub modified: Vec<PlaidTransaction>,
    #[serde(default)]
    pub removed: Vec<RemovedTransaction>,
    pub next_cursor: String,
    pub has_more: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemovedTransaction {
    pub transaction_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaidTransaction {
    pub transaction_id: String,
    pub account_id: String,
    pub amount: f64,
    pub iso_currency_code: Option<String>,
    pub name: String,
    pub merchant_name: Option<String>,
    pub authorized_date: Option<String>,
    pub date: Option<String>,
    pub pending: bool,
    pub pending_transaction_id: Option<String>,
    pub personal_finance_category: Option<PersonalFinanceCategory>,
    pub category: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PersonalFinanceCategory {
    pub primary: Option<String>,
    pub detailed: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LiabilitiesResponse {
    pub liabilities: Liabilities,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Liabilities {
    pub credit: Option<Vec<CreditLiability>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreditLiability {
    pub account_id: String,
    pub next_payment_due_date: Option<String>,
    pub minimum_payment_amount: Option<f64>,
    pub last_statement_balance: Option<f64>,
    pub last_statement_issue_date: Option<String>,
    pub next_payment_due_date_metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItemResponse {
    pub item: ItemInfo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItemInfo {
    pub item_id: String,
    pub institution_id: Option<String>,
    pub consent_expiration_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct PlaidErrorResponse {
    error_code: String,
}

fn map_plaid_error(code: Option<&str>) -> ProviderError {
    match code.unwrap_or("") {
        "ITEM_LOGIN_REQUIRED"
        | "INVALID_ACCESS_TOKEN"
        | "ITEM_NOT_FOUND"
        | "ACCESS_NOT_GRANTED" => ProviderError::new(
            ProviderErrorKind::ReauthRequired,
            "The bank connection needs to be reconnected.",
        ),
        "RATE_LIMIT_EXCEEDED" => ProviderError::new(
            ProviderErrorKind::RateLimited,
            "Plaid rate-limited this request.",
        ),
        "PRODUCT_NOT_READY"
        | "PRODUCTS_NOT_SUPPORTED"
        | "NO_LIABILITIES"
        | "PRODUCT_NOT_ENABLED" => ProviderError::new(
            ProviderErrorKind::Unsupported,
            "This Plaid product is unavailable for the Item.",
        ),
        "INSTITUTION_NOT_RESPONDING"
        | "INTERNAL_SERVER_ERROR"
        | "PLANNED_MAINTENANCE"
        | "INSTITUTION_DOWN" => ProviderError::new(
            ProviderErrorKind::Transient,
            "Plaid is temporarily unavailable.",
        ),
        "INVALID_LINK_TOKEN" | "INVALID_PUBLIC_TOKEN" | "LINK_TOKEN_EXPIRED" => ProviderError::new(
            ProviderErrorKind::Permanent,
            "The linking session expired or was already used.",
        ),
        "INVALID_API_KEYS" | "INVALID_CLIENT_ID" | "INVALID_SECRET" | "UNAUTHORIZED" => {
            ProviderError::new(
                ProviderErrorKind::Permanent,
                "Plaid rejected the configured credentials.",
            )
        }
        _ => ProviderError::new(ProviderErrorKind::Permanent, "Plaid rejected the request."),
    }
}

fn to_minor(value: f64) -> Option<i64> {
    if !value.is_finite() || value.abs() > 1_000_000_000_000.0 {
        return None;
    }
    Some((value * 100.0).round() as i64)
}

fn currency(iso: Option<String>, unofficial: Option<String>) -> String {
    iso.or(unofficial).unwrap_or_else(|| "USD".into())
}

pub fn map_provider_account(account: &PlaidAccount) -> ProviderAccount {
    let account_type = match (account.account_type.as_deref(), account.subtype.as_deref()) {
        (Some("credit"), Some("credit card")) => "credit_card",
        (Some("credit"), Some("paypal")) => "credit_card",
        (Some("credit"), _) => "line_of_credit",
        (Some("loan"), _) => "other",
        _ => "other",
    };
    ProviderAccount {
        provider_account_id: account.account_id.clone(),
        name: account
            .official_name
            .clone()
            .or_else(|| account.name.clone())
            .unwrap_or_else(|| "Plaid account".into()),
        mask: account.mask.clone().unwrap_or_default(),
        currency: currency(
            account.balances.iso_currency_code.clone(),
            account.balances.unofficial_currency_code.clone(),
        ),
        account_type: account_type.into(),
    }
}

pub fn map_provider_balance(account: &PlaidAccount) -> ProviderBalance {
    ProviderBalance {
        provider_account_id: account.account_id.clone(),
        current_balance_minor: account.balances.current.and_then(to_minor).unwrap_or(0),
        available_credit_minor: account.balances.available.and_then(to_minor),
        credit_limit_minor: account.balances.limit.and_then(to_minor),
        currency: currency(
            account.balances.iso_currency_code.clone(),
            account.balances.unofficial_currency_code.clone(),
        ),
        as_of: None,
    }
}

pub fn map_provider_transaction(tx: &PlaidTransaction) -> ProviderTransaction {
    let amount_minor = to_minor(tx.amount).unwrap_or(0);
    let category = tx
        .personal_finance_category
        .as_ref()
        .and_then(|pfc| pfc.detailed.clone().or_else(|| pfc.primary.clone()))
        .or_else(|| {
            tx.category
                .as_ref()
                .and_then(|values| values.first().cloned())
        });
    let primary = tx
        .personal_finance_category
        .as_ref()
        .and_then(|pfc| pfc.primary.as_deref())
        .unwrap_or("");
    let tx_type = match primary {
        "LOAN_PAYMENTS" => "payment",
        "BANK_FEES" => "fee",
        "TRANSFER_IN" | "TRANSFER_OUT" => "transfer",
        _ if amount_minor < 0 => "refund",
        _ => "purchase",
    };
    ProviderTransaction {
        provider_transaction_id: tx.transaction_id.clone(),
        provider_account_id: tx.account_id.clone(),
        amount_minor,
        currency: currency(tx.iso_currency_code.clone(), None),
        description: tx
            .merchant_name
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| tx.name.clone()),
        authorized_date: tx.authorized_date.clone(),
        posted_date: tx.date.clone(),
        pending: tx.pending,
        removed: false,
        pending_transaction_id: tx.pending_transaction_id.clone(),
        tx_type: tx_type.into(),
        category,
    }
}

pub fn map_provider_liability(row: &CreditLiability) -> ProviderLiability {
    ProviderLiability {
        provider_account_id: row.account_id.clone(),
        next_due_date: row.next_payment_due_date.clone(),
        minimum_due_minor: row.minimum_payment_amount.and_then(to_minor),
        statement_balance_minor: row.last_statement_balance.and_then(to_minor),
        last_statement_issue_date: row.last_statement_issue_date.clone(),
        next_closing_date: None,
    }
}

pub struct PlaidProvider {
    client: PlaidClient,
    capabilities: ProviderCapabilities,
}

impl PlaidProvider {
    pub fn new(client: PlaidClient) -> Self {
        Self {
            client,
            capabilities: ProviderCapabilities {
                transactions: Capability::Supported,
                balances: Capability::Supported,
                liabilities: Capability::Supported,
            },
        }
    }
}

impl ProviderClient for PlaidProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities
    }

    fn accounts(&self) -> ProviderResult<Vec<ProviderAccount>> {
        let response = self.client.accounts()?;
        Ok(response
            .accounts
            .iter()
            .filter(|account| account.account_type.as_deref() == Some("credit"))
            .map(map_provider_account)
            .collect())
    }

    fn transactions_page(&self, cursor: Option<&str>) -> ProviderResult<TransactionsPage> {
        let response = self.client.transactions_sync(cursor)?;
        let mut changes: Vec<ProviderTransaction> = response
            .added
            .iter()
            .chain(response.modified.iter())
            .map(map_provider_transaction)
            .collect();
        changes.extend(response.removed.iter().map(|removed| ProviderTransaction {
            provider_transaction_id: removed.transaction_id.clone(),
            provider_account_id: String::new(),
            amount_minor: 0,
            currency: "USD".into(),
            description: String::new(),
            authorized_date: None,
            posted_date: None,
            pending: false,
            removed: true,
            pending_transaction_id: None,
            tx_type: "other".into(),
            category: None,
        }));
        Ok(TransactionsPage {
            changes,
            next_cursor: Some(response.next_cursor),
            has_more: response.has_more,
        })
    }

    fn balances(&self) -> ProviderResult<Vec<ProviderBalance>> {
        let response = self.client.balances()?;
        Ok(response
            .accounts
            .iter()
            .filter(|account| {
                account.account_type.as_deref() == Some("credit")
                    && account.balances.current.is_some()
            })
            .map(map_provider_balance)
            .collect())
    }

    fn revoke(&self) -> ProviderResult<()> {
        self.client.item_remove()
    }

    fn liabilities(&self) -> ProviderResult<LiabilitiesResult> {
        match self.client.liabilities() {
            Ok(response) => {
                let rows = response
                    .liabilities
                    .credit
                    .as_ref()
                    .map(|credit| credit.iter().map(map_provider_liability).collect())
                    .unwrap_or_default();
                let capability = if response.liabilities.credit.is_some() {
                    Capability::Supported
                } else {
                    Capability::AvailableButNull
                };
                Ok(LiabilitiesResult { capability, rows })
            }
            Err(error) if error.kind == ProviderErrorKind::Unsupported => Ok(LiabilitiesResult {
                capability: Capability::AvailableButNull,
                rows: vec![],
            }),
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_credit_account_and_amounts() {
        let account = PlaidAccount {
            account_id: "acct".into(),
            name: Some("Plaid Credit".into()),
            official_name: None,
            mask: Some("0000".into()),
            account_type: Some("credit".into()),
            subtype: Some("credit card".into()),
            balances: PlaidBalances {
                current: Some(123.45),
                available: Some(876.55),
                limit: Some(1000.0),
                iso_currency_code: Some("USD".into()),
                unofficial_currency_code: None,
            },
        };
        assert_eq!(map_provider_account(&account).account_type, "credit_card");
        let balance = map_provider_balance(&account);
        assert_eq!(balance.current_balance_minor, 12_345);
        assert_eq!(balance.credit_limit_minor, Some(100_000));
    }

    #[test]
    fn maps_transactions_and_categories() {
        let tx = PlaidTransaction {
            transaction_id: "tx".into(),
            account_id: "acct".into(),
            amount: 12.34,
            iso_currency_code: Some("USD".into()),
            name: "Coffee".into(),
            merchant_name: Some("Coffee Shop".into()),
            authorized_date: Some("2026-10-01".into()),
            date: Some("2026-10-02".into()),
            pending: false,
            pending_transaction_id: None,
            personal_finance_category: Some(PersonalFinanceCategory {
                primary: Some("FOOD_AND_DRINK".into()),
                detailed: Some("FOOD_AND_DRINK_COFFEE".into()),
            }),
            category: None,
        };
        let mapped = map_provider_transaction(&tx);
        assert_eq!(mapped.amount_minor, 1_234);
        assert_eq!(mapped.tx_type, "purchase");
        assert_eq!(mapped.description, "Coffee Shop");
        assert_eq!(mapped.category.as_deref(), Some("FOOD_AND_DRINK_COFFEE"));
    }

    #[test]
    fn link_sessions_extract_one_item() {
        let response: LinkTokenGetResponse = serde_json::from_str(
            r#"{"link_sessions":[{"link_session_id":"s","results":{"item_add_results":[{"public_token":"public-sandbox-x","institution":{"institution_id":"ins_1","name":"Sandbox Bank"}}]}}]}"#,
        )
        .unwrap();
        let item = response.first_linked_item().unwrap();
        assert_eq!(item.public_token.expose(), "public-sandbox-x");
        assert_eq!(item.institution_name.as_deref(), Some("Sandbox Bank"));
    }

    #[test]
    fn no_public_token_before_finish() {
        let response: LinkTokenGetResponse =
            serde_json::from_str(r#"{"link_sessions":[{"link_session_id":"s"}]}"#).unwrap();
        assert!(response.first_linked_item().is_none());
        assert!(!response.session_finished_without_item());
    }
}
