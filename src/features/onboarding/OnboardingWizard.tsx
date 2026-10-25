import { useState } from "react";
import { command, type AccountPayload, type Settings } from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { AccountForm } from "../accounts/AccountForm";
import { BrandMark } from "../../components/BrandMark";
import { Button } from "../../components/Button";

type Step = "account" | "provider" | "finish";

export function OnboardingWizard({
  settings,
  onFinished,
}: {
  settings: Settings;
  onFinished: (settings: Settings) => void;
}) {
  const [step, setStep] = useState<Step>("account");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [accountName, setAccountName] = useState<string | null>(null);

  async function addAccount(payload: AccountPayload) {
    setBusy(true);
    setError(null);
    try {
      const account = await command("account_create", payload);
      setAccountName(account.displayName);
      setStep("provider");
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function finish() {
    setBusy(true);
    setError(null);
    try {
      const result = await command("settings_update", { onboardingCompleted: true });
      onFinished(result.settings);
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  const stepIndex = step === "account" ? 1 : step === "provider" ? 2 : 3;

  return (
    <main className="wizard" aria-labelledby="wizard-title">
      <div className="wizard-card">
        <header className="wizard-header">
          <div className="vault-brand">
            <BrandMark />
            <span className="brand-name">cardigan_money</span>
          </div>
          <button
            type="button"
            className="btn btn-ghost btn-small"
            onClick={() => void finish()}
            disabled={busy}
          >
            Skip setup
          </button>
        </header>
        <p className="wizard-progress" role="status">
          Step {stepIndex} of 3 · {settings.timezone}
        </p>

        {step === "account" ? (
          <>
            <h1 id="wizard-title">Add your first account.</h1>
            <p className="muted">
              Optional and skippable. The app stays usable with zero accounts. No provider or
              network is required, and nothing is seeded with demo data.
            </p>
            <AccountForm submitLabel="Add account" busy={busy} onSubmit={addAccount} />
            <Button variant="ghost" onClick={() => setStep("provider")} disabled={busy}>
              Skip for now
            </Button>
          </>
        ) : step === "provider" ? (
          <>
            <h1 id="wizard-title">Connect a provider later.</h1>
            <p className="muted">
              Live Plaid linking is unavailable in this build: no credentials have been enrolled and
              the desktop flow is unverified. No credentials are requested here.
            </p>
            <p className="fine-print">
              You can still use the full offline ledger, manual accounts, and CSV import. Provider
              status is always labeled; demo data is never disguised as live.
            </p>
            {accountName ? (
              <p className="inline-notice" role="status">
                Added {accountName}. You can connect a provider later in Settings.
              </p>
            ) : null}
            <div className="button-row">
              <Button variant="primary" onClick={() => setStep("finish")}>
                Continue without a provider
              </Button>
              <Button variant="ghost" onClick={() => setStep("account")} disabled={busy}>
                Back
              </Button>
            </div>
          </>
        ) : (
          <>
            <h1 id="wizard-title">You are ready.</h1>
            <p className="muted">
              Your encrypted ledger is unlocked. Add more accounts, import CSV history, or use the
              read-only dashboard as it arrives in the next build stage.
            </p>
            <dl className="facts">
              <div>
                <dt>Passphrase</dt>
                <dd>Never stored · native sheet only</dd>
              </div>
              <div>
                <dt>Timezone</dt>
                <dd>{settings.timezone}</dd>
              </div>
              <div>
                <dt>Accounts added</dt>
                <dd>{accountName ?? "None yet"}</dd>
              </div>
            </dl>
            <div className="button-row">
              <Button variant="primary" onClick={() => void finish()} disabled={busy}>
                {busy ? "Finishing…" : "Finish setup"}
              </Button>
              <Button variant="ghost" onClick={() => setStep("provider")} disabled={busy}>
                Back
              </Button>
            </div>
          </>
        )}

        {error ? (
          <p className="inline-error" role="alert">
            {error}
          </p>
        ) : null}
        <p className="fine-print">
          The passphrase step is native and already complete. There is no recovery if the passphrase
          and every backup passphrase are lost.
        </p>
      </div>
    </main>
  );
}
