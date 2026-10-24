import { useEffect, useState } from "react";
import { command, type SyncStatus } from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { Button } from "../../components/Button";

export function ConnectionsSyncPanel({
  onSyncFinished,
  readOnly,
}: {
  onSyncFinished: () => void;
  readOnly: boolean;
}) {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  async function load() {
    try {
      setStatus(await command("sync_status", {}));
    } catch (caught) {
      setError(friendlyError(caught));
    }
  }

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function runSync() {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const summary = await command("sync_now", {});
      if (!summary.accepted) {
        setMessage(
          summary.skipReason === "no_connections"
            ? "No provider connection is configured; CSV import remains available."
            : `Sync skipped: ${summary.skipReason?.replaceAll("_", " ") ?? "unknown reason"}.`,
        );
      } else {
        const totals = summary.results.reduce(
          (sum, result) => ({
            added: sum.added + result.transactionsAdded,
            modified: sum.modified + result.transactionsModified,
            removed: sum.removed + result.transactionsRemoved,
            failed: sum.failed + (result.status === "failed" ? 1 : 0),
          }),
          { added: 0, modified: 0, removed: 0, failed: 0 },
        );
        setMessage(
          `Sync finished: ${totals.added} added, ${totals.modified} updated, ${totals.removed} removed, ${totals.failed} connection(s) failed.`,
        );
      }
      await load();
      onSyncFinished();
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  const connections = status?.connections ?? [];
  const runs = status?.recentRuns ?? [];

  return (
    <section className="panel panel-wide" aria-labelledby="connections-title">
      <h2 id="connections-title">Connections &amp; sync</h2>
      <p className="inline-notice" role="note">
        Live Plaid linking is unavailable in this build: no owner Plaid credentials or authorized
        Item were available. The offline, CSV, and demo workflows remain fully usable. See
        docs/plaid-feasibility.md for the concrete blocker and verification requirements.
      </p>
      <p className="fine-print">
        Sync runs in Rust after unlock, advances each connection's cursor only after the page is
        committed, and keeps previously reported values when a stream is partially unavailable. The
        renderer never sees provider tokens. Scheduled windows are 08:00 and 20:00 local time; a
        scheduled run is skipped while locked and retried inside the five-minute window.
        {!status?.providerLive ? " Live Plaid linking is not enabled in this build; CSV import works without a provider." : null}
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

      <div className="button-row">
        <Button
          variant="secondary"
          onClick={runSync}
          disabled={busy || connections.length === 0 || readOnly}
        >
          {busy ? "Syncing…" : "Sync now"}
        </Button>
        <Button variant="ghost" onClick={() => void load()} disabled={busy}>
          Refresh status
        </Button>
      </div>

      {connections.length === 0 ? (
        <p className="muted">
          No provider connected. Import a CSV in the panel below, or add a provider when live linking
          is available.
        </p>
      ) : (
        <table className="accounts-table">
          <caption className="sr-only">Provider connections</caption>
          <thead>
            <tr>
              <th scope="col">Institution</th>
              <th scope="col">Provider</th>
              <th scope="col">Status</th>
              <th scope="col">Last success</th>
              <th scope="col">Accounts</th>
            </tr>
          </thead>
          <tbody>
            {connections.map((connection) => (
              <tr key={connection.id}>
                <td>{connection.institutionName ?? "Provider"}</td>
                <td>{connection.provider}</td>
                <td>
                  <span
                    className={
                      connection.status === "healthy"
                        ? "status-pill status-ok"
                        : connection.status === "reconnect_required"
                          ? "status-pill status-danger"
                          : "status-pill status-warn"
                    }
                    title={connection.sanitizedError ?? undefined}
                  >
                    {connection.status.replaceAll("_", " ")}
                  </span>
                </td>
                <td>{connection.lastSuccessAt ? connection.lastSuccessAt.slice(0, 10) : "Never"}</td>
                <td>{connection.accountCount}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <h3>Recent sync runs</h3>
      {runs.length === 0 ? (
        <p className="muted">No sync runs yet.</p>
      ) : (
        <div className="table-wrap">
          <table className="accounts-table">
            <caption className="sr-only">Recent sync runs</caption>
            <thead>
              <tr>
                <th scope="col">Started</th>
                <th scope="col">Trigger</th>
                <th scope="col">Status</th>
                <th scope="col">Steps</th>
              </tr>
            </thead>
            <tbody>
              {runs.map((run) => (
                <tr key={run.id}>
                  <td>{run.startedAt.slice(0, 16).replace("T", " ")}</td>
                  <td>{run.trigger}</td>
                  <td>
                    <span
                      className={
                        run.status === "success"
                          ? "status-pill status-ok"
                          : run.status === "failed"
                            ? "status-pill status-danger"
                            : "status-pill status-warn"
                      }
                    >
                      {run.status}
                    </span>
                  </td>
                  <td className="fine-print">
                    {run.steps
                      .map((step) => `${step.stream}: ${step.status} (+${step.itemsAdded}/~${step.itemsModified}/-${step.itemsRemoved})`)
                      .join(" · ")}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
