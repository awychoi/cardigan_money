import { useEffect, useState } from "react";
import { command, type MerchantRule } from "../../lib/bridge";
import { friendlyError } from "../../lib/bridge/errors";
import { Button } from "../../components/Button";

const EMPTY = {
  matchType: "contains" as "exact" | "contains" | "starts_with",
  matchText: "",
  canonicalMerchant: "",
  category: "",
  priority: 100,
  enabled: true,
};

export function MerchantRulesPanel({ onChanged }: { onChanged: () => void }) {
  const [rules, setRules] = useState<MerchantRule[]>([]);
  const [form, setForm] = useState({ ...EMPTY });
  const [editingId, setEditingId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  async function load() {
    try {
      const result = await command("merchant_rules_list", {});
      setRules(result.rules);
    } catch (caught) {
      setError(friendlyError(caught));
    }
  }

  useEffect(() => {
    void load();
  }, []);

  async function submit() {
    setBusy(true);
    setError(null);
    setMessage(null);
    const payload = {
      matchType: form.matchType,
      matchText: form.matchText,
      canonicalMerchant: form.canonicalMerchant,
      category: form.category.trim() ? form.category.trim() : null,
      priority: form.priority,
      enabled: form.enabled,
    };
    try {
      if (editingId) {
        await command("merchant_rule_update", { id: editingId, rule: payload });
        setMessage("Rule updated and reapplied.");
      } else {
        await command("merchant_rule_create", payload);
        setMessage("Rule added and reapplied.");
      }
      setForm({ ...EMPTY });
      setEditingId(null);
      await load();
      onChanged();
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  async function remove(id: string) {
    setBusy(true);
    setError(null);
    try {
      await command("merchant_rule_delete", { id });
      setMessage("Rule deleted; display fields were recomputed.");
      await load();
      onChanged();
    } catch (caught) {
      setError(friendlyError(caught));
    } finally {
      setBusy(false);
    }
  }

  function edit(rule: MerchantRule) {
    setEditingId(rule.id);
    setForm({
      matchType: rule.matchType as typeof EMPTY.matchType,
      matchText: rule.matchText,
      canonicalMerchant: rule.canonicalMerchant,
      category: rule.category ?? "",
      priority: rule.priority,
      enabled: rule.enabled,
    });
  }

  return (
    <section className="panel panel-wide" aria-labelledby="rules-title">
      <h2 id="rules-title">Merchant rules</h2>
      <p className="fine-print">
        Rules merge display labels and optionally assign a category. They never merge transaction
        records, and a user override still wins.
      </p>
      {message ? (
        <p className="inline-notice" role="status">
          {message}
        </p>
      ) : null}
      {error ? (
        <p className="inline-error" role="alert">
          {error}
        </p>
      ) : null}

      <div className="rule-form">
        <label>
          <span className="field-label">Match</span>
          <select
            value={form.matchType}
            onChange={(event) => setForm({ ...form, matchType: event.target.value as typeof form.matchType })}
          >
            <option value="contains">Contains</option>
            <option value="exact">Exact</option>
            <option value="starts_with">Starts with</option>
          </select>
        </label>
        <label>
          <span className="field-label">Match text</span>
          <input
            value={form.matchText}
            onChange={(event) => setForm({ ...form, matchText: event.target.value })}
            maxLength={120}
            placeholder="amzn"
          />
        </label>
        <label>
          <span className="field-label">Canonical merchant</span>
          <input
            value={form.canonicalMerchant}
            onChange={(event) => setForm({ ...form, canonicalMerchant: event.target.value })}
            maxLength={120}
            placeholder="Amazon"
          />
        </label>
        <label>
          <span className="field-label">Category (optional)</span>
          <input
            value={form.category}
            onChange={(event) => setForm({ ...form, category: event.target.value })}
            maxLength={80}
          />
        </label>
        <label>
          <span className="field-label">Priority</span>
          <input
            type="number"
            min={0}
            max={10000}
            value={form.priority}
            onChange={(event) => setForm({ ...form, priority: Number(event.target.value) })}
          />
        </label>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={form.enabled}
            onChange={(event) => setForm({ ...form, enabled: event.target.checked })}
          />
          Enabled
        </label>
        <div className="button-row">
          <Button
            variant="primary"
            onClick={submit}
            disabled={busy || !form.matchText.trim() || !form.canonicalMerchant.trim()}
          >
            {editingId ? "Save rule" : "Add rule"}
          </Button>
          {editingId ? (
            <Button
              variant="ghost"
              onClick={() => {
                setEditingId(null);
                setForm({ ...EMPTY });
              }}
            >
              Cancel
            </Button>
          ) : null}
        </div>
      </div>

      {rules.length === 0 ? (
        <p className="muted">No merchant rules yet.</p>
      ) : (
        <table className="accounts-table">
          <caption className="sr-only">Merchant rules</caption>
          <thead>
            <tr>
              <th scope="col">Match</th>
              <th scope="col">Canonical</th>
              <th scope="col">Category</th>
              <th scope="col">Priority</th>
              <th scope="col">State</th>
              <th scope="col">Actions</th>
            </tr>
          </thead>
          <tbody>
            {rules.map((rule) => (
              <tr key={rule.id}>
                <td>
                  {rule.matchType.replaceAll("_", " ")} “{rule.matchText}”
                </td>
                <td>{rule.canonicalMerchant}</td>
                <td>{rule.category ?? "—"}</td>
                <td>{rule.priority}</td>
                <td>
                  <span className={rule.enabled ? "status-pill status-ok" : "status-pill status-info"}>
                    {rule.enabled ? "enabled" : "disabled"}
                  </span>
                </td>
                <td>
                  <span className="button-row">
                    <Button variant="ghost" className="btn-small" onClick={() => edit(rule)}>
                      Edit
                    </Button>
                    <Button
                      variant="ghost"
                      className="btn-small"
                      disabled={busy}
                      onClick={() => void remove(rule.id)}
                    >
                      Delete
                    </Button>
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
