import { useEffect, useState, type ReactNode } from "react";
import { command, type AppInfo, type Dashboard, type Settings, type VaultState } from "../lib/bridge";
import { friendlyError } from "../lib/bridge/errors";
import { OverviewView } from "../features/dashboard/OverviewView";
import { CardsView } from "../features/dashboard/CardsView";
import { LedgerView } from "../features/dashboard/LedgerView";
import { SettingsView } from "../features/settings/SettingsView";
import { Button } from "../components/Button";
import { formatMonthRange, monthLabel } from "../lib/formatting";

type View = "overview" | "cards" | "transactions" | "settings";

const VIEWS: { id: View; label: string }[] = [
  { id: "overview", label: "Overview" },
  { id: "cards", label: "Cards" },
  { id: "transactions", label: "Transactions" },
  { id: "settings", label: "Settings" },
];

export function AppShell({
  info,
  vault,
  usingMock,
  notice,
  onDismissNotice,
  onLock,
  onSettingsChanged,
  onExitDemo,
  onResetDemo,
  onOpenDemo,
  themeControl,
  sessionNonce,
}: {
  info: AppInfo;
  vault: VaultState;
  usingMock: boolean;
  notice: string | null;
  onDismissNotice: () => void;
  onLock: () => Promise<void>;
  onSettingsChanged: (settings: Settings) => void;
  onExitDemo: () => Promise<void>;
  onResetDemo: () => Promise<void>;
  onOpenDemo: () => Promise<void>;
  themeControl: ReactNode;
  sessionNonce: number;
}) {
  const [view, setView] = useState<View>("overview");
  const [month, setMonth] = useState<string | null>(null);
  const [dashboard, setDashboard] = useState<Dashboard | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  const [preset, setPreset] = useState<{ accountId: string | null; search: string | null; nonce: number }>({
    accountId: null,
    search: null,
    nonce: 0,
  });
  const [resetConfirm, setResetConfirm] = useState(false);
  const demo = vault.mode === "demo";

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    (async () => {
      try {
        const result = await command("dashboard_get", { month });
        if (!cancelled) setDashboard(result);
      } catch (caught) {
        if (!cancelled) setError(friendlyError(caught));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [month, refresh, sessionNonce]);

  function filterAccount(accountId: string) {
    setPreset({ accountId, search: null, nonce: Date.now() });
    setView("transactions");
  }

  function addAccount() {
    setView("settings");
  }

  return (
    <div className="app">
      <header className="header">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            cm
          </span>
          <span className="brand-name">cardigan_money</span>
          {demo ? <span className="status-pill status-warn">Demo</span> : null}
        </div>

        <div className="month-nav" aria-label="Selected calendar month">
          <button
            type="button"
            className="icon-btn"
            aria-label="Previous month"
            disabled={!dashboard}
            onClick={() => dashboard && setMonth(dashboard.previousMonth)}
          >
            ‹
          </button>
          <div className="month-display">
            <span className="month-label">{dashboard ? monthLabel(dashboard.month) : "—"}</span>
            <span className="month-range">
              {dashboard ? formatMonthRange(dashboard.startDate, dashboard.endDateExclusive) : ""}
            </span>
          </div>
          <button
            type="button"
            className="icon-btn"
            aria-label="Next month"
            disabled={!dashboard}
            onClick={() => dashboard && setMonth(dashboard.nextMonth)}
          >
            ›
          </button>
          <button
            type="button"
            className="btn btn-ghost btn-small"
            disabled={!dashboard || dashboard.month === dashboard.currentMonth}
            onClick={() => setMonth(null)}
          >
            This month
          </button>
        </div>

        <div className="header-actions">
          <span className="sync-state" role="status">
            <span className="dot dot-muted" aria-hidden="true" />
            {demo ? "Synthetic demo data" : "No provider connected"}
          </span>
          <button type="button" className="btn btn-secondary btn-small" disabled>
            Sync now
          </button>
          {themeControl}
          <Button variant="ghost" aria-label="Lock now" onClick={() => void onLock()}>
            {demo ? "Exit demo" : "Lock"}
          </Button>
        </div>
      </header>

      {demo ? (
        <div className="demo-banner demo-banner-strong" role="status">
          <strong>Demo mode — synthetic data.</strong>
          <span> Separate database. No real ledger, provider, Keychain, import, backup, or export.</span>
          <span className="banner-actions">
            {resetConfirm ? (
              <>
                <span>Reset demo data?</span>
                <button
                  type="button"
                  className="btn btn-small btn-danger"
                  onClick={() => {
                    setResetConfirm(false);
                    void onResetDemo();
                  }}
                >
                  Reset
                </button>
                <button type="button" className="btn btn-small btn-ghost" onClick={() => setResetConfirm(false)}>
                  Cancel
                </button>
              </>
            ) : (
              <>
                <button type="button" className="btn btn-small btn-ghost" onClick={() => setResetConfirm(true)}>
                  Reset demo
                </button>
                <button type="button" className="btn btn-small btn-ghost" onClick={() => void onExitDemo()}>
                  Exit demo
                </button>
              </>
            )}
          </span>
        </div>
      ) : null}

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
        {error ? (
          <section className="empty-state" role="alert">
            <h1>Dashboard unavailable</h1>
            <p className="error-text">{error}</p>
            <Button variant="primary" onClick={() => setRefresh((value) => value + 1)}>
              Retry
            </Button>
          </section>
        ) : loading && !dashboard ? (
          <p className="muted" role="status">
            Loading the encrypted ledger…
          </p>
        ) : dashboard ? (
          view === "overview" ? (
            <OverviewView dashboard={dashboard} onAddAccount={addAccount} />
          ) : view === "cards" ? (
            <CardsView dashboard={dashboard} onFilterAccount={filterAccount} />
          ) : view === "transactions" ? (
            <LedgerView
              dashboard={dashboard}
              presetAccountId={preset.accountId}
              presetSearch={preset.search}
              presetNonce={preset.nonce}
              onClearPreset={() => setPreset({ accountId: null, search: null, nonce: 0 })}
              onChanged={() => setRefresh((value) => value + 1)}
              readOnly={demo}
            />
          ) : (
            <SettingsView
              key={`${vault.mode}-${sessionNonce}`}
              vault={vault}
              onSettingsChanged={onSettingsChanged}
              onLock={onLock}
              onAccountsChanged={() => setRefresh((value) => value + 1)}
              onDataChanged={() => setRefresh((value) => value + 1)}
              demo={demo}
              onOpenDemo={onOpenDemo}
              onResetDemo={onResetDemo}
            />
          )
        ) : (
          <p className="muted">No dashboard data available.</p>
        )}
      </main>

      <footer className="footer">
        <span>Encrypted local ledger · no telemetry</span>
        <span>
          v{info.version} · SQLCipher {vault.security?.cipherVersion ?? ""}
          {demo ? " · synthetic demo" : ""}
        </span>
      </footer>
    </div>
  );
}
