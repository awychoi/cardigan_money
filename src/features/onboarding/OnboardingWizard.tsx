import { useEffect, useMemo, useState } from "react";
import {
  command,
  type AccountPayload,
  type PlaidEnvironment,
  type PlaidStatus,
  type Settings,
} from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import {
  detectTimeZone,
  FALLBACK_TIMEZONE,
  formatTimeZonePreview,
  timeZoneSummary,
} from "../../lib/timezone";
import { AccountForm } from "../accounts/AccountForm";
import { BrandMark } from "../../components/BrandMark";
import { BrandName } from "../../components/BrandName";
import { Button } from "../../components/Button";
import { TimezoneSelect } from "../../components/TimezoneSelect";

type Step = "connect" | "account" | "timezone" | "finish";

const STEPS: { id: Step; label: string; optional?: boolean }[] = [
  { id: "connect", label: "Bank connection" },
  { id: "account", label: "Manual card", optional: true },
  { id: "timezone", label: "Timezone" },
  { id: "finish", label: "Ready" },
];

function environmentLabel(environment: PlaidEnvironment): string {
  return environment === "production" ? "Trial / Production" : "Sandbox";
}

export function OnboardingWizard({
  settings,
  onSettingsChanged,
  onFinished,
}: {
  settings: Settings;
  onSettingsChanged: (settings: Settings) => void;
  onFinished: (settings: Settings) => void;
}) {
  const [step, setStep] = useState<Step>("connect");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [accountName, setAccountName] = useState<string | null>(null);
  const [plaid, setPlaid] = useState<PlaidStatus | null>(null);
  const [plaidLoading, setPlaidLoading] = useState(true);
  const [plaidError, setPlaidError] = useState<string | null>(null);
  const [environment, setEnvironment] = useState<PlaidEnvironment>("sandbox");
  const detected = useMemo(() => detectTimeZone(), []);
  const [timezone, setTimezone] = useState(() =>
    settings.timezone && settings.timezone !== FALLBACK_TIMEZONE
      ? settings.timezone
      : detected.zone,
  );

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const result = await command("plaid_status", {});
        if (cancelled) return;
        setPlaid(result.plaid);
        if (!result.plaid.sandbox.configured && result.plaid.production.configured) {
          setEnvironment("production");
        }
      } catch (caught) {
        if (!cancelled) setPlaidError(friendlyError(caught));
      } finally {
        if (!cancelled) setPlaidLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  function go(next: Step) {
    setStep(next);
    setError(null);
    setNotice(null);
  }

  async function saveCredentials() {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const result = await command("plaid_setup", { environment });
      setPlaid(result.plaid);
      setNotice(
        `${environmentLabel(environment)} credentials saved to the macOS Keychain. This window never received them.`,
      );
    } catch (caught) {
      if ((caught as { code?: string }).code === "cancelled") {
        setNotice("Credential setup cancelled. Nothing was saved.");
      } else {
        setError(friendlyError(caught));
      }
    } finally {
      setBusy(false);
    }
  }

  async function addAccount(payload: AccountPayload) {
    setBusy(true);
    setError(null);
    try {
      const account = await command("account_create", payload);
      setAccountName(account.displayName);
      setStep("timezone");
      setNotice(null);
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function saveTimezone() {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const result = await command("settings_update", { timezone });
      onSettingsChanged(result.settings);
      setStep("finish");
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
      const result = await command("settings_update", { timezone, onboardingCompleted: true });
      onFinished(result.settings);
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  const stepIndex = STEPS.findIndex((item) => item.id === step);
  const environmentStatus = plaid?.[environment] ?? null;
  const configured = environmentStatus?.configured ?? false;
  const configuredEnvironments: PlaidEnvironment[] = plaid
    ? (["sandbox", "production"] as const).filter((value) => plaid[value].configured)
    : [];
  const currentLabel = environmentLabel(environment);
  const bankSummary =
    configuredEnvironments.length === 0
      ? "Not configured yet"
      : `${configuredEnvironments.map(environmentLabel).join(" + ")} credentials (Keychain)`;
  const timezonePreview = formatTimeZonePreview(timezone);

  return (
    <main className="wizard" aria-labelledby="wizard-title">
      <div className="wizard-card">
        <header className="wizard-header">
          <div className="vault-brand">
            <BrandMark themeId={settings.themeId} />
            <BrandName />
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

        <ol className="wizard-steps" aria-label="Setup progress">
          {STEPS.map((item, index) => (
            <li
              key={item.id}
              className={`wizard-step${index === stepIndex ? " wizard-step-active" : ""}${
                index < stepIndex ? " wizard-step-past" : ""
              }`}
              aria-current={index === stepIndex ? "step" : undefined}
            >
              <span className="wizard-step-number" aria-hidden="true">
                {index + 1}
              </span>
              <span>
                {item.label}
                {item.optional ? <span className="wizard-step-optional"> · optional</span> : null}
              </span>
            </li>
          ))}
        </ol>
        <p className="wizard-progress">
          Optional steps can be skipped. Everything here can be changed later in Settings.
        </p>

        {step === "connect" ? (
          <>
            <h1 id="wizard-title">Connect your bank.</h1>
            <p className="muted">
              Plaid can sync card balances and transactions automatically. It is optional: you can
              add a card by hand, import a CSV, or set up a bank after the vault opens.
            </p>

            <div className="wizard-callout">
              <strong>How your Plaid credentials are stored</strong>
              <p>
                A native macOS sheet asks for the Client ID and secret. They are stored only in the
                macOS Keychain, never in the ledger or exports.
              </p>
            </div>

            <fieldset className="restore-options" disabled={busy || plaidLoading}>
              <legend>Plaid environment</legend>
              <label className="restore-option">
                <input
                  type="radio"
                  name="onboarding-plaid-environment"
                  value="sandbox"
                  checked={environment === "sandbox"}
                  onChange={() => {
                    setEnvironment("sandbox");
                    setNotice(null);
                  }}
                />
                <span>
                  <strong>Sandbox</strong> — test Items only. Safe for trying the flow; no real bank
                  data.
                </span>
              </label>
              <label className="restore-option">
                <input
                  type="radio"
                  name="onboarding-plaid-environment"
                  value="production"
                  checked={environment === "production"}
                  onChange={() => {
                    setEnvironment("production");
                    setNotice(null);
                  }}
                />
                <span>
                  <strong>Trial / Production</strong> — real institutions and real account data.
                  Trial limits and Plaid billing may apply.
                </span>
              </label>
            </fieldset>

            {environment === "production" ? (
              <p className="inline-notice" role="note">
                Real bank data will be imported. Plaid&apos;s free Trial includes up to 10
                Production Items; paid products can bill while an Item exists. Only link
                institutions you are authorized to access.
              </p>
            ) : null}

            {plaidError ? (
              <p className="fine-print">
                Plaid status is unavailable ({plaidError}). You can still skip this step and
                configure it later in Settings → Accounts → Bank connections.
              </p>
            ) : null}

            <div className="button-row wizard-credential-row">
              <Button
                variant={configured ? "secondary" : "primary"}
                onClick={() => void saveCredentials()}
                disabled={busy || plaidLoading}
              >
                {busy
                  ? "Waiting for the macOS sheet…"
                  : configured
                    ? `Replace ${currentLabel} credentials…`
                    : `Add ${currentLabel} credentials…`}
              </Button>
              {configured ? (
                <span className="status-pill status-ok">Stored in Keychain</span>
              ) : null}
            </div>
            <p className="fine-print">
              {environmentStatus?.message ?? "Add credentials to enable linking."} Find the client
              ID and secret in the Plaid dashboard under Developers → Keys. Only credit card
              accounts are imported. A linked institution is synced from Settings → Accounts → Bank
              connections.
            </p>

            {notice ? (
              <p className="inline-notice" role="status">
                {notice}
              </p>
            ) : null}

            <div className="button-row">
              <Button variant="primary" onClick={() => go("timezone")} disabled={busy}>
                {configured ? "Continue" : "Continue without a bank"}
              </Button>
              <Button variant="secondary" onClick={() => go("account")} disabled={busy}>
                Add a card manually
              </Button>
            </div>
            <p className="fine-print">
              You can also skip any of this now and do it later in Settings → Accounts.
            </p>
          </>
        ) : step === "account" ? (
          <>
            <h1 id="wizard-title">Add a card manually.</h1>
            <p className="muted">
              No bank connection is needed. Enter the card now, or skip and add it later in Settings
              → Accounts.
            </p>
            <AccountForm submitLabel="Add account" busy={busy} onSubmit={addAccount} />
            <div className="button-row">
              <Button variant="ghost" onClick={() => go("timezone")} disabled={busy}>
                Skip for now
              </Button>
              <Button variant="ghost" onClick={() => go("connect")} disabled={busy}>
                Back
              </Button>
            </div>
          </>
        ) : step === "timezone" ? (
          <>
            <h1 id="wizard-title">Set your reporting timezone.</h1>
            <p className="muted">
              Calendar months are evaluated in this timezone. We detected it from this Mac — change
              it if your statements are based somewhere else.
            </p>
            {accountName ? (
              <p className="inline-notice" role="status">
                Added {accountName}. You can add more cards in Settings → Accounts.
              </p>
            ) : null}

            <div className="wizard-field">
              <label className="field-label" htmlFor="onboarding-timezone">
                Reporting timezone
              </label>
              <TimezoneSelect
                id="onboarding-timezone"
                value={timezone}
                onChange={setTimezone}
                disabled={busy}
              />
            </div>
            <p className="fine-print">
              {detected.detected ? (
                <>
                  Detected from this Mac: <strong>{timeZoneSummary(detected.zone)}</strong>.{" "}
                  {timezone !== detected.zone ? "You selected a different zone below. " : ""}
                </>
              ) : (
                <>
                  We could not detect this Mac&apos;s timezone, so we selected{" "}
                  <strong>{timeZoneSummary(FALLBACK_TIMEZONE)}</strong> by default.{" "}
                </>
              )}
              You can change this later in Settings → Lock &amp; idle.
            </p>
            {timezonePreview ? (
              <p className="fine-print">It is currently {timezonePreview} in this timezone.</p>
            ) : null}
            {detected.detected && timezone !== detected.zone ? (
              <div>
                <Button variant="ghost" onClick={() => setTimezone(detected.zone)} disabled={busy}>
                  Use detected timezone
                </Button>
              </div>
            ) : null}

            <div className="button-row">
              <Button variant="primary" onClick={() => void saveTimezone()} disabled={busy}>
                {busy ? "Saving…" : "Continue"}
              </Button>
              <Button variant="ghost" onClick={() => go("account")} disabled={busy}>
                Back
              </Button>
            </div>
          </>
        ) : (
          <>
            <h1 id="wizard-title">You are ready.</h1>
            <p className="muted">
              Your encrypted ledger is unlocked. Everything from here is optional.
            </p>
            <dl className="facts">
              <div>
                <dt>Passphrase</dt>
                <dd>Never stored · native sheet only</dd>
              </div>
              <div>
                <dt>Bank connection</dt>
                <dd>{bankSummary}</dd>
              </div>
              <div>
                <dt>Cards</dt>
                <dd>{accountName ?? "None yet"}</dd>
              </div>
              <div>
                <dt>Timezone</dt>
                <dd>{timeZoneSummary(timezone)}</dd>
              </div>
            </dl>

            <div className="wizard-callout">
              <strong>Where to finish setting up</strong>
              <ul className="wizard-next-list">
                <li>
                  <strong>Cards, payment details, and CSV import</strong> — Settings → Accounts
                </li>
                <li>
                  <strong>Bank connections and sync</strong> — Settings → Accounts → Bank
                  connections
                </li>
                <li>
                  <strong>Timezone and idle lock</strong> — Settings → Lock &amp; idle
                </li>
              </ul>
            </div>

            {configuredEnvironments.length > 0 ? (
              <p className="fine-print">
                Credentials are stored, but no bank is linked yet. Link one in Settings → Accounts →
                Bank connections.
              </p>
            ) : null}

            <div className="button-row">
              <Button variant="primary" onClick={() => void finish()} disabled={busy}>
                {busy ? "Finishing…" : "Finish setup"}
              </Button>
              <Button variant="ghost" onClick={() => go("timezone")} disabled={busy}>
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
          Your passphrase is already set. There is no recovery if it is lost.
        </p>
      </div>
    </main>
  );
}
