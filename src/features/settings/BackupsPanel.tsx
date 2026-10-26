import { useEffect, useState } from "react";
import { command, type BackupItem, type RestoreResult, type VaultState } from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { formatDateOnly } from "../../lib/formatting";
import { Button } from "../../components/Button";
import { RestoreBackupFlow } from "./RestoreBackupFlow";

function restoreMessage(result: RestoreResult): string {
  const scope =
    result.scope === "full"
      ? "full database"
      : result.scope === "financial-settings"
        ? "financial data + settings"
        : "financial data only";
  const notes = [`Restored a ${result.kind.replaceAll("-", " ")} backup (${scope}).`];
  if (result.preRestoreBackupId) {
    notes.push("A pre-restore backup was created and verified.");
    if (result.preRestoreProtection === "previous") {
      notes.push("That pre-restore backup still requires your previous vault passphrase.");
    }
  } else {
    notes.push("No pre-restore backup was created.");
  }
  if (result.vaultIdChanged) {
    notes.push("The restored vault uses the backup's vault identity.");
  }
  if (result.adoptedBackupKey) {
    notes.push("The backup passphrase is now the vault passphrase.");
  }
  if (result.retiredBackups > 0) {
    notes.push(
      `${result.retiredBackups} previous-vault backup${result.retiredBackups === 1 ? "" : "s"} renamed with the retired prefix; they are kept until deleted.`,
    );
  }
  if (result.vaultKeyChanged) {
    notes.push("The vault passphrase was changed.");
  }
  return notes.join(" ");
}

export function BackupsPanel({
  onDataChanged,
  onVaultChanged,
  lastBackupAt,
  lastBackupStatus,
}: {
  onDataChanged: () => void;
  onVaultChanged: (vault: VaultState) => void;
  lastBackupAt: string | null;
  lastBackupStatus: string | null;
}) {
  const [backups, setBackups] = useState<BackupItem[]>([]);
  const [retired, setRetired] = useState<BackupItem[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [restoreOpen, setRestoreOpen] = useState(false);
  const [eraseConfirm, setEraseConfirm] = useState("");
  const [eraseOpen, setEraseOpen] = useState(false);

  async function refreshVault() {
    try {
      const vault = await command("vault_status", {});
      onVaultChanged(vault);
    } catch {
      // The gate/lock event will correct the view if the session ended.
    }
  }

  async function load() {
    try {
      const [current, old] = await Promise.all([
        command("backup_list", {}),
        command("backup_list_retired", {}),
      ]);
      setBackups(current.backups);
      setRetired(old.backups);
    } catch (caught) {
      setError(friendlyError(caught));
    }
  }

  useEffect(() => {
    void load();
  }, []);

  async function createBackup() {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const result = await command("backup_create", {});
      setMessage(
        result.listed
          ? `Encrypted backup created: ${result.backup.id}`
          : `Encrypted backup created, but it is not listed or retained automatically: ${result.backup.id}. Rename it to a standard cm-backup-… name in the app backup folder to include it in retention.`,
      );
      await load();
      await refreshVault();
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function deleteBackup(id: string) {
    setBusy(true);
    setError(null);
    try {
      await command("backup_delete", { id });
      setMessage("Backup deleted.");
      await load();
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function restored(result: RestoreResult) {
    setRestoreOpen(false);
    setMessage(restoreMessage(result));
    setError(null);
    await load();
    onDataChanged();
    await refreshVault();
  }

  async function eraseEverything() {
    setBusy(true);
    setError(null);
    try {
      const vault = await command("crypto_erase", {});
      onVaultChanged(vault);
    } catch (caught) {
      setError(friendlyError(caught));
      setBusy(false);
    }
  }

  return (
    <section className="panel panel-wide" aria-labelledby="backups-title">
      <h2 id="backups-title">Backups &amp; recovery</h2>
      <p className="fine-print">
        Automatic daily backups are kept (7), plus a few pre-migration and pre-restore copies.
        Losing your passphrase and every backup passphrase makes the data unrecoverable. “Back up
        now” can use a separate passphrase — leave it blank to reuse the vault passphrase. Backups
        saved outside the app backup folder are never pruned.
      </p>
      <p className="fine-print">
        Last backup:{" "}
        {lastBackupAt
          ? `${formatDateOnly(lastBackupAt.slice(0, 10))} (${lastBackupStatus ?? "unknown"})`
          : "None recorded"}
        . Auto-lock can make a backup older than your last edit.
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
        <Button variant="primary" onClick={createBackup} disabled={busy || restoreOpen}>
          Back up now
        </Button>
        <Button
          variant="secondary"
          onClick={() => {
            setError(null);
            setMessage(null);
            setRestoreOpen(true);
          }}
          disabled={busy || restoreOpen}
        >
          Restore backup…
        </Button>
      </div>

      {restoreOpen ? (
        <RestoreBackupFlow
          onRestored={(result) => void restored(result)}
          onCancel={() => setRestoreOpen(false)}
        />
      ) : null}

      {backups.length === 0 ? (
        <p className="muted">No encrypted backups yet.</p>
      ) : (
        <table className="accounts-table">
          <caption className="sr-only">Encrypted backups</caption>
          <thead>
            <tr>
              <th scope="col">Kind</th>
              <th scope="col">Created</th>
              <th scope="col">Size</th>
              <th scope="col">Actions</th>
            </tr>
          </thead>
          <tbody>
            {backups.map((backup) => (
              <tr key={backup.id}>
                <td>{backup.kind.replaceAll("-", " ")}</td>
                <td>{formatDateOnly(backup.createdAt.slice(0, 10))}</td>
                <td>{(backup.sizeBytes / 1024).toFixed(0)} KiB</td>
                <td>
                  <Button
                    variant="ghost"
                    className="btn-small"
                    disabled={busy || restoreOpen}
                    onClick={() => void deleteBackup(backup.id)}
                  >
                    Delete
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {retired.length > 0 ? (
        <details className="retired-backups">
          <summary>
            Retired backups from a previous vault ({retired.length}) — never pruned automatically
          </summary>
          <p className="fine-print">
            These belong to a vault replaced by a cross-vault restore. Restoring one needs the
            previous passphrase. They stay until you delete them.
          </p>
          <table className="accounts-table">
            <caption className="sr-only">Retired backups</caption>
            <thead>
              <tr>
                <th scope="col">Kind</th>
                <th scope="col">Created</th>
                <th scope="col">Size</th>
                <th scope="col">Actions</th>
              </tr>
            </thead>
            <tbody>
              {retired.map((backup) => (
                <tr key={backup.id}>
                  <td>{backup.kind.replaceAll("-", " ")}</td>
                  <td>{formatDateOnly(backup.createdAt.slice(0, 10))}</td>
                  <td>{(backup.sizeBytes / 1024).toFixed(0)} KiB</td>
                  <td>
                    <Button
                      variant="ghost"
                      className="btn-small"
                      disabled={busy || restoreOpen}
                      onClick={() => void deleteBackup(backup.id)}
                    >
                      Delete
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </details>
      ) : null}

      <div className="danger-zone">
        <h3>Crypto-erase</h3>
        <p className="fine-print">
          Deletes the ledger, backups, demo database, and staged files. There is no recovery.
          Exports and Time Machine snapshots are unaffected.
        </p>
        {eraseOpen ? (
          <div className="export-warning" role="alert">
            <strong>This cannot be undone.</strong>
            <p>Type “erase” to enable the native confirmation.</p>
            <input
              aria-label="Type erase to confirm"
              value={eraseConfirm}
              onChange={(event) => setEraseConfirm(event.target.value)}
              placeholder="erase"
            />
            <div className="button-row">
              <Button
                variant="danger"
                disabled={eraseConfirm.trim().toLowerCase() !== "erase" || busy || restoreOpen}
                onClick={() => void eraseEverything()}
              >
                Erase all local data…
              </Button>
              <Button variant="ghost" onClick={() => setEraseOpen(false)} disabled={busy}>
                Cancel
              </Button>
            </div>
          </div>
        ) : (
          <Button
            variant="danger"
            onClick={() => setEraseOpen(true)}
            disabled={busy || restoreOpen}
          >
            Erase all local data…
          </Button>
        )}
      </div>
    </section>
  );
}
