import { useCallback, useEffect, useState } from "react";
import { BrandMark } from "../components/BrandMark";
import { BrandName } from "../components/BrandName";
import { ErrorBoundary } from "../components/ErrorBoundary";
import { AppShell } from "./AppShell";
import { ThemeToggle } from "../components/ThemeToggle";
import { VaultGate } from "../features/vault/VaultGate";
import { OnboardingWizard } from "../features/onboarding/OnboardingWizard";
import {
  command,
  usingMockBridge,
  type AppInfo,
  type Settings,
  type VaultState,
} from "../lib/bridge";
import { friendlyError } from "../lib/bridge/errors";
import { logEvent } from "../lib/logging";
import { applyTheme } from "../theme/apply";
import { DEFAULT_THEME_ID, getTheme, resolveMode } from "../theme";

type Boot =
  | { kind: "loading" }
  | { kind: "ready"; info: AppInfo; vault: VaultState }
  | { kind: "error"; message: string };

const THEME_MEDIA = "(prefers-color-scheme: dark)";

export function App() {
  const [boot, setBoot] = useState<Boot>({ kind: "loading" });
  const [notice, setNotice] = useState<string | null>(null);
  const [sessionNonce, setSessionNonce] = useState(0);
  // Theme ids are not sensitive; remembering one keeps the lock screen in the
  // same visual language after the vault settings are cleared on lock.
  const [rememberedThemeId, setRememberedThemeId] = useState(DEFAULT_THEME_ID);

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

  const themePreference: Settings["theme"] =
    boot.kind === "ready" ? (boot.vault.settings?.theme ?? "system") : "system";
  const bootThemeId = boot.kind === "ready" ? boot.vault.settings?.themeId : undefined;
  const theme = getTheme(bootThemeId ?? rememberedThemeId);

  useEffect(() => {
    if (bootThemeId) setRememberedThemeId(bootThemeId);
  }, [bootThemeId]);

  useEffect(() => {
    const media = window.matchMedia?.(THEME_MEDIA);
    const apply = () => applyTheme(theme, resolveMode(themePreference, media?.matches ?? false));
    apply();
    if (themePreference !== "system" || !media) return;
    media.addEventListener?.("change", apply);
    return () => media.removeEventListener?.("change", apply);
  }, [theme, themePreference]);

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
      current.kind === "ready" ? { ...current, vault: { ...current.vault, settings } } : current,
    );
  }, []);

  const handleTheme = useCallback(
    async (next: Settings["theme"]) => {
      if (boot.kind !== "ready" || !boot.vault.unlocked) {
        applyTheme(theme, resolveMode(next, window.matchMedia?.(THEME_MEDIA).matches ?? false));
        return;
      }
      try {
        const result = await command("settings_update", { theme: next });
        handleSettingsChanged(result.settings);
      } catch (error) {
        setNotice(friendlyError(error));
      }
    },
    [boot, handleSettingsChanged, theme],
  );

  const handleDemoOpen = useCallback(async () => {
    try {
      const vault = await command("demo_open", {});
      setNotice("Demo mode opened with synthetic data.");
      setBoot((current) => (current.kind === "ready" ? { ...current, vault } : current));
      setSessionNonce((value) => value + 1);
    } catch (error) {
      setNotice(friendlyError(error));
    }
  }, []);

  const handleDemoExit = useCallback(async () => {
    try {
      const vault = await command("demo_close", {});
      setNotice("Demo mode closed. The real vault is still locked.");
      setBoot((current) => (current.kind === "ready" ? { ...current, vault } : current));
      setSessionNonce((value) => value + 1);
    } catch (error) {
      setNotice(friendlyError(error));
    }
  }, []);

  const handleDemoReset = useCallback(async () => {
    try {
      const vault = await command("demo_reset", {});
      setNotice("Demo data was recreated.");
      setBoot((current) => (current.kind === "ready" ? { ...current, vault } : current));
      setSessionNonce((value) => value + 1);
    } catch (error) {
      setNotice(friendlyError(error));
    }
  }, []);

  if (boot.kind === "loading") {
    return (
      <main className="boot-screen" aria-labelledby="app-title">
        <BrandMark className="boot-mark" themeId={theme.id} />
        <h1 id="app-title">
          <BrandName />
        </h1>
        <p className="muted">Local-first, encrypted personal card ledger.</p>
        <p className="status" role="status">
          Opening the secure bridge…
        </p>
        <p className="fine-print">
          Your ledger password is collected by a native macOS sheet and never enters this window.
        </p>
      </main>
    );
  }

  if (boot.kind === "error") {
    return (
      <main className="boot-screen" role="alert">
        <BrandMark className="boot-mark" themeId={theme.id} />
        <h1>
          <BrandName />
        </h1>
        <p className="error-text">{boot.message}</p>
        <button type="button" className="btn btn-primary" onClick={() => window.location.reload()}>
          Retry
        </button>
        <p className="fine-print">
          No ledger data is displayed while the secure bridge is unavailable.
        </p>
      </main>
    );
  }

  if (!boot.vault.unlocked) {
    return (
      <>
        <VaultGate exists={boot.vault.exists} themeId={theme.id} onOpened={handleOpened} />
        <div className="gate-banner" role="status">
          {usingMockBridge ? "Mock desktop bridge — browser test mode." : ""}
        </div>
      </>
    );
  }

  if (boot.vault.settings && !boot.vault.settings.onboardingCompleted) {
    return (
      <OnboardingWizard
        settings={boot.vault.settings}
        onSettingsChanged={handleSettingsChanged}
        onFinished={handleSettingsChanged}
      />
    );
  }

  return (
    <ErrorBoundary>
      <AppShell
        info={boot.info}
        vault={boot.vault}
        theme={theme}
        usingMock={usingMockBridge}
        notice={notice}
        onDismissNotice={() => setNotice(null)}
        onLock={handleLock}
        onSettingsChanged={handleSettingsChanged}
        onExitDemo={handleDemoExit}
        onResetDemo={handleDemoReset}
        onOpenDemo={handleDemoOpen}
        onVaultChanged={handleOpened}
        themeControl={<ThemeToggle theme={themePreference} onChange={handleTheme} />}
        sessionNonce={sessionNonce}
      />
    </ErrorBoundary>
  );
}
