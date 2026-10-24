//! Deterministic mock provider. It exposes the same shapes a real provider
//! adapter will produce (capabilities, accounts, incremental changes, balances)
//! without network access. Phase 8 extends this into the sync engine.

use chrono::NaiveDate;
use serde::Serialize;

use super::{
    Capability, LiabilitiesResult, ProviderAccount, ProviderBalance, ProviderCapabilities,
    ProviderClient, ProviderError, ProviderErrorKind, ProviderLiability, ProviderResult,
    ProviderTransaction, TransactionsPage,
};

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
    fail_cursor: Option<String>,
    liabilities: Capability,
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new(NaiveDate::from_ymd_opt(2026, 10, 23).expect("valid demo date"))
    }
}

impl MockProvider {
    pub fn new(today: NaiveDate) -> Self {
        Self { today, fail_cursor: None, liabilities: Capability::Unsupported }
    }

    pub fn with_failure(today: NaiveDate, cursor: &str) -> Self {
        Self { today, fail_cursor: Some(cursor.to_string()), liabilities: Capability::Unsupported }
    }

    pub fn with_liabilities(today: NaiveDate, capability: Capability) -> Self {
        Self { today, fail_cursor: None, liabilities: capability }
    }

    fn mock_accounts(&self) -> Vec<ProviderAccount> {
        vec![
            ProviderAccount {
                provider_account_id: "mock-checking-1".into(),
                name: "Demo Everyday".into(),
                mask: "4101".into(),
                currency: "USD".into(),
                account_type: "credit_card".into(),
            },
            ProviderAccount {
                provider_account_id: "mock-savings-1".into(),
                name: "Demo Travel".into(),
                mask: "5501".into(),
                currency: "USD".into(),
                account_type: "credit_card".into(),
            },
        ]
    }

    fn mock_transactions(&self) -> Vec<ProviderTransaction> {
        let day = self.today.format("%Y-%m-%d").to_string();
        vec![
            ProviderTransaction {
                provider_transaction_id: "mock-tx-1".into(),
                provider_account_id: "mock-checking-1".into(),
                amount_minor: 2_450,
                currency: "USD".into(),
                description: "Synthetic Coffee".into(),
                authorized_date: Some(day.clone()),
                posted_date: Some(day.clone()),
                pending: false,
                removed: false,
                pending_transaction_id: None,
                tx_type: "purchase".into(),
                category: Some("Dining".into()),
            },
            ProviderTransaction {
                provider_transaction_id: "mock-tx-2".into(),
                provider_account_id: "mock-savings-1".into(),
                amount_minor: 7_500,
                currency: "USD".into(),
                description: "Synthetic Hotel".into(),
                authorized_date: Some(day.clone()),
                posted_date: None,
                pending: true,
                removed: false,
                pending_transaction_id: None,
                tx_type: "purchase".into(),
                category: Some("Travel".into()),
            },
            ProviderTransaction {
                provider_transaction_id: "mock-tx-3".into(),
                provider_account_id: "mock-checking-1".into(),
                amount_minor: 1_200,
                currency: "USD".into(),
                description: "Synthetic Refund".into(),
                authorized_date: Some(day.clone()),
                posted_date: Some(day.clone()),
                pending: false,
                removed: false,
                pending_transaction_id: None,
                tx_type: "refund".into(),
                category: Some("Dining".into()),
            },
        ]
    }

    fn page(&self, cursor: Option<&str>) -> ProviderResult<TransactionsPage> {
        if let Some(fail) = &self.fail_cursor {
            if cursor == Some(fail.as_str()) {
                return Err(ProviderError::new(ProviderErrorKind::Transient, "provider temporarily unavailable"));
            }
        }
        let all = self.mock_transactions();
        match cursor {
            None => Ok(TransactionsPage {
                changes: vec![all[0].clone()],
                next_cursor: Some("page-2".into()),
                has_more: true,
            }),
            Some("page-2") => Ok(TransactionsPage {
                changes: vec![all[1].clone(), all[2].clone()],
                next_cursor: Some("page-3".into()),
                has_more: true,
            }),
            Some("page-3") => Ok(TransactionsPage {
                changes: vec![],
                next_cursor: Some("done".into()),
                has_more: false,
            }),
            Some("done") => Ok(TransactionsPage {
                changes: vec![],
                next_cursor: Some("done".into()),
                has_more: false,
            }),
            _ => Err(ProviderError::new(ProviderErrorKind::Permanent, "invalid cursor")),
        }
    }
}

impl ProviderClient for MockProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            transactions: Capability::Supported,
            balances: Capability::Supported,
            liabilities: self.liabilities,
        }
    }

    fn accounts(&self) -> ProviderResult<Vec<ProviderAccount>> {
        Ok(self.mock_accounts())
    }

    fn transactions_page(&self, cursor: Option<&str>) -> ProviderResult<TransactionsPage> {
        self.page(cursor)
    }

    fn balances(&self) -> ProviderResult<Vec<ProviderBalance>> {
        Ok(vec![
            ProviderBalance {
                provider_account_id: "mock-checking-1".into(),
                current_balance_minor: 42_000,
                available_credit_minor: Some(458_000),
                credit_limit_minor: Some(500_000),
                currency: "USD".into(),
                as_of: Some(format!("{}T12:00:00Z", self.today.format("%Y-%m-%d"))),
            },
            ProviderBalance {
                provider_account_id: "mock-savings-1".into(),
                current_balance_minor: 75_000,
                available_credit_minor: None,
                credit_limit_minor: None,
                currency: "USD".into(),
                as_of: None,
            },
        ])
    }

    fn liabilities(&self) -> ProviderResult<LiabilitiesResult> {
        if self.liabilities == Capability::Unsupported {
            return Ok(LiabilitiesResult {
                capability: Capability::Unsupported,
                rows: vec![],
            });
        }
        Ok(LiabilitiesResult {
            capability: self.liabilities,
            rows: vec![ProviderLiability {
                provider_account_id: "mock-checking-1".into(),
                next_due_date: Some("2026-10-30".into()),
                minimum_due_minor: Some(3_000),
                statement_balance_minor: Some(42_000),
                last_statement_issue_date: Some("2026-10-05".into()),
                next_closing_date: None,
            }],
        })
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
        assert_eq!(provider.accounts().unwrap().len(), 2);
        assert_eq!(provider.transactions_page(None).unwrap().changes.len(), 1);
        assert_eq!(provider.balances().unwrap().len(), 2);
        assert!(provider.transactions_page(Some("page-2")).unwrap().changes.iter().any(|tx| tx.pending));
        assert_eq!(provider.accounts().unwrap(), provider.accounts().unwrap());
    }

    #[test]
    fn provider_never_contains_real_credentials() {
        let json = serde_json::to_string(&MockProvider::default().transactions_page(None).unwrap().changes).unwrap();
        for forbidden in ["token", "secret", "passphrase", "access_key"] {
            assert!(!json.contains(forbidden));
        }
    }
}
