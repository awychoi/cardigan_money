import { BridgeError } from "../lib/bridge/errors";
import { API_VERSION, isCommandName, type CommandName } from "../lib/bridge/contract";
import type { AppInfo, VaultStatus, HealthCheck } from "../lib/bridge/contract";

/**
 * Browser-only mock of the Rust gateway. It implements the same contract and
 * fails loudly on unknown commands. It is aliased out of production builds.
 */
const mockState = {
  vaultExists: true,
  vaultUnlocked: true,
};

const appInfo: AppInfo = {
  name: "cardigan_money",
  version: "0.1.0",
  bundleIdentifier: "com.awychoi.cardiganmoney",
  platform: "browser-mock",
  apiVersion: 1,
  vaultExists: mockState.vaultExists,
  vaultUnlocked: mockState.vaultUnlocked,
  demoAvailable: true,
};

const vaultStatus: VaultStatus = { exists: mockState.vaultExists, unlocked: mockState.vaultUnlocked, mode: "real" };
const healthCheck: HealthCheck = { ok: true, apiVersion: 1 };

export async function mockCommand(name: string, _payload: unknown): Promise<unknown> {
  if (!isCommandName(name)) {
    throw new BridgeError("unknown_command", "Unknown command.");
  }
  const known: CommandName = name;
  let data: unknown;
  switch (known) {
    case "app_info":
      data = { ...appInfo, vaultExists: mockState.vaultExists, vaultUnlocked: mockState.vaultUnlocked };
      break;
    case "health_check":
      data = healthCheck;
      break;
    case "vault_status":
      data = { ...vaultStatus, exists: mockState.vaultExists, unlocked: mockState.vaultUnlocked };
      break;
    default: {
      const exhaustive: never = known;
      throw new BridgeError("unknown_command", `Unhandled command: ${String(exhaustive)}`);
    }
  }
  return { apiVersion: API_VERSION, data };
}

/** Test hook used by browser tests to simulate lock/unlock flows later. */
export function __setMockVaultState(next: Partial<typeof mockState>) {
  Object.assign(mockState, next);
}

// A unique marker used by scripts/check-release-artifacts.mjs to prove this
// module is excluded from production bundles. Do not remove.
export const MOCK_BRIDGE_SENTINEL_DO_NOT_SHIP = "mock-bridge-sentinel-4c1f";
