import { useEffect, useMemo, useState } from "react";
import { command, type Dashboard, type SpendHistory } from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import {
  formatDateOnly,
  formatDateShort,
  formatMoney,
  formatMonthRange,
  monthLabel,
  parseDateOnly,
} from "../../lib/formatting";
import { Button } from "../../components/Button";
import type { Theme } from "../../theme/schema";
import { buildComposition, CategoryRing, RankedBars, toChartData, TrendBars } from "./charts";
import "./overview.css";

type AccountCardView = Dashboard["overview"]["accounts"][number];
type UpcomingEntry = Dashboard["overview"]["upcoming"]["entries"][number];

function addDays(date: string, days: number): string {
  const parsed = parseDateOnly(date);
  parsed.setDate(parsed.getDate() + days);
  return `${parsed.getFullYear()}-${String(parsed.getMonth() + 1).padStart(2, "0")}-${String(
    parsed.getDate(),
  ).padStart(2, "0")}`;
}

function dayNumber(date: string): string {
  return String(Number(date.slice(8)));
}

function balanceState(account: AccountCardView): { label: string; className: string } {
  if (account.currentBalanceMinor === null) {
    return { label: "Balance unknown", className: "status-warn" };
  }
  if (account.connectionStatus === "error" || account.connectionStatus === "reconnect_required") {
    return { label: "Needs attention", className: "status-warn" };
  }
  if (account.balanceStale) {
    return { label: `Stale · ${account.balanceAgeDays ?? "?"}d`, className: "status-warn" };
  }
  return { label: "Fresh", className: "status-ok" };
}

interface AttentionItem {
  id: string;
  title: string;
  detail: string;
  actionLabel?: string;
  onAction?: () => void;
}

export function OverviewView({
  dashboard,
  theme,
  onAddAccount,
  onOpenCards,
  onOpenSettings,
  onOpenTransactions,
}: {
  dashboard: Dashboard;
  theme: Theme;
  onAddAccount: () => void;
  onOpenCards: () => void;
  onOpenSettings: () => void;
  onOpenTransactions: () => void;
}) {
  const { summary, overview } = dashboard;
  const [history, setHistory] = useState<SpendHistory | null>(null);
  const [trendLoading, setTrendLoading] = useState(false);
  const [trendError, setTrendError] = useState<string | null>(null);

  const trendEnabled = theme.options.overview.trend !== "hidden";
  const showComposition = theme.options.overview.composition !== "hidden";
  const composition = useMemo(
    () => toChartData(buildComposition(summary.categories, theme.options.overview.topCategories)),
    [summary.categories, theme.options.overview.topCategories],
  );

  useEffect(() => {
    if (!trendEnabled) {
      setHistory(null);
      setTrendError(null);
      setTrendLoading(false);
      return;
    }
    let cancelled = false;
    setTrendLoading(true);
    setTrendError(null);
    (async () => {
      try {
        const result = await command("spend_history", {
          months: theme.options.overview.trendMonths,
          endMonth: dashboard.month,
        });
        if (!cancelled) setHistory(result);
      } catch (error) {
        if (!cancelled) {
          setTrendError(friendlyError(error));
          setHistory(null);
        }
      } finally {
        if (!cancelled) setTrendLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [dashboard.month, summary, trendEnabled, theme.options.overview.trendMonths]);

  const includedAccounts = overview.accounts.filter((account) => account.included);
  const staleCount = includedAccounts.filter((account) => account.balanceStale).length;
  const errorConnections = includedAccounts.filter(
    (account) =>
      account.connectionStatus === "error" || account.connectionStatus === "reconnect_required",
  ).length;
  const pendingOnly = summary.netSpendingMinor === 0 && summary.pendingSubtotalMinor !== 0;
  const totalAccounts =
    overview.balances.accountsWithBalance + overview.balances.accountsMissingBalance;
  const displayedAccounts = includedAccounts.slice(0, 4);
  const remainingAccounts = Math.max(0, includedAccounts.length - displayedAccounts.length);

  const entries = [...overview.upcoming.entries].sort(
    (a, b) => a.dueDate.localeCompare(b.dueDate) || a.displayName.localeCompare(b.displayName),
  );
  const nearestEntries = entries.slice(0, 4);
  const dueDays = Array.from({ length: overview.upcoming.windowDays }, (_, index) =>
    addDays(overview.upcoming.asOfDate, index),
  );
  const dueCounts = new Map<string, { count: number; unknown: boolean }>();
  for (const entry of entries) {
    const current = dueCounts.get(entry.dueDate) ?? { count: 0, unknown: false };
    current.count += 1;
    current.unknown = current.unknown || entry.amountMinor === null;
    dueCounts.set(entry.dueDate, current);
  }

  const attention: AttentionItem[] = [];
  if (errorConnections > 0) {
    attention.push({
      id: "connections",
      title: `${errorConnections} connection${errorConnections === 1 ? "" : "s"} need attention`,
      detail: "Balances may be stale until the connection is repaired.",
      actionLabel: "Open connections",
      onAction: onOpenSettings,
    });
  }
  if (staleCount > 0) {
    attention.push({
      id: "stale",
      title: `${staleCount} balance${staleCount === 1 ? "" : "s"} stale`,
      detail: "Last-known values are preserved and labeled, never treated as current.",
      actionLabel: "Review balances",
      onAction: onOpenCards,
    });
  }
  if (overview.balances.accountsMissingBalance > 0) {
    attention.push({
      id: "missing",
      title: `${overview.balances.accountsMissingBalance} account${
        overview.balances.accountsMissingBalance === 1 ? "" : "s"
      } missing a balance`,
      detail: "A missing balance is not the same as $0.00.",
      actionLabel: "Review balances",
      onAction: onOpenCards,
    });
  }
  if (overview.upcoming.unknownAmountCount > 0) {
    attention.push({
      id: "unknown-due",
      title: `${overview.upcoming.unknownAmountCount} payment amount${
        overview.upcoming.unknownAmountCount === 1 ? "" : "s"
      } not reported`,
      detail: "The due date is known; the amount is not.",
      actionLabel: "Review balances",
      onAction: onOpenCards,
    });
  }
  if (pendingOnly) {
    attention.push({
      id: "pending-only",
      title: "Pending activity only",
      detail: "Nothing is counted as posted spending yet.",
    });
  }
  const visibleAttention = attention.slice(0, 3);

  if (overview.accounts.length === 0) {
    return (
      <section className="empty-state" aria-labelledby="overview-title">
        <h1 id="overview-title">Your cards, in one quiet place.</h1>
        <p className="muted">No accounts yet. The app is usable with an empty ledger.</p>
        <Button variant="primary" onClick={onAddAccount}>
          Add account
        </Button>
        <p className="fine-print">No provider or network is required.</p>
      </section>
    );
  }

  return (
    <section className="dashboard overview" aria-labelledby="overview-title">
      <header className="page-heading">
        <div>
          <h1 id="overview-title">Overview</h1>
          <p className="muted">
            {monthLabel(dashboard.month)} ·{" "}
            {formatMonthRange(dashboard.startDate, dashboard.endDateExclusive)}
          </p>
        </div>
        <span className="backend-note">Calendar-month totals from the encrypted ledger</span>
      </header>

      <div className="overview-grid">
        <section className="overview-hero" aria-labelledby="hero-title">
          <h2 id="hero-title" className="hero-label">
            Net spending
          </h2>
          <p className="hero-value">{formatMoney(summary.netSpendingMinor)}</p>
          <p className="hero-arithmetic">
            {formatMoney(summary.grossPurchasesMinor)} purchases
            <span aria-hidden="true"> · </span>
            {formatMoney(Math.abs(summary.refundsMinor))} refunds
            <span aria-hidden="true"> · </span>
            {formatMoney(summary.feesInterestMinor)} fees
          </p>
          <p className="hero-pending">
            <span className="hero-pending-label">Pending in this month</span>
            <strong>{formatMoney(summary.pendingSubtotalMinor)}</strong>
            <span className="fine-print">shown separately</span>
          </p>
          {pendingOnly ? (
            <p className="hero-note">Pending activity only — nothing is posted yet.</p>
          ) : null}
          <div className="hero-foot">
            <dl className="hero-stats">
              <div>
                <dt>Eligible rows</dt>
                <dd>{summary.eligibleCount}</dd>
              </div>
              <div>
                <dt>Pending rows</dt>
                <dd>{summary.pendingCount}</dd>
              </div>
              <div>
                <dt>All rows</dt>
                <dd>{summary.rowCount}</dd>
              </div>
            </dl>
            <Button variant="ghost" className="btn-small" onClick={onOpenTransactions}>
              Open ledger
            </Button>
          </div>
        </section>

        <section className="overview-due" aria-labelledby="due-title">
          <header className="overview-panel-head">
            <div>
              <h2 id="due-title">Upcoming reported payments</h2>
              <p className="fine-print">
                Next {overview.upcoming.windowDays} days ·{" "}
                {overview.upcoming.unknownAmountCount > 0
                  ? `${overview.upcoming.unknownAmountCount} amount${
                      overview.upcoming.unknownAmountCount === 1 ? "" : "s"
                    } not reported`
                  : "all reported amounts known"}
              </p>
            </div>
            <strong className="due-total">
              {formatMoney(overview.upcoming.knownTotalMinor)}
              <span className="due-total-currency"> USD</span>
            </strong>
          </header>

          <ol className="due-strip" aria-hidden="true">
            {dueDays.map((day) => {
              const count = dueCounts.get(day);
              return (
                <li
                  className={`due-day${count ? " due-day-active" : ""}${
                    day === overview.upcoming.asOfDate ? " due-day-today" : ""
                  }`}
                  key={day}
                >
                  <span className="due-day-number">{dayNumber(day)}</span>
                  {count ? (
                    <span
                      className={`due-pip${count.unknown ? " due-pip-unknown" : ""}`}
                      title={`${count.count} payment${count.count === 1 ? "" : "s"}`}
                    />
                  ) : null}
                </li>
              );
            })}
          </ol>

          {nearestEntries.length > 0 ? (
            <ul className="due-list">
              {nearestEntries.map((entry: UpcomingEntry, index) => (
                <li className="due-item" key={`${entry.accountId}-${entry.dueDate}-${index}`}>
                  <span className="due-item-date">{formatDateShort(entry.dueDate)}</span>
                  <span className="due-item-name">
                    {entry.displayName}
                    {entry.lastFour ? ` · •••• ${entry.lastFour}` : ""}
                  </span>
                  <span
                    className={`due-item-amount${entry.amountMinor === null ? " due-unknown" : ""}`}
                  >
                    {entry.amountMinor === null ? (
                      <>
                        <span className="due-pip due-pip-unknown" aria-hidden="true" />
                        amount not reported
                      </>
                    ) : (
                      <>
                        {formatMoney(entry.amountMinor, entry.currency)}
                        {entry.currency !== "USD" ? ` ${entry.currency}` : ""}
                      </>
                    )}
                  </span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="chart-empty">
              No payments reported in the next {overview.upcoming.windowDays} days.
            </p>
          )}
          {overview.upcoming.nonUsdEntryCount > 0 ? (
            <p className="fine-print">Other currencies are listed separately.</p>
          ) : null}
        </section>

        {showComposition ? (
          <section
            className={`overview-panel overview-composition${
              trendEnabled ? "" : " overview-span-12"
            }`}
            aria-label="Spending composition"
          >
            {theme.options.overview.composition === "ring" ? (
              <CategoryRing
                data={composition}
                totalMinor={summary.grossPurchasesMinor + summary.feesInterestMinor}
                refundsMinor={summary.refundsMinor}
                netMinor={summary.netSpendingMinor}
              />
            ) : (
              <RankedBars data={composition} />
            )}
          </section>
        ) : null}

        {trendEnabled ? (
          <section
            className={`overview-panel overview-trend${showComposition ? "" : " overview-span-12"}`}
            aria-label="Spending trend"
          >
            {trendLoading && !history ? (
              <p className="chart-empty">Loading trend…</p>
            ) : trendError ? (
              <p className="inline-error" role="alert">
                Trend unavailable: {trendError}
              </p>
            ) : history ? (
              <TrendBars
                months={history.months}
                selectedMonth={dashboard.month}
                currentMonth={dashboard.currentMonth}
                currency={history.currency}
              />
            ) : null}
          </section>
        ) : null}

        <section
          className={`overview-balances${visibleAttention.length === 0 ? " overview-span-12" : ""}`}
          aria-label="Current card balances"
        >
          <article className="metric overview-balance-metric">
            <span className="metric-label">Current card balances</span>
            <strong className="metric-value">
              {formatMoney(overview.balances.knownTotalMinor)}
            </strong>
            <span className="metric-detail">
              {overview.balances.accountsWithBalance} of {totalAccounts} reported
              {overview.balances.complete ? " · all included USD accounts" : ""}
            </span>
          </article>

          <ul className="balance-list">
            {displayedAccounts.map((account) => {
              const state = balanceState(account);
              return (
                <li className="balance-row" key={account.accountId}>
                  <span className="balance-name">
                    {account.displayName}
                    {account.lastFour ? ` · •••• ${account.lastFour}` : ""}
                  </span>
                  <span className="balance-amount">
                    {account.currentBalanceMinor === null
                      ? "—"
                      : formatMoney(account.currentBalanceMinor, account.currency)}
                    {account.currentBalanceMinor !== null && account.currency !== "USD"
                      ? ` ${account.currency}`
                      : ""}
                  </span>
                  <span className={`status-pill ${state.className}`}>{state.label}</span>
                </li>
              );
            })}
          </ul>

          {remainingAccounts > 0 ? (
            <Button variant="ghost" className="btn-small" onClick={onOpenCards}>
              +{remainingAccounts} more in Cards
            </Button>
          ) : null}
        </section>

        {visibleAttention.length > 0 ? (
          <section className="overview-attention" aria-labelledby="attention-title">
            <h2 id="attention-title">Needs attention</h2>
            <ul className="attention-list">
              {visibleAttention.map((item) => (
                <li className="attention-item" key={item.id}>
                  <div>
                    <span className="attention-title">{item.title}</span>
                    <span className="attention-detail">{item.detail}</span>
                  </div>
                  {item.actionLabel && item.onAction ? (
                    <Button variant="ghost" className="btn-small" onClick={item.onAction}>
                      {item.actionLabel}
                    </Button>
                  ) : null}
                </li>
              ))}
            </ul>
          </section>
        ) : null}
      </div>

      <footer className="overview-footnote">
        <p className="fine-print">
          Current balances and upcoming payments are current as of{" "}
          {formatDateOnly(overview.upcoming.asOfDate)}. Changing the spending month does not change
          them.
        </p>
        {overview.balances.otherCurrency.length > 0 ? (
          <p className="fine-print overview-currency-note">
            Other currencies are shown separately and never summed with USD:{" "}
            {overview.balances.otherCurrency
              .map(([currency, value]) => `${formatMoney(value, currency)} ${currency}`)
              .join(", ")}
            .
          </p>
        ) : null}
      </footer>
    </section>
  );
}
