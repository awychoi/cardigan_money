import { useState } from "react";
import { command, type RestoreResult, type VaultState } from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { Button } from "../../components/Button";
import { BrandMark } from "../../components/BrandMark";
import { MakerLine } from "../../components/MadeBy";
import { RestoreBackupFlow } from "../settings/RestoreBackupFlow";

type Busy = "create" | "unlock" | "backup" | null;

export function VaultGate({
  exists,
  themeId,
  onOpened,
}: {
  exists: boolean;
  themeId?: string;
  onOpened: (state: VaultState) => void;
}) {
  const [busy, setBusy] = useState<Busy>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [restoreOpen, setRestoreOpen] = useState(false);

  async function createVault() {
    setBusy("create");
    setError(null);
    setNotice(null);
    try {
      onOpened(await command("vault_create", {}));
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(null);
    }
  }

  async function openDemo() {
    setBusy("unlock");
    setError(null);
    setNotice(null);
    try {
      onOpened(await command("demo_open", {}));
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(null);
    }
  }

  async function backUpNow() {
    setBusy("backup");
    setError(null);
    setNotice(null);
    try {
      const result = await command("backup_create", {});
      setNotice(
        result.listed
          ? `Encrypted backup created: ${result.backup.id}`
          : `Encrypted backup created, but it is not listed or retained automatically: ${result.backup.id}. Rename it to a standard cm-backup-… name in the app backup folder to include it in retention.`,
      );
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(null);
    }
  }

  async function restored(result: RestoreResult) {
    setRestoreOpen(false);
    setError(null);
    try {
      const vault = await command("vault_status", {});
      onOpened(vault);
      if (vault.unlocked) {
        setNotice(
          result.reusedConnections > 0
            ? `Backup restored. ${result.reusedConnections} Plaid Item${
                result.reusedConnections === 1 ? "" : "s"
              } reused; syncing now.`
            : "Backup restored. The vault is unlocked.",
        );
      } else if (result.vaultKeyChanged) {
        setNotice("Backup restored. Unlock with the new vault passphrase.");
      } else if (result.adoptedBackupKey) {
        setNotice("Backup restored. Unlock with the backup passphrase.");
      } else {
        setNotice("Backup restored. Unlock to continue.");
      }
    } catch (caught) {
      setError(friendlyError(caught));
    }
  }

  async function unlockVault() {
    setBusy("unlock");
    setError(null);
    setNotice(null);
    try {
      onOpened(await command("vault_unlock", {}));
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(null);
    }
  }

  return (
    <main className="vault-gate" aria-labelledby="vault-title">
      <div className="vault-card">
        <div className="vault-brand">
          <BrandMark themeId={themeId} />
          <span className="brand-name">cardigan_money</span>
        </div>

        {exists ? (
          <>
            <h1 id="vault-title">Your encrypted ledger is locked.</h1>
            <p className="muted">
              Your cards, balances, and spending by calendar month. The ledger is encrypted and
              opens only with your passphrase.
            </p>
            <div className="vault-actions">
              <Button
                variant="primary"
                onClick={unlockVault}
                disabled={busy !== null || restoreOpen}
              >
                {busy === "unlock" ? "Waiting for the native sheet…" : "Unlock vault"}
              </Button>
              <Button
                variant="secondary"
                onClick={() => {
                  setError(null);
                  setNotice(null);
                  setRestoreOpen(true);
                }}
                disabled={busy !== null || restoreOpen}
              >
                Restore from backup…
              </Button>
              <Button
                variant="secondary"
                onClick={backUpNow}
                disabled={busy !== null || restoreOpen}
              >
                {busy === "backup" ? "Waiting for the native sheets…" : "Back up now"}
              </Button>
            </div>
            <p className="sheet-note" role="note">
              The passphrase is requested by a native macOS sheet. This window never sees it.
            </p>
            <div className="vault-demo">
              <Button variant="ghost" onClick={openDemo} disabled={busy !== null || restoreOpen}>
                Open demo (synthetic data)
              </Button>
              <span className="fine-print">
                Separate database. No passphrase, no real data, no provider.
              </span>
            </div>
          </>
        ) : (
          <>
            <h1 id="vault-title">Create your encrypted ledger.</h1>
            <p className="muted">
              First launch starts empty. You choose the passphrase; the app never stores it and
              there is no recovery if you lose it.
            </p>
            <ul className="vault-facts">
              <li>SQLCipher database with an Argon2id-wrapped random key.</li>
              <li>Passphrase collected only by a native macOS sheet.</li>
              <li>No account, provider, or network is required to begin.</li>
            </ul>
            <div className="vault-actions">
              <Button
                variant="primary"
                onClick={createVault}
                disabled={busy !== null || restoreOpen}
              >
                {busy === "create" ? "Waiting for the native sheet…" : "Create vault…"}
              </Button>
              <Button
                variant="secondary"
                onClick={() => {
                  setError(null);
                  setNotice(null);
                  setRestoreOpen(true);
                }}
                disabled={busy !== null || restoreOpen}
              >
                Restore from backup…
              </Button>
            </div>
            <p className="sheet-note" role="note">
              You will enter the passphrase twice in a native sheet. Losing it makes the ledger
              permanently unreadable.
            </p>
            <div className="vault-demo">
              <Button variant="ghost" onClick={openDemo} disabled={busy !== null || restoreOpen}>
                Open demo (synthetic data)
              </Button>
              <span className="fine-print">No passphrase needed. Synthetic data only.</span>
            </div>
          </>
        )}

        {restoreOpen ? (
          <RestoreBackupFlow
            onRestored={(result) => void restored(result)}
            onCancel={() => setRestoreOpen(false)}
          />
        ) : null}

        {error ? (
          <p className="inline-error" role="alert">
            {error}
          </p>
        ) : null}
        {notice ? (
          <p className="inline-notice" role="status">
            {notice}
          </p>
        ) : null}

        <div className="vault-maker">
          <MakerLine />
        </div>
      </div>
      <span className="sr-only">{notice ?? ""}</span>
    </main>
  );
}
