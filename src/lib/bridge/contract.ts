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

export type AppInfo = z.infer<typeof zAppInfo>;
export type VaultStatus = z.infer<typeof zVaultStatus>;
export type HealthCheck = z.infer<typeof zHealthCheck>;

export interface CommandDefs {
  app_info: { request: Record<string, never>; response: AppInfo };
  health_check: { request: Record<string, never>; response: HealthCheck };
  vault_status: { request: Record<string, never>; response: VaultStatus };
}

export type CommandName = keyof CommandDefs;
export type CommandRequest<K extends CommandName> = CommandDefs[K]["request"];
export type CommandResponse<K extends CommandName> = CommandDefs[K]["response"];

export const requestSchemas: { [K in CommandName]: z.ZodType<CommandDefs[K]["request"]> } = {
  app_info: zEmpty,
  health_check: zEmpty,
  vault_status: zEmpty,
};

export const responseSchemas: { [K in CommandName]: z.ZodType<CommandDefs[K]["response"]> } = {
  app_info: zAppInfo,
  health_check: zHealthCheck,
  vault_status: zVaultStatus,
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
