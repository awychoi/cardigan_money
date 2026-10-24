//! Rust-only network policy for provider egress.
//!
//! HTTPS-only, exact host allowlist, no redirects, TLS verification, timeouts,
//! and bounded response bodies. The renderer has no network source at all.

use std::io::Read;
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::redirect::Policy;
use reqwest::Url;

use crate::error::{AppError, AppResult};

pub const MAX_RESPONSE_BYTES: u64 = 4 * 1024 * 1024;
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const TOTAL_TIMEOUT: Duration = Duration::from_secs(30);
pub const ALLOWED_HOSTS: &[&str] = &["sandbox.plaid.com", "production.plaid.com"];

#[derive(Debug, Clone)]
pub struct NetworkPolicy {
    allowed_hosts: Vec<&'static str>,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self { allowed_hosts: ALLOWED_HOSTS.to_vec() }
    }
}

impl NetworkPolicy {
    pub fn validate_url(&self, value: &str) -> AppResult<Url> {
        let url = Url::parse(value).map_err(|_| AppError::Blocked("invalid provider URL"))?;
        if url.scheme() != "https" {
            return Err(AppError::Blocked("provider requests must use HTTPS"));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(AppError::Blocked("provider URL credentials"));
        }
        if url.fragment().is_some() {
            return Err(AppError::Blocked("provider URL fragment"));
        }
        if url.port().is_some() && url.port() != Some(443) {
            return Err(AppError::Blocked("provider URL port"));
        }
        let host = url.host_str().ok_or(AppError::Blocked("provider URL host"))?;
        if !self.allowed_hosts.iter().any(|allowed| *allowed == host) {
            return Err(AppError::Blocked("provider host is not allowlisted"));
        }
        Ok(url)
    }

    pub fn client(&self) -> AppResult<Client> {
        Client::builder()
            .https_only(true)
            .redirect(Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(TOTAL_TIMEOUT)
            .user_agent("cardigan_money/0.1 (local desktop app)")
            .build()
            .map_err(|_| AppError::Internal)
    }
}

/// Read a response body with a hard cap; rejects declared/oversized bodies.
pub fn read_bounded<R: Read>(reader: &mut R, max_bytes: u64) -> AppResult<Vec<u8>> {
    let mut out = Vec::new();
    reader
        .take(max_bytes + 1)
        .read_to_end(&mut out)
        .map_err(|_| AppError::Blocked("provider response read failed"))?;
    if out.len() as u64 > max_bytes {
        return Err(AppError::Blocked("provider response too large"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_is_exact_and_https_only() {
        let policy = NetworkPolicy::default();
        assert!(policy.validate_url("https://sandbox.plaid.com/link/token/create").is_ok());
        assert!(policy.validate_url("https://production.plaid.com/item/get").is_ok());
        for bad in [
            "http://sandbox.plaid.com/x",
            "https://sandbox.plaid.com.evil.example/x",
            "https://evil.example/x",
            "https://localhost:8443/x",
            "https://user:pass@sandbox.plaid.com/x",
            "https://sandbox.plaid.com:444/x",
            "not a url",
        ] {
            assert!(policy.validate_url(bad).is_err(), "accepted {bad}");
        }
    }

    #[test]
    fn response_cap_is_enforced() {
        let mut small = std::io::Cursor::new(vec![0u8; 100]);
        assert_eq!(read_bounded(&mut small, 100).unwrap().len(), 100);
        let mut large = std::io::Cursor::new(vec![0u8; 101]);
        assert!(read_bounded(&mut large, 100).is_err());
    }
}
