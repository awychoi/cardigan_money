import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import {
  API_VERSION,
  MAX_REQUEST_BYTES,
  isCommandName,
  requestSchemas,
  responseSchemas,
  zGatewayEnvelope,
  type CommandName,
  type CommandRequest,
  type CommandResponse,
} from "./contract";
import { BridgeError, toBridgeError } from "./errors";

/** Development/test only. The Vite config aliases @mock-bridge to a disabled
 * stub in production builds, so mock code is not shipped. */
export const usingMockBridge =
  import.meta.env.DEV && import.meta.env.VITE_MOCK_BRIDGE === "1";

export async function command<K extends CommandName>(
  name: K,
  payload: CommandRequest<K>,
): Promise<CommandResponse<K>> {
  if (!isCommandName(name)) {
    throw new BridgeError("unknown_command", "Unknown command.");
  }
  const requestPayload = requestSchemas[name].safeParse(payload);
  if (!requestPayload.success) {
    throw new BridgeError("invalid_input", "The request was invalid.");
  }
  const encoded = JSON.stringify(requestPayload.data);
  if (encoded.length > MAX_REQUEST_BYTES) {
    throw new BridgeError("invalid_input", "The request was too large.");
  }

  let raw: unknown;
  try {
    if (usingMockBridge) {
      const { mockCommand } = await import("@mock-bridge");
      raw = await mockCommand(name, requestPayload.data);
    } else {
      raw = await invoke("gateway", {
        request: { v: API_VERSION, command: name, payload: requestPayload.data },
      });
    }
  } catch (error) {
    throw toBridgeError(error);
  }

  const envelope = zGatewayEnvelope.safeParse(raw);
  if (!envelope.success) {
    throw new BridgeError("contract_error", "The desktop bridge returned an unexpected response.");
  }
  const parsed = responseSchemas[name].safeParse(envelope.data.data);
  if (!parsed.success) {
    throw new BridgeError("contract_error", "The desktop bridge response did not match the contract.");
  }
  return parsed.data as CommandResponse<K>;
}

/** Exposed for tests: validates an enveloped response for a command. */
export function validateEnvelopeFor(commandName: CommandName, raw: unknown): unknown {
  const envelope = zGatewayEnvelope.parse(raw);
  return responseSchemas[commandName].parse(envelope.data);
}

export { BridgeError };
export type {
  AppInfo,
  VaultStatus,
  HealthCheck,
  CommandName,
  VaultState,
  Settings,
  SettingsPatch,
  SecuritySummary,
  SecurityEvent,
  ThemeChoice,
  Account,
  AccountPayload,
} from "./contract";
export const _internals = { z } as const;
