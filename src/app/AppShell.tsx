import { useEffect, useState, type ReactNode } from "react";
import { command, type AppInfo, type Dashboard, type Settings, type SyncStatus, type VaultState } from "../lib/bridge";
import { friendlyError } from "../lib/bridge/errors";
import { OverviewView } from "../features/dashboard/OverviewView";
import { CardsView } from "../features/dashboard/CardsView";
import { LedgerView } from "../features/dashboard/LedgerView";
import { SettingsView } from "../features/settings/SettingsView";
import { Button } from "../components/Button";
import { formatMonthRange, monthLabel } from "../lib/formatting";
import type { Theme } from "../theme/schema";

type View = "overview" | "cards" | "transactions" | "settings";

const VIEWS: { id: View; label: string }[] = [
  { id: "overview", label: "Overview" },
  { id: "cards", label: "Cards" },
  { id: "transactions", label: "Transactions" },
  { id: "settings", label: "Settings" },
];

const NAV_ICONS: Record<View, string> = {
  overview: "M2 2h5v5H2zM9 2h5v5H9zM2 9h5v5H2zM9 9h5v5H9z",
  cards: "M1.5 3.5h13v9h-13zM1.5 6.5h13",
  transactions: "M2 4h12M2 8h12M2 12h7",
  settings: "M2 5.5h12M2 10.5h12",
};

export function AppShell({
  info,
  vault,
  theme,
  usingMock,
  notice,
  onDismissNotice,
  onLock,
  onSettingsChanged,
  onExitDemo,
  onResetDemo,
  onOpenDemo,
  onVaultChanged,
  themeControl,
  sessionNonce,
}: {
  info: AppInfo;
  vault: VaultState;
  theme: Theme;
  usingMock: boolean;
  notice: string | null;
  onDismissNotice: () => void;
  onLock: () => Promise<void>;
  onSettingsChanged: (settings: Settings) => void;
  onExitDemo: () => Promise<void>;
  onResetDemo: () => Promise<void>;
  onOpenDemo: () => Promise<void>;
  onVaultChanged: (vault: VaultState) => void;
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
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [syncNote, setSyncNote] = useState<string | null>(null);
  const demo = vault.mode === "demo";
  const sidebar = theme.options.navigation === "sidebar";

  useEffect(() => {
    if (demo) {
      setSyncStatus(null);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const result = await command("sync_status", {});
        if (!cancelled) setSyncStatus(result);
      } catch {
        // Dashboard remains usable if status cannot be loaded; the panel shows details.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [demo, refresh, sessionNonce]);

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

  const connections = syncStatus?.connections ?? [];
  const connectionError = connections.some((connection) => connection.status !== "healthy");
  const latestSuccess = connections
    .map((connection) => connection.lastSuccessAt)
    .filter((value): value is string => Boolean(value))
    .sort()
    .at(-1);
  const syncLabel = demo
    ? "Synthetic demo data"
    : connections.length === 0
      ? "No provider connected"
      : connectionError
        ? "Provider needs attention"
        : `${connections.length} connection${connections.length === 1 ? "" : "s"} · ${latestSuccess ? `synced ${latestSuccess.slice(0, 10)}` : "not synced yet"}`;

  async function runSyncNow() {
    setSyncing(true);
    setSyncNote(null);
    try {
      const summary = await command("sync_now", {});
      if (!summary.accepted) {
        setSyncNote(
          summary.skipReason === "no_connections"
            ? "No provider connected."
            : `Sync skipped: ${summary.skipReason?.replaceAll("_", " ") ?? "unknown"}.`,
        );
      } else {
        const added = summary.results.reduce((sum, result) => sum + result.transactionsAdded, 0);
        const failed = summary.results.filter((result) => result.status === "failed").length;
        setSyncNote(`Sync complete: ${added} new transaction(s)${failed ? `, ${failed} failed` : ""}.`);
      }
      setRefresh((value) => value + 1);
    } catch (caught) {
      setSyncNote(friendlyError(caught));
    } finally {
      setSyncing(false);
    }
  }

  const brand = (
    <div className="brand">
      <span className="brand-mark" aria-hidden="true">
        cm
      </span>
      <span className="brand-name">cardigan_money</span>
      {demo ? <span className="status-pill status-warn">Demo</span> : null}
    </div>
  );

  const navigation = (
    <nav className={sidebar ? "side-nav" : "nav"} aria-label="Primary">
      {VIEWS.map((item) => (
        <button
          key={item.id}
          type="button"
          className={
            sidebar
              ? `side-nav-item${view === item.id ? " side-nav-item-active" : ""}`
              : `nav-item${view === item.id ? " nav-item-active" : ""}`
          }
          aria-current={view === item.id ? "page" : undefined}
          onClick={() => setView(item.id)}
        >
          {sidebar ? (
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
              <path d={NAV_ICONS[item.id]} />
            </svg>
          ) : null}
          {item.label}
        </button>
      ))}
    </nav>
  );

  const lockButton = (
    <Button variant="ghost" aria-label="Lock now" onClick={() => void onLock()}>
      {demo ? "Exit demo" : "Lock"}
    </Button>
  );

  return (
    <div className={`app ${sidebar ? "app-sidebar" : "app-topbar"}`}>
      {sidebar ? (
        <aside className="sidebar">
          {brand}
          {navigation}
          <div className="side-foot">
            <div className="side-status">
              <span className={connectionError && !demo ? "dot sync-dot-warn" : "dot dot-muted"} aria-hidden="true" />
              <span className="side-status-label">{demo ? "Demo ledger" : "Encrypted ledger"}</span>
            </div>
            {themeControl}
            {lockButton}
            <span className="side-version">
              v{info.version} · SQLCipher {vault.security?.cipherVersion ?? ""}
            </span>
          </div>
        </aside>
      ) : null}

      <div className="app-main">
        <header className="header">
          {sidebar ? null : brand}

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
            <span className="sync-state" role="status" title={syncNote ?? undefined}>
              <span
                className={connectionError && !demo ? "dot sync-dot-warn" : "dot dot-muted"}
                aria-hidden="true"
              />
              {syncNote ?? syncLabel}
            </span>
            {!demo && connections.length > 0 ? (
              <button
                type="button"
                className="btn btn-secondary btn-small"
                onClick={() => void runSyncNow()}
                disabled={syncing}
              >
                {syncing ? "Syncing…" : "Sync now"}
              </button>
            ) : (
              <button type="button" className="btn btn-secondary btn-small" disabled>
                Sync now
              </button>
            )}
            {sidebar ? null : themeControl}
            {sidebar ? null : lockButton}
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

        {sidebar ? null : navigation}

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
              <CardsView dashboard={dashboard} theme={theme} onFilterAccount={filterAccount} />
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
                onVaultChanged={onVaultChanged}
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
            Theme {theme.name} · v{info.version} · SQLCipher {vault.security?.cipherVersion ?? ""}
            {demo ? " · synthetic demo" : ""}
          </span>
        </footer>
      </div>
    </div>
  );
}
