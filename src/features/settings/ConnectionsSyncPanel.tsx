import { useEffect, useState } from "react";
import {
  command,
  type PlaidLinkStatus,
  type PlaidStatus,
  type SyncStatus,
} from "../../lib/bridge";
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
  const [plaid, setPlaid] = useState<PlaidStatus | null>(null);
  const [link, setLink] = useState<PlaidLinkStatus | null>(null);
  const [report, setReport] = useState<string | null>(null);
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
    if (!readOnly) {
      void (async () => {
        try {
          const result = await command("plaid_status", {});
          setPlaid(result.plaid);
        } catch (caught) {
          setError(friendlyError(caught));
        }
      })();
    }
  }, [readOnly]);

  useEffect(() => {
    if (readOnly || !link || (link.state !== "waiting" && link.state !== "starting")) {
      return;
    }
    const timer = window.setInterval(() => {
      void (async () => {
        try {
          const result = await command("plaid_link_status", {});
          setLink(result.link);
          if (result.link.state === "linked") {
            setPlaid((current) => (current ? { ...current, configured: true } : current));
            await load();
            onSyncFinished();
          } else if (result.link.state === "error" || result.link.state === "expired") {
            setError(result.link.message ?? "Plaid link failed.");
          }
        } catch (caught) {
          setError(friendlyError(caught));
        }
      })();
    }, 2000);
    return () => window.clearInterval(timer);
    // Poll only while the link state changes; load() and onSyncFinished() are
    // intentionally read from the current render rather than re-arming the timer.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [link?.state, readOnly]);

  async function configurePlaid() {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const result = await command("plaid_setup", {});
      setPlaid(result.plaid);
      setMessage("Sandbox credentials saved to the macOS Keychain.");
      await load();
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function testCredentials() {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      await command("plaid_test", {});
      setMessage("Plaid accepted the Sandbox credentials.");
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function loadVerificationReport() {
    setBusy(true);
    setError(null);
    try {
      const result = await command("plaid_verification_report", {});
      setReport(JSON.stringify(result, null, 2));
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function copyReport() {
    if (!report) return;
    try {
      await navigator.clipboard.writeText(report);
      setMessage("Verification report copied. It contains counts and statuses only.");
    } catch {
      setMessage("Select the report text below and copy it manually.");
    }
  }

  async function startLink() {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const result = await command("plaid_link_start", {});
      setLink(result.link);
      if (result.link.message) setMessage(result.link.message);
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function reconnect(connectionId: string) {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const result = await command("plaid_reconnect", { connectionId });
      setLink(result.link);
      setMessage(result.link.message ?? "Reconnect the Sandbox Item in your browser.");
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function cancelLink() {
    setBusy(true);
    try {
      const result = await command("plaid_link_cancel", {});
      setLink(result.link);
      setMessage("Plaid link cancelled.");
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

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
        Plaid is Sandbox-only in this build. Live bank coverage is not claimed until a Sandbox Item
        is linked and synced successfully. The offline, CSV, and demo workflows remain fully usable.
      </p>
      <p className="fine-print">
        Sync runs in Rust after unlock, advances each connection's cursor only after the page is
        committed, and keeps previously reported values when a stream is partially unavailable. The
        renderer never sees provider tokens. Scheduled windows are 08:00 and 20:00 local time; a
        scheduled run is skipped while locked and retried inside the five-minute window.
        {!status?.providerLive
          ? " Add Sandbox credentials below to enable live linking; CSV import works without a provider."
          : " Sandbox credentials are configured; Production remains disabled."}
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

      {!readOnly ? (
        <div className="plaid-setup">
          <h3>Plaid Sandbox</h3>
          <p className="fine-print">
            Client ID and Sandbox secret are entered in a native sheet and stored only in the macOS
            Keychain. The renderer never receives them. This build permits Sandbox only; Production
            linking stays disabled until Sandbox is verified.
          </p>
          <div className="button-row">
            <Button variant="secondary" onClick={configurePlaid} disabled={busy}>
              {plaid?.configured ? "Replace Sandbox credentials…" : "Add Sandbox credentials…"}
            </Button>
            {plaid?.configured ? (
              <Button variant="ghost" onClick={testCredentials} disabled={busy}>
                Test credentials
              </Button>
            ) : null}
            {plaid?.configured ? (
              <Button
                variant="primary"
                onClick={startLink}
                disabled={busy || (link !== null && (link.state === "waiting" || link.state === "starting"))}
              >
                Link a Sandbox institution…
              </Button>
            ) : null}
            {link?.state === "waiting" || link?.state === "starting" ? (
              <Button variant="ghost" onClick={cancelLink} disabled={busy}>
                Cancel link
              </Button>
            ) : null}
          </div>
          {link?.state === "waiting" ? (
            <p className="inline-notice" role="status">
              Waiting for the Plaid Sandbox flow in your browser… This window will update when the
              Item is added.
            </p>
          ) : null}
          {link?.state === "linked" ? (
            <p className="inline-notice" role="status">
              {link.message ?? "Linked. Connection ID is held locally; no token is shown."}
            </p>
          ) : null}
          <p className="fine-print">
            Capabilities requested: Transactions (required), Balance, and optional Liabilities.
            Liabilities is shown as unavailable rather than assumed when the Item or plan lacks it.
          </p>
          {connections.length > 0 ? (
            <div className="button-row">
              <Button variant="ghost" onClick={loadVerificationReport} disabled={busy}>
                Show verification report
              </Button>
              {report ? (
                <Button variant="ghost" onClick={copyReport} disabled={busy}>
                  Copy report
                </Button>
              ) : null}
            </div>
          ) : null}
          {report ? (
            <pre className="verification-report" aria-label="Plaid verification report">
              {report}
            </pre>
          ) : null}
        </div>
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
              <th scope="col">Actions</th>
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
                <td>
                  {connection.provider === "plaid" && connection.status !== "healthy" ? (
                    <Button
                      variant="ghost"
                      className="btn-small"
                      onClick={() => void reconnect(connection.id)}
                      disabled={busy}
                    >
                      Reconnect…
                    </Button>
                  ) : (
                    <span className="fine-print">—</span>
                  )}
                </td>
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
