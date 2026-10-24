pub mod commands;
pub mod error;
pub mod ipc;
pub mod security;
pub mod state;

use tauri::Manager;

const CSP_SAFE_TITLE: &str = "cardigan_money";

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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::gateway]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running cardigan_money");
}
