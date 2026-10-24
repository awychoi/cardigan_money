//! Native lock monitors.
//!
//! Manual lock, idle timeout, screen lock, sleep, fast user switching, logout,
//! and quit all converge on `AppState::lock`, which closes the database,
//! zeroizes the DEK, and notifies the renderer. Hiding the window does not lock
//! the vault; the idle timer continues while hidden.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime};

use tauri::{AppHandle, Manager};

use crate::state::AppState;

#[cfg(target_os = "macos")]
mod native {
    use std::ffi::c_void;

    use core_foundation::base::{TCFType, CFType};
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::string::CFString;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGSessionCopyCurrentDictionary() -> CFDictionaryRef;
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event: u32) -> f64;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFBooleanTrue: *const c_void;
        fn CFDictionaryGetValue(
            the_dict: *const c_void,
            key: *const c_void,
        ) -> *const c_void;
    }

    /// (screen_locked, on_console). `None` when the session dictionary is
    /// unavailable; callers must not treat that as permission to skip a lock.
    pub fn session_state() -> Option<(bool, bool)> {
        unsafe {
            let dict_ref = CGSessionCopyCurrentDictionary();
            if dict_ref.is_null() {
                return None;
            }
            let dict: CFDictionary<CFString, CFType> =
                CFDictionary::wrap_under_create_rule(dict_ref);
            let get = |name: &str| -> Option<bool> {
                let key = CFString::new(name);
                let value = CFDictionaryGetValue(
                    dict.as_concrete_TypeRef() as *const c_void,
                    key.as_concrete_TypeRef() as *const c_void,
                );
                if value.is_null() {
                    None
                } else {
                    Some(value == kCFBooleanTrue)
                }
            };
            let locked = get("CGSSessionScreenIsLocked").unwrap_or(false);
            let on_console = get("kCGSSessionOnConsoleKey").unwrap_or(true);
            Some((locked, on_console))
        }
    }

    /// Seconds since the last native HID input, from the window server. This is
    /// a Rust-side source; renderer-reported activity is not trusted alone.
    pub fn idle_seconds() -> Option<f64> {
        // kCGEventSourceStateCombinedSessionState = 0,
        // kCGAnyInputEventType = 0xFFFFFFFF.
        let seconds = unsafe { CGEventSourceSecondsSinceLastEventType(0, 0xFFFF_FFFF) };
        if seconds.is_finite() && seconds >= 0.0 {
            Some(seconds)
        } else {
            None
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod native {
    pub fn session_state() -> Option<(bool, bool)> {
        None
    }
    pub fn idle_seconds() -> Option<f64> {
        None
    }
}

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const SLEEP_GAP: Duration = Duration::from_secs(5);

pub fn spawn_monitor<R: tauri::Runtime + 'static>(app: AppHandle<R>) {
    std::thread::spawn(move || {
        let mut last_wall = SystemTime::now();
        let mut last_mono = Instant::now();
        loop {
            std::thread::sleep(POLL_INTERVAL);
            let state = app.state::<AppState>();
            if state.quitting.load(Ordering::SeqCst) {
                return;
            }
            let unlocked = state
                .session
                .lock()
                .map(|session| session.is_unlocked())
                .unwrap_or(false);

            let now_wall = SystemTime::now();
            let wall_delta = now_wall.duration_since(last_wall).unwrap_or_default();
            let mono_delta = last_mono.elapsed();
            last_wall = now_wall;
            last_mono = Instant::now();

            if !unlocked {
                continue;
            }

            // A wall-clock jump far larger than monotonic time indicates sleep.
            if wall_delta > mono_delta + SLEEP_GAP {
                state.lock(&app, "sleep");
                continue;
            }

            if let Some((screen_locked, on_console)) = native::session_state() {
                if screen_locked {
                    state.lock(&app, "screen_lock");
                    continue;
                }
                if !on_console {
                    state.lock(&app, "session_switch");
                    continue;
                }
            }

            // Demo data is synthetic; do not auto-close it on idle. Screen
            // lock/sleep still close it above.
            if state.mode() == crate::state::SessionMode::Demo {
                continue;
            }

            let idle = native::idle_seconds().unwrap_or(0.0);
            let auto_lock_minutes = state
                .with_db(|_conn, _header, settings| Ok(settings.auto_lock_minutes))
                .unwrap_or(15);
            if auto_lock_minutes > 0
                && idle >= f64::from(auto_lock_minutes) * 60.0
            {
                state.lock(&app, "idle");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_constants_are_sane() {
        assert!(POLL_INTERVAL.as_secs() >= 1);
        assert!(SLEEP_GAP > POLL_INTERVAL);
    }
}
