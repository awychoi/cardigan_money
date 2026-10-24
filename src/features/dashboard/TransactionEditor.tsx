import { useEffect, useState } from "react";
import { command, type LedgerRow, type TransactionDetail } from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { formatDateOnly, formatMoney } from "../../lib/formatting";
import { Button } from "../../components/Button";

const TYPES = ["purchase", "refund", "fee", "interest", "payment", "transfer", "other"] as const;

export function TransactionEditor({
  row,
  readOnly,
  onSaved,
  onClose,
}: {
  row: LedgerRow;
  readOnly: boolean;
  onSaved: () => void;
  onClose: () => void;
}) {
  const [detail, setDetail] = useState<TransactionDetail | null>(null);
  const [editing, setEditing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [merchantAlias, setMerchantAlias] = useState("");
  const [category, setCategory] = useState("");
  const [typeCorrection, setTypeCorrection] = useState("");
  const [excluded, setExcluded] = useState(false);
  const [notes, setNotes] = useState("");

  useEffect(() => {
    let cancelled = false;
    setDetail(null);
    setError(null);
    setMessage(null);
    setEditing(false);
    (async () => {
      try {
        const value = await command("transaction_detail", { id: row.id });
        if (cancelled) return;
        setDetail(value);
        setMerchantAlias(value.merchantAlias ?? "");
        setCategory(value.sourceCategory ?? "");
        setTypeCorrection(value.typeCorrection ?? "");
        setExcluded(value.excluded);
        setNotes(value.notes ?? "");
      } catch (caught) {
        if (!cancelled) setError(friendlyError(caught));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [row.id]);

  async function save() {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const updated = await command("transaction_override_set", {
        id: row.id,
        override: {
          merchantAlias: merchantAlias.trim() ? merchantAlias.trim() : null,
          category: category.trim() ? category.trim() : null,
          typeCorrection: (typeCorrection || null) as
            | "purchase"
            | "refund"
            | "fee"
            | "interest"
            | "payment"
            | "transfer"
            | "other"
            | null,
          excluded: excluded !== detail?.excluded ? excluded : null,
          notes: notes.trim() ? notes.trim() : null,
        },
      });
      setDetail(updated);
      setEditing(false);
      setMessage("Saved. The source fields are unchanged.");
      onSaved();
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function clearEdits() {
    setBusy(true);
    setError(null);
    try {
      const updated = await command("transaction_override_clear", { id: row.id });
      setDetail(updated);
      setMerchantAlias("");
      setCategory(updated.sourceCategory ?? "");
      setTypeCorrection("");
      setExcluded(false);
      setNotes("");
      setEditing(false);
      setMessage("User edits cleared.");
      onSaved();
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  return (
    <aside className="detail-panel" aria-labelledby="detail-title">
      <div className="detail-head">
        <h2 id="detail-title">{detail?.merchant ?? row.merchant}</h2>
        <button
          type="button"
          className="icon-btn"
          aria-label="Close transaction details"
          onClick={onClose}
        >
          ×
        </button>
      </div>

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

      {!detail ? (
        <p className="muted">Loading transaction details…</p>
      ) : (
        <>
          <dl className="facts">
            <div>
              <dt>Amount</dt>
              <dd>{formatMoney(detail.amountMinor, detail.currency)}</dd>
            </div>
            <div>
              <dt>Reporting date</dt>
              <dd>{formatDateOnly(detail.reportingDate)}</dd>
            </div>
            <div>
              <dt>Date source</dt>
              <dd>{detail.reportingDateSource}</dd>
            </div>
            <div>
              <dt>Purchase / authorization</dt>
              <dd>{detail.authorizedDate ? formatDateOnly(detail.authorizedDate) : "Unknown"}</dd>
            </div>
            <div>
              <dt>Posted</dt>
              <dd>
                {detail.postedDate
                  ? formatDateOnly(detail.postedDate)
                  : detail.pending
                    ? "Pending"
                    : "Unknown"}
              </dd>
            </div>
            <div>
              <dt>Card</dt>
              <dd>
                {detail.accountName}
                {detail.accountLastFour ? ` · ${detail.accountLastFour}` : ""}
              </dd>
            </div>
            <div>
              <dt>Source type</dt>
              <dd>{detail.sourceKind}</dd>
            </div>
            <div>
              <dt>Effective type</dt>
              <dd>{detail.txType}</dd>
            </div>
            <div>
              <dt>Excluded</dt>
              <dd>{detail.excluded ? "Yes" : "No"}</dd>
            </div>
            {detail.notes ? (
              <div>
                <dt>User notes</dt>
                <dd>{detail.notes}</dd>
              </div>
            ) : null}
          </dl>

          {readOnly ? (
            <p className="fine-print">
              Demo data is read-only. Editing, rules, imports, and exports are disabled.
            </p>
          ) : editing ? (
            <div className="edit-form">
              <label>
                <span className="field-label">Merchant alias</span>
                <input
                  value={merchantAlias}
                  onChange={(event) => setMerchantAlias(event.target.value)}
                  maxLength={120}
                  placeholder={detail.merchant}
                />
              </label>
              <label>
                <span className="field-label">Category</span>
                <input
                  value={category}
                  onChange={(event) => setCategory(event.target.value)}
                  maxLength={80}
                  placeholder={detail.sourceCategory ?? "Uncategorized"}
                />
              </label>
              <label>
                <span className="field-label">Type correction</span>
                <select
                  value={typeCorrection}
                  onChange={(event) => setTypeCorrection(event.target.value)}
                >
                  <option value="">Use source type ({detail.txType})</option>
                  {TYPES.map((type) => (
                    <option key={type} value={type}>
                      {type}
                    </option>
                  ))}
                </select>
              </label>
              <label className="checkbox-row">
                <input
                  type="checkbox"
                  checked={excluded}
                  onChange={(event) => setExcluded(event.target.checked)}
                />
                Exclude from spending totals
              </label>
              <label>
                <span className="field-label">Notes</span>
                <textarea
                  value={notes}
                  onChange={(event) => setNotes(event.target.value)}
                  maxLength={2000}
                  rows={3}
                />
              </label>
              <div className="button-row">
                <Button variant="primary" onClick={save} disabled={busy}>
                  {busy ? "Saving…" : "Save edits"}
                </Button>
                <Button variant="ghost" onClick={() => setEditing(false)} disabled={busy}>
                  Cancel
                </Button>
              </div>
            </div>
          ) : (
            <div className="button-row">
              <Button variant="secondary" onClick={() => setEditing(true)}>
                Edit transaction…
              </Button>
              <Button variant="ghost" onClick={clearEdits} disabled={busy || !detail.editedAt}>
                Clear user edits
              </Button>
            </div>
          )}

          <p className="fine-print">Raw description:</p>
          <p className="raw-description">{detail.rawDescription}</p>
        </>
      )}
    </aside>
  );
}
