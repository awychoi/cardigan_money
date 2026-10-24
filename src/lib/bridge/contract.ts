import { z } from "zod";

/**
 * IPC contract, mirrored from Rust (`src-tauri/src/commands`).
 * The Rust gateway is authoritative. These schemas validate responses at the
 * renderer boundary so a contract drift fails loudly instead of silently
 * becoming `undefined` in the UI.
 *
 * Keep `AppInfo` and `VaultStatus` in sync with `fixtures/contract/*.json`,
 * which both test suites parse.
 */

export const API_VERSION = 1;
export const MAX_REQUEST_BYTES = 256 * 1024;

export const zEmpty = z.object({}).strict();

export const zAppInfo = z
  .object({
    name: z.literal("cardigan_money"),
    version: z.string().min(1).max(32),
    bundleIdentifier: z.literal("com.awychoi.cardiganmoney"),
    platform: z.string().min(1).max(32),
    apiVersion: z.literal(1),
    vaultExists: z.boolean(),
    vaultUnlocked: z.boolean(),
    demoAvailable: z.boolean(),
  })
  .strict();

export const zVaultStatus = z
  .object({
    exists: z.boolean(),
    unlocked: z.boolean(),
    mode: z.enum(["real", "demo"]),
  })
  .strict();

export const zHealthCheck = z
  .object({
    ok: z.literal(true),
    apiVersion: z.literal(1),
  })
  .strict();


export const zTheme = z.enum(["light", "dark", "system"]);
export const zCloseBehavior = z.enum(["unset", "hide", "quit"]);

export const zSettings = z
  .object({
    timezone: z.string().min(1).max(64),
    theme: zTheme,
    closeBehavior: zCloseBehavior,
    onboardingCompleted: z.boolean(),
    autoLockMinutes: z.number().int().min(1).max(240),
    lockOnSystemEvents: z.boolean(),
    contentProtection: z.boolean(),
    syncWindows: z.array(z.string().regex(/^\d{2}:\d{2}$/)).max(8),
    syncOnLaunch: z.boolean(),
    backupRetentionDaily: z.number().int().min(1).max(30),
    lastBackupAt: z.string().nullable(),
    lastBackupStatus: z.string().nullable(),
  })
  .strict();

export const zSecuritySummary = z
  .object({
    cipherVersion: z.string(),
    cipherProvider: z.string(),
    kdfSummary: z.string(),
    generation: z.number().int().positive(),
    schemaVersion: z.number().int().positive(),
    lastIntegrityCheckAt: z.string().nullable(),
    vaultIdPrefix: z.string(),
    cipherStatusSupported: z.boolean(),
  })
  .strict();

export const zVaultState = z
  .object({
    exists: z.boolean(),
    unlocked: z.boolean(),
    mode: z.enum(["real", "demo"]),
    settings: zSettings.nullable(),
    security: zSecuritySummary.nullable(),
  })
  .strict();

export const zSecurityEvent = z
  .object({
    id: z.string(),
    eventType: z.string(),
    outcome: z.enum(["success", "failure", "info"]),
    createdAt: z.string(),
    metadata: z.record(z.string(), z.unknown()).nullable(),
  })
  .strict();

export const zSettingsView = z.object({ settings: zSettings }).strict();
export const zSecurityView = z.object({ security: zSecuritySummary }).strict();
export const zEventsView = z.object({ events: z.array(zSecurityEvent) }).strict();

export const zSettingsPatch = z
  .object({
    timezone: z.string().min(1).max(64).optional(),
    theme: zTheme.optional(),
    closeBehavior: zCloseBehavior.optional(),
    onboardingCompleted: z.boolean().optional(),
    autoLockMinutes: z.number().int().min(1).max(240).optional(),
    syncWindows: z.array(z.string().regex(/^\d{2}:\d{2}$/)).max(8).optional(),
    syncOnLaunch: z.boolean().optional(),
    backupRetentionDaily: z.number().int().min(1).max(30).optional(),
  })
  .strict();

export const zEventsRequest = z
  .object({ limit: z.number().int().min(1).max(100).optional() })
  .strict();

export type Settings = z.infer<typeof zSettings>;
export type SettingsPatch = z.infer<typeof zSettingsPatch>;
export type SecuritySummary = z.infer<typeof zSecuritySummary>;
export type VaultState = z.infer<typeof zVaultState>;
export type SecurityEvent = z.infer<typeof zSecurityEvent>;
export type ThemeChoice = z.infer<typeof zTheme>;

export type AppInfo = z.infer<typeof zAppInfo>;
export type VaultStatus = z.infer<typeof zVaultStatus>;
export type HealthCheck = z.infer<typeof zHealthCheck>;

export interface CommandDefs {
  app_info: { request: Record<string, never>; response: AppInfo };
  health_check: { request: Record<string, never>; response: HealthCheck };
  vault_status: { request: Record<string, never>; response: VaultState };
  vault_create: { request: Record<string, never>; response: VaultState };
  vault_unlock: { request: Record<string, never>; response: VaultState };
  vault_lock: { request: Record<string, never>; response: VaultState };
  vault_change_passphrase: { request: Record<string, never>; response: VaultState };
  settings_get: { request: Record<string, never>; response: { settings: Settings } };
  settings_update: { request: SettingsPatch; response: { settings: Settings } };
  security_status: { request: Record<string, never>; response: { security: SecuritySummary } };
  security_events_list: { request: { limit?: number }; response: { events: SecurityEvent[] } };
}

export type CommandName = keyof CommandDefs;
export type CommandRequest<K extends CommandName> = CommandDefs[K]["request"];
export type CommandResponse<K extends CommandName> = CommandDefs[K]["response"];

export const requestSchemas: { [K in CommandName]: z.ZodType<CommandDefs[K]["request"]> } = {
  app_info: zEmpty,
  health_check: zEmpty,
  vault_status: zEmpty,
  vault_create: zEmpty,
  vault_unlock: zEmpty,
  vault_lock: zEmpty,
  vault_change_passphrase: zEmpty,
  settings_get: zEmpty,
  settings_update: zSettingsPatch,
  security_status: zEmpty,
  security_events_list: zEventsRequest,
};

export const responseSchemas: { [K in CommandName]: z.ZodType<CommandDefs[K]["response"]> } = {
  app_info: zAppInfo,
  health_check: zHealthCheck,
  vault_status: zVaultState,
  vault_create: zVaultState,
  vault_unlock: zVaultState,
  vault_lock: zVaultState,
  vault_change_passphrase: zVaultState,
  settings_get: zSettingsView,
  settings_update: zSettingsView,
  security_status: zSecurityView,
  security_events_list: zEventsView,
};

export const zGatewayEnvelope = z
  .object({
    apiVersion: z.literal(API_VERSION),
    data: z.unknown(),
  })
  .strict();

export const zErrorPayload = z
  .object({
    code: z.string().min(1).max(64),
    message: z.string().min(1).max(500),
  })
  .strict();

export function isCommandName(value: string): value is CommandName {
  return Object.prototype.hasOwnProperty.call(responseSchemas, value);
}
