import { useCallback, useEffect, useState } from "react";
import { ErrorBoundary } from "../components/ErrorBoundary";
import { AppShell } from "./AppShell";
import { ThemeToggle, type ThemeChoice } from "../components/ThemeToggle";
import { command, usingMockBridge, type AppInfo } from "../lib/bridge";
import { friendlyError } from "../lib/bridge/errors";
import { logEvent } from "../lib/logging";

const THEME_KEY = "cm.theme";
type BootState =
  | { kind: "loading" }
  | { kind: "ready"; info: AppInfo }
  | { kind: "error"; message: string };

function readStoredTheme(): ThemeChoice {
  try {
    const value = window.localStorage.getItem(THEME_KEY);
    return value === "light" || value === "dark" || value === "system" ? value : "system";
  } catch {
    return "system";
  }
}

export function App() {
  const [boot, setBoot] = useState<BootState>({ kind: "loading" });
  const [theme, setTheme] = useState<ThemeChoice>(readStoredTheme);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const info = await command("app_info", {});
        const health = await command("health_check", {});
        if (!cancelled && health.ok) {
          setBoot({ kind: "ready", info });
          logEvent("bridge_ready", { ok: true });
        }
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

  useEffect(() => {
    if (theme === "system") {
      document.documentElement.removeAttribute("data-theme");
    } else {
      document.documentElement.setAttribute("data-theme", theme);
    }
    try {
      // Non-financial preference only. Never store ledger data or secrets here.
      window.localStorage.setItem(THEME_KEY, theme);
    } catch {
      // Storage may be disabled; the theme still applies for this session.
    }
  }, [theme]);

  const changeTheme = useCallback((next: ThemeChoice) => setTheme(next), []);

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

  return (
    <ErrorBoundary>
      <AppShell
        info={boot.info}
        usingMock={usingMockBridge}
        themeControl={<ThemeToggle theme={theme} onChange={changeTheme} />}
      />
    </ErrorBoundary>
  );
}
