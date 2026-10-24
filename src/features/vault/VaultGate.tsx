import { useState } from "react";
import { command, type VaultState } from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { Button } from "../../components/Button";

type Busy = "create" | "unlock" | null;

export function VaultGate({
  exists,
  onOpened,
}: {
  exists: boolean;
  onOpened: (state: VaultState) => void;
}) {
  const [busy, setBusy] = useState<Busy>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

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
          <span className="brand-mark" aria-hidden="true">
            cm
          </span>
          <span className="brand-name">cardigan_money</span>
        </div>

        {exists ? (
          <>
            <h1 id="vault-title">Your encrypted ledger is locked.</h1>
            <p className="muted">
              A calendar-month view of your cards, balances, and spending. The local ledger is
              protected by SQLCipher and opens only with your passphrase.
            </p>
            <div className="vault-actions">
              <Button variant="primary" onClick={unlockVault} disabled={busy !== null}>
                {busy === "unlock" ? "Waiting for the native sheet…" : "Unlock vault"}
              </Button>
            </div>
            <p className="sheet-note" role="note">
              The passphrase is requested by a native macOS sheet. This window never sees it.
            </p>
          </>
        ) : (
          <>
            <h1 id="vault-title">Create your encrypted ledger.</h1>
            <p className="muted">
              First launch starts empty. You choose the passphrase; the app never stores it and there
              is no recovery if you lose it.
            </p>
            <ul className="vault-facts">
              <li>SQLCipher database with an Argon2id-wrapped random key.</li>
              <li>Passphrase collected only by a native macOS sheet.</li>
              <li>No account, provider, or network is required to begin.</li>
            </ul>
            <div className="vault-actions">
              <Button variant="primary" onClick={createVault} disabled={busy !== null}>
                {busy === "create" ? "Waiting for the native sheet…" : "Create vault…"}
              </Button>
            </div>
            <p className="sheet-note" role="note">
              You will enter the passphrase twice in a native sheet. Spaces and case are preserved.
              Losing it makes the ledger permanently unreadable.
            </p>
          </>
        )}

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
        <p className="fine-print">
          This is a local, single-user build. It is not distribution-ready and does not claim to be
          “watertight”.
        </p>
      </div>
      <span className="sr-only">{notice ?? ""}</span>
    </main>
  );
}
