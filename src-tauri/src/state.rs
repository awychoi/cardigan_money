use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Mutex;
use std::time::Instant;

use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager};

use crate::error::{AppError, AppResult};
use crate::security::cipher::CipherInfo;
use crate::security::crypto::LockedKey;
use crate::security::files::ensure_private_dir;
use crate::security::ratelimit::RateLimiter;
use crate::security::vault::{VaultHeader, VaultPaths};
use crate::storage::events::record_event;
use crate::services::imports::ImportJobs;
use crate::storage::settings::Settings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMode {
    Real,
    Demo,
}

pub struct VaultSession {
    pub mode: SessionMode,
    pub header: Option<VaultHeader>,
    pub conn: Option<Connection>,
    pub cipher: Option<CipherInfo>,
    pub dek: Option<LockedKey>,
    pub settings: Option<Settings>,
    pub unlocked_at: Option<Instant>,
}

impl VaultSession {
    pub fn locked() -> Self {
        Self {
            mode: SessionMode::Real,
            header: None,
            conn: None,
            cipher: None,
            dek: None,
            settings: None,
            unlocked_at: None,
        }
    }

    pub fn is_unlocked(&self) -> bool {
        self.conn.is_some()
    }

    /// Drop connections and zeroize keys. Called on every lock path.
    pub fn clear(&mut self) {
        self.conn.take();
        self.dek.take();
        self.header.take();
        self.cipher.take();
        self.settings.take();
        self.unlocked_at.take();
        self.mode = SessionMode::Real;
    }

    pub fn mode(&self) -> SessionMode {
        self.mode
    }
}

pub struct AppState {
    pub app_data: PathBuf,
    pub paths: VaultPaths,
    pub session: Mutex<VaultSession>,
    pub unlock_failures: AtomicU32,
    pub rate: Mutex<RateLimiter>,
    pub imports: ImportJobs,
    pub sync_gate: Mutex<()>,
    pub sync_runtime: Mutex<crate::services::sync::SyncRuntimeStatus>,
    pub quitting: AtomicBool,
    pub close_prompt_open: AtomicBool,
}

impl AppState {
    pub fn new(app_data: PathBuf) -> AppResult<Self> {
        ensure_private_dir(&app_data)?;
        let paths = VaultPaths::new(&app_data);
        Ok(Self {
            app_data,
            paths,
            session: Mutex::new(VaultSession::locked()),
            unlock_failures: AtomicU32::new(0),
            rate: Mutex::new(RateLimiter::default()),
            imports: Mutex::new(std::collections::HashMap::new()),
            sync_gate: Mutex::new(()),
            sync_runtime: Mutex::new(Default::default()),
            quitting: AtomicBool::new(false),
            close_prompt_open: AtomicBool::new(false),
        })
    }

    /// Run a closure with the open connection, header, and settings. Fails
    /// closed when locked. Blocking database work runs outside the UI thread
    /// because commands are declared `#[tauri::command(async)]`.
    pub fn with_db<T>(
        &self,
        f: impl FnOnce(&Connection, Option<&VaultHeader>, &Settings) -> AppResult<T>,
    ) -> AppResult<T> {
        let guard = self.session.lock().map_err(|_| AppError::Internal)?;
        let conn = guard.conn.as_ref().ok_or(AppError::Locked)?;
        let settings = guard.settings.as_ref().ok_or(AppError::Locked)?;
        f(conn, guard.header.as_ref(), settings)
    }

    pub fn mode(&self) -> SessionMode {
        self.session
            .lock()
            .map(|session| session.mode)
            .unwrap_or(SessionMode::Real)
    }

    /// Demo mode is deliberately read-only for ledger mutations and never
    /// touches the real vault, Keychain, backups, or exports.
    pub fn require_real(&self, action: &'static str) -> AppResult<()> {
        if self.mode() == SessionMode::Demo {
            return Err(AppError::Blocked(action));
        }
        Ok(())
    }

    pub fn rate_limit(&self, key: &'static str, max: usize, seconds: u64) -> AppResult<()> {
        let mut rate = self.rate.lock().map_err(|_| AppError::Internal)?;
        rate.check(key, max, std::time::Duration::from_secs(seconds))
    }

    /// Lock, zeroize, close the database, and notify the renderer. The renderer
    /// is never responsible for enforcing the lock.
    pub fn lock<R: tauri::Runtime>(&self, app: &AppHandle<R>, reason: &str) -> bool {
        let state = {
            let mut session = match self.session.lock() {
                Ok(guard) => guard,
                Err(_) => return false,
            };
            if !session.is_unlocked() {
                return false;
            }
            if let Some(conn) = session.conn.as_ref() {
                let _ = record_event(
                    conn,
                    "lock",
                    "info",
                    Some(serde_json::json!({ "reason": reason })),
                );
            }
            let was_demo = session.mode == SessionMode::Demo;
            session.clear();
            (true, was_demo)
        };
        let (unlocked, was_demo) = state;
        if was_demo {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("cardigan_money");
            }
        }
        if unlocked {
            // Minimal event; no ledger data.
            let _ = app.emit_to(
                "main",
                "vault:locked",
                serde_json::json!({ "reason": reason }),
            );
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }
        unlocked
    }
}
