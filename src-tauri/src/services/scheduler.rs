//! Two-window scheduler. It never syncs while the vault is locked, never runs
//! two syncs concurrently, and records skip reasons in memory (the encrypted DB
//! is unavailable while locked).

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use chrono::Timelike;
use tauri::{AppHandle, Manager};

use crate::services::sync;
use crate::state::AppState;

pub struct SchedulerState {
    launched: AtomicBool,
    windows_run: Mutex<HashSet<String>>,
}

pub fn spawn<R: tauri::Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    std::thread::Builder::new()
        .name("cm-sync-scheduler".into())
        .spawn(move || run_loop(handle))
        .ok();
}

fn run_loop<R: tauri::Runtime>(app: AppHandle<R>) {
    let state = Arc::new(SchedulerState {
        launched: AtomicBool::new(false),
        windows_run: Mutex::new(HashSet::new()),
    });
    loop {
        std::thread::sleep(std::time::Duration::from_secs(30));
        let Some(app_state) = app.try_state::<AppState>() else { continue };
        if app_state.quitting.load(Ordering::SeqCst) {
            return;
        }
        if !app_state.session.lock().map(|guard| guard.is_unlocked()).unwrap_or(false) {
            if let Ok(mut runtime) = app_state.sync_runtime.lock() {
                runtime.last_skip_reason = Some("vault_locked".into());
            }
            continue;
        }
        let settings = app_state.session.lock().ok().and_then(|guard| guard.settings.clone());
        let Some(settings) = settings else { continue };
        let now = chrono::Local::now();
        if !state.launched.swap(true, Ordering::SeqCst) && settings.sync_on_launch {
            let _ = trigger(&app_state, "launch");
            continue;
        }
        let window = current_window(&settings.sync_windows, now.hour(), now.minute());
        let Some(window) = window else { continue };
        let key = format!("{}-{:02}-{:02}-{}", now.date_naive(), window.0, window.1, window.2);
        {
            let mut runs = match state.windows_run.lock() {
                Ok(runs) => runs,
                Err(_) => continue,
            };
            if runs.contains(&key) {
                continue;
            }
            runs.insert(key.clone());
        }
        let succeeded = trigger(&app_state, "scheduled");
        if succeeded {
            if let Ok(mut runtime) = app_state.sync_runtime.lock() {
                runtime.last_skip_reason = None;
            }
        } else {
            // Allow another attempt at the next poll while the window is open.
            if let Ok(mut runs) = state.windows_run.lock() {
                runs.remove(&key);
            }
        }
    }
}

fn trigger(state: &AppState, trigger: &str) -> bool {
    match sync::sync_now(state, trigger) {
        Ok(summary) => summary.accepted && summary.skip_reason.is_none(),
        Err(_) => false,
    }
}

/// Returns (hour, minute, window index) when `now` is inside a configured
/// window. Windows are interpreted in the device's local timezone.
pub fn current_window(windows: &[String], hour: u32, minute: u32) -> Option<(u32, u32, usize)> {
    for (index, window) in windows.iter().enumerate() {
        let Some((hour_text, minute_text)) = window.split_once(':') else { continue };
        let (Ok(hour_part), Ok(minute_part)) = (hour_text.parse::<u32>(), minute_text.parse::<u32>()) else {
            continue;
        };
        let target = hour_part * 60 + minute_part.min(59);
        let now_minutes = hour * 60 + minute;
        if now_minutes >= target && now_minutes < target + 5 {
            return Some((hour_part, minute_part, index));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn windows() -> Vec<String> {
        vec!["08:00".into(), "20:00".into()]
    }

    #[test]
    fn matches_only_a_five_minute_window() {
        assert_eq!(current_window(&windows(), 8, 0), Some((8, 0, 0)));
        assert_eq!(current_window(&windows(), 8, 4), Some((8, 0, 0)));
        assert_eq!(current_window(&windows(), 8, 5), None);
        assert_eq!(current_window(&windows(), 20, 2), Some((20, 0, 1)));
        assert_eq!(current_window(&windows(), 12, 0), None);
    }
}
