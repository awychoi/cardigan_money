import { useEffect, useState, type ReactNode } from "react";
import { command, type AppInfo, type Settings, type VaultState } from "../lib/bridge";
import { SettingsView } from "../features/settings/SettingsView";
import { Button } from "../components/Button";

type View = "overview" | "cards" | "transactions" | "suppliers" | "settings";

const VIEWS: { id: View; label: string }[] = [
  { id: "overview", label: "Overview" },
  { id: "cards", label: "Cards" },
  { id: "transactions", label: "Transactions" },
  { id: "suppliers", label: "Suppliers" },
  { id: "settings", label: "Settings" },
];

const MONTH_LABEL = new Intl.DateTimeFormat(undefined, { month: "long", year: "numeric" });

export function AppShell({
  info,
  vault,
  usingMock,
  notice,
  onDismissNotice,
  onLock,
  onRefreshVault,
  onSettingsChanged,
  themeControl,
}: {
  info: AppInfo;
  vault: VaultState;
  usingMock: boolean;
  notice: string | null;
  onDismissNotice: () => void;
  onLock: () => Promise<void>;
  onRefreshVault: () => Promise<void>;
  onSettingsChanged: (settings: Settings) => void;
  themeControl: ReactNode;
}) {
  const [view, setView] = useState<View>("overview");
  const [accountCount, setAccountCount] = useState<number | null>(null);
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const result = await command("accounts_list", {});
        if (!cancelled) setAccountCount(result.accounts.length);
      } catch {
        if (!cancelled) setAccountCount(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [view]);
  const now = new Date();
  const monthStart = new Date(now.getFullYear(), now.getMonth(), 1);
  const monthEnd = new Date(now.getFullYear(), now.getMonth() + 1, 0);

  return (
    <div className="app">
      <header className="header">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">cm</span>
          <span className="brand-name">cardigan_money</span>
        </div>

        <div className="month-nav" aria-label="Selected calendar month">
          <button type="button" className="icon-btn" aria-label="Previous month" disabled>
            ‹
          </button>
          <div className="month-display">
            <span className="month-label">{MONTH_LABEL.format(monthStart)}</span>
            <span className="month-range">
              {monthStart.toLocaleDateString()} – {monthEnd.toLocaleDateString()}
            </span>
          </div>
          <button type="button" className="icon-btn" aria-label="Next month" disabled>
            ›
          </button>
          <button type="button" className="btn btn-ghost btn-small" disabled>
            This month
          </button>
        </div>

        <div className="header-actions">
          <span className="sync-state" role="status">
            <span className="dot dot-muted" aria-hidden="true" /> No provider connected
          </span>
          <button type="button" className="btn btn-secondary btn-small" disabled>
            Sync now
          </button>
          {themeControl}
          <Button variant="ghost" aria-label="Lock now" onClick={() => void onLock()}>
            Lock
          </Button>
        </div>
      </header>

      {usingMock ? (
        <div className="demo-banner" role="status">
          Mock desktop bridge — browser test mode. No real ledger is open.
        </div>
      ) : null}

      {notice ? (
        <div className="notice-bar" role="status">
          <span>{notice}</span>
          <button type="button" className="icon-btn" aria-label="Dismiss notice" onClick={onDismissNotice}>
            ×
          </button>
        </div>
      ) : null}

      <nav className="nav" aria-label="Primary">
        {VIEWS.map((item) => (
          <button
            key={item.id}
            type="button"
            className={`nav-item${view === item.id ? " nav-item-active" : ""}`}
            aria-current={view === item.id ? "page" : undefined}
            onClick={() => setView(item.id)}
          >
            {item.label}
          </button>
        ))}
      </nav>

      <main className="content" id="main-content">
        {view === "settings" ? (
          <SettingsView vault={vault} onSettingsChanged={onSettingsChanged} onLock={onLock} />
        ) : view === "overview" ? (
          <section className="empty-state" aria-labelledby="overview-title">
            <h1 id="overview-title">Your cards, in one quiet place.</h1>
            {accountCount === 0 ? (
              <>
                <p className="muted">
                  No accounts yet. Add your first account manually; no bank connection is required,
                  and the app is usable with an empty ledger.
                </p>
                <div className="button-row">
                  <button
                    type="button"
                    className="btn btn-primary"
                    onClick={() => setView("settings")}
                  >
                    Add account
                  </button>
                </div>
              </>
            ) : (
              <p className="muted">
                The vault is unlocked. The calendar-month dashboard and the unified ledger arrive in
                the next build stage.
              </p>
            )}
            <p className="fine-print">
              Nothing on this screen contacts a network. Missing provider data is never shown as zero.
            </p>
            <button type="button" className="btn btn-ghost btn-small" onClick={() => void onRefreshVault()}>
              Refresh security status
            </button>
          </section>
        ) : (
          <section className="empty-state" aria-labelledby="placeholder-title">
            <h1 id="placeholder-title">
              {VIEWS.find((item) => item.id === view)?.label ?? "View"}
            </h1>
            <p className="muted">This view is not connected to the ledger yet.</p>
            <p className="fine-print">Build stage 2 — encrypted vault and lock lifecycle.</p>
          </section>
        )}
      </main>

      <footer className="footer">
        <span>Encrypted local ledger · no telemetry</span>
        <span>
          v{info.version} · SQLCipher {vault.security?.cipherVersion ?? ""}
        </span>
      </footer>
    </div>
  );
}
