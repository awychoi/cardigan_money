//! Date-only financial calendar days and calendar-month ranges.

use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateError {
    InvalidDate,
    InvalidMonth,
    InvalidTimezone,
    MissingDate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateSource {
    Authorized,
    Posted,
    Manual,
}

impl DateSource {
    pub fn as_str(self) -> &'static str {
        match self {
            DateSource::Authorized => "authorized",
            DateSource::Posted => "posted",
            DateSource::Manual => "manual",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "authorized" => Some(Self::Authorized),
            "posted" => Some(Self::Posted),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

/// Strict `YYYY-MM-DD` parsing. Timestamps are rejected: provider calendar dates
/// must not become midnight-UTC instants.
pub fn parse_date(value: &str) -> Result<NaiveDate, DateError> {
    if value.len() != 10 {
        return Err(DateError::InvalidDate);
    }
    let bytes = value.as_bytes();
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return Err(DateError::InvalidDate);
    }
    if !value
        .chars()
        .enumerate()
        .all(|(index, c)| index == 4 || index == 7 || c.is_ascii_digit())
    {
        return Err(DateError::InvalidDate);
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| DateError::InvalidDate)
}

pub fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Month {
    pub year: i32,
    pub month: u32,
}

impl Month {
    pub fn new(year: i32, month: u32) -> Result<Self, DateError> {
        if !(1..=12).contains(&month) || !(1900..=2200).contains(&year) {
            return Err(DateError::InvalidMonth);
        }
        Ok(Self { year, month })
    }

    pub fn from_date(date: NaiveDate) -> Self {
        Self {
            year: date.year(),
            month: date.month(),
        }
    }

    pub fn parse(value: &str) -> Result<Self, DateError> {
        if value.len() != 7 || value.as_bytes()[4] != b'-' {
            return Err(DateError::InvalidMonth);
        }
        let year: i32 = value[0..4].parse().map_err(|_| DateError::InvalidMonth)?;
        let month: u32 = value[5..7].parse().map_err(|_| DateError::InvalidMonth)?;
        Self::new(year, month)
    }

    pub fn start(self) -> NaiveDate {
        NaiveDate::from_ymd_opt(self.year, self.month, 1)
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).expect("epoch is valid"))
    }

    /// Exclusive first day of the next month.
    pub fn end_exclusive(self) -> NaiveDate {
        if self.month == 12 {
            NaiveDate::from_ymd_opt(self.year + 1, 1, 1).expect("valid next year")
        } else {
            NaiveDate::from_ymd_opt(self.year, self.month + 1, 1).expect("valid next month")
        }
    }

    pub fn previous(self) -> Self {
        if self.month == 1 {
            Self {
                year: self.year - 1,
                month: 12,
            }
        } else {
            Self {
                year: self.year,
                month: self.month - 1,
            }
        }
    }

    pub fn next(self) -> Self {
        if self.month == 12 {
            Self {
                year: self.year + 1,
                month: 1,
            }
        } else {
            Self {
                year: self.year,
                month: self.month + 1,
            }
        }
    }

    pub fn key(self) -> String {
        format!("{:04}-{:02}", self.year, self.month)
    }

    pub fn start_string(self) -> String {
        format_date(self.start())
    }

    pub fn end_exclusive_string(self) -> String {
        format_date(self.end_exclusive())
    }
}

pub fn timezone(name: &str) -> Result<Tz, DateError> {
    name.parse::<Tz>().map_err(|_| DateError::InvalidTimezone)
}

/// Current calendar month in the configured reporting timezone.
pub fn today(tz_name: &str) -> Result<NaiveDate, DateError> {
    let tz = timezone(tz_name)?;
    Ok(Utc::now().with_timezone(&tz).date_naive())
}

pub fn current_month(tz_name: &str) -> Result<Month, DateError> {
    let tz = timezone(tz_name)?;
    let now = Utc::now().with_timezone(&tz);
    Ok(Month::from_date(now.date_naive()))
}

pub fn month_for_instant(
    tz_name: &str,
    instant: chrono::DateTime<Utc>,
) -> Result<Month, DateError> {
    let tz = timezone(tz_name)?;
    let _ = tz.from_utc_datetime(&instant.naive_utc());
    Ok(Month::from_date(instant.with_timezone(&tz).date_naive()))
}

/// Reporting date rule: authorization/purchase date when available; otherwise
/// the posted date. A manual entry may choose a date explicitly. The chosen date
/// and its provenance are preserved.
pub fn choose_reporting_date(
    authorized: Option<NaiveDate>,
    posted: Option<NaiveDate>,
    manual: Option<NaiveDate>,
) -> Result<(NaiveDate, DateSource), DateError> {
    if let Some(date) = manual {
        return Ok((date, DateSource::Manual));
    }
    if let Some(date) = authorized {
        return Ok((date, DateSource::Authorized));
    }
    if let Some(date) = posted {
        return Ok((date, DateSource::Posted));
    }
    Err(DateError::MissingDate)
}

pub fn is_within_month(date: NaiveDate, month: Month) -> bool {
    date >= month.start() && date < month.end_exclusive()
}

pub fn days_in_month(month: Month) -> u32 {
    (month.end_exclusive() - month.start()).num_days() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_date_only_parsing() {
        assert_eq!(
            parse_date("2024-02-29").unwrap(),
            NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()
        );
        assert!(parse_date("2026-02-29").is_err());
        assert!(parse_date("2026-02-30").is_err());
        assert!(parse_date("2026-2-03").is_err());
        assert!(parse_date("2026-02-03T00:00:00Z").is_err());
        assert!(parse_date("2026-02-03 ").is_err());
        assert!(parse_date("").is_err());
    }

    #[test]
    fn month_boundaries_year_change_and_leap_day() {
        let feb = Month::new(2024, 2).unwrap();
        assert_eq!(feb.start_string(), "2024-02-01");
        assert_eq!(feb.end_exclusive_string(), "2024-03-01");
        assert_eq!(days_in_month(feb), 29);
        let dec = Month::new(2026, 12).unwrap();
        assert_eq!(dec.end_exclusive_string(), "2027-01-01");
        assert_eq!(dec.next().key(), "2027-01");
        assert_eq!(dec.next().previous().key(), "2026-12");
    }

    #[test]
    fn new_york_dst_days_keep_their_calendar_date() {
        // 2026-03-08 is US spring-forward; 2026-11-01 is fall-back.
        for (y, m, d) in [(2026, 3, 8), (2026, 11, 1)] {
            let instant = Utc.with_ymd_and_hms(y, m, d, 12, 0, 0).unwrap();
            let month = month_for_instant("America/New_York", instant).unwrap();
            assert_eq!(month.year, 2026);
            assert_eq!(month.month, m, "local calendar date must not shift");
        }
        let instant = Utc.with_ymd_and_hms(2026, 3, 1, 4, 30, 0).unwrap(); // 23:30 EST Feb 28
        let month = month_for_instant("America/New_York", instant).unwrap();
        assert_eq!(month.key(), "2026-02");
    }

    #[test]
    fn reporting_date_prefers_authorization_then_posted_then_manual() {
        let auth = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        let posted = NaiveDate::from_ymd_opt(2026, 2, 2).unwrap();
        assert_eq!(
            choose_reporting_date(Some(auth), Some(posted), None).unwrap(),
            (auth, DateSource::Authorized)
        );
        assert_eq!(
            choose_reporting_date(None, Some(posted), None).unwrap(),
            (posted, DateSource::Posted)
        );
        assert_eq!(
            choose_reporting_date(None, None, Some(auth)).unwrap(),
            (auth, DateSource::Manual)
        );
        assert!(choose_reporting_date(None, None, None).is_err());
    }

    #[test]
    fn invalid_timezones_are_rejected() {
        assert!(timezone("Mars/Olympus").is_err());
        assert!(current_month("America/New_York").is_ok());
        assert!(current_month("Not/AZone").is_err());
    }
}
