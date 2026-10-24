import { useState, type FormEvent } from "react";
import type { Account, AccountPayload } from "../../lib/bridge";
import { zAccountPayload } from "../../lib/bridge/contract";
import { Button } from "../../components/Button";

interface Props {
  initial?: Account | null;
  submitLabel: string;
  busy?: boolean;
  onSubmit: (payload: AccountPayload) => Promise<void>;
  onCancel?: () => void;
}

type Errors = Partial<Record<keyof AccountPayload, string>>;

export function AccountForm({ initial, submitLabel, busy, onSubmit, onCancel }: Props) {
  const [issuer, setIssuer] = useState(initial?.issuer ?? "");
  const [displayName, setDisplayName] = useState(initial?.displayName ?? "");
  const [lastFour, setLastFour] = useState(initial?.lastFour ?? "");
  const [accountType, setAccountType] = useState<AccountPayload["accountType"]>(
    (initial?.accountType as AccountPayload["accountType"] | undefined) ?? "credit_card",
  );
  const [creditLimit, setCreditLimit] = useState(
    initial?.creditLimitMinor != null ? (initial.creditLimitMinor / 100).toFixed(2) : "",
  );
  const [includeInOverview, setIncludeInOverview] = useState(initial?.includeInOverview ?? true);
  const [errors, setErrors] = useState<Errors>({});

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    const candidate = {
      issuer,
      displayName,
      lastFour: lastFour.trim() === "" ? null : lastFour.trim(),
      accountType,
      currency: "USD",
      creditLimit: creditLimit.trim() === "" ? null : creditLimit.trim(),
      includeInOverview,
    };
    const parsed = zAccountPayload.safeParse(candidate);
    if (!parsed.success) {
      const next: Errors = {};
      for (const issue of parsed.error.issues) {
        const key = issue.path[0];
        if (typeof key === "string" && !(key in next)) next[key as keyof AccountPayload] = issue.message;
      }
      setErrors(next);
      return;
    }
    setErrors({});
    await onSubmit(parsed.data);
  }

  return (
    <form className="account-form" onSubmit={handleSubmit} noValidate>
      <div className="form-grid">
        <label>
          <span className="field-label">Issuer</span>
          <input
            value={issuer}
            onChange={(event) => setIssuer(event.target.value)}
            maxLength={80}
            placeholder="Chase"
            aria-invalid={Boolean(errors.issuer)}
          />
          {errors.issuer ? <span className="field-error">{errors.issuer}</span> : null}
        </label>
        <label>
          <span className="field-label">Display name</span>
          <input
            value={displayName}
            onChange={(event) => setDisplayName(event.target.value)}
            maxLength={120}
            placeholder="Sapphire Reserve"
            aria-invalid={Boolean(errors.displayName)}
          />
          {errors.displayName ? <span className="field-error">{errors.displayName}</span> : null}
        </label>
        <label>
          <span className="field-label">Last four digits (optional)</span>
          <input
            value={lastFour}
            onChange={(event) => setLastFour(event.target.value)}
            inputMode="numeric"
            maxLength={4}
            placeholder="1234"
            aria-invalid={Boolean(errors.lastFour)}
          />
          {errors.lastFour ? <span className="field-error">Enter exactly four digits.</span> : null}
        </label>
        <label>
          <span className="field-label">Account type</span>
          <select
            value={accountType}
            onChange={(event) => setAccountType(event.target.value as AccountPayload["accountType"])}
          >
            <option value="credit_card">Credit card</option>
            <option value="charge_card">Charge card</option>
            <option value="line_of_credit">Line of credit</option>
            <option value="other">Other</option>
          </select>
        </label>
        <label>
          <span className="field-label">Currency</span>
          <input value="USD" readOnly aria-readonly="true" />
        </label>
        <label>
          <span className="field-label">Credit limit (optional)</span>
          <input
            value={creditLimit}
            onChange={(event) => setCreditLimit(event.target.value)}
            placeholder="12,500.00"
            inputMode="decimal"
            aria-invalid={Boolean(errors.creditLimit)}
          />
          {errors.creditLimit ? <span className="field-error">{errors.creditLimit}</span> : null}
        </label>
      </div>
      <label className="checkbox-row">
        <input
          type="checkbox"
          checked={includeInOverview}
          onChange={(event) => setIncludeInOverview(event.target.checked)}
        />
        Include in overview totals
      </label>
      <div className="button-row">
        <Button variant="primary" type="submit" disabled={busy}>
          {busy ? "Saving…" : submitLabel}
        </Button>
        {onCancel ? (
          <Button variant="ghost" type="button" onClick={onCancel} disabled={busy}>
            Cancel
          </Button>
        ) : null}
      </div>
    </form>
  );
}
