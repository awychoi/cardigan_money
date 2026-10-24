//! Money is signed integer minor units. Provider/CSV decimal strings are parsed
//! with a decimal parser at the boundary; binary floating point is never used.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoneyError {
    Empty,
    InvalidFormat,
    TooMuchPrecision,
    OverflowsMinorUnits,
    UnsupportedCurrency,
}

/// Currency exponent (digits after the decimal separator). Unknown currencies
/// are rejected rather than silently assumed to have two decimals.
pub fn currency_exponent(currency: &str) -> Option<u32> {
    match currency {
        "USD" | "EUR" | "GBP" | "CAD" | "AUD" | "NZD" | "CHF" => Some(2),
        "JPY" | "KRW" => Some(0),
        "BHD" | "KWD" | "OMR" => Some(3),
        _ => None,
    }
}

/// Parse a decimal amount into integer minor units.
///
/// Accepts optional leading `$`/`€`/`£`/`¥`, thousands separators (validated
/// grouping), a single decimal point, and parentheses for negatives. Rejects
/// exponents, binary-float notation, embedded signs, more precision than the
/// currency permits, and values outside ±1e12 minor units.
pub fn parse_amount_to_minor(input: &str, exponent: u32) -> Result<i64, MoneyError> {
    if exponent > 3 {
        return Err(MoneyError::UnsupportedCurrency);
    }
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(MoneyError::Empty);
    }
    if trimmed.chars().any(|c| c.is_control() || c == '\u{0}') {
        return Err(MoneyError::InvalidFormat);
    }

    let mut text = trimmed.to_string();
    let mut negative = false;
    if text.starts_with('(') {
        if !text.ends_with(')') {
            return Err(MoneyError::InvalidFormat);
        }
        negative = true;
        text = text[1..text.len() - 1].trim().to_string();
    }
    for symbol in ['$', '€', '£', '¥'] {
        if let Some(stripped) = text.strip_prefix(symbol) {
            text = stripped.trim().to_string();
            break;
        }
    }
    if text.is_empty() {
        return Err(MoneyError::Empty);
    }
    if let Some(rest) = text.strip_prefix('-') {
        if negative {
            return Err(MoneyError::InvalidFormat);
        }
        negative = true;
        text = rest.trim().to_string();
    } else if let Some(rest) = text.strip_prefix('+') {
        if negative {
            return Err(MoneyError::InvalidFormat);
        }
        text = rest.trim().to_string();
    }
    if text.is_empty() || text.contains(['-', '+']) {
        return Err(MoneyError::InvalidFormat);
    }

    // Split integer/fraction and validate thousands grouping before removing
    // commas; "1,23.45" must not silently become 123.45.
    let mut parts = text.split('.');
    let integer_part = parts.next().unwrap_or("");
    let fraction_part = parts.next();
    if parts.next().is_some() {
        return Err(MoneyError::InvalidFormat);
    }
    if integer_part.is_empty()
        && fraction_part.map(|f| f.is_empty()).unwrap_or(true)
    {
        return Err(MoneyError::InvalidFormat);
    }
    let integer_digits = if integer_part.contains(',') {
        let groups: Vec<&str> = integer_part.split(',').collect();
        if groups.iter().any(|group| group.is_empty()) {
            return Err(MoneyError::InvalidFormat);
        }
        if groups[0].len() < 1 || groups[0].len() > 3 {
            return Err(MoneyError::InvalidFormat);
        }
        if groups.iter().skip(1).any(|group| group.len() != 3) {
            return Err(MoneyError::InvalidFormat);
        }
        groups.concat()
    } else {
        integer_part.to_string()
    };
    let integer_digits = if integer_digits.is_empty() {
        "0".to_string()
    } else {
        integer_digits
    };
    if !integer_digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(MoneyError::InvalidFormat);
    }
    let fraction = fraction_part.unwrap_or("");
    if !fraction.chars().all(|c| c.is_ascii_digit()) {
        return Err(MoneyError::InvalidFormat);
    }
    if fraction.len() > exponent as usize {
        // Allow extra zeroes only; never round silently.
        if fraction[exponent as usize..].chars().any(|c| c != '0') {
            return Err(MoneyError::TooMuchPrecision);
        }
    }
    let normalized = if fraction.is_empty() {
        integer_digits
    } else {
        format!("{integer_digits}.{fraction}")
    };
    let decimal = Decimal::from_str(&normalized).map_err(|_| MoneyError::InvalidFormat)?;
    if !decimal.is_sign_negative() && !decimal.is_sign_positive() {
        return Err(MoneyError::InvalidFormat);
    }
    let scale = 10i64
        .checked_pow(exponent)
        .ok_or(MoneyError::UnsupportedCurrency)?;
    let scaled = decimal
        .checked_mul(Decimal::from(scale))
        .ok_or(MoneyError::OverflowsMinorUnits)?;
    if !scaled.fract().is_zero() {
        return Err(MoneyError::TooMuchPrecision);
    }
    let minor = scaled
        .to_i64()
        .ok_or(MoneyError::OverflowsMinorUnits)?;
    let signed = if negative { -minor } else { minor };
    if !(-1_000_000_000_000..=1_000_000_000_000).contains(&signed) {
        return Err(MoneyError::OverflowsMinorUnits);
    }
    Ok(signed)
}

pub fn parse_currency_amount(input: &str, currency: &str) -> Result<i64, MoneyError> {
    let exponent = currency_exponent(currency).ok_or(MoneyError::UnsupportedCurrency)?;
    parse_amount_to_minor(input, exponent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_decimal_strings_exactly() {
        assert_eq!(parse_currency_amount("12.34", "USD"), Ok(1234));
        assert_eq!(parse_currency_amount("$1,234.56", "USD"), Ok(123456));
        assert_eq!(parse_currency_amount(" (45.00) ", "USD"), Ok(-4500));
        assert_eq!(parse_currency_amount("-0.01", "USD"), Ok(-1));
        assert_eq!(parse_currency_amount("0.00", "USD"), Ok(0));
        assert_eq!(parse_currency_amount("1,000", "USD"), Ok(100000));
        assert_eq!(parse_currency_amount("¥1,234", "JPY"), Ok(1234));
        assert_eq!(parse_currency_amount("1.234", "BHD"), Ok(1234));
    }

    #[test]
    fn rejects_ambiguous_or_rounded_input() {
        for bad in [
            "1,23.45", "1.2.3", "1e3", "12,34", "NaN", "inf", "", "  ", "$", "--1.00",
            "1.005", "0x10", "1 2", "12.3.4",
        ] {
            assert!(
                parse_currency_amount(bad, "USD").is_err(),
                "accepted malformed amount {bad:?}"
            );
        }
        assert_eq!(
            parse_currency_amount("1.2300", "USD"),
            Ok(123),
            "extra zero precision is accepted exactly"
        );
        assert!(parse_currency_amount("1.00", "XYZ").is_err());
    }

    #[test]
    fn rejects_overflow_and_huge_values() {
        assert!(parse_currency_amount("10000000000000000.00", "USD").is_err());
        assert!(parse_currency_amount("-9999999999999999999", "USD").is_err());
    }

    #[test]
    fn parses_without_binary_float_rounding() {
        // 0.1 + 0.2 would be 0.30000000000000004 with floats; cents are exact.
        let a = parse_currency_amount("0.10", "USD").unwrap();
        let b = parse_currency_amount("0.20", "USD").unwrap();
        assert_eq!(a + b, 30);
        assert_eq!(parse_currency_amount("2.675", "USD"), Err(MoneyError::TooMuchPrecision));
    }
}
