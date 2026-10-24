use reqwest::Url;

use crate::error::{AppError, AppResult};

/// Only the verified Plaid Hosted Link origin is allowed through the opener.
/// This is deliberately an exact host match, not a suffix match.
pub fn validate_plaid_hosted_url(value: &str) -> AppResult<Url> {
    let url = Url::parse(value).map_err(|_| AppError::Blocked("invalid Hosted Link URL"))?;
    if url.scheme() != "https"
        || url.host_str() != Some("secure.plaid.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(AppError::Blocked("untrusted Hosted Link URL"));
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosted_link_host_is_exact() {
        assert!(validate_plaid_hosted_url("https://secure.plaid.com/hl/abc").is_ok());
        for bad in [
            "http://secure.plaid.com/hl/abc",
            "https://secure.plaid.com.evil.example/hl",
            "https://plaid.com/hl/abc",
            "https://secure.plaid.com:444/hl",
            "https://user:pass@secure.plaid.com/hl",
        ] {
            assert!(validate_plaid_hosted_url(bad).is_err(), "accepted {bad}");
        }
    }
}
