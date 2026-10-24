import type { Dashboard } from "../../lib/bridge";
import {
  connectionLabel,
  formatDateOnly,
  formatMoney,
  formatMonthRange,
  monthLabel,
} from "../../lib/formatting";
import { Button } from "../../components/Button";

export function OverviewView({
  dashboard,
  onAddAccount,
}: {
  dashboard: Dashboard;
  onAddAccount: () => void;
}) {
  const { summary, overview } = dashboard;
  const missing = overview.balances.accountsMissingBalance;
  const pendingOnly = summary.netSpendingMinor === 0 && summary.pendingSubtotalMinor !== 0;
  const staleCount = overview.accounts.filter((account) => account.balanceStale).length;
  const errorConnections = overview.accounts.filter(
    (account) => account.connectionStatus === "error" || account.connectionStatus === "reconnect_required",
  ).length;

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
    <section className="dashboard" aria-labelledby="overview-title">
      <header className="page-heading">
        <div>
          <h1 id="overview-title">Overview</h1>
          <p className="muted">
            {monthLabel(dashboard.month)} · {formatMonthRange(dashboard.startDate, dashboard.endDateExclusive)}
          </p>
        </div>
        <span className="backend-note">Calendar-month totals from the encrypted ledger</span>
      </header>

      <div className="metrics" role="list" aria-label="Headline metrics">
        <article className="metric" role="listitem">
          <span className="metric-label">Net spending</span>
          <strong className="metric-value">{formatMoney(summary.netSpendingMinor)}</strong>
          <span className="metric-detail">
            {formatMoney(summary.grossPurchasesMinor)} purchases ·{" "}
            {formatMoney(summary.refundsMinor)} refunds · {formatMoney(summary.feesInterestMinor)} fees
          </span>
        </article>
        <article className="metric" role="listitem">
          <span className="metric-label">Current card balances</span>
          <strong className="metric-value">{formatMoney(overview.balances.knownTotalMinor)}</strong>
          <span className="metric-detail">
            {overview.balances.complete
              ? "All included USD accounts report a balance"
              : `${missing} account${missing === 1 ? "" : "s"} missing a balance — not treated as $0.00`}
          </span>
        </article>
        <article className="metric" role="listitem">
          <span className="metric-label">Upcoming reported payments</span>
          <strong className="metric-value">{formatMoney(overview.upcoming.knownTotalMinor)}</strong>
          <span className="metric-detail">
            Next {overview.upcoming.windowDays} days ·{" "}
            {overview.upcoming.unknownAmountCount > 0
              ? `${overview.upcoming.unknownAmountCount} amount(s) not reported`
              : "all reported amounts known"}
          </span>
        </article>
        <article className="metric" role="listitem">
          <span className="metric-label">Pending in this month</span>
          <strong className="metric-value">{formatMoney(summary.pendingSubtotalMinor)}</strong>
          <span className="metric-detail">Shown separately; excluded from net spending</span>
        </article>
      </div>

      <p className="fine-print block-note">
        Current balances and upcoming payments are current as of{" "}
        {formatDateOnly(overview.upcoming.asOfDate)}. Changing the spending month does not change them.
      </p>

      {overview.balances.otherCurrency.length > 0 ? (
        <p className="inline-notice" role="status">
          Other currencies are shown separately and never summed with USD:{" "}
          {overview.balances.otherCurrency
            .map(([currency, value]) => `${formatMoney(value, currency)} ${currency}`)
            .join(", ")}
          .
        </p>
      ) : null}

      {pendingOnly ? (
        <p className="inline-notice" role="status">
          This month has pending activity only. Nothing is counted as posted spending yet.
        </p>
      ) : null}

      {staleCount > 0 || errorConnections > 0 ? (
        <p className="inline-notice" role="status">
          {staleCount > 0 ? `${staleCount} account balance(s) are stale. ` : ""}
          {errorConnections > 0 ? `${errorConnections} connection(s) need attention. ` : ""}
          Last-known values are preserved and clearly labeled.
        </p>
      ) : null}

      <section className="panel" aria-labelledby="freshness-title">
        <h2 id="freshness-title">Account freshness</h2>
        <ul className="freshness-list">
          {overview.accounts.map((account) => (
            <li key={account.accountId}>
              <span className="freshness-name">
                {account.displayName}
                {account.lastFour ? ` · •••• ${account.lastFour}` : ""}
              </span>
              <span className={account.balanceStale ? "status-pill status-warn" : "status-pill status-ok"}>
                {account.currentBalanceMinor === null
                  ? "Balance unknown"
                  : account.balanceStale
                    ? `Stale · ${account.balanceAgeDays ?? "?"}d old`
                    : "Fresh"}
              </span>
              <span className="fine-print">
                {connectionLabel(account.connectionStatus, account.sourceKind)}
                {account.currentBalanceMinor !== null
                  ? ` · ${formatMoney(account.currentBalanceMinor, account.currency)}`
                  : ""}
              </span>
            </li>
          ))}
        </ul>
      </section>

      <p className="fine-print">
        {" "}
        Net spending excludes payments, transfers, excluded rows, and pending amounts. Refunds count
        in the month received. Supplier and category totals reconcile exactly to the eligible ledger
        rows in the backend.
      </p>
    </section>
  );
}
