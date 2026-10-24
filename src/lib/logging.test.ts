import { describe, expect, it } from "vitest";
import { sanitizeMeta, redactForDisplay } from "./logging";

describe("redacting logger", () => {
  it("drops unknown and sensitive fields", () => {
    const safe = sanitizeMeta({
      command: "app_info",
      code: "ok",
      passphrase: "sentinel-passphrase",
      token: "sentinel-token",
      merchant: "Acme",
      path: "/Users/awychoi/secret",
    });
    expect(safe).toEqual({ command: "app_info", code: "ok" });
    expect(JSON.stringify(safe)).not.toContain("sentinel");
  });

  it("redacts sensitive-looking display strings", () => {
    expect(redactForDisplay("passphrase: correct horse")).toBe("[redacted]");
    expect(redactForDisplay("Overview")).toBe("Overview");
  });
});
