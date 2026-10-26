import { useCallback, useEffect, useState } from "react";
import { command, type Account, type AccountPayload } from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { Button } from "../../components/Button";
import { AccountForm } from "./AccountForm";
import { PaymentDetailsForm } from "./PaymentDetailsForm";

type Mode =
  | { kind: "list" }
  | { kind: "create" }
  | { kind: "edit"; account: Account }
  | { kind: "payment"; account: Account };

function accountMeta(account: Account): string {
  return [
    account.issuer,
    account.lastFour ? `•••• ${account.lastFour}` : null,
    account.accountType.replaceAll("_", " "),
  ]
    .filter((value): value is string => Boolean(value))
    .join(" · ");
}

export function AccountManager({
  onChanged,
  readOnly = false,
}: {
  onChanged?: () => void;
  readOnly?: boolean;
}) {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [mode, setMode] = useState<Mode>({ kind: "list" });
  const [includeArchived, setIncludeArchived] = useState(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [archiving, setArchiving] = useState<string | null>(null);

  const load = useCallback(async (showArchived: boolean) => {
    try {
      const result = await command("accounts_list", { includeArchived: showArchived });
      setAccounts(result.accounts);
      setError(null);
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load(includeArchived);
  }, [includeArchived, load]);

  async function submit(payload: AccountPayload) {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      if (mode.kind === "edit") {
        await command("account_update", { id: mode.account.id, account: payload });
        setMessage("Card updated.");
      } else {
        await command("account_create", payload);
        setMessage("Card added. You can add its payment details below.");
      }
      setMode({ kind: "list" });
      await load(includeArchived);
      onChanged?.();
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function archive(account: Account, archived: boolean) {
    setBusy(true);
    setError(null);
    try {
      await command("account_archive", { id: account.id, archived });
      setMessage(archived ? "Card archived." : "Card restored.");
      setArchiving(null);
      await load(includeArchived);
      onChanged?.();
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  const heading =
    mode.kind === "create"
      ? "Add a card"
      : mode.kind === "edit"
        ? "Edit card"
        : mode.kind === "payment"
          ? "Payment details"
          : "All accounts";
  const intro =
    mode.kind === "create"
      ? "No bank connection needed. Connect or import from the tabs above."
      : mode.kind === "edit"
        ? "Edit this card’s details; past transactions keep their records."
        : mode.kind === "payment"
          ? `Optional statement and due-date facts for ${mode.account.displayName}. They survive syncs.`
          : "Every card in one list, labeled by source.";

  return (
    <section className="panel panel-wide" aria-labelledby="accounts-title">
      <div className="panel-heading">
        <div>
          <h2 id="accounts-title">{heading}</h2>
          <p className="fine-print">{intro}</p>
        </div>
        {mode.kind === "list" && !readOnly ? (
          <Button variant="secondary" onClick={() => setMode({ kind: "create" })}>
            Add card manually
          </Button>
        ) : null}
      </div>

      {message ? (
        <p className="inline-notice" role="status">
          {message}
        </p>
      ) : null}
      {error ? (
        <p className="inline-error" role="alert">
          {error}
        </p>
      ) : null}

      {readOnly ? (
        <p className="fine-print">
          Demo accounts are read-only. They come from synthetic fixtures and are recreated by “Reset
          demo”.
        </p>
      ) : null}

      {mode.kind === "create" ? (
        <AccountForm
          submitLabel="Add account"
          busy={busy}
          onSubmit={submit}
          onCancel={() => setMode({ kind: "list" })}
        />
      ) : mode.kind === "edit" ? (
        <AccountForm
          initial={mode.account}
          submitLabel="Save changes"
          busy={busy}
          onSubmit={submit}
          onCancel={() => setMode({ kind: "list" })}
        />
      ) : mode.kind === "payment" ? (
        <PaymentDetailsForm
          accountId={mode.account.id}
          onChanged={() => onChanged?.()}
          onClose={() => setMode({ kind: "list" })}
        />
      ) : (
        <>
          {loading && accounts.length === 0 ? (
            <p className="muted">Loading cards…</p>
          ) : accounts.length === 0 ? (
            <p className="muted">
              No cards yet. Add one manually, connect a bank under Bank connections, or import a CSV
              file.
            </p>
          ) : (
            <table className="accounts-table">
              <caption className="sr-only">Your accounts</caption>
              <thead>
                <tr>
                  <th scope="col">Account</th>
                  <th scope="col">Source</th>
                  <th scope="col">Actions</th>
                </tr>
              </thead>
              <tbody>
                {accounts.map((account) => (
                  <tr key={account.id} className={account.archived ? "row-archived" : undefined}>
                    <td>
                      <span className="account-name">{account.displayName}</span>
                      <span className="account-meta">{accountMeta(account)}</span>
                    </td>
                    <td>
                      <span className="status-pill status-info">{account.sourceKind}</span>
                    </td>
                    <td>
                      {readOnly ? (
                        <span className="fine-print">Read-only</span>
                      ) : archiving === account.id ? (
                        <span className="archive-confirm">
                          Archive?{" "}
                          <Button
                            variant="danger"
                            className="btn-small"
                            disabled={busy}
                            onClick={() => void archive(account, true)}
                          >
                            Archive
                          </Button>{" "}
                          <Button
                            variant="ghost"
                            className="btn-small"
                            onClick={() => setArchiving(null)}
                          >
                            Cancel
                          </Button>
                        </span>
                      ) : (
                        <span className="button-row account-actions">
                          {!account.archived ? (
                            <>
                              <Button
                                variant="ghost"
                                className="btn-small"
                                onClick={() => setMode({ kind: "edit", account })}
                              >
                                Edit
                              </Button>
                              <Button
                                variant="ghost"
                                className="btn-small"
                                onClick={() => setMode({ kind: "payment", account })}
                              >
                                Payment details
                              </Button>
                            </>
                          ) : null}
                          <Button
                            variant="ghost"
                            className="btn-small"
                            onClick={() =>
                              account.archived
                                ? void archive(account, false)
                                : setArchiving(account.id)
                            }
                          >
                            {account.archived ? "Restore" : "Archive"}
                          </Button>
                        </span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={includeArchived}
              onChange={(event) => setIncludeArchived(event.target.checked)}
            />
            Show archived accounts
          </label>
        </>
      )}
    </section>
  );
}
