import { defineConfig, devices } from "@playwright/test";

// Browser tests run against the mock desktop bridge only. They never prove
// native Tauri behavior; native smoke tests are documented separately.
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [["list"]],
  timeout: 30_000,
  use: {
    baseURL: "http://127.0.0.1:1420",
    trace: "retain-on-failure",
    serviceWorkers: "block",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1440, height: 960 } } }],
  webServer: {
    command: "pnpm dev",
    url: "http://127.0.0.1:1420",
    reuseExistingServer: false,
    timeout: 60_000,
    env: { VITE_MOCK_BRIDGE: "1", VITE_MOCK_SCENARIO: "demo" },
  },
});
