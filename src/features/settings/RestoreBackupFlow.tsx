import { useEffect, useRef, useState } from "react";
import {
  command,
  type PlaidTokenChoice,
  type RestorePreview,
  type RestoreResult,
  type RestoreScope,
} from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { Button } from "../../components/Button";

const SCOPES: { id: RestoreScope; label: string; detail: string }[] = [
  {
    id: "full",
    label: "Full database",
    detail: "Everything in the backup: financial data, settings, events, and import history.",
  },
  {
    id: "financial-settings",
    label: "Financial data + settings",
    detail:
      "Accounts, balances, transactions, rules, settings, and import history. Connections, account sources, sync history, and security events are reset.",
  },
  {
    id: "financial-only",
    label: "Financial data only",
    detail:
      "Accounts, balances, transactions, manual payment details, and merchant rules. Settings return to defaults; connections, sync history, security events, and import history are reset.",
  },
];

const CROSS_VAULT_CHOICES: { id: PlaidTokenChoice; label: string }[] = [
  { id: "delete-local", label: "Delete the old vault's local tokens." },
  {
    id: "delete-and-revoke",
    label: "Delete local tokens and revoke the old Plaid Items remotely.",
  },
  { id: "keep", label: "Keep the old tokens for rollback." },
];

const DESTRUCTIVE_CHOICES: { id: PlaidTokenChoice; label: string }[] = [
  { id: "delete-all", label: "Delete all app-managed provider tokens." },
  { id: "leave-all", label: "Leave app-managed provider tokens in place." },
];

export function RestoreBackupFlow({
  onRestored,
  onCancel,
}: {
  onRestored: (result: RestoreResult) => void;
  onCancel: () => void;
}) {
  const started = useRef(false);
  const [preview, setPreview] = useState<RestorePreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [scope, setScope] = useState<RestoreScope>("full");
  const [confirmation, setConfirmation] = useState("");
  const [allowNonPrivate, setAllowNonPrivate] = useState(false);
  const [destructive, setDestructive] = useState(false);
  const [plaidChoice, setPlaidChoice] = useState<PlaidTokenChoice | null>(null);

  useEffect(() => {
    if (started.current) return;
    started.current = true;
    void chooseBackup();
    // The native picker is intentionally opened once per mounted flow.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function chooseBackup() {
    setBusy(true);
    setError(null);
    try {
      const result = await command("backup_restore_prepare", {});
      setPreview(result);
    } catch (caught) {
      const code = (caught as { code?: string }).code;
      if (code === "cancelled") {
        onCancel();
        return;
      }
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function cancel() {
    if (preview) {
      try {
        await command("backup_restore_cancel", {});
      } catch {
        // The Rust pending state is advisory; the flow is closing anyway.
      }
    }
    onCancel();
  }

  async function apply() {
    if (!preview) return;
    setBusy(true);
    setError(null);
    try {
      const result = await command("backup_restore_apply", {
        scope: destructive ? "full" : scope,
        allowNonPrivate: preview.nonPrivate ? allowNonPrivate : false,
        confirmation:
          preview.requiresTypedConfirmation && !preview.freshInstall
            ? confirmation.trim().toLowerCase()
            : undefined,
        plaidChoice:
          destructive || (scope === "full" && preview.requiresPlaidChoice) ? plaidChoice : null,
        destructive,
      });
      onRestored(result);
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  const typedConfirmationReady =
    !preview?.requiresTypedConfirmation ||
    preview.freshInstall ||
    confirmation.trim().toLowerCase() === "replace";
  const privateReady = !preview?.nonPrivate || allowNonPrivate;
  const plaidReady = destructive
    ? plaidChoice === "delete-all" || plaidChoice === "leave-all"
    : !(scope === "full" && preview?.requiresPlaidChoice) || plaidChoice !== null;
  const canApply =
    preview !== null && typedConfirmationReady && privateReady && plaidReady && !busy;

  if (!preview) {
    return (
      <div className="export-warning" role="alert" aria-live="polite">
        <strong>Choose an encrypted backup</strong>
        <p>
          A native file picker is open. Nothing is replaced until validation passes and you confirm
          the restore.
        </p>
        {error ? (
          <p className="inline-error" role="alert">
            {error}
          </p>
        ) : null}
        <div className="button-row">
          <Button variant="ghost" onClick={() => void cancel()} disabled={busy}>
            Cancel
          </Button>
        </div>
      </div>
    );
  }

  const isCrossVault = !preview.freshInstall && !preview.sameVault;

  return (
    <div className="export-warning" role="alert">
      <strong>
        {preview.freshInstall
          ? "Create a vault from this backup?"
          : preview.sameVault
            ? "Replace the current ledger?"
            : "Restore a backup from a different vault?"}
      </strong>
      <p>
        {preview.backup.kind.replaceAll("-", " ")} backup, {preview.backupVaultPrefix}…, schema{" "}
        {preview.backup.schemaVersion}, generation {preview.backup.generation}.
      </p>
      {preview.freshInstall ? (
        <p>
          Fresh install: the backup&apos;s vault identity and passphrase become the new vault. No
          pre-restore backup is made.
        </p>
      ) : null}
      {isCrossVault && !destructive && !preview.freshInstall ? (
        scope === "full" ? (
          <p>
            This backup belongs to a different vault. A full restore adopts its identity and
            passphrase. The current pre-restore backup is retired and kept until you delete it, and
            still requires the previous passphrase.
          </p>
        ) : (
          <p>
            This backup belongs to a different vault. A partial restore keeps the current identity
            and passphrase; unselected data is reset.
          </p>
        )
      ) : null}
      {destructive ? (
        <p>
          <strong>Destructive restore.</strong> No pre-restore backup is made. The backup passphrase
          becomes the vault passphrase, and the header and data key are replaced.
          {isCrossVault
            ? " Previous-vault normal backups are renamed with the retired prefix when the vault identity changes."
            : ""}
        </p>
      ) : null}
      {preview.nonPrivate ? (
        <p>
          <strong>This file is not owner-only (0600).</strong> Acknowledging bypasses only the
          file-mode check; the remaining validation still runs.
        </p>
      ) : null}
      {preview.requiresCurrentUnlock && !preview.freshInstall && !destructive ? (
        <p>
          The vault is locked. A native sheet will ask for the current passphrase so the app can
          create a verified pre-restore backup.
        </p>
      ) : null}

      <fieldset className="restore-options">
        <legend>Restore scope</legend>
        {SCOPES.map((option) => (
          <label key={option.id} className="restore-option">
            <input
              type="radio"
              name="restore-scope"
              value={option.id}
              checked={destructive ? option.id === "full" : scope === option.id}
              disabled={busy || destructive}
              onChange={() => setScope(option.id)}
            />
            <span>
              <strong>{option.label}.</strong> {option.detail}
            </span>
          </label>
        ))}
      </fieldset>

      {preview.destructiveAvailable ? (
        <label className="restore-option">
          <input
            type="checkbox"
            checked={destructive}
            disabled={busy}
            onChange={(event) => {
              setDestructive(event.target.checked);
              if (event.target.checked) setScope("full");
            }}
          />
          <span>
            <strong>I do not have the current vault passphrase.</strong> Full database only, no
            pre-restore backup, old vault files kept until the new vault is verified. This cannot be
            undone if the restore then fails.
          </span>
        </label>
      ) : null}

      {scope === "full" && preview.requiresPlaidChoice && !destructive ? (
        <fieldset className="restore-options">
          <legend>The current vault&apos;s Plaid Items</legend>
          {CROSS_VAULT_CHOICES.map((choice) => (
            <label key={choice.id} className="restore-option">
              <input
                type="radio"
                name="plaid-choice"
                value={choice.id}
                checked={plaidChoice === choice.id}
                disabled={busy}
                onChange={() => setPlaidChoice(choice.id)}
              />
              <span>{choice.label}</span>
            </label>
          ))}
        </fieldset>
      ) : null}

      {destructive ? (
        <fieldset className="restore-options">
          <legend>The old vault&apos;s provider tokens</legend>
          <p>
            Provider tokens cannot be read while the vault is locked. Choose whether to delete the
            app-managed tokens or leave them.
          </p>
          {DESTRUCTIVE_CHOICES.map((choice) => (
            <label key={choice.id} className="restore-option">
              <input
                type="radio"
                name="plaid-choice-destructive"
                value={choice.id}
                checked={plaidChoice === choice.id}
                disabled={busy}
                onChange={() => setPlaidChoice(choice.id)}
              />
              <span>{choice.label}</span>
            </label>
          ))}
        </fieldset>
      ) : null}

      {preview.nonPrivate ? (
        <label className="restore-option">
          <input
            type="checkbox"
            checked={allowNonPrivate}
            disabled={busy}
            onChange={(event) => setAllowNonPrivate(event.target.checked)}
          />
          <span>I understand this file is readable by other local users and want to proceed.</span>
        </label>
      ) : null}

      {preview.requiresTypedConfirmation && !preview.freshInstall ? (
        <p>
          Type “replace” to confirm. Nothing is replaced until the backup validates; a pre-restore
          backup is made first unless you chose destructive.
        </p>
      ) : null}
      {preview.requiresTypedConfirmation && !preview.freshInstall ? (
        <input
          aria-label="Type replace to confirm"
          value={confirmation}
          disabled={busy}
          onChange={(event) => setConfirmation(event.target.value)}
          placeholder="replace"
        />
      ) : null}

      {error ? (
        <p className="inline-error" role="alert">
          {error}
        </p>
      ) : null}

      <div className="button-row">
        <Button variant="primary" onClick={() => void apply()} disabled={!canApply}>
          {destructive ? "Replace without a pre-restore backup…" : "Restore this backup…"}
        </Button>
        <Button variant="ghost" onClick={() => void cancel()} disabled={busy}>
          Cancel
        </Button>
      </div>
    </div>
  );
}
