pub mod commands;
pub mod demo;
pub mod domain;
pub mod error;
pub mod ipc;
pub mod providers;
pub mod security;
pub mod services;
pub mod state;
pub mod storage;
#[cfg(test)]
pub mod test_support;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::Manager;
use crate::state::AppState;
use crate::storage::settings::CloseBehavior;

const CSP_SAFE_TITLE: &str = "cardigan_money";

pub static APP_HANDLE: std::sync::OnceLock<tauri::AppHandle<tauri::Wry>> =
    std::sync::OnceLock::new();

/// Resolve the application-data directory. A debug-only override exists for
/// sandboxed development/test runs; release builds always use the standard
/// macOS Application Support directory and ignore the environment variable.
fn resolve_app_data(app: &tauri::AppHandle) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    #[cfg(debug_assertions)]
    {
        if let Ok(dir) = std::env::var("CARDIGAN_MONEY_APP_DATA_DIR") {
            if !dir.trim().is_empty() {
                return Ok(std::path::PathBuf::from(dir));
            }
        }
    }
    app.path()
        .app_data_dir()
        .map_err(|_| "app data path unavailable".into())
}

fn harden_process() {
    // Disable core dumps; no heap contents in crash artifacts.
    #[cfg(unix)]
    unsafe {
        let limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        libc::setrlimit(libc::RLIMIT_CORE, &limit);
        // Every file the process creates is owner-only by default; explicit
        // mode checks remain in the filesystem layer.
        libc::umask(0o077);
    }
    // Panic hook: debug builds may name the failing location to aid local
    // development; release builds emit a fixed message with no payload.
    std::panic::set_hook(Box::new(|info| {
        #[cfg(debug_assertions)]
        {
            eprintln!("cardigan_money dev panic: {info}");
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = info;
            eprintln!("cardigan_money stopped unexpectedly");
        }
    }));
}

#[cfg(target_os = "macos")]
fn harden_native_window(window: &tauri::WebviewWindow) {
    let _ = window.set_content_protected(true);
    if let Ok(ptr) = window.ns_window() {
        let ns = ptr as *mut objc2_app_kit::NSWindow;
        if !ns.is_null() {
            unsafe {
                let ns = &*ns;
                ns.setSharingType(objc2_app_kit::NSWindowSharingType::None);
                // Disable state restoration; the app must not restore ledger
                // content or window snapshots on next launch.
                let restore: i8 = 0;
                let _: () = objc2::msg_send![ns, setRestorable: restore];
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn harden_native_window(_window: &tauri::WebviewWindow) {}

fn persist_close_behavior<R: tauri::Runtime>(app: &tauri::AppHandle<R>, behavior: CloseBehavior) {
    use crate::storage::settings::save_settings;
    let state = app.state::<AppState>();
    let updated = state
        .with_db(|conn, _header, current| {
            let mut next = current.clone();
            next.close_behavior = behavior;
            save_settings(conn, &next)?;
            crate::storage::events::record_event(conn, "settings_changed", "success", None)?;
            Ok(next)
        })
        .ok();
    if let Some(next) = updated {
        if let Ok(mut session) = state.session.lock() {
            session.settings = Some(next);
        }
    }
}

fn handle_close_request<R: tauri::Runtime>(window: &tauri::Window<R>, app: &tauri::AppHandle<R>) {
    let state = app.state::<AppState>();
    if state.quitting.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    // A locked vault has no settings to consult; allow the close/quit.
    let Ok(behavior) = state.with_db(|_conn, _header, settings| Ok(settings.close_behavior)) else {
        return;
    };
    match crate::security::native_dialog::decision(behavior) {
        crate::security::native_dialog::CloseDecision::Hide => {
            let _ = window.hide();
        }
        crate::security::native_dialog::CloseDecision::Quit => {
            app.exit(0);
        }
        crate::security::native_dialog::CloseDecision::Ask => {
            if state.close_prompt_open.swap(true, std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            let app = app.clone();
            let window = window.clone();
            std::thread::spawn(move || {
                use crate::security::native_dialog::CloseAction;
                let result = crate::security::native_dialog::close_choice(&app);
                app.state::<AppState>()
                    .close_prompt_open
                    .store(false, std::sync::atomic::Ordering::SeqCst);
                if let Ok(choice) = result {
                    if choice.remember && choice.action != CloseAction::Cancel {
                        let behavior = match choice.action {
                            CloseAction::Hide => CloseBehavior::Hide,
                            CloseAction::Quit => CloseBehavior::Quit,
                            CloseAction::Cancel => CloseBehavior::Unset,
                        };
                        persist_close_behavior(&app, behavior);
                    }
                    match choice.action {
                        CloseAction::Hide => {
                            let _ = window.hide();
                        }
                        CloseAction::Quit => app.exit(0),
                        CloseAction::Cancel => {}
                    }
                }
            });
        }
    }
}

pub fn run() {
    harden_process();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let app_data = resolve_app_data(app.handle())?;
            let state = state::AppState::new(app_data.clone()).map_err(|e| e.code())?;
            app.manage(state);
            let _ = APP_HANDLE.set(app.handle().clone());
            crate::services::scheduler::spawn(app.handle());

            let lock_item = MenuItem::with_id(
                app,
                "lock-now",
                "Lock Now",
                true,
                Some("CmdOrCtrl+L"),
            )?;
            let quit_item = PredefinedMenuItem::quit(app, Some("Quit cardigan_money"))?;
            let app_menu = Submenu::with_items(
                app,
                "cardigan_money",
                true,
                &[&lock_item, &PredefinedMenuItem::separator(app)?, &quit_item],
            )?;
            let edit_menu = Submenu::with_items(
                app,
                "Edit",
                true,
                &[
                    &PredefinedMenuItem::undo(app, None)?,
                    &PredefinedMenuItem::redo(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::cut(app, None)?,
                    &PredefinedMenuItem::copy(app, None)?,
                    &PredefinedMenuItem::paste(app, None)?,
                    &PredefinedMenuItem::select_all(app, None)?,
                ],
            )?;
            let menu = Menu::with_items(app, &[&app_menu, &edit_menu])?;
            app.set_menu(menu)?;

            #[cfg(debug_assertions)]
            if std::env::var("CARDIGAN_MONEY_OPEN_DEMO").as_deref() == Ok("1") {
                let state = app.state::<AppState>();
                match crate::demo::open(&state, app.handle()) {
                    Ok(view) => {
                        let dashboard = crate::services::dashboard::get(&state, Some("2026-10".into()));
                        match dashboard {
                            Ok(dashboard) => eprintln!(
                                "DEMO_NATIVE_OPEN=passed accounts={} rows={} net={} mode={}",
                                dashboard.overview.accounts.len(),
                                dashboard.summary.row_count,
                                dashboard.summary.net_spending_minor,
                                view.mode
                            ),
                            Err(_) => eprintln!("DEMO_NATIVE_OPEN=failed-dashboard"),
                        }
                    }
                    Err(_) => eprintln!("DEMO_NATIVE_OPEN=failed"),
                }
            }

            let window = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title(CSP_SAFE_TITLE)
            .inner_size(1440.0, 960.0)
            .min_inner_size(1100.0, 760.0)
            .resizable(true)
            .visible(false)
            .content_protected(true)
            .disable_drag_drop_handler()
            .devtools(false)
            // Keep WebKit website data inside the app data directory. This is
            // not ledger data; it prevents stray browser storage elsewhere and
            // keeps the app tidy under a sandbox.
            .data_directory(app_data.join("webkit"))
            .on_navigation(ipc::is_allowed_navigation)
            .on_new_window(|_url, _features| tauri::webview::NewWindowResponse::Deny)
            .build()?;
            harden_native_window(&window);
            window.show()?;
            #[cfg(debug_assertions)]
            if std::env::var("CARDIGAN_MONEY_OPEN_DEMO").as_deref() == Ok("1") {
                crate::demo::touch_title(app.handle());
            }
            security::lock::spawn_monitor(app.handle().clone());

            #[cfg(debug_assertions)]
            if std::env::var("CARDIGAN_MONEY_NATIVE_SHEET_SMOKE").as_deref() == Ok("1") {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    match security::secure_input::smoke_test(&handle) {
                        Ok(()) => eprintln!(
                            "NATIVE_SHEET_SMOKE=passed: sheet presented and cancelled"
                        ),
                        Err(_) => eprintln!("NATIVE_SHEET_SMOKE=failed"),
                    }
                    let _ = handle.exit(0);
                });
            }
            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id() == "lock-now" {
                let state = app.state::<AppState>();
                state.lock(app, "manual");
            }
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.app_handle().state::<AppState>();
                if state.quitting.load(std::sync::atomic::Ordering::SeqCst) {
                    return;
                }
                // The locked gate may be closed without a settings read.
                if state
                    .session
                    .lock()
                    .map(|session| !session.is_unlocked())
                    .unwrap_or(true)
                {
                    return;
                }
                api.prevent_close();
                handle_close_request(window, window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![commands::gateway]);

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building cardigan_money");

    app.run(|app, event| {
        if let tauri::RunEvent::Reopen { .. } = event {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        if matches!(
            event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        ) {
            let state = app.state::<AppState>();
            state.quitting.store(true, std::sync::atomic::Ordering::SeqCst);
            state.lock(app, "quit");
        }
    });
}
