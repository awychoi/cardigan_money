import { useCallback, useEffect, useState } from "react";
import { command, type Account, type AccountPayload } from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { Button } from "../../components/Button";
import { AccountForm } from "./AccountForm";

type Mode = { kind: "list" } | { kind: "create" } | { kind: "edit"; account: Account };

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
        setMessage("Account updated.");
      } else {
        await command("account_create", payload);
        setMessage("Account added.");
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
      setMessage(archived ? "Account archived." : "Account restored.");
      setArchiving(null);
      await load(includeArchived);
      onChanged?.();
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel panel-wide" aria-labelledby="accounts-title">
      <div className="panel-heading">
        <div>
          <h2 id="accounts-title">Accounts</h2>
          <p className="fine-print">
            Manual accounts need no bank connection. Archiving keeps historical transactions.
          </p>
        </div>
        {mode.kind === "list" && !readOnly ? (
          <Button variant="secondary" onClick={() => setMode({ kind: "create" })}>
            Add account
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
      ) : (
        <>
          {accounts.length === 0 ? (
            <p className="muted">No accounts yet. Add one manually or import a CSV later.</p>
          ) : (
            <table className="accounts-table">
              <caption className="sr-only">Your accounts</caption>
              <thead>
                <tr>
                  <th scope="col">Issuer</th>
                  <th scope="col">Name</th>
                  <th scope="col">Last four</th>
                  <th scope="col">Type</th>
                  <th scope="col">Source</th>
                  <th scope="col">Actions</th>
                </tr>
              </thead>
              <tbody>
                {accounts.map((account) => (
                  <tr key={account.id} className={account.archived ? "row-archived" : undefined}>
                    <td>{account.issuer}</td>
                    <td>{account.displayName}</td>
                    <td>{account.lastFour ?? "—"}</td>
                    <td>{account.accountType.replaceAll("_", " ")}</td>
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
                        <span className="button-row">
                          {!account.archived ? (
                            <Button
                              variant="ghost"
                              className="btn-small"
                              onClick={() => setMode({ kind: "edit", account })}
                            >
                              Edit
                            </Button>
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
