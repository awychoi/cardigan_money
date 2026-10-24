export class BridgeError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "BridgeError";
    this.code = code;
  }
}

export function toBridgeError(error: unknown): BridgeError {
  if (error instanceof BridgeError) return error;
  if (typeof error === "object" && error !== null) {
    const candidate = error as { code?: unknown; message?: unknown };
    if (typeof candidate.code === "string" && typeof candidate.message === "string") {
      return new BridgeError(candidate.code, candidate.message);
    }
  }
  // Never surface raw exception text (it may contain internal details).
  return new BridgeError("bridge_error", "The desktop bridge rejected the request.");
}

const SAFE_UI_MESSAGES: Record<string, string> = {
  vault_missing: "No encrypted vault was found on this Mac.",
  vault_exists: "A vault already exists.",
  wrong_passphrase: "That passphrase did not unlock the vault.",
  vault_header_invalid: "The vault header is damaged or has been tampered with.",
  vault_header_missing: "The vault header is missing. A self-contained backup is required.",
  kdf_out_of_bounds: "The vault header contains unsafe key-derivation parameters.",
  cipher_unavailable: "The encrypted database engine is unavailable. Refusing to continue.",
  plaintext_database: "That file is not an encrypted ledger.",
  integrity_failure: "The ledger failed its encryption integrity check.",
  database_error: "The ledger could not be read or written.",
  io_error: "A local file operation failed.",
  vault_locked: "The vault is locked.",
  cancelled: "Cancelled.",
  invalid_input: "That request was not valid.",
  not_found: "Not found.",
  permission_denied: "The app refused an unsafe file or window permission.",
  unsupported: "Not supported in this build.",
  blocked: "That action is unavailable.",
  native_input_failed: "The native secure input sheet could not be shown.",
  keychain_error: "The Keychain operation failed. The offline ledger is still usable.",
  rate_limited: "Too many attempts. Wait a moment and try again.",
  schema_too_new: "This ledger was created by a newer app version.",
  unknown_command: "Unknown command.",
  contract_error: "The desktop bridge returned an unexpected response.",
  bridge_error: "The desktop bridge is unavailable.",
  internal_error: "An internal error occurred.",
};

/** Never display a raw server message. Codes map to fixed, safe strings. */
export function friendlyError(error: unknown): string {
  if (error instanceof BridgeError) {
    return SAFE_UI_MESSAGES[error.code] ?? "The request could not be completed.";
  }
  return "The request could not be completed.";
}
