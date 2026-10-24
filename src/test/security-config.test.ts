// @vitest-environment node
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";

const root = resolve(__dirname, "../..");

function read(relative: string): string {
  return readFileSync(join(root, relative), "utf8");
}

function walk(dir: string, extensions: string[]): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const stat = statSync(full);
    if (stat.isDirectory()) out.push(...walk(full, extensions));
    else if (extensions.some((ext) => entry.endsWith(ext))) out.push(full);
  }
  return out;
}

describe("Tauri security baseline", () => {
  const config = JSON.parse(read("src-tauri/tauri.conf.json"));
  const csp: string = config.app.security.csp;
  const capability = JSON.parse(read("src-tauri/capabilities/main-window.json"));

  it("release CSP has no HTTPS source and no unsafe-eval", () => {
    expect(csp).not.toMatch(/https:/i);
    expect(csp).not.toMatch(/unsafe-eval/i);
    expect(csp).not.toMatch(/unsafe-inline[^;]*script-src/i);
    expect(csp).toContain("default-src 'none'");
    expect(csp).toContain("script-src 'self'");
    expect(csp).toMatch(/connect-src ipc: http:\/\/ipc\.localhost/);
    expect(csp).not.toMatch(/http:\/\/(?!ipc\.localhost)/);
  });

  it("dev CSP only allows the loopback Vite server", () => {
    const devCsp: string = config.app.security.devCsp;
    expect(devCsp).not.toMatch(/https:/i);
    expect(devCsp).not.toMatch(/http:\/\/(?!127\.0\.0\.1:1420|ipc\.localhost)/);
    expect(devCsp).not.toMatch(/unsafe-eval/i);
  });

  it("hardening options are enabled", () => {
    expect(config.app.security.freezePrototype).toBe(true);
    expect(config.app.withGlobalTauri).toBe(false);
    expect(config.app.security.assetProtocol.enable).toBe(false);
    expect(config.app.security.dangerousDisableAssetCspModification).toBe(false);
    expect(config.app.security.capabilities).toEqual(["main-window"]);
    expect(config.identifier).toBe("com.awychoi.cardiganmoney");
    expect(config.productName).toBe("cardigan_money");
  });

  it("capability is main-window only and has no forbidden plugin permissions", () => {
    expect(capability.windows).toEqual(["main"]);
    const forbidden = [
      "fs:",
      "shell:",
      "http:",
      "opener:",
      "sql:",
      "store:",
      "process:",
      "updater:",
      "websocket:",
      "upload:",
      "clipboard:",
      "notification:",
      "deep-link:",
      "autostart:",
      "global-shortcut:",
      "dialog:",
      "path:",
    ];
    for (const permission of capability.permissions as string[]) {
      for (const prefix of forbidden) {
        expect(permission.startsWith(prefix), `forbidden permission ${permission}`).toBe(false);
      }
    }
    expect(capability.permissions).toEqual(["core:event:default"]);
    expect(JSON.stringify(capability)).not.toContain("remote");
  });

  it("source has no HTML injection or dynamic-evaluation sinks", () => {
    const files = walk(join(root, "src"), [".ts", ".tsx", ".css"]);
    for (const file of files) {
      if (file.includes(".test.")) continue;
      const source = readFileSync(file, "utf8");
      expect(source, file).not.toContain("dangerouslySetInnerHTML");
      expect(source, file).not.toContain("innerHTML");
      expect(source, file).not.toMatch(/\beval\s*\(/);
      expect(source, file).not.toContain("new Function");
      expect(source, file).not.toContain("document.write");
      expect(source, file).not.toMatch(/<iframe/i);
    }
  });

  it("UI source contains no network fetch and no window.open", () => {
    const files = walk(join(root, "src"), [".ts", ".tsx"]);
    for (const file of files) {
      if (file.includes(".test.")) continue;
      const source = readFileSync(file, "utf8");
      expect(source, file).not.toMatch(/\bfetch\s*\(/);
      expect(source, file).not.toMatch(/XMLHttpRequest/);
      expect(source, file).not.toMatch(/window\.open\s*\(/);
      expect(source, file).not.toMatch(/localStorage\.setItem\((?!THEME_KEY)/);
    }
  });
});
