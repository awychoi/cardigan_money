import { BridgeError } from "../lib/bridge/errors";
import {
  archiveMockAccount,
  createMockAccount,
  mockAccounts,
  mockChooseImportFile,
  mockCommitImport,
  mockCreateMerchantRule,
  mockDashboard,
  mockDeleteMerchantRule,
  mockExportCount,
  mockImportBatches,
  mockLedger,
  mockManualPayment,
  mockMerchantRules,
  mockRollbackImport,
  mockSetManualPayment,
  mockSetOverride,
  mockTransactionDetail,
  mockUpdateMerchantRule,
  mockValidateImport,
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
let mockBackups: import("../lib/bridge/contract").BackupItem[] = [];
const mockSyncRuns: import("../lib/bridge/contract").SyncRunView[] = [];
let mockPlaidConfigured = false;
let mockPlaidLink = {
  state: "idle",
  message: null as string | null,
  connectionId: null as string | null,
  expiresAt: null as string | null,
};
let mockPlaidConnection: import("../lib/bridge/contract").ConnectionView | null = null;

const defaultSettings: Settings = {
  timezone: "America/New_York",
  theme: "system",
  themeId: "ledger",
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
    case "transaction_detail":
      requireUnlocked();
      data = mockTransactionDetail((payload as { id: string }).id);
      break;
    case "transaction_override_set": {
      requireUnlocked();
      const request = payload as {
        id: string;
        override: {
          merchantAlias: string | null;
          category: string | null;
          typeCorrection: "purchase" | "refund" | "fee" | "interest" | "payment" | "transfer" | "other" | null;
          excluded: boolean | null;
          notes: string | null;
        };
      };
      data = mockSetOverride(request.id, request.override);
      break;
    }
    case "transaction_override_clear":
      requireUnlocked();
      data = mockSetOverride((payload as { id: string }).id, {
        merchantAlias: null,
        category: null,
        typeCorrection: null,
        excluded: null,
        notes: null,
      });
      break;
    case "merchant_rules_list":
      requireUnlocked();
      data = { rules: mockMerchantRules() };
      break;
    case "merchant_rule_create":
      requireUnlocked();
      data = mockCreateMerchantRule(
        payload as Parameters<typeof mockCreateMerchantRule>[0],
      );
      break;
    case "merchant_rule_update": {
      requireUnlocked();
      const request = payload as {
        id: string;
        rule: Parameters<typeof mockCreateMerchantRule>[0];
      };
      data = mockUpdateMerchantRule(request.id, request.rule);
      break;
    }
    case "merchant_rule_delete":
      requireUnlocked();
      mockDeleteMerchantRule((payload as { id: string }).id);
      data = { deleted: true };
      break;
    case "manual_payment_get":
      requireUnlocked();
      data = {
        payment: mockManualPayment((payload as { accountId: string }).accountId),
      };
      break;
    case "manual_payment_set":
      requireUnlocked();
      data = {
        payment: mockSetManualPayment(
          (payload as { accountId: string }).accountId,
          (payload as { payment: Parameters<typeof mockSetManualPayment>[1] }).payment,
        ),
      };
      break;
    case "manual_payment_clear":
      requireUnlocked();
      data = {
        payment: mockSetManualPayment((payload as { accountId: string }).accountId, {
          statementBalance: null,
          minimumDue: null,
          nextDueDate: null,
          validThroughDate: null,
        }),
      };
      break;
    case "import_choose_file":
      requireUnlocked();
      data = mockChooseImportFile((payload as { accountId: string }).accountId);
      break;
    case "import_validate":
      requireUnlocked();
      data = mockValidateImport((payload as { jobId: string }).jobId);
      break;
    case "import_commit": {
      requireUnlocked();
      const request = payload as { jobId: string; acceptReview: boolean };
      data = mockCommitImport(request.jobId, request.acceptReview);
      break;
    }
    case "import_cancel":
      data = { cancelled: true };
      break;
    case "import_batches_list":
      requireUnlocked();
      data = { batches: mockImportBatches() };
      break;
    case "import_rollback":
      requireUnlocked();
      data = mockRollbackImport((payload as { batchId: string }).batchId);
      break;
    case "export_ledger_csv": {
      requireUnlocked();
      const request = payload as { query: Parameters<typeof mockExportCount>[0] };
      data = { rows: mockExportCount(request.query), written: true };
      break;
    }
    case "backup_create": {
      requireUnlocked();
      const backup = {
        id: `cm-backup-manual-5a17c0de-gen1-20261023T120000Z-${crypto.randomUUID().slice(0, 4)}.cmbak`,
        kind: "manual",
        createdAt: "2026-10-23T12:00:00Z",
        sizeBytes: 4096,
        generation: 1,
        schemaVersion: 3,
      };
      mockBackups = [backup, ...mockBackups];
      data = { backup, pruned: 0 };
      break;
    }
    case "backup_list":
      requireUnlocked();
      data = { backups: structuredClone(mockBackups) };
      break;
    case "backup_delete": {
      requireUnlocked();
      const id = (payload as { id: string }).id;
      mockBackups = mockBackups.filter((item) => item.id !== id);
      data = { deleted: true };
      break;
    }
    case "backup_restore":
      requireUnlocked();
      data = {
        restored: true,
        kind: "manual",
        preRestoreBackupId: "cm-backup-pre-restore-5a17c0de-gen1-20261023T120000Z-0000.cmbak",
      };
      break;
    case "portable_export":
      requireUnlocked();
      data = {
        id: "cardigan-money-portable-2026-10-23.cmbak",
        kind: "portable",
        createdAt: "2026-10-23T12:00:00Z",
        sizeBytes: 4096,
        generation: 1,
        schemaVersion: 3,
      };
      break;
    case "crypto_erase":
      mockState.vaultExists = false;
      mockState.unlocked = false;
      mockState.mode = "real";
      mockState.events = [];
      mockBackups = [];
      resetMockData();
      data = vaultState();
      break;
    case "plaid_status":
      requireUnlocked();
      data = {
        plaid: {
          configured: mockPlaidConfigured,
          environment: "sandbox",
          linkAvailable: mockPlaidConfigured,
          message: mockPlaidConfigured
            ? "Sandbox credentials are stored in the macOS Keychain."
            : "Add Plaid Sandbox credentials in the native setup sheet.",
        },
      };
      break;
    case "plaid_setup":
      requireUnlocked();
      mockPlaidConfigured = true;
      data = {
        plaid: {
          configured: true,
          environment: "sandbox",
          linkAvailable: true,
          message: "Sandbox credentials are stored in the macOS Keychain.",
        },
      };
      break;
    case "plaid_test":
      requireUnlocked();
      if (!mockPlaidConfigured) throw new BridgeError("provider_not_configured", "No provider credentials.");
      data = {
        plaid: {
          configured: true,
          environment: "sandbox",
          linkAvailable: true,
          message: "Sandbox credentials are stored in the macOS Keychain.",
        },
      };
      break;
    case "plaid_link_start":
      requireUnlocked();
      mockPlaidLink = {
        state: "waiting",
        message: "Complete the Plaid Sandbox flow in your browser.",
        connectionId: null,
        expiresAt: "2026-10-24T00:30:00Z",
      };
      mockPlaidConnection = {
        id: "mock-plaid-connection",
        provider: "plaid",
        environment: "sandbox",
        institutionName: "Sandbox Bank",
        status: "healthy",
        lastAttemptAt: "2026-10-24T00:00:00Z",
        lastSuccessAt: "2026-10-24T00:00:00Z",
        sanitizedError: null,
        hasCursor: true,
        accountCount: 1,
      };
      data = { link: structuredClone(mockPlaidLink) };
      break;
    case "plaid_link_status":
      requireUnlocked();
      if (mockPlaidLink.state === "waiting") {
        mockPlaidLink = {
          ...mockPlaidLink,
          state: "linked",
          message: mockPlaidLink.connectionId
            ? "Sandbox Item reconnected. Syncing again."
            : "Sandbox Item linked. Syncing transactions and balances.",
          connectionId: mockPlaidLink.connectionId ?? "mock-plaid-connection",
        };
      }
      data = { link: structuredClone(mockPlaidLink) };
      break;
    case "plaid_link_cancel":
      requireUnlocked();
      mockPlaidLink = {
        state: "cancelled",
        message: "Plaid link cancelled.",
        connectionId: null,
        expiresAt: null,
      };
      data = { link: structuredClone(mockPlaidLink) };
      break;
    case "plaid_verification_report":
      requireUnlocked();
      data = {
        environment: "sandbox",
        connections: mockPlaidConnection
          ? [
              {
                institutionName: mockPlaidConnection.institutionName,
                environment: "sandbox",
                status: mockPlaidConnection.status,
                accountCount: mockPlaidConnection.accountCount,
                providerTransactionCount: 3,
                pendingTransactionCount: 1,
                lastSuccessAt: mockPlaidConnection.lastSuccessAt,
                cursorPresent: true,
              },
            ]
          : [],
        latestRunStatus: "success",
        latestLiabilitiesStatus: "partial",
        recentRuns: structuredClone(mockSyncRuns),
        generatedAt: "2026-10-24T00:00:00Z",
      };
      break;
    case "plaid_reconnect":
      requireUnlocked();
      mockPlaidLink = {
        state: "waiting",
        message: "Reconnect the Sandbox Item in your browser.",
        connectionId: (payload as { connectionId: string }).connectionId,
        expiresAt: "2026-10-24T00:30:00Z",
      };
      data = { link: structuredClone(mockPlaidLink) };
      break;
    case "sync_status":
      requireUnlocked();
      data = {
        connections: mockPlaidConnection ? [structuredClone(mockPlaidConnection)] : [],
        recentRuns: structuredClone(mockSyncRuns),
        runtime: {
          running: false,
          lastResult: mockSyncRuns[0]?.status ?? null,
          lastSkipReason: null,
          lastAttemptAt: mockSyncRuns[0]?.startedAt ?? null,
          lastFinishedAt: mockSyncRuns[0]?.finishedAt ?? null,
        },
        providerLive: false,
      };
      break;
    case "sync_now":
      requireUnlocked();
      data = {
        accepted: true,
        skipReason: "no_connections",
        results: [],
        startedAt: "2026-10-23T12:00:00Z",
        finishedAt: "2026-10-23T12:00:01Z",
      };
      break;
    case "sync_runs_list":
      requireUnlocked();
      data = { runs: structuredClone(mockSyncRuns) };
      break;
    case "connection_disconnect":
      requireUnlocked();
      data = { deleted: true };
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
  mockBackups = [];
  mockState.dashboardVersion += 1;
  resetMockData();
  settings = { ...defaultSettings };
}

// A unique marker used by scripts/check-release-artifacts.mjs to prove this
// module is excluded from production bundles. Do not remove.
export const MOCK_BRIDGE_SENTINEL_DO_NOT_SHIP = "mock-bridge-sentinel-4c1f";
