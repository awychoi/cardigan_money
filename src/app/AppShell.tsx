import { useState, type ReactNode } from "react";
import type { AppInfo } from "../lib/bridge";

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
  usingMock,
  themeControl,
}: {
  info: AppInfo;
  usingMock: boolean;
  themeControl: ReactNode;
}) {
  const [view, setView] = useState<View>("overview");
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
          <button type="button" className="btn btn-ghost" aria-label="Lock now" disabled>
            Lock
          </button>
        </div>
      </header>

      {usingMock ? (
        <div className="demo-banner" role="status">
          Mock desktop bridge — browser test mode. No real ledger is open.
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
        {view === "overview" ? (
          <section className="empty-state" aria-labelledby="overview-title">
            <h1 id="overview-title">Your cards, in one quiet place.</h1>
            <p className="muted">
              The secure shell is running{info.vaultExists ? " with an existing vault" : ""}. Account setup,
              the encrypted ledger, and the calendar-month dashboard arrive in the next stages.
            </p>
            <p className="fine-print">
              No financial data is loaded in this build stage. Nothing on this screen contacts a network.
            </p>
          </section>
        ) : (
          <section className="empty-state" aria-labelledby="placeholder-title">
            <h1 id="placeholder-title">
              {VIEWS.find((item) => item.id === view)?.label ?? "View"}
            </h1>
            <p className="muted">This view is not connected to the ledger yet.</p>
            <p className="fine-print">Build stage 1 — secure shell and typed bridge.</p>
          </section>
        )}
      </main>

      <footer className="footer">
        <span>Encrypted local ledger · no telemetry</span>
        <span>v{info.version}</span>
      </footer>
    </div>
  );
}
