import { describe, expect, it } from "vitest";
import { command, usingMockBridge } from "./index";
import { isCommandName, zAppInfo, zVaultState } from "./contract";
import { BridgeError, friendlyError, toBridgeError } from "./errors";

describe("bridge contract", () => {
  it("uses the mock only in development test mode", () => {
    expect(usingMockBridge).toBe(true);
  });

  it("round-trips a typed command", async () => {
    const info = await command("app_info", {});
    expect(zAppInfo.parse(info).name).toBe("cardigan_money");
    const status = await command("vault_status", {});
    expect(zVaultState.parse(status).mode).toBe("real");
  });

  it("rejects unknown command names before dispatch", async () => {
    expect(isCommandName("run_sql")).toBe(false);
    await expect(command("run_sql" as never, {})).rejects.toBeInstanceOf(BridgeError);
  });

  it("rejects an invalid request payload", async () => {
    await expect(command("app_info", { extra: true } as never)).rejects.toMatchObject({
      code: "invalid_input",
    });
  });

  it("round-trips spend history through the mock bridge", async () => {
    await command("vault_unlock", {});
    const history = await command("spend_history", { months: 12, endMonth: "2026-10" });
    expect(history.currency).toBe("USD");
    expect(history.startMonth).toBe("2025-11");
    expect(history.endMonth).toBe("2026-10");
    expect(history.months).toHaveLength(12);

    const dashboard = await command("dashboard_get", { month: "2026-10" });
    expect(history.months[11].netMinor).toBe(dashboard.summary.netSpendingMinor);
  });

  it("rejects invalid spend history requests", async () => {
    await expect(command("spend_history", { months: 0 })).rejects.toMatchObject({
      code: "invalid_input",
    });
    await expect(command("spend_history", { months: 25 })).rejects.toMatchObject({
      code: "invalid_input",
    });
    await expect(command("spend_history", { endMonth: "2026-13" })).rejects.toMatchObject({
      code: "invalid_input",
    });
  });

  it("normalizes unknown errors without leaking their text", () => {
    const err = toBridgeError(new Error("secret=abc123"));
    expect(err.code).toBe("bridge_error");
    expect(err.message).not.toContain("abc123");
  });

  it("never renders a raw server message, even if it contains a sentinel", () => {
    const sentinel = "SENTINEL-PASSPHRASE-do-not-render";
    const message = friendlyError(new BridgeError("wrong_passphrase", sentinel));
    expect(message).not.toContain(sentinel);
    expect(message).toMatch(/passphrase/i);
  });
});
