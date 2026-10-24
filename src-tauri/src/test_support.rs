//! Test-only helpers. Compiled in test builds only; not part of the release API.

use tauri::test::MockRuntime;
use tauri::AppHandle;

/// Build a minimal Tauri app for command tests. Tauri's mock runtime keeps this
/// allocation alive for the duration of the test process.
pub fn mock_app() -> AppHandle<MockRuntime> {
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("mock tauri app");
    app.handle().clone()
}
