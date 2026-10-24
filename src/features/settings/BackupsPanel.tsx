import { useEffect, useState } from "react";
import { command, type BackupItem, type VaultState } from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { formatDateOnly } from "../../lib/formatting";
import { Button } from "../../components/Button";

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
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [restoreConfirm, setRestoreConfirm] = useState(false);
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
      const result = await command("backup_list", {});
      setBackups(result.backups);
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
      setMessage(`Encrypted backup created: ${result.backup.id}`);
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

  async function restoreBackup() {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const result = await command("backup_restore", {});
      setMessage(`Restored from ${result.kind} backup. A pre-restore backup was created.`);
      setRestoreConfirm(false);
      await load();
      onDataChanged();
      await refreshVault();
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function portableExport() {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const result = await command("portable_export", {});
      setMessage(
        `Portable encrypted export written: ${result.id}. Store its separate password safely.`,
      );
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
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
        Backups are self-contained authenticated SQLCipher containers. Automatic daily backups are
        kept (7), plus capped pre-migration, pre-restore, and pre-change backups. Losing the
        passphrase and every backup passphrase makes the data unrecoverable. Backups exclude
        provider tokens and the Plaid client secret. Restore is validated before the live ledger is
        replaced.
      </p>
      <p className="fine-print">
        Last backup:{" "}
        {lastBackupAt
          ? `${formatDateOnly(lastBackupAt.slice(0, 10))} (${lastBackupStatus ?? "unknown"})`
          : "None recorded"}
        . Auto-lock and scheduled sync may mean a backup is older than the last edit; use “Back up
        now” after important changes.
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
        <Button variant="primary" onClick={createBackup} disabled={busy}>
          Back up now
        </Button>
        <Button variant="secondary" onClick={() => setRestoreConfirm(true)} disabled={busy}>
          Restore backup…
        </Button>
        <Button variant="secondary" onClick={portableExport} disabled={busy}>
          Portable encrypted export…
        </Button>
      </div>

      {restoreConfirm ? (
        <div className="export-warning" role="alert">
          <strong>Replace the current ledger?</strong>
          <p>
            Choose an encrypted backup, then enter its passphrase in a native sheet. Validation runs
            first; a pre-restore encrypted backup is created; the database is replaced atomically.
            The current passphrase remains in effect. A failed restore leaves the original intact.
          </p>
          <div className="button-row">
            <Button variant="primary" onClick={() => void restoreBackup()} disabled={busy}>
              Choose backup and restore…
            </Button>
            <Button variant="ghost" onClick={() => setRestoreConfirm(false)} disabled={busy}>
              Cancel
            </Button>
          </div>
        </div>
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
                    disabled={busy}
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

      <div className="danger-zone">
        <h3>Crypto-erase</h3>
        <p className="fine-print">
          Deletes the encrypted ledger, header, backups, demo database, and staged files. There is
          no recovery. Exports, Time Machine snapshots, and other devices are outside this action.
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
                disabled={eraseConfirm.trim().toLowerCase() !== "erase" || busy}
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
          <Button variant="danger" onClick={() => setEraseOpen(true)}>
            Erase all local data…
          </Button>
        )}
      </div>
    </section>
  );
}
