use std::collections::BTreeMap;

use chrono_tz::Tz;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::storage::time::now_rfc3339;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CloseBehavior {
    Unset,
    Hide,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub timezone: String,
    pub theme: Theme,
    /// Visual theme id. Unknown ids fall back to the default in the front end;
    /// this value is persisted but never trusted as a path or CSS.
    pub theme_id: String,
    pub close_behavior: CloseBehavior,
    pub onboarding_completed: bool,
    pub auto_lock_minutes: u32,
    pub lock_on_system_events: bool,
    pub content_protection: bool,
    pub sync_windows: Vec<String>,
    pub sync_on_launch: bool,
    pub backup_retention_daily: u32,
    pub last_backup_at: Option<String>,
    pub last_backup_status: Option<String>,
}

impl Settings {
    /// Demo settings are synthetic; no real preference is read or written.
    pub fn note_demo(&mut self) {
        self.onboarding_completed = true;
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            timezone: "America/New_York".to_string(),
            theme: Theme::System,
            theme_id: "ledger".to_string(),
            close_behavior: CloseBehavior::Unset,
            onboarding_completed: false,
            auto_lock_minutes: 15,
            lock_on_system_events: true,
            content_protection: true,
            sync_windows: vec!["08:00".to_string(), "20:00".to_string()],
            sync_on_launch: true,
            backup_retention_daily: 7,
            last_backup_at: None,
            last_backup_status: None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatch {
    pub timezone: Option<String>,
    pub theme: Option<Theme>,
    pub theme_id: Option<String>,
    pub close_behavior: Option<CloseBehavior>,
    pub onboarding_completed: Option<bool>,
    pub auto_lock_minutes: Option<u32>,
    pub sync_windows: Option<Vec<String>>,
    pub sync_on_launch: Option<bool>,
    pub backup_retention_daily: Option<u32>,
}

const KEYS: &[&str] = &[
    "timezone",
    "theme",
    "themeId",
    "closeBehavior",
    "onboardingCompleted",
    "autoLockMinutes",
    "lockOnSystemEvents",
    "contentProtection",
    "syncWindows",
    "syncOnLaunch",
    "backupRetentionDaily",
    "lastBackupAt",
    "lastBackupStatus",
];

pub fn load_settings(conn: &Connection) -> AppResult<Settings> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut map = BTreeMap::new();
    for row in rows {
        let (key, value) = row?;
        if KEYS.contains(&key.as_str()) {
            map.insert(key, value);
        }
    }
    let mut settings = Settings::default();
    macro_rules! take {
        ($key:literal) => {
            map.get($key).and_then(|v| serde_json::from_str(v).ok())
        };
    }
    if let Some(v) = take!("timezone") {
        settings.timezone = v;
    }
    if let Some(v) = take!("theme") {
        settings.theme = v;
    }
    if let Some(v) = take!("themeId") {
        settings.theme_id = v;
    }
    if let Some(v) = take!("closeBehavior") {
        settings.close_behavior = v;
    }
    if let Some(v) = take!("onboardingCompleted") {
        settings.onboarding_completed = v;
    }
    if let Some(v) = take!("autoLockMinutes") {
        settings.auto_lock_minutes = v;
    }
    if let Some(v) = take!("lockOnSystemEvents") {
        settings.lock_on_system_events = v;
    }
    if let Some(v) = take!("contentProtection") {
        settings.content_protection = v;
    }
    if let Some(v) = take!("syncWindows") {
        settings.sync_windows = v;
    }
    if let Some(v) = take!("syncOnLaunch") {
        settings.sync_on_launch = v;
    }
    if let Some(v) = take!("backupRetentionDaily") {
        settings.backup_retention_daily = v;
    }
    if let Some(v) = take!("lastBackupAt") {
        settings.last_backup_at = v;
    }
    if let Some(v) = take!("lastBackupStatus") {
        settings.last_backup_status = v;
    }
    // Values that must never be weakened are pinned here.
    settings.lock_on_system_events = true;
    settings.content_protection = true;
    validate_settings(&settings)?;
    Ok(settings)
}

/// Theme ids are lowercase slugs. They are stored, compared, and used as a
/// lookup key only — never interpolated into CSS, a path, or HTML.
fn valid_theme_id(id: &str) -> bool {
    if id.is_empty() || id.len() > 40 {
        return false;
    }
    let mut chars = id.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    first_ok && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub fn validate_settings(settings: &Settings) -> AppResult<()> {
    settings
        .timezone
        .parse::<Tz>()
        .map_err(|_| AppError::InvalidInput("timezone"))?;
    if !valid_theme_id(&settings.theme_id) {
        return Err(AppError::InvalidInput("theme id"));
    }
    if !(1..=240).contains(&settings.auto_lock_minutes) {
        return Err(AppError::InvalidInput("auto-lock minutes"));
    }
    if !(1..=30).contains(&settings.backup_retention_daily) {
        return Err(AppError::InvalidInput("backup retention"));
    }
    if settings.sync_windows.len() > 8 {
        return Err(AppError::InvalidInput("sync windows"));
    }
    for window in &settings.sync_windows {
        let (hour, minute) = window
            .split_once(':')
            .ok_or(AppError::InvalidInput("sync window"))?;
        if hour.len() != 2 || minute.len() != 2 {
            return Err(AppError::InvalidInput("sync window"));
        }
        let hour: u32 = hour
            .parse()
            .map_err(|_| AppError::InvalidInput("sync window"))?;
        let minute: u32 = minute
            .parse()
            .map_err(|_| AppError::InvalidInput("sync window"))?;
        if hour > 23 || minute > 59 {
            return Err(AppError::InvalidInput("sync window"));
        }
    }
    Ok(())
}

pub fn apply_patch(current: &Settings, patch: SettingsPatch) -> AppResult<Settings> {
    let mut next = current.clone();
    if let Some(v) = patch.timezone {
        next.timezone = v;
    }
    if let Some(v) = patch.theme {
        next.theme = v;
    }
    if let Some(v) = patch.theme_id {
        next.theme_id = v;
    }
    if let Some(v) = patch.close_behavior {
        next.close_behavior = v;
    }
    if let Some(v) = patch.onboarding_completed {
        next.onboarding_completed = v;
    }
    if let Some(v) = patch.auto_lock_minutes {
        next.auto_lock_minutes = v;
    }
    if let Some(v) = patch.sync_windows {
        next.sync_windows = v;
    }
    if let Some(v) = patch.sync_on_launch {
        next.sync_on_launch = v;
    }
    if let Some(v) = patch.backup_retention_daily {
        next.backup_retention_daily = v;
    }
    validate_settings(&next)?;
    Ok(next)
}

pub fn save_settings(conn: &Connection, settings: &Settings) -> AppResult<()> {
    validate_settings(settings)?;
    let now = now_rfc3339();
    let pairs: [(&str, String); 13] = [
        (
            "timezone",
            serde_json::to_string(&settings.timezone).map_err(|_| AppError::Internal)?,
        ),
        (
            "theme",
            serde_json::to_string(&settings.theme).map_err(|_| AppError::Internal)?,
        ),
        (
            "themeId",
            serde_json::to_string(&settings.theme_id).map_err(|_| AppError::Internal)?,
        ),
        (
            "closeBehavior",
            serde_json::to_string(&settings.close_behavior).map_err(|_| AppError::Internal)?,
        ),
        (
            "onboardingCompleted",
            serde_json::to_string(&settings.onboarding_completed)
                .map_err(|_| AppError::Internal)?,
        ),
        (
            "autoLockMinutes",
            serde_json::to_string(&settings.auto_lock_minutes).map_err(|_| AppError::Internal)?,
        ),
        (
            "lockOnSystemEvents",
            serde_json::to_string(&settings.lock_on_system_events)
                .map_err(|_| AppError::Internal)?,
        ),
        (
            "contentProtection",
            serde_json::to_string(&settings.content_protection).map_err(|_| AppError::Internal)?,
        ),
        (
            "syncWindows",
            serde_json::to_string(&settings.sync_windows).map_err(|_| AppError::Internal)?,
        ),
        (
            "syncOnLaunch",
            serde_json::to_string(&settings.sync_on_launch).map_err(|_| AppError::Internal)?,
        ),
        (
            "backupRetentionDaily",
            serde_json::to_string(&settings.backup_retention_daily)
                .map_err(|_| AppError::Internal)?,
        ),
        (
            "lastBackupAt",
            serde_json::to_string(&settings.last_backup_at).map_err(|_| AppError::Internal)?,
        ),
        (
            "lastBackupStatus",
            serde_json::to_string(&settings.last_backup_status).map_err(|_| AppError::Internal)?,
        ),
    ];
    let tx = conn.unchecked_transaction()?;
    for (key, value) in pairs {
        tx.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            rusqlite::params![key, value, now],
        )?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safe_and_valid() {
        let s = Settings::default();
        validate_settings(&s).unwrap();
        assert_eq!(s.timezone, "America/New_York");
        assert_eq!(s.theme_id, "ledger");
        assert_eq!(s.auto_lock_minutes, 15);
        assert!(s.lock_on_system_events);
        assert!(s.content_protection);
    }

    #[test]
    fn patch_bounds_are_enforced() {
        let s = Settings::default();
        let too_low = apply_patch(
            &s,
            SettingsPatch {
                auto_lock_minutes: Some(0),
                ..Default::default()
            },
        );
        assert!(too_low.is_err());
        let too_high = apply_patch(
            &s,
            SettingsPatch {
                auto_lock_minutes: Some(241),
                ..Default::default()
            },
        );
        assert!(too_high.is_err());
        let bad_tz = apply_patch(
            &s,
            SettingsPatch {
                timezone: Some("Mars/Olympus".into()),
                ..Default::default()
            },
        );
        assert!(bad_tz.is_err());
        let ok = apply_patch(
            &s,
            SettingsPatch {
                theme: Some(Theme::Dark),
                auto_lock_minutes: Some(30),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(ok.theme, Theme::Dark);
        assert_eq!(ok.auto_lock_minutes, 30);
    }

    #[test]
    fn theme_ids_are_validated_as_slugs() {
        let s = Settings::default();
        let long = "a".repeat(41);
        for bad in [
            "",
            "Ledger",
            "../theme",
            "theme id",
            "ledger!",
            long.as_str(),
        ] {
            assert!(apply_patch(
                &s,
                SettingsPatch {
                    theme_id: Some(bad.to_string()),
                    ..Default::default()
                }
            )
            .is_err());
        }
        for good in ["ledger", "graphite-2", "editorial", "x"] {
            assert!(apply_patch(
                &s,
                SettingsPatch {
                    theme_id: Some(good.to_string()),
                    ..Default::default()
                }
            )
            .is_ok());
        }
    }

    #[test]
    fn invalid_sync_windows_are_rejected() {
        let s = Settings::default();
        for bad in [vec!["8:00"], vec!["24:00"], vec!["08:60"], vec!["nope"]] {
            assert!(apply_patch(
                &s,
                SettingsPatch {
                    sync_windows: Some(bad.into_iter().map(String::from).collect()),
                    ..Default::default()
                }
            )
            .is_err());
        }
    }
}
