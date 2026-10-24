/**
 * Redacting logger. Off by default.
 *
 * Rules:
 *  - never log financial payloads, descriptions, merchant names, account numbers,
 *    passphrases, tokens, keys, SQL, or filesystem paths;
 *  - only an allowlisted set of event names and primitive metadata fields;
 *  - development console output is opt-in with VITE_DEBUG_LOGS=1.
 */
const ALLOWED_FIELDS = new Set([
  "command",
  "code",
  "phase",
  "status",
  "count",
  "ok",
  "view",
  "theme",
  "mode",
  "durationMs",
]);

const SENSITIVE_PATTERNS = [
  /passphrase/i,
  /password/i,
  /secret/i,
  /token/i,
  /credit.?card/i,
  /account.?number/i,
  /ssn/i,
  /private.?key/i,
];

export function sanitizeMeta(meta: Record<string, unknown>): Record<string, string | number | boolean | null> {
  const out: Record<string, string | number | boolean | null> = {};
  for (const [key, value] of Object.entries(meta)) {
    if (!ALLOWED_FIELDS.has(key)) continue;
    if (SENSITIVE_PATTERNS.some((pattern) => pattern.test(key))) continue;
    if (value === null || typeof value === "boolean" || typeof value === "number") {
      out[key] = value;
    } else if (typeof value === "string" && /^[a-z0-9_.:-]{0,64}$/i.test(value)) {
      out[key] = value;
    }
  }
  return out;
}

const enabled = import.meta.env.DEV && import.meta.env.VITE_DEBUG_LOGS === "1";

export function logEvent(event: string, meta: Record<string, unknown> = {}): void {
  if (!enabled) return;
  const safeEvent = /^[a-z0-9_.-]{1,64}$/i.test(event) ? event : "invalid_event";
  console.debug(`[${safeEvent}]`, sanitizeMeta(meta));
}

export function redactForDisplay(value: string): string {
  return SENSITIVE_PATTERNS.some((pattern) => pattern.test(value)) ? "[redacted]" : value;
}
