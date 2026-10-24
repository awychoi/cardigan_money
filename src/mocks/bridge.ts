import { BridgeError } from "../lib/bridge/errors";
import {
  archiveMockAccount,
  createMockAccount,
  mockAccounts,
  mockDashboard,
  mockLedger,
  resetMockData,
  updateMockAccount,
} from "./data";
import {
  API_VERSION,
  isCommandName,
  type CommandName,
  type AccountPayload,
  type SecurityEvent,
  type SecuritySummary,
  type Settings,
  type SettingsPatch,
  type VaultState,
} from "../lib/bridge/contract";

/**
 * Browser-only mock of the Rust gateway, used by Vite development and
 * Playwright. It implements the same contract and fails loudly on unknown
 * commands. Vite aliases this module to `bridge.disabled.ts` in production, so
 * the mock cannot ship. The fixed cipher strings identify it as a mock.
 */
const params = new URLSearchParams(window.location.search);

function initialState() {
  const empty = params.get("mockVault") === "empty";
  return {
    vaultExists: !empty,
    unlocked: false,
    mode: "real" as "real" | "demo",
    generation: 1,
    events: [] as SecurityEvent[],
    dashboardVersion: 1,
  };
}

const mockState = initialState();

const defaultSettings: Settings = {
  timezone: "America/New_York",
  theme: "system",
  closeBehavior: "unset",
  onboardingCompleted: params.get("mockOnboarding") !== "1",
  autoLockMinutes: 15,
  lockOnSystemEvents: true,
  contentProtection: true,
  syncWindows: ["08:00", "20:00"],
  syncOnLaunch: true,
  backupRetentionDaily: 7,
  lastBackupAt: null,
  lastBackupStatus: null,
};

let settings: Settings = { ...defaultSettings };

function recordEvent(eventType: string, outcome: SecurityEvent["outcome"]): void {
  mockState.events.unshift({
    id: crypto.randomUUID(),
    eventType,
    outcome,
    createdAt: new Date().toISOString(),
    metadata: null,
  });
  mockState.events = mockState.events.slice(0, 100);
}

function requireUnlocked(): void {
  if (!mockState.vaultExists) throw new BridgeError("vault_missing", "No vault.");
  if (!mockState.unlocked) throw new BridgeError("vault_locked", "The vault is locked.");
}

function securitySummary(): SecuritySummary {
  return {
    cipherVersion: "4.6.1 community (browser mock)",
    cipherProvider: "openssl (browser mock)",
    kdfSummary: "Argon2id, 64 MiB memory, 3 iterations, 1 lane",
    generation: mockState.generation,
    schemaVersion: 1,
    lastIntegrityCheckAt: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
    vaultIdPrefix: "5a17c0de",
    cipherStatusSupported: false,
  };
}

function vaultState(): VaultState {
  return {
    exists: mockState.vaultExists,
    unlocked: mockState.unlocked,
    mode: mockState.mode,
    settings: mockState.unlocked ? structuredClone(settings) : null,
    security: mockState.unlocked ? securitySummary() : null,
  };
}

const appInfo = {
  name: "cardigan_money" as const,
  version: "0.1.0",
  bundleIdentifier: "com.awychoi.cardiganmoney" as const,
  platform: "browser-mock",
  apiVersion: 1 as const,
  demoAvailable: true,
};

export async function mockCommand(name: string, payload: unknown): Promise<unknown> {
  if (!isCommandName(name)) {
    throw new BridgeError("unknown_command", "Unknown command.");
  }
  const known: CommandName = name;
  let data: unknown;
  switch (known) {
    case "app_info":
      data = {
        ...appInfo,
        vaultExists: mockState.vaultExists,
        vaultUnlocked: mockState.unlocked,
      };
      break;
    case "health_check":
      data = { ok: true, apiVersion: 1 };
      break;
    case "vault_status":
      data = vaultState();
      break;
    case "vault_create":
      if (mockState.vaultExists) throw new BridgeError("vault_exists", "Vault exists.");
      mockState.vaultExists = true;
      mockState.unlocked = true;
      mockState.mode = "real";
      recordEvent("vault_created", "success");
      data = vaultState();
      break;
    case "vault_unlock":
      if (!mockState.vaultExists) throw new BridgeError("vault_missing", "No vault.");
      mockState.unlocked = true;
      mockState.mode = "real";
      recordEvent("unlock_success", "success");
      data = vaultState();
      break;
    case "vault_lock":
      mockState.unlocked = false;
      mockState.mode = "real";
      recordEvent("lock", "info");
      data = vaultState();
      break;
    case "vault_change_passphrase":
      requireUnlocked();
      mockState.generation += 1;
      recordEvent("rekey", "success");
      data = vaultState();
      break;
    case "settings_get":
      requireUnlocked();
      data = { settings: structuredClone(settings) };
      break;
    case "settings_update": {
      requireUnlocked();
      const patch = payload as SettingsPatch;
      settings = { ...settings, ...patch };
      recordEvent("settings_changed", "success");
      data = { settings: structuredClone(settings) };
      break;
    }
    case "security_status":
      requireUnlocked();
      data = { security: securitySummary() };
      break;
    case "accounts_list": {
      requireUnlocked();
      const includeArchived = (payload as { includeArchived?: boolean }).includeArchived ?? false;
      data = {
        accounts: mockAccounts().filter((account) => includeArchived || !account.archived),
      };
      break;
    }
    case "account_create":
      requireUnlocked();
      data = createMockAccount(payload as AccountPayload);
      break;
    case "account_update": {
      requireUnlocked();
      const update = payload as { id: string; account: AccountPayload };
      data = updateMockAccount(update.id, update.account);
      break;
    }
    case "account_archive": {
      requireUnlocked();
      const archive = payload as { id: string; archived: boolean };
      data = archiveMockAccount(archive.id, archive.archived);
      break;
    }
    case "app_quit":
      mockState.unlocked = false;
      mockState.mode = "real";
      recordEvent("lock", "info");
      data = { quitting: true };
      break;
    case "demo_open":
      mockState.mode = "demo";
      mockState.vaultExists = true;
      mockState.unlocked = true;
      recordEvent("demo_reset", "info");
      data = vaultState();
      break;
    case "demo_close":
      mockState.mode = "real";
      mockState.unlocked = false;
      recordEvent("lock", "info");
      data = vaultState();
      break;
    case "demo_reset":
      resetMockData();
      mockState.dashboardVersion += 1;
      mockState.mode = "demo";
      mockState.vaultExists = true;
      mockState.unlocked = true;
      recordEvent("demo_reset", "success");
      data = vaultState();
      break;
    case "dashboard_get": {
      requireUnlocked();
      const request = payload as { month?: string | null };
      data = mockDashboard(mockState.mode, request.month);
      break;
    }
    case "ledger_query": {
      requireUnlocked();
      data = mockLedger(payload as Parameters<typeof mockLedger>[0]);
      break;
    }
    case "security_events_list": {
      requireUnlocked();
      const limit = (payload as { limit?: number }).limit ?? 20;
      data = { events: structuredClone(mockState.events.slice(0, limit)) };
      break;
    }
    default: {
      const exhaustive: never = known;
      throw new BridgeError("unknown_command", `Unhandled command: ${String(exhaustive)}`);
    }
  }
  return { apiVersion: API_VERSION, data };
}

/** Test hook used by browser tests to simulate vault state changes. */
export function __setMockVaultState(next: Partial<typeof mockState>) {
  Object.assign(mockState, next);
}

/** Test hook: change mock settings between tests. */
export function __setMockSettings(patch: Partial<Settings>): void {
  settings = { ...settings, ...patch };
}

/** Test hook: return the mock to its default locked state between tests. */
export function __resetMockBridge(): void {
  const next = initialState();
  mockState.vaultExists = next.vaultExists;
  mockState.unlocked = false;
  mockState.generation = 1;
  mockState.events = [];
  mockState.mode = "real";
  mockState.dashboardVersion += 1;
  resetMockData();
  settings = { ...defaultSettings };
}

// A unique marker used by scripts/check-release-artifacts.mjs to prove this
// module is excluded from production bundles. Do not remove.
export const MOCK_BRIDGE_SENTINEL_DO_NOT_SHIP = "mock-bridge-sentinel-4c1f";
