import { useEffect, useState } from "react";
import {
  command,
  type ConnectionView,
  type PlaidEnvironment,
  type PlaidLinkStatus,
  type PlaidStatus,
  type SyncStatus,
} from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { Button } from "../../components/Button";

function environmentLabel(environment: PlaidEnvironment | null): string {
  return environment === "production" ? "Trial / Production" : "Sandbox";
}

function connectionEnvironment(value: string): PlaidEnvironment {
  return value === "production" ? "production" : "sandbox";
}

function connectionStatusView(connection: ConnectionView): {
  label: string;
  className: string;
} {
  if (connection.status === "reconnect_required") {
    return { label: "reconnect required", className: "status-pill status-danger" };
  }
  // A restored connection that kept its Item is healthy but has no cursor yet.
  // It resyncs automatically with the same Plaid Item.
  if (connection.status === "healthy" && !connection.hasCursor) {
    return { label: "resync pending", className: "status-pill status-warn" };
  }
  if (connection.status === "healthy") {
    return { label: "healthy", className: "status-pill status-ok" };
  }
  return {
    label: connection.status.replaceAll("_", " "),
    className: "status-pill status-warn",
  };
}

export function ConnectionsSyncPanel({
  onSyncFinished,
  readOnly,
}: {
  onSyncFinished: () => void;
  readOnly: boolean;
}) {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [plaid, setPlaid] = useState<PlaidStatus | null>(null);
  const [environment, setEnvironment] = useState<PlaidEnvironment>("sandbox");
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
    } finally {
      setLoading(false);
    }
  }

  async function loadPlaid() {
    try {
      const result = await command("plaid_status", {});
      setPlaid(result.plaid);
      setEnvironment((current) => {
        if (result.plaid[current].configured) return current;
        if (result.plaid.sandbox.configured) return "sandbox";
        if (result.plaid.production.configured) return "production";
        return current;
      });
    } catch (caught) {
      setError(friendlyError(caught));
    }
  }

  useEffect(() => {
    void load();
    if (!readOnly) void loadPlaid();
    // The initial load is intentionally tied to readOnly, not to the helpers.
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
            await loadPlaid();
            await load();
            onSyncFinished();
          } else if (result.link.state === "duplicate") {
            setMessage(
              result.link.message ??
                "This bank is already connected. Use Update or Reconnect instead of linking it again.",
            );
            setError(null);
            await load();
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

  const environmentStatus = plaid?.[environment] ?? null;
  const configured = environmentStatus?.configured ?? false;
  const label = environmentLabel(environment);
  const linkActive = link?.state === "waiting" || link?.state === "starting";

  async function configurePlaid() {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const result = await command("plaid_setup", { environment });
      setPlaid(result.plaid);
      setMessage(`${label} credentials saved to the macOS Keychain.`);
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
      const result = await command("plaid_test", { environment });
      setPlaid(result.plaid);
      setMessage(`Plaid accepted the ${label} credentials.`);
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

  async function startLink(target: PlaidEnvironment = environment) {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const result = await command("plaid_link_start", { environment: target });
      setEnvironment(target);
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
      setMessage(result.link.message ?? "Update the Plaid Item in your browser.");
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
  const environmentConnections = connections.filter(
    (connection) => connectionEnvironment(connection.environment) === environment,
  );
  const linkedEnvironment = link?.environment
    ? environmentLabel(link.environment)
    : environmentLabel(environment);

  return (
    <section className="panel panel-wide" aria-labelledby="connections-title">
      <h2 id="connections-title">Bank connections</h2>
      <p className="inline-notice" role="note">
        Plaid is optional. Sandbox uses test data; Trial/Production links real banks and counts
        against your Plaid plan. Only credit accounts are imported.
      </p>
      <p className="fine-print">
        Sync runs after unlock and keeps the last reported values when a provider stream is
        unavailable. Scheduled windows are 08:00 and 20:00 local time; a scheduled run is skipped
        while locked.
        {!status?.providerLive
          ? " Add Plaid credentials below to enable linking; CSV import and manual cards work without a provider."
          : " Plaid credentials are configured; linked connections sync into this ledger."}
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
          <h3>Plaid connection</h3>
          <fieldset className="restore-options" disabled={busy || linkActive}>
            <legend>Environment</legend>
            <label className="restore-option">
              <input
                type="radio"
                name="plaid-environment"
                value="sandbox"
                checked={environment === "sandbox"}
                onChange={() => setEnvironment("sandbox")}
              />
              <span>
                <strong>Sandbox</strong> — test Items only; no real bank data.
              </span>
            </label>
            <label className="restore-option">
              <input
                type="radio"
                name="plaid-environment"
                value="production"
                checked={environment === "production"}
                onChange={() => setEnvironment("production")}
              />
              <span>
                <strong>Trial / Production</strong> — real institutions via{" "}
                <code>production.plaid.com</code>.
              </span>
            </label>
          </fieldset>
          {environment === "production" ? (
            <p className="inline-notice" role="note">
              Real bank data will be imported. Plaid&apos;s free Trial covers up to 10 Production
              Items; paid plans bill while an Item exists, and removing one does not restore Trial
              capacity. Only link institutions you are authorized to access.
            </p>
          ) : null}
          <p className="fine-print">
            The {label} client ID and secret are entered in a native sheet and stored only in the
            macOS Keychain. {environmentStatus?.message ?? "Add credentials to enable linking."}
          </p>
          {environmentConnections.length > 0 ? (
            <p className="inline-notice" role="note">
              Already connected here? Use <strong>Update…</strong> or <strong>Reconnect…</strong>{" "}
              below. Only one active Plaid Item per institution is allowed.
            </p>
          ) : null}
          <div className="button-row">
            <Button variant="secondary" onClick={configurePlaid} disabled={busy || linkActive}>
              {configured ? `Replace ${label} credentials…` : `Add ${label} credentials…`}
            </Button>
            {configured ? (
              <Button variant="ghost" onClick={testCredentials} disabled={busy || linkActive}>
                Test credentials
              </Button>
            ) : null}
            {configured ? (
              <Button
                variant="primary"
                onClick={() => void startLink()}
                disabled={busy || linkActive}
              >
                {environment === "sandbox"
                  ? "Link a Sandbox institution…"
                  : "Link a real institution…"}
              </Button>
            ) : null}
            {linkActive ? (
              <Button variant="ghost" onClick={cancelLink} disabled={busy}>
                Cancel link
              </Button>
            ) : null}
          </div>
          {link?.state === "waiting" ? (
            <p className="inline-notice" role="status">
              Waiting for the Plaid {linkedEnvironment} flow in your browser… This window will
              update when the Item is added.
            </p>
          ) : null}
          {link?.state === "linked" ? (
            <p className="inline-notice" role="status">
              {link.message ?? "Linked. Connection ID is held locally; no token is shown."}
            </p>
          ) : null}
          <p className="fine-print">
            Requests Transactions (required), Balance, and Liabilities where available.
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

      {loading && connections.length === 0 ? (
        <p className="muted">Checking connections…</p>
      ) : connections.length === 0 ? (
        <p className="muted">
          No bank connected yet. Add Plaid credentials above, or import a CSV.
        </p>
      ) : (
        <table className="accounts-table">
          <caption className="sr-only">Provider connections</caption>
          <thead>
            <tr>
              <th scope="col">Institution</th>
              <th scope="col">Provider</th>
              <th scope="col">Environment</th>
              <th scope="col">Status</th>
              <th scope="col">Last success</th>
              <th scope="col">Accounts</th>
              <th scope="col">Actions</th>
            </tr>
          </thead>
          <tbody>
            {connections.map((connection) => {
              const statusView = connectionStatusView(connection);
              return (
                <tr key={connection.id}>
                  <td>{connection.institutionName ?? "Provider"}</td>
                  <td>{connection.provider}</td>
                  <td>{environmentLabel(connectionEnvironment(connection.environment))}</td>
                  <td>
                    <span
                      className={statusView.className}
                      title={connection.sanitizedError ?? undefined}
                    >
                      {statusView.label}
                    </span>
                  </td>
                  <td>
                    {connection.lastSuccessAt ? connection.lastSuccessAt.slice(0, 10) : "Never"}
                  </td>
                  <td>{connection.accountCount}</td>
                  <td>
                    {connection.provider === "plaid" ? (
                      connection.hasToken ? (
                        <Button
                          variant="ghost"
                          className="btn-small"
                          onClick={() => void reconnect(connection.id)}
                          disabled={busy || linkActive}
                          title={
                            connection.status === "healthy"
                              ? "Use Plaid update mode to add accounts or refresh this Item. This reuses the existing Item; it does not create another one."
                              : "Repair this Item in Plaid update mode without creating another Item."
                          }
                        >
                          {connection.status === "healthy" ? "Update…" : "Reconnect…"}
                        </Button>
                      ) : (
                        <Button
                          variant="ghost"
                          className="btn-small"
                          onClick={() =>
                            void startLink(connectionEnvironment(connection.environment))
                          }
                          disabled={
                            busy ||
                            linkActive ||
                            !plaid?.[connectionEnvironment(connection.environment)].configured
                          }
                          title="This connection came from a restore and has no local token. A new link is required."
                        >
                          Link again…
                        </Button>
                      )
                    ) : (
                      <span className="fine-print">—</span>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}

      {connections.some(
        (connection) =>
          connection.provider === "plaid" &&
          connection.status === "healthy" &&
          !connection.hasCursor,
      ) ? (
        <p className="fine-print">
          Restored connections resync with the Plaid Item already stored in the Keychain; no new
          Item is created.
        </p>
      ) : null}

      {connections.some(
        (connection) =>
          connection.provider === "plaid" &&
          connection.status !== "healthy" &&
          !connection.hasToken,
      ) ? (
        <p className="fine-print">
          “Link again…” creates a fresh Plaid Item because no local token exists; the restored
          connection stays until you disconnect it.
        </p>
      ) : null}

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
                      .map(
                        (step) =>
                          `${step.stream}: ${step.status} (+${step.itemsAdded}/~${step.itemsModified}/-${step.itemsRemoved})`,
                      )
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
