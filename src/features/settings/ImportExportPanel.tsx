import { useEffect, useState } from "react";
import {
  command,
  type Account,
  type CsvMapping,
  type ImportBatch,
  type ImportFileView,
  type ValidationPreview,
} from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { formatDateOnly, formatMoney } from "../../lib/formatting";
import { Button } from "../../components/Button";

type MappingForm = {
  dateColumn: string;
  descriptionColumn: string;
  amountColumn: string;
  debitColumn: string;
  creditColumn: string;
  categoryColumn: string;
  sourceIdColumn: string;
  dateFormat: "" | "iso" | "us" | "eu";
  signConvention: "signed_amount" | "debit_credit";
  creditType: "refund" | "payment" | "other";
};

const EMPTY_MAPPING: MappingForm = {
  dateColumn: "",
  descriptionColumn: "",
  amountColumn: "",
  debitColumn: "",
  creditColumn: "",
  categoryColumn: "",
  sourceIdColumn: "",
  dateFormat: "",
  signConvention: "signed_amount",
  creditType: "refund",
};

function optionIndex(value: string): number | null {
  return value === "" ? null : Number(value);
}

export function ImportExportPanel({ onChanged }: { onChanged: () => void }) {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [accountId, setAccountId] = useState("");
  const [job, setJob] = useState<ImportFileView | null>(null);
  const [mapping, setMapping] = useState<MappingForm>({ ...EMPTY_MAPPING });
  const [preview, setPreview] = useState<ValidationPreview | null>(null);
  const [acceptReview, setAcceptReview] = useState(false);
  const [batches, setBatches] = useState<ImportBatch[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [rollbackConfirm, setRollbackConfirm] = useState<string | null>(null);

  async function loadBatches() {
    try {
      const result = await command("import_batches_list", { limit: 20 });
      setBatches(result.batches);
    } catch (caught) {
      setError(friendlyError(caught));
    }
  }

  useEffect(() => {
    (async () => {
      try {
        const result = await command("accounts_list", {});
        setAccounts(result.accounts);
        if (result.accounts[0]) setAccountId(result.accounts[0].id);
        await loadBatches();
      } catch (caught) {
        setError(friendlyError(caught));
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function inferMapping(headers: string[]) {
    const find = (...terms: string[]) =>
      headers.findIndex((header) => terms.some((term) => header.toLowerCase().includes(term)));
    const date = find("date");
    const description = find("description", "merchant", "memo", "payee");
    const amount = find("amount");
    const debit = find("debit");
    const credit = find("credit");
    const category = find("category");
    const source = find("source", "id", "reference");
    setMapping({
      dateColumn: date >= 0 ? String(date) : "",
      descriptionColumn: description >= 0 ? String(description) : "",
      amountColumn: amount >= 0 ? String(amount) : "",
      debitColumn: amount < 0 && debit >= 0 ? String(debit) : "",
      creditColumn: amount < 0 && credit >= 0 ? String(credit) : "",
      categoryColumn: category >= 0 ? String(category) : "",
      sourceIdColumn: source >= 0 ? String(source) : "",
      dateFormat: "",
      signConvention: amount >= 0 ? "signed_amount" : "debit_credit",
      creditType: "refund",
    });
  }

  async function chooseFile() {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const result = await command("import_choose_file", { accountId });
      setJob(result);
      setPreview(null);
      inferMapping(result.headers);
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  function buildMapping(): CsvMapping | null {
    if (mapping.dateColumn === "" || mapping.descriptionColumn === "" || mapping.dateFormat === "") {
      setError("Choose a date column, a description column, and an explicit date format.");
      return null;
    }
    return {
      dateColumn: Number(mapping.dateColumn),
      descriptionColumn: Number(mapping.descriptionColumn),
      amountColumn: optionIndex(mapping.amountColumn),
      debitColumn: optionIndex(mapping.debitColumn),
      creditColumn: optionIndex(mapping.creditColumn),
      categoryColumn: optionIndex(mapping.categoryColumn),
      sourceIdColumn: optionIndex(mapping.sourceIdColumn),
      dateFormat: mapping.dateFormat,
      signConvention: mapping.signConvention,
      creditType: mapping.creditType,
    };
  }

  async function validate() {
    if (!job) return;
    const built = buildMapping();
    if (!built) return;
    setBusy(true);
    setError(null);
    try {
      const result = await command("import_validate", { jobId: job.jobId, mapping: built });
      setPreview(result);
      setMessage(null);
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function commit() {
    if (!job) return;
    const built = buildMapping();
    if (!built) return;
    setBusy(true);
    setError(null);
    try {
      const result = await command("import_commit", {
        jobId: job.jobId,
        mapping: built,
        acceptReview,
      });
      setMessage(
        result.noOp
          ? "This exact file was already imported; nothing changed."
          : `Imported ${result.inserted} rows; ${result.duplicates} duplicates skipped, ${result.review} left for review, ${result.rejected} rejected.`,
      );
      setPreview(null);
      setJob(null);
      setAcceptReview(false);
      setMapping({ ...EMPTY_MAPPING });
      await loadBatches();
      onChanged();
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function cancelJob() {
    if (job) {
      try {
        await command("import_cancel", { jobId: job.jobId });
      } catch {
        // An expired job is effectively cancelled.
      }
    }
    setJob(null);
    setPreview(null);
    setMapping({ ...EMPTY_MAPPING });
    setError(null);
  }

  async function rollback(batchId: string) {
    setBusy(true);
    setError(null);
    try {
      await command("import_rollback", { batchId });
      setRollbackConfirm(null);
      setMessage("Batch rolled back. Only that batch's unique contributions were removed.");
      await loadBatches();
      onChanged();
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  const columnOptions = job?.headers.map((header, index) => (
    <option key={`${index}-${header}`} value={index}>
      {index + 1}. {header}
    </option>
  ));

  return (
    <section className="panel panel-wide" aria-labelledby="import-title">
      <h2 id="import-title">CSV import</h2>
      <p className="fine-print">
        Files are read locally by Rust. The renderer receives an opaque job id, counts, and preview
        rows — never a file path. Limits: 20 MiB, 100,000 rows, 40 columns, 4,096 characters per
        field. Ambiguous duplicates are shown for review, never deleted automatically.
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

      {!job ? (
        <div className="import-start">
          <label>
            <span className="field-label">Target account</span>
            <select
              aria-label="Import target account"
              value={accountId}
              onChange={(event) => setAccountId(event.target.value)}
            >
              {accounts.map((account) => (
                <option key={account.id} value={account.id}>
                  {account.displayName}
                </option>
              ))}
            </select>
          </label>
          <Button variant="secondary" onClick={chooseFile} disabled={busy || !accountId}>
            Choose CSV file…
          </Button>
        </div>
      ) : (
        <div className="import-wizard">
          <div className="import-file">
            <strong>{job.headers.length} columns</strong> · {job.rowCount.toLocaleString()} rows ·{" "}
            {(job.sizeBytes / 1024).toFixed(1)} KiB
          </div>
          <div className="form-grid mapping-grid">
            <label>
              <span className="field-label">Date column</span>
              <select
                aria-label="Date column"
                value={mapping.dateColumn}
                onChange={(event) => setMapping({ ...mapping, dateColumn: event.target.value })}
              >
                <option value="">Choose…</option>
                {columnOptions}
              </select>
            </label>
            <label>
              <span className="field-label">Description column</span>
              <select
                aria-label="Description column"
                value={mapping.descriptionColumn}
                onChange={(event) => setMapping({ ...mapping, descriptionColumn: event.target.value })}
              >
                <option value="">Choose…</option>
                {columnOptions}
              </select>
            </label>
            <label>
              <span className="field-label">Date format (required)</span>
              <select
                aria-label="Date format"
                value={mapping.dateFormat}
                onChange={(event) =>
                  setMapping({ ...mapping, dateFormat: event.target.value as MappingForm["dateFormat"] })
                }
              >
                <option value="">Choose explicitly…</option>
                <option value="iso">YYYY-MM-DD</option>
                <option value="us">MM/DD/YYYY</option>
                <option value="eu">DD/MM/YYYY</option>
              </select>
            </label>
            <label>
              <span className="field-label">Sign convention</span>
              <select
                aria-label="Sign convention"
                value={mapping.signConvention}
                onChange={(event) =>
                  setMapping({
                    ...mapping,
                    signConvention: event.target.value as MappingForm["signConvention"],
                  })
                }
              >
                <option value="signed_amount">One signed amount column</option>
                <option value="debit_credit">Separate debit and credit columns</option>
              </select>
            </label>
            {mapping.signConvention === "signed_amount" ? (
              <label>
                <span className="field-label">Amount column</span>
                <select
                  aria-label="Amount column"
                  value={mapping.amountColumn}
                  onChange={(event) => setMapping({ ...mapping, amountColumn: event.target.value })}
                >
                  <option value="">Choose…</option>
                  {columnOptions}
                </select>
              </label>
            ) : (
              <>
                <label>
                  <span className="field-label">Debit column</span>
                  <select
                    aria-label="Debit column"
                    value={mapping.debitColumn}
                    onChange={(event) => setMapping({ ...mapping, debitColumn: event.target.value })}
                  >
                    <option value="">None</option>
                    {columnOptions}
                  </select>
                </label>
                <label>
                  <span className="field-label">Credit column</span>
                  <select
                    aria-label="Credit column"
                    value={mapping.creditColumn}
                    onChange={(event) => setMapping({ ...mapping, creditColumn: event.target.value })}
                  >
                    <option value="">None</option>
                    {columnOptions}
                  </select>
                </label>
              </>
            )}
            <label>
              <span className="field-label">Category column (optional)</span>
              <select
                aria-label="Category column"
                value={mapping.categoryColumn}
                onChange={(event) => setMapping({ ...mapping, categoryColumn: event.target.value })}
              >
                <option value="">None</option>
                {columnOptions}
              </select>
            </label>
            <label>
              <span className="field-label">Source ID column (optional)</span>
              <select
                aria-label="Source ID column"
                value={mapping.sourceIdColumn}
                onChange={(event) => setMapping({ ...mapping, sourceIdColumn: event.target.value })}
              >
                <option value="">None</option>
                {columnOptions}
              </select>
            </label>
            <label>
              <span className="field-label">Credit rows are</span>
              <select
                aria-label="Credit type"
                value={mapping.creditType}
                onChange={(event) =>
                  setMapping({ ...mapping, creditType: event.target.value as MappingForm["creditType"] })
                }
              >
                <option value="refund">Refunds</option>
                <option value="payment">Payments</option>
                <option value="other">Other</option>
              </select>
            </label>
          </div>
          <div className="button-row">
            <Button variant="secondary" onClick={validate} disabled={busy}>
              Validate and preview
            </Button>
            <Button variant="ghost" onClick={cancelJob} disabled={busy}>
              Cancel import
            </Button>
          </div>

          {preview ? (
            <div className="import-preview">
              <dl className="facts">
                <div>
                  <dt>Rows</dt>
                  <dd>{preview.rowCount}</dd>
                </div>
                <div>
                  <dt>New</dt>
                  <dd>{preview.insertCount}</dd>
                </div>
                <div>
                  <dt>Duplicates</dt>
                  <dd>{preview.duplicateCount}</dd>
                </div>
                <div>
                  <dt>Needs review</dt>
                  <dd>{preview.reviewCount}</dd>
                </div>
                <div>
                  <dt>Rejected</dt>
                  <dd>{preview.rejectedCount}</dd>
                </div>
              </dl>
              {preview.noOp ? (
                <p className="inline-notice" role="status">
                  This exact file was already imported for this account; importing again is a
                  no-op.
                </p>
              ) : null}
              {preview.months.length > 0 ? (
                <table className="accounts-table">
                  <caption className="sr-only">Preview totals by month</caption>
                  <thead>
                    <tr>
                      <th scope="col">Month</th>
                      <th scope="col">Purchases</th>
                      <th scope="col">Refunds</th>
                      <th scope="col">Net</th>
                      <th scope="col">Rows</th>
                    </tr>
                  </thead>
                  <tbody>
                    {preview.months.map((month) => (
                      <tr key={month.month}>
                        <td>{month.month}</td>
                        <td>{formatMoney(month.purchasesMinor)}</td>
                        <td>{formatMoney(month.refundsMinor)}</td>
                        <td>{formatMoney(month.netMinor)}</td>
                        <td>{month.count}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              ) : null}
              {preview.reviewCount > 0 ? (
                <label className="checkbox-row">
                  <input
                    type="checkbox"
                    checked={acceptReview}
                    onChange={(event) => setAcceptReview(event.target.checked)}
                  />
                  Import the {preview.reviewCount} possible duplicate row(s) anyway
                </label>
              ) : null}
              {preview.sampleRows.length > 0 ? (
                <div className="table-wrap">
                  <table className="accounts-table">
                    <caption className="sr-only">Row preview</caption>
                    <thead>
                      <tr>
                        <th scope="col">Row</th>
                        <th scope="col">Date</th>
                        <th scope="col">Merchant</th>
                        <th scope="col">Amount</th>
                        <th scope="col">Status</th>
                      </tr>
                    </thead>
                    <tbody>
                      {preview.sampleRows.map((row) => (
                        <tr key={row.rowNumber}>
                          <td>{row.rowNumber}</td>
                          <td>{row.date ? formatDateOnly(row.date) : "—"}</td>
                          <td>{row.merchant || "—"}</td>
                          <td>{row.date ? formatMoney(row.amountMinor) : "—"}</td>
                          <td>
                            <span
                              className={
                                row.status === "insert"
                                  ? "status-pill status-ok"
                                  : row.status === "review"
                                    ? "status-pill status-warn"
                                    : "status-pill status-info"
                              }
                            >
                              {row.status}
                            </span>
                            {row.reason ? <span className="fine-print"> {row.reason}</span> : null}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : null}
              <div className="button-row">
                <Button
                  variant="primary"
                  onClick={commit}
                  disabled={busy || preview.noOp || (preview.insertCount === 0 && !acceptReview)}
                >
                  {busy ? "Importing…" : "Confirm import"}
                </Button>
              </div>
            </div>
          ) : null}
        </div>
      )}

      <h3>Import history</h3>
      {batches.length === 0 ? (
        <p className="muted">No import batches yet.</p>
      ) : (
        <table className="accounts-table">
          <caption className="sr-only">Import batches</caption>
          <thead>
            <tr>
              <th scope="col">Account</th>
              <th scope="col">Rows</th>
              <th scope="col">Inserted</th>
              <th scope="col">Review</th>
              <th scope="col">Status</th>
              <th scope="col">Created</th>
              <th scope="col">Actions</th>
            </tr>
          </thead>
          <tbody>
            {batches.map((batch) => (
              <tr key={batch.id}>
                <td>{batch.accountName}</td>
                <td>{batch.rowCount}</td>
                <td>{batch.insertedCount}</td>
                <td>{batch.reviewCount}</td>
                <td>
                  <span
                    className={
                      batch.status === "imported"
                        ? "status-pill status-ok"
                        : batch.status === "rolled_back"
                          ? "status-pill status-info"
                          : "status-pill status-warn"
                    }
                  >
                    {batch.status}
                  </span>
                </td>
                <td>{formatDateOnly(batch.createdAt.slice(0, 10))}</td>
                <td>
                  {batch.status === "imported" ? (
                    rollbackConfirm === batch.id ? (
                      <span className="archive-confirm">
                        Roll back?{" "}
                        <Button
                          variant="danger"
                          className="btn-small"
                          disabled={busy}
                          onClick={() => void rollback(batch.id)}
                        >
                          Roll back
                        </Button>{" "}
                        <Button variant="ghost" className="btn-small" onClick={() => setRollbackConfirm(null)}>
                          Cancel
                        </Button>
                      </span>
                    ) : (
                      <Button
                        variant="ghost"
                        className="btn-small"
                        onClick={() => setRollbackConfirm(batch.id)}
                      >
                        Roll back
                      </Button>
                    )
                  ) : (
                    <span className="fine-print">—</span>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <p className="fine-print">
        Export uses the ledger toolbar and always writes plaintext. Imported rows are labeled with
        CSV provenance; rollback removes only that batch's unique contributions.
      </p>
    </section>
  );
}
