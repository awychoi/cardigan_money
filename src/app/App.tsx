import { useCallback, useEffect, useState } from "react";
import { ErrorBoundary } from "../components/ErrorBoundary";
import { AppShell } from "./AppShell";
import { ThemeToggle } from "../components/ThemeToggle";
import { VaultGate } from "../features/vault/VaultGate";
import { command, usingMockBridge, type AppInfo, type Settings, type VaultState } from "../lib/bridge";
import { friendlyError } from "../lib/bridge/errors";
import { logEvent } from "../lib/logging";

type Boot =
  | { kind: "loading" }
  | { kind: "ready"; info: AppInfo; vault: VaultState }
  | { kind: "error"; message: string };

const THEME_MEDIA = "(prefers-color-scheme: dark)";

function applyTheme(theme: Settings["theme"]): void {
  if (theme === "system") {
    document.documentElement.removeAttribute("data-theme");
  } else {
    document.documentElement.setAttribute("data-theme", theme);
  }
}

export function App() {
  const [boot, setBoot] = useState<Boot>({ kind: "loading" });
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [info, health, vault] = await Promise.all([
          command("app_info", {}),
          command("health_check", {}),
          command("vault_status", {}),
        ]);
        if (!cancelled && health.ok) setBoot({ kind: "ready", info, vault });
      } catch (error) {
        if (cancelled) return;
        setBoot({ kind: "error", message: friendlyError(error) });
        logEvent("bridge_error", { ok: false });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const theme = boot.kind === "ready" ? (boot.vault.settings?.theme ?? "system") : "system";

  useEffect(() => {
    applyTheme(theme);
    if (theme !== "system") return;
    const media = window.matchMedia?.(THEME_MEDIA);
    if (!media) return;
    const onChange = () => applyTheme("system");
    media.addEventListener?.("change", onChange);
    return () => media.removeEventListener?.("change", onChange);
  }, [theme]);

  // The Rust side enforces the lock; this listener only clears the renderer.
  useEffect(() => {
    if (usingMockBridge) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const stop = await listen("vault:locked", () => {
          setNotice("The vault was locked.");
          setBoot((current) =>
            current.kind === "ready"
              ? {
                  ...current,
                  vault: { ...current.vault, unlocked: false, settings: null, security: null },
                }
              : current,
          );
        });
        if (cancelled) stop();
        else unlisten = stop;
      } catch {
        // If event subscription fails the UI still cannot read the ledger: the
        // Rust session is already closed and commands fail with vault_locked.
      }
    })();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const handleOpened = useCallback((vault: VaultState) => {
    setNotice(null);
    setBoot((current) => (current.kind === "ready" ? { ...current, vault } : current));
  }, []);

  const handleLock = useCallback(async () => {
    try {
      const vault = await command("vault_lock", {});
      setNotice("The vault is locked.");
      setBoot((current) => (current.kind === "ready" ? { ...current, vault } : current));
    } catch (error) {
      setNotice(friendlyError(error));
    }
  }, []);

  const handleSettingsChanged = useCallback((settings: Settings) => {
    setBoot((current) =>
      current.kind === "ready"
        ? { ...current, vault: { ...current.vault, settings } }
        : current,
    );
  }, []);

  const handleTheme = useCallback(
    async (next: Settings["theme"]) => {
      if (boot.kind !== "ready" || !boot.vault.unlocked) {
        applyTheme(next);
        return;
      }
      try {
        const result = await command("settings_update", { theme: next });
        handleSettingsChanged(result.settings);
      } catch (error) {
        setNotice(friendlyError(error));
      }
    },
    [boot, handleSettingsChanged],
  );

  const handleRefreshVault = useCallback(async () => {
    try {
      const vault = await command("vault_status", {});
      setBoot((current) => (current.kind === "ready" ? { ...current, vault } : current));
    } catch {
      // Leave the current view; the lock event or next command will correct it.
    }
  }, []);

  if (boot.kind === "loading") {
    return (
      <main className="boot-screen" aria-labelledby="app-title">
        <div className="boot-mark" aria-hidden="true">cm</div>
        <h1 id="app-title">cardigan_money</h1>
        <p className="muted">Local-first, encrypted personal card ledger.</p>
        <p className="status" role="status">Opening the secure bridge…</p>
        <p className="fine-print">
          Your ledger password is collected by a native macOS sheet and never enters this window.
        </p>
      </main>
    );
  }

  if (boot.kind === "error") {
    return (
      <main className="boot-screen" role="alert">
        <div className="boot-mark" aria-hidden="true">cm</div>
        <h1>cardigan_money</h1>
        <p className="error-text">{boot.message}</p>
        <button type="button" className="btn btn-primary" onClick={() => window.location.reload()}>
          Retry
        </button>
        <p className="fine-print">No ledger data is displayed while the secure bridge is unavailable.</p>
      </main>
    );
  }

  if (!boot.vault.unlocked) {
    return (
      <>
        <VaultGate exists={boot.vault.exists} onOpened={handleOpened} />
        <div className="gate-banner" role="status">
          {usingMockBridge ? "Mock desktop bridge — browser test mode." : ""}
        </div>
      </>
    );
  }

  return (
    <ErrorBoundary>
      <AppShell
        info={boot.info}
        vault={boot.vault}
        usingMock={usingMockBridge}
        notice={notice}
        onDismissNotice={() => setNotice(null)}
        onLock={handleLock}
        onRefreshVault={handleRefreshVault}
        onSettingsChanged={handleSettingsChanged}
        themeControl={<ThemeToggle theme={theme} onChange={handleTheme} />}
      />
    </ErrorBoundary>
  );
}
