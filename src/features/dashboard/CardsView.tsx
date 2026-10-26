import type { CSSProperties } from "react";
import type { Dashboard, AccountCard } from "../../lib/bridge";
import {
  connectionLabel,
  dueAmountLabel,
  formatDateOnly,
  formatMoney,
  monthLabel,
} from "../../lib/formatting";
import { issuerVariables } from "../../theme/tokens";
import type { Theme } from "../../theme/schema";

type CardStatus = "ok" | "warn" | "fail" | "info";

function statusFor(account: AccountCard): CardStatus {
  if (account.sourceKind === "manual") return "info";
  if (account.connectionStatus === "healthy") return "ok";
  if (account.connectionStatus === "error") return "fail";
  return "warn";
}

export function CardsView({
  dashboard,
  theme,
  onFilterAccount,
}: {
  dashboard: Dashboard;
  theme: Theme;
  onFilterAccount: (accountId: string) => void;
}) {
  const spendFor = (accountId: string, currency: string) =>
    dashboard.accountSpend.find((row) => row.accountId === accountId && row.currency === currency);

  const dueText = (account: AccountCard, compact = false): string => {
    const payment = account.payment;
    if (payment.dueAmountKind === "no_due_date") return "No due date reported";
    const full = payment.nextDueDate ? formatDateOnly(payment.nextDueDate) : "";
    const date = compact ? full.replace(/,\s*\d{4}$/, "") : full;
    const amount = dueAmountLabel(payment.dueAmountKind, payment.dueAmountMinor, account.currency);
    return date ? `${date} · ${amount}` : amount;
  };

  const balanceText = (account: AccountCard): string =>
    account.currentBalanceMinor === null
      ? "Balance unknown"
      : formatMoney(account.currentBalanceMinor, account.currency);

  const statementText = (account: AccountCard): string => {
    const payment = account.payment;
    if (payment.statementBalanceMinor === null) return "Not reported";
    return `${formatMoney(payment.statementBalanceMinor, account.currency)}${
      payment.statementIssueDate ? ` · ${formatDateOnly(payment.statementIssueDate)}` : ""
    }`;
  };

  const faces = theme.options.cardArt === "faces";

  return (
    <section className="dashboard" aria-labelledby="cards-title">
      <header className="page-heading">
        <div>
          <h1 id="cards-title">Cards</h1>
          <p className="muted">
            {monthLabel(dashboard.month)} · monthly spending plus current balance and reported
            payment metadata
          </p>
        </div>
      </header>

      {dashboard.overview.accounts.length === 0 ? (
        <p className="muted">No accounts yet.</p>
      ) : faces ? (
        <>
          <div className="cards-grid cards-grid-faces">
            {dashboard.overview.accounts.map((account) => {
              const payment = account.payment;
              const missing = account.currentBalanceMinor === null;
              const status = statusFor(account);
              return (
                <button
                  type="button"
                  key={account.accountId}
                  className={[
                    "cardface",
                    account.balanceStale ? "cardface-stale" : "",
                    missing ? "cardface-missing" : "",
                  ]
                    .filter(Boolean)
                    .join(" ")}
                  style={issuerVariables(theme, account.issuer) as CSSProperties}
                  onClick={() => onFilterAccount(account.accountId)}
                  aria-label={`Filter transactions to ${account.displayName}`}
                >
                  <span className="cf-top">
                    <span className="cf-issuer">{account.issuer}</span>
                    <span className="cf-last">•••• {account.lastFour ?? "----"}</span>
                  </span>
                  <span className="cf-mid">
                    <span className="cf-label">Current balance</span>
                    <span className={`cf-bal${missing ? " cf-bal-unknown" : ""}`}>
                      {missing ? "Unknown" : balanceText(account)}
                    </span>
                    <span className="cf-note">
                      {missing
                        ? "No snapshot — never shown as $0.00"
                        : `as of ${
                            account.balanceRetrievedAt
                              ? formatDateOnly(account.balanceRetrievedAt.slice(0, 10))
                              : "unknown"
                          }${account.balanceStale ? " · stale" : ""}`}
                    </span>
                  </span>
                  <span className="cf-foot">
                    <span className="cf-next">
                      {payment.dueAmountKind === "no_due_date" ? "" : "Next "}
                      {dueText(account, true)}
                    </span>
                    <span className={`cf-chip cf-chip-${status}`}>
                      <span className="cf-dot" aria-hidden="true" />
                      {connectionLabel(account.connectionStatus, account.sourceKind)}
                    </span>
                  </span>
                </button>
              );
            })}
          </div>

          <div className="card-details">
            <h2 className="card-details-title">Card details</h2>
            <div className="table-wrap">
              <table className="detail-table">
                <thead>
                  <tr>
                    <th scope="col">Card</th>
                    <th scope="col">Month spend</th>
                    <th scope="col">Next payment</th>
                    <th scope="col">Statement</th>
                    <th scope="col">Due source</th>
                    <th scope="col">Balance as of</th>
                  </tr>
                </thead>
                <tbody>
                  {dashboard.overview.accounts.map((account) => {
                    const spend = spendFor(account.accountId, account.currency);
                    return (
                      <tr key={account.accountId}>
                        <th scope="row" className="detail-card">
                          {account.displayName}
                          <span className="fine-print"> ···· {account.lastFour ?? "----"}</span>
                        </th>
                        <td>{spend ? formatMoney(spend.netMinor, account.currency) : "—"}</td>
                        <td>{dueText(account)}</td>
                        <td>{statementText(account)}</td>
                        <td>
                          {account.payment.dueSource
                            ? `Due source: ${account.payment.dueSource}${
                                account.payment.manualExpired ? " · expired" : ""
                              }`
                            : "—"}
                        </td>
                        <td>
                          {account.balanceRetrievedAt
                            ? formatDateOnly(account.balanceRetrievedAt.slice(0, 10))
                            : "Unknown"}
                          {account.balanceStale ? " · stale" : ""}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
            <p className="fine-print">
              A statement balance is not the remaining amount due. Missing values are never shown as
              $0.00.
            </p>
          </div>
        </>
      ) : (
        <div className="cards-grid">
          {dashboard.overview.accounts.map((account) => {
            const spend = spendFor(account.accountId, account.currency);
            const payment = account.payment;
            const dueLabel = payment.nextDueDate
              ? `${formatDateOnly(payment.nextDueDate)} · ${dueAmountLabel(
                  payment.dueAmountKind,
                  payment.dueAmountMinor,
                  account.currency,
                )}`
              : dueAmountLabel(payment.dueAmountKind, payment.dueAmountMinor, account.currency);
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

                <span className="card-balance">{balanceText(account)}</span>
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
                  <span>{dueLabel}</span>
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
                  <span>{statementText(account)}</span>
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
                {account.connectionStatus === "error" ||
                account.connectionStatus === "reconnect_required" ? (
                  <span className="fine-print">
                    Statement details may be unavailable until this connection is repaired; missing
                    values are never shown as $0.00.
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
