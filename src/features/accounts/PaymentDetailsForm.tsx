import { useEffect, useState } from "react";
import { command, type ManualPayment } from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { Button } from "../../components/Button";

function money(minor: number | null): string {
  return minor === null ? "" : (minor / 100).toFixed(2);
}

export function PaymentDetailsForm({
  accountId,
  onChanged,
  onClose,
}: {
  accountId: string;
  onChanged: () => void;
  onClose: () => void;
}) {
  const [payment, setPayment] = useState<ManualPayment | null>(null);
  const [statement, setStatement] = useState("");
  const [minimum, setMinimum] = useState("");
  const [due, setDue] = useState("");
  const [validThrough, setValidThrough] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
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
      setMessage("Payment details saved. They survive syncs and never overwrite provider data.");
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
    setMessage(null);
    try {
      const result = await command("manual_payment_clear", { accountId });
      setPayment(result.payment);
      setStatement("");
      setMinimum("");
      setDue("");
      setValidThrough("");
      setMessage("Payment details cleared.");
      onChanged();
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
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
            type="date"
            value={due}
            onChange={(event) => setDue(event.target.value)}
          />
        </label>
        <label>
          <span className="field-label">Valid through (optional)</span>
          <input
            aria-label="Valid through"
            type="date"
            value={validThrough}
            onChange={(event) => setValidThrough(event.target.value)}
          />
        </label>
      </div>
      <p className="fine-print">
        A statement balance is not the remaining amount due. Missing amounts are never shown as
        $0.00. These dated facts are independent of provider snapshots.
      </p>
      <div className="button-row">
        <Button variant="primary" onClick={() => void save()} disabled={busy}>
          {busy ? "Saving…" : "Save payment details"}
        </Button>
        <Button variant="ghost" onClick={() => void clear()} disabled={busy || !payment}>
          Clear
        </Button>
        <Button variant="ghost" onClick={onClose} disabled={busy}>
          Done
        </Button>
      </div>
    </>
  );
}
