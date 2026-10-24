//! Deterministic mock provider. It exposes the same shapes a real provider
//! adapter will produce (capabilities, accounts, incremental changes, balances)
//! without network access. Phase 8 extends this into the sync engine.

use chrono::NaiveDate;
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
#[serde(rename_all = "camelCase")]
pub struct MockAccount {
    pub provider_account_id: String,
    pub name: String,
    pub mask: String,
    pub currency: String,
    pub account_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MockTransaction {
    pub provider_transaction_id: String,
    pub account_id: String,
    pub authorized_date: String,
    pub posted_date: Option<String>,
    pub amount_minor: i64,
    pub currency: String,
    pub description: String,
    pub pending: bool,
    pub removed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MockBalance {
    pub account_id: String,
    pub current_minor: i64,
    pub available_credit_minor: Option<i64>,
    pub currency: String,
    pub provider_as_of: String,
}

pub struct MockProvider {
    today: NaiveDate,
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new(NaiveDate::from_ymd_opt(2026, 10, 23).expect("valid demo date"))
    }
}

impl MockProvider {
    pub fn new(today: NaiveDate) -> Self {
        Self { today }
    }

    pub fn capabilities(&self, // liabilities intentionally unsupported for one shape
    ) -> ProviderCapabilities {
        ProviderCapabilities {
            transactions: Capability::Supported,
            balances: Capability::Supported,
            liabilities: Capability::Unsupported,
        }
    }

    pub fn accounts(&self) -> Vec<MockAccount> {
        vec![
            MockAccount {
                provider_account_id: "mock-checking-1".into(),
                name: "Demo Everyday".into(),
                mask: "4101".into(),
                currency: "USD".into(),
                account_type: "credit_card".into(),
            },
            MockAccount {
                provider_account_id: "mock-savings-1".into(),
                name: "Demo Travel".into(),
                mask: "5501".into(),
                currency: "USD".into(),
                account_type: "credit_card".into(),
            },
        ]
    }

    pub fn transactions(&self) -> Vec<MockTransaction> {
        let day = self.today.format("%Y-%m-%d").to_string();
        vec![
            MockTransaction {
                provider_transaction_id: "mock-tx-1".into(),
                account_id: "mock-checking-1".into(),
                authorized_date: day.clone(),
                posted_date: Some(day.clone()),
                amount_minor: 2_450,
                currency: "USD".into(),
                description: "Synthetic Coffee".into(),
                pending: false,
                removed: false,
            },
            MockTransaction {
                provider_transaction_id: "mock-tx-2".into(),
                account_id: "mock-savings-1".into(),
                authorized_date: day.clone(),
                posted_date: None,
                amount_minor: 7_500,
                currency: "USD".into(),
                description: "Synthetic Hotel".into(),
                pending: true,
                removed: false,
            },
        ]
    }

    pub fn balances(&self) -> Vec<MockBalance> {
        vec![MockBalance {
            account_id: "mock-checking-1".into(),
            current_minor: 42_000,
            available_credit_minor: Some(458_000),
            currency: "USD".into(),
            provider_as_of: format!("{}T12:00:00Z", self.today.format("%Y-%m-%d")),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_provider_is_deterministic_and_exposes_capabilities() {
        let provider = MockProvider::default();
        let caps = provider.capabilities();
        assert_eq!(caps.transactions, Capability::Supported);
        assert_eq!(caps.balances, Capability::Supported);
        assert_eq!(caps.liabilities, Capability::Unsupported);
        assert_eq!(provider.accounts().len(), 2);
        assert_eq!(provider.transactions().len(), 2);
        assert_eq!(provider.balances().len(), 1);
        assert!(provider.transactions().iter().any(|tx| tx.pending));
        assert_eq!(provider.accounts(), provider.accounts());
    }

    #[test]
    fn provider_never_contains_real_credentials() {
        let json = serde_json::to_string(&MockProvider::default().transactions()).unwrap();
        for forbidden in ["token", "secret", "passphrase", "access_key"] {
            assert!(!json.contains(forbidden));
        }
    }
}
