//! Property tests for the money and date parsers. Bounded and deterministic.

use cardigan_money_lib::domain::dates::{format_date, parse_date};
use cardigan_money_lib::domain::money::parse_currency_amount;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn usd_minor_units_round_trip(minor in -1_000_000_000_000i64..=1_000_000_000_000i64) {
        let abs = minor.unsigned_abs();
        let text = format!("{}{}.{:02}", if minor < 0 { "-" } else { "" }, abs / 100, abs % 100);
        prop_assert_eq!(parse_currency_amount(&text, "USD"), Ok(minor), "text was {}", text);
    }

    #[test]
    fn arbitrary_text_never_panics(s in "\\PC{0,64}") {
        let _ = parse_currency_amount(&s, "USD");
        let _ = parse_currency_amount(&s, "JPY");
    }

    #[test]
    fn thousands_grouping_round_trips(cents in 0u32..100_000_000u32) {
        let dollars = cents / 100;
        let grouped = {
            let digits = dollars.to_string();
            let mut out = String::new();
            for (index, ch) in digits.chars().enumerate() {
                if index > 0 && (digits.len() - index) % 3 == 0 {
                    out.push(',');
                }
                out.push(ch);
            }
            out
        };
        let text = format!("${grouped}.{:02}", cents % 100);
        prop_assert_eq!(parse_currency_amount(&text, "USD"), Ok(i64::from(cents)));
    }

    #[test]
    fn valid_dates_round_trip(year in 1900i32..2200, month in 1u32..=12, day in 1u32..=28) {
        let date = chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap();
        let text = format_date(date);
        prop_assert_eq!(parse_date(&text), Ok(date));
    }
}
