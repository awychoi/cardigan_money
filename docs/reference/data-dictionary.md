# Data dictionary (core tables)

All timestamps are UTC RFC 3339 with `Z`. Financial dates are date-only `YYYY-MM-DD`; never timezone-converted. Money is integer minor units + ISO-4217 currency. `NULL` means missing, never zero.

| Table | Purpose / key fields |
| --- | --- |
| `accounts` | Local card metadata: issuer, display name, last four (optional), type, currency, optional credit limit, include flag, archived, `source_kind` (manual/provider/demo). |
| `account_sources` | Provider mapping: account ↔ connection + provider account id/mask; `unlinked_at` preserves mapping history. Unique (connection, provider account). |
| `connections` | Provider connection metadata only: provider/environment, institution, opaque Keychain `credential_ref`, status, cursor, sanitized error, timestamps. No token. |
| `transactions` | Source facts plus effective display/category/type fields. Identity `(account_id, provider_transaction_id)` unique when provider id present. `pending`, `removed`, dates/source, amount/currency, raw description, normalized merchant, display merchant, category, type, excluded, note/override columns, pending link, provenance, import batch/fingerprint. |
| `transaction_overrides` | One row per edited transaction: alias, category, type correction, excluded, notes, timestamps. Source fields remain untouched. |
| `merchant_rules` | Match type/text, canonical merchant, optional category, priority, enabled. Derived display columns on transactions are recomputed; rules never merge rows. |
| `import_batches` / `import_rows` | File hash, account, counts, status, timestamps; per-row source index/status/reason/fingerprint. Reimport of the same hash is a no-op. |
| `balance_snapshots` | Append-only provider/manual balance observations: current/available/limit, currency, retrieved/as-of, provenance, connection. |
| `payment_snapshots` | Provider/CSV payment metadata snapshots with due date, minimum due, statement balance, statement/closing dates, provenance. Missing amounts stay NULL. |
| `manual_payment_metadata` | Owner-entered dated facts (statement balance, minimum due, next due, valid-through) independent of provider snapshots. |
| `settings` | Close behavior, appearance, sync on launch/windows, currency, idle lock, backup status. No secrets. |
| `security_events` | Typed, sanitized audit events. No passphrases, tokens, URLs, account numbers, or provider payloads. |
| `sync_runs` / `sync_steps` | Trigger, status, timestamps, sanitized error; per-stream added/modified/removed counts. |
| `vault_metadata` | Vault id, generation, schema version, cipher provider/version, created/integrity timestamps. Ciphertext at rest. |

Derived rules: reporting month uses `reporting_date`; month range is `[first day, first day of next month)`. Purchases/refunds/fees/interest are spending; payments/transfers are not. Pending rows are excluded from settled totals. Statement balance is not the amount due. Manual metadata is shown separately with expiry.
