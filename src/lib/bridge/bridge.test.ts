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

  it("keeps the restore preview and result free of paths and secret fields", async () => {
    await command("vault_unlock", {});
    const preview = await command("backup_restore_prepare", {});
    const previewText = JSON.stringify(preview).toLowerCase();
    expect(previewText).not.toContain("path");
    expect(previewText).not.toContain("passphrase");
    expect(previewText).not.toContain("password");
    expect(preview.backup.id).not.toContain("/");
    expect(preview.backup.id).not.toContain("\\");

    const result = await command("backup_restore_apply", {
      scope: "full",
      confirmation: "replace",
    });
    const resultText = JSON.stringify(result).toLowerCase();
    expect(resultText).not.toContain("path");
    expect(resultText).not.toContain("passphrase");
    expect(resultText).not.toContain("password");
    expect(result.preRestoreBackupId).toBeTruthy();
    expect(result.preRestoreProtection).toBe("current");
    expect(result.adoptedBackupKey).toBe(false);
  });

  it("requires the typed confirmation before a replacing restore", async () => {
    await command("vault_unlock", {});
    await command("backup_restore_prepare", {});
    await expect(command("backup_restore_apply", { scope: "full" })).rejects.toMatchObject({
      code: "invalid_input",
    });
  });

  it("keeps Sandbox and Trial/Production Plaid credentials and links separate", async () => {
    await command("vault_unlock", {});
    const initial = await command("plaid_status", {});
    expect(initial.plaid.sandbox.configured).toBe(false);
    expect(initial.plaid.production.configured).toBe(false);

    const saved = await command("plaid_setup", { environment: "production" });
    expect(saved.plaid.production.configured).toBe(true);
    expect(saved.plaid.sandbox.configured).toBe(false);
    await expect(command("plaid_test", { environment: "sandbox" })).rejects.toMatchObject({
      code: "provider_not_configured",
    });

    const started = await command("plaid_link_start", { environment: "production" });
    expect(started.link.environment).toBe("production");
    const linked = await command("plaid_link_status", {});
    expect(linked.link.state).toBe("linked");
    expect(linked.link.message).toContain("Trial / Production Item linked");

    const sync = await command("sync_status", {});
    expect(sync.connections).toHaveLength(1);
    expect(sync.connections[0].environment).toBe("production");
    expect(sync.providerLive).toBe(true);
  });

  it("removes the old portable-export action and keeps retired backups separate", async () => {
    expect(isCommandName("portable_export")).toBe(false);
    expect(isCommandName("backup_restore")).toBe(false);
    await command("vault_unlock", {});
    const retired = await command("backup_list_retired", {});
    expect(retired.backups).toEqual([]);
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
