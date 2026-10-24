import { BridgeError } from "../lib/bridge/errors";
import {
  API_VERSION,
  isCommandName,
  type CommandName,
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
function initialState() {
  const params = new URLSearchParams(window.location.search);
  const empty = params.get("mockVault") === "empty";
  return {
    vaultExists: !empty,
    unlocked: false,
    generation: 1,
    events: [] as SecurityEvent[],
  };
}

const mockState = initialState();

const defaultSettings: Settings = {
  timezone: "America/New_York",
  theme: "system",
  closeBehavior: "unset",
  onboardingCompleted: false,
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
    mode: "real",
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
      recordEvent("vault_created", "success");
      data = vaultState();
      break;
    case "vault_unlock":
      if (!mockState.vaultExists) throw new BridgeError("vault_missing", "No vault.");
      mockState.unlocked = true;
      recordEvent("unlock_success", "success");
      data = vaultState();
      break;
    case "vault_lock":
      mockState.unlocked = false;
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

/** Test hook: return the mock to its default locked state between tests. */
export function __resetMockBridge(): void {
  const next = initialState();
  mockState.vaultExists = next.vaultExists;
  mockState.unlocked = false;
  mockState.generation = 1;
  mockState.events = [];
  settings = { ...defaultSettings };
}

// A unique marker used by scripts/check-release-artifacts.mjs to prove this
// module is excluded from production bundles. Do not remove.
export const MOCK_BRIDGE_SENTINEL_DO_NOT_SHIP = "mock-bridge-sentinel-4c1f";
