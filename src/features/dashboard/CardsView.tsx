import type { Dashboard } from "../../lib/bridge";
import {
  connectionLabel,
  dueAmountLabel,
  formatDateOnly,
  formatMoney,
  monthLabel,
} from "../../lib/formatting";

export function CardsView({
  dashboard,
  onFilterAccount,
}: {
  dashboard: Dashboard;
  onFilterAccount: (accountId: string) => void;
}) {
  const spendFor = (accountId: string, currency: string) =>
    dashboard.accountSpend.find(
      (row) => row.accountId === accountId && row.currency === currency,
    );

  return (
    <section className="dashboard" aria-labelledby="cards-title">
      <header className="page-heading">
        <div>
          <h1 id="cards-title">Cards</h1>
          <p className="muted">
            {monthLabel(dashboard.month)} · monthly spending plus current balance and reported payment
            metadata
          </p>
        </div>
      </header>

      {dashboard.overview.accounts.length === 0 ? (
        <p className="muted">No accounts yet.</p>
      ) : (
        <div className="cards-grid">
          {dashboard.overview.accounts.map((account) => {
            const spend = spendFor(account.accountId, account.currency);
            const payment = account.payment;
            const dueText =
              payment.dueAmountKind === "no_due_date"
                ? "No due date reported"
                : `${formatDateOnly(payment.nextDueDate ?? "")} · ${dueAmountLabel(
                    payment.dueAmountKind,
                    payment.dueAmountMinor,
                    account.currency,
                  )}`;
            return (
              <button
                type="button"
                key={account.accountId}
                className="card-tile"
                onClick={() => onFilterAccount(account.accountId)}
                aria-label={`Filter transactions to ${account.displayName}`}
              >
                <span className="card-head">
                  <span>
                    <span className="card-issuer">{account.issuer}</span>
                    <span className="card-name">{account.displayName}</span>
                  </span>
                  <span className="card-mask">•••• {account.lastFour ?? "----"}</span>
                </span>

                <span className="card-balance">
                  {account.currentBalanceMinor === null
                    ? "Balance unknown"
                    : formatMoney(account.currentBalanceMinor, account.currency)}
                </span>
                <span className="fine-print">
                  {account.currentBalanceMinor === null
                    ? "No balance snapshot — not shown as $0.00"
                    : `as of ${account.balanceRetrievedAt ? formatDateOnly(account.balanceRetrievedAt.slice(0, 10)) : "unknown"}`}
                  {account.balanceStale ? " · stale" : ""}
                </span>

                <span className="card-row">
                  <span className="card-label">Month spend</span>
                  <span>{spend ? formatMoney(spend.netMinor, account.currency) : "—"}</span>
                </span>
                <span className="card-row">
                  <span className="card-label">Payment</span>
                  <span>{dueText}</span>
                </span>
                {payment.dueSource ? (
                  <span className="fine-print">
                    Due source: {payment.dueSource}
                    {payment.dueSource === "manual" ? " (dated manual entry)" : ""}
                    {payment.manualExpired ? " · manual entry expired" : ""}
                  </span>
                ) : null}
                <span className="card-row">
                  <span className="card-label">Last statement</span>
                  <span>
                    {payment.statementBalanceMinor === null
                      ? "Not reported"
                      : `${formatMoney(payment.statementBalanceMinor, account.currency)}${
                          payment.statementIssueDate
                            ? ` · ${formatDateOnly(payment.statementIssueDate)}`
                            : ""
                        }`}
                  </span>
                </span>
                <span className="fine-print">
                  A statement balance is not the remaining amount due.
                </span>
                {payment.nextClosingDate ? (
                  <span className="card-row">
                    <span className="card-label">Next closing</span>
                    <span>{formatDateOnly(payment.nextClosingDate)}</span>
                  </span>
                ) : null}
                {account.connectionStatus === "error" || account.connectionStatus === "reconnect_required" ? (
                  <span className="fine-print">
                    Some statement or payment metadata may be unavailable until this connection is
                    repaired. Missing values are never shown as $0.00.
                  </span>
                ) : null}
                <span className="card-foot">
                  <span
                    className={`status-pill ${
                      account.sourceKind === "manual"
                        ? "status-info"
                        : account.connectionStatus === "healthy"
                          ? "status-ok"
                          : account.connectionStatus === "error"
                            ? "status-fail"
                            : "status-warn"
                    }`}
                  >
                    {connectionLabel(account.connectionStatus, account.sourceKind)}
                  </span>
                  <span className="fine-print">Click to filter the ledger</span>
                </span>
              </button>
            );
          })}
        </div>
      )}
    </section>
  );
}
