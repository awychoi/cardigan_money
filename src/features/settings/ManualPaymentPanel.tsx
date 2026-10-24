import { useEffect, useState } from "react";
import { command, type Account, type ManualPayment } from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { Button } from "../../components/Button";

function money(minor: number | null): string {
  return minor === null ? "" : (minor / 100).toFixed(2);
}

export function ManualPaymentPanel({ onChanged }: { onChanged: () => void }) {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [accountId, setAccountId] = useState("");
  const [payment, setPayment] = useState<ManualPayment | null>(null);
  const [statement, setStatement] = useState("");
  const [minimum, setMinimum] = useState("");
  const [due, setDue] = useState("");
  const [validThrough, setValidThrough] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const result = await command("accounts_list", {});
        setAccounts(result.accounts);
        if (result.accounts[0]) setAccountId(result.accounts[0].id);
      } catch (caught) {
        setError(friendlyError(caught));
      }
    })();
  }, []);

  useEffect(() => {
    if (!accountId) return;
    let cancelled = false;
    (async () => {
      try {
        const result = await command("manual_payment_get", { accountId });
        if (cancelled) return;
        setPayment(result.payment);
        setStatement(money(result.payment?.statementBalanceMinor ?? null));
        setMinimum(money(result.payment?.minimumDueMinor ?? null));
        setDue(result.payment?.nextDueDate ?? "");
        setValidThrough(result.payment?.validThroughDate ?? "");
      } catch (caught) {
        if (!cancelled) setError(friendlyError(caught));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [accountId]);

  async function save() {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const result = await command("manual_payment_set", {
        accountId,
        payment: {
          statementBalance: statement.trim() || null,
          minimumDue: minimum.trim() || null,
          nextDueDate: due.trim() || null,
          validThroughDate: validThrough.trim() || null,
        },
      });
      setPayment(result.payment);
      setMessage("Manual payment metadata saved. It is independent of provider snapshots.");
      onChanged();
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function clear() {
    setBusy(true);
    setError(null);
    try {
      const result = await command("manual_payment_clear", { accountId });
      setPayment(result.payment);
      setStatement("");
      setMinimum("");
      setDue("");
      setValidThrough("");
      setMessage("Manual payment metadata cleared.");
      onChanged();
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel panel-wide" aria-labelledby="manual-payment-title">
      <h2 id="manual-payment-title">Manual payment metadata</h2>
      <p className="fine-print">
        Dated facts entered by you. They never overwrite provider data and survive syncs. A
        statement balance is not the remaining amount due; missing amounts are never shown as $0.00.
      </p>
      {error ? (
        <p className="inline-error" role="alert">
          {error}
        </p>
      ) : null}
      {message ? (
        <p className="inline-notice" role="status">
          {message}
        </p>
      ) : null}
      <div className="form-grid">
        <label>
          <span className="field-label">Account</span>
          <select aria-label="Manual metadata account" value={accountId} onChange={(event) => setAccountId(event.target.value)}>
            {accounts.map((account) => (
              <option key={account.id} value={account.id}>
                {account.displayName}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span className="field-label">Statement balance</span>
          <input
            aria-label="Statement balance"
            value={statement}
            onChange={(event) => setStatement(event.target.value)}
            placeholder="120.00"
            inputMode="decimal"
          />
        </label>
        <label>
          <span className="field-label">Minimum due</span>
          <input
            aria-label="Minimum due"
            value={minimum}
            onChange={(event) => setMinimum(event.target.value)}
            placeholder="25.00"
            inputMode="decimal"
          />
        </label>
        <label>
          <span className="field-label">Next due date</span>
          <input
            aria-label="Next due date"
            value={due}
            onChange={(event) => setDue(event.target.value)}
            placeholder="YYYY-MM-DD"
            maxLength={10}
          />
        </label>
        <label>
          <span className="field-label">Valid through (optional)</span>
          <input
            aria-label="Valid through"
            value={validThrough}
            onChange={(event) => setValidThrough(event.target.value)}
            placeholder="YYYY-MM-DD"
            maxLength={10}
          />
        </label>
      </div>
      <div className="button-row">
        <Button variant="primary" onClick={save} disabled={busy || !accountId}>
          Save manual metadata
        </Button>
        <Button variant="ghost" onClick={clear} disabled={busy || !payment}>
          Clear
        </Button>
      </div>
    </section>
  );
}
