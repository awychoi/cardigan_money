# Implementation plan: cardigan_money

Plan status: ready to execute. Derived from [personal-cards-build-spec.md](personal-cards-build-spec.md), revised 2026-10-23, and the security review in [security-review.md](security-review.md). The specification is the source of truth; this plan is the build sequence and verification checklist. Security is a phase gate, not a final cleanup.

Effort is expressed as sequence and phase gates, not calendar dates. Plaid approval, Trial eligibility, and the owner's availability are external variables.

## 1. How this plan is used

- Build in vertical slices. Every phase leaves the app runnable and tested at its current scope.
- Do not start a phase until the previous phase's exit gate is met, except for explicitly parallel work called out below.
- Do not mark a phase complete while a relevant Critical/High security finding is open without a written owner acceptance.
- Update `docs/implementation-status.md` at the end of every phase with: completed work, tests actually run, measured results, security findings and their status, blockers, and live institutions actually verified.
- Keep the app usable with demo data and CSV even if Plaid is blocked. A live-provider dependency must never stop the offline application from shipping.
- Ask the owner only when a consequential decision is missing. Record the answer in the spec's decision log or in `docs/implementation-status.md`; do not silently change scope.
- Never claim a live connection, build, test, security property, or performance target that was not actually verified. Never call the app "watertight" or "unhackable"; report residual risk.

### Phase progress tracker

| Phase | Outcome | Status |
| --- | --- | --- |
| 0 | Foundation, threat model, crypto design, SQLCipher and Plaid feasibility | **Complete** |
| 1 | Tauri shell, command gateway, renderer lockdown | **Complete** |
| 2 | Vault envelope, native secret input, lock lifecycle, migrations | **Complete** (owner passphrase-entry smoke pending) |
| 3 | Domain rules, input bounds, calendar-month accounting | **Complete** |
| 4 | Onboarding, accounts, settings, theme, security settings | **Complete** (owner native close/passphrase smoke pending) |
| 5 | Demo data and read-only dashboard | **Complete** |
| 6 | Edits, manual metadata, CSV import/export security | **Complete** |
| 7 | Encrypted backup, restore, rotation, recovery | Not started |
| 8 | Sync engine, scheduler, mock provider | Not started |
| 9 | Plaid adapter and verified linking (conditional) | Not started |
| 10 | Independent security review, performance, packaging, handoff | Not started |

Every phase's exit gate includes the security work relevant to its features. The Phase 0, 1, and 2 gates are especially strict: no later phase starts on a foundation that can fail open or expose the passphrase.

## 2. Working rules and non-negotiables

1. **Offline first.** The app must launch, unlock, show data, and import/export without any network access.
2. **One source of financial truth.** Calendar-month totals, supplier totals, and category totals come from tested Rust functions. The frontend formats and displays; it does not calculate financial rules.
3. **No plaintext ledger, fail closed.** SQLCipher with envelope key management (Argon2id KEK, random DEK, authenticated external `vault.header`), an in-memory key, and no plaintext temporary copies. If SQLCipher is not actually active, integrity fails, or KDF parameters are invalid, refuse to open or create the ledger. No plaintext fallback, no silent reset.
4. **No secret exposure.** The passphrase, DEK, KEK, Plaid client secret, and provider tokens never enter the renderer, logs, URLs, localStorage, or committed files. Passphrases and the client secret are collected by a native secure-input sheet. Plaid enrollment is the owner's action.
5. **Missing is not zero.** Unknown, unsupported, stale, and explicitly zero remain distinct in the schema and UI.
6. **No invented coverage.** Actual institution and card support is recorded only after verification. Demo labels stay demo labels.
7. **Small, tested increments.** Prefer a working narrow path over a broad unfinished one.
8. **Preserve the audit trail.** User overrides and historical transactions survive syncs, imports, and corrections.
9. **No speculative features.** No notifications, login item, launch-at-login, auto-update, budgets, or AI categorization in version one. Lock lifecycle and security settings are not speculative; they are required security behavior.
10. **Documentation travels with code.** Commands, versions, migrations, and limitations are documented in the phase where they are introduced.
11. **Renderer is untrusted.** One `main` capability, no fs/shell/http/opener/sql/store/process/updater/deep-link/dialog plugins exposed to the renderer, strict CSP with no HTTPS source, blocked navigation/new windows, devtools off in release.
12. **IPC is a gateway.** Typed, allowlisted, bounded commands with window/origin checks. No generic SQL/file/URL/shell command. Native dialogs and file I/O stay in Rust.
13. **Locking is required.** Manual Lock Now; lock on screen lock, sleep, fast user switching, logout, quit, and idle timeout (default 15 minutes). Lock closes connections, zeroizes keys, and clears renderer caches. Hiding is not a lock.
14. **Backups are self-contained.** Encrypted snapshot + authenticated header carrying the wrapped DEK. Automatic backups need no prompt; restore always asks. Passphrase change rotates the DEK; old backups require the old passphrase.
15. **No network in the renderer.** All network is Rust with TLS, HTTPS-only, no redirects, fixed host allowlist, and response caps.
16. **Supply chain and build integrity.** Lockfiles/frozen installs, `cargo audit`/`cargo deny`/`pnpm audit`, official plugins only, no unapproved postinstall scripts, ad-hoc signed artifact, recorded SHA-256.
17. **No security theater.** Every claim is tested or marked unverified. Residual risks are documented; "watertight" is not a claim.

## 3. Cross-cutting test strategy

| Layer | Tool | What it proves |
| --- | --- | --- |
| Domain unit tests | `cargo test` | Money arithmetic, dates, month boundaries, pending/posted, refunds, exclusions, reconciliation |
| Storage integration tests | `cargo test` with temporary encrypted DBs | Migrations, transactions, constraints, overrides, import batches, backup/restore |
| Vault tests | `cargo test` | Fail-closed SQLCipher, wrong passphrase, header tamper, KDF bounds, rekey/rotation, plaintext checks, encrypted backup behavior |
| Sync tests | `cargo test` with a mock provider | Cursors, pagination, partial failures, idempotency, locks, no destructive clears, no token leakage |
| Frontend unit/component tests | Vitest + React Testing Library | Formatting, filters, empty states, wizard behavior, no HTML injection, accessibility basics |
| Browser UI tests | Playwright against the mock bridge | Primary workflows plus CSP/network assertions and blocked navigation |
| Native smoke tests | Manual on the owner's M1 Max | Startup, native secure input, file dialogs, Keychain, content protection, screen-lock lock, state restoration, IPC |
| Performance tests | Synthetic 50,000-transaction encrypted dataset | Warm search/month change under 200 ms; cached startup within roughly two seconds after unlock |
| Security checks | Manual review plus tests | Fail-closed cipher, CSP, capabilities, IPC validation, secret redaction, permissions, no plaintext artifacts |
| Fuzz/property tests | `cargo-fuzz`/`proptest` plus frontend fuzzing | CSV, provider JSON, and IPC parsers fail safely on malformed input |
| Dependency/build audit | `cargo audit`, `cargo deny`, `pnpm audit`, lockfile verification | No known vulnerable/unapproved dependencies; reproducible install; artifact hash |

Native automation is not assumed. Browser tests through the mock bridge never count as proof of Tauri functionality or of native security behavior.

### Security invariants tested throughout

- A plain-SQLite build is rejected; the app never creates or opens a plaintext ledger.
- The passphrase never exists in the renderer, URL, localStorage, logs, or error messages.
- IPC has no generic SQL/file/URL/shell command and rejects unknown, oversized, or malformed input.
- The release CSP has no HTTPS source; the renderer makes no external requests; navigation/new windows are blocked.
- No access token, client secret, DEK, or KEK appears in a log, backup, export, or renderer state.
- Locking closes connections, zeroizes keys, clears renderer caches, and stops sync without advancing the cursor.
- Backups are authenticated ciphertext; tamper/truncation/wrong-passphrase/wrong-vault-ID restores fail safely.
- Restore is atomic and leaves the original DB intact on failure.
- File permissions are 0700/0600; symlinks are rejected; temp files never contain plaintext ledger data.
- Demo mode cannot open the real ledger or import real data.

### Data-integrity invariants tested throughout

- Pending and posted forms of the same purchase never both contribute to spending.
- Payments, transfers, and excluded transactions never contribute to spending.
- Refunds contribute in the month received, not the original purchase month.
- Supplier and category totals equal the eligible ledger rows exactly.
- A later source correction changes historical totals only through an explainable transaction.
- Historical month selection never changes current balances or current obligations.
- Manual payment metadata survives provider syncs and is visually distinguished from provider data.
- Import rollback removes only that batch's unique contributions.
- Repeating a sync or an identical CSV import creates no duplicates.
- Current balance is never derived by summing imported transactions.
- A statement balance is never labeled as the remaining amount due.

## 4. Phase 0 — Foundation and feasibility

**Goal:** de-risk the toolchain and external provider assumptions before product code depends on them.

### Build steps

1. Confirm repository state. Initialize Git if absent, add a `.gitignore` for `node_modules`, Rust `target`, local databases, backups, `.env`, and build artifacts. Do not commit secrets or financial data.
2. Record the toolchain: macOS version, Apple Silicon target, Xcode command-line tools, Rust stable, Node LTS, pnpm, and Tauri CLI. Pin versions and commit lockfiles when the scaffold exists.
3. Scaffold a minimal Tauri 2 + React + TypeScript + Vite app. Confirm `pnpm tauri dev` launches a native window on the M1 Max.
4. Threat model and crypto design before coding the vault:
   - Write `docs/threat-model.md` (assets, adversaries, trust boundaries, assumptions, out-of-scope) using spec §13.1.
   - Write `docs/security-architecture.md`: Argon2id parameters/bounds, KEK/DEK wrapping, `vault.header` format and AAD, SQLCipher raw-key opening, generation/rotation journal, backup container, test vectors.
   - Verify SQLCipher raw-key opening **and raw-key rekey** support (`PRAGMA rekey` vs `sqlite3_rekey`); record the exact API and a test vector. If raw-key rekey is unsupported, stop and redesign rather than falling back to a passphrase key.
   - Decide and record the library versions (`argon2`, `chacha20poly1305`, `zeroize`, `secrecy`, `getrandom`, `subtle`).
5. SQLCipher fail-closed spike:
   - Compile rusqlite with a maintained SQLCipher integration (record exact feature flags, provider, and version).
   - Create an encrypted DB with a raw random DEK; close/reopen; reject the wrong key; verify `cipher_version`/`cipher_provider` and `cipher_status`.
   - Build a test-only plain-SQLite binary and prove the app rejects it.
   - Prove the main DB, WAL, and SHM are not readable as plaintext SQLite; run `cipher_integrity_check`.
   - Test wrong-key behavior for non-destructiveness and document any sidecar files.
6. Establish the Tauri security baseline:
   - Explicit CSP with no HTTPS sources; one `main` capability; no fs/shell/http/opener/sql/store/process/updater/deep-link/dialog permissions exposed to the renderer.
   - `freezePrototype: true`, `withGlobalTauri: false`, asset protocol disabled, devtools off in release.
   - Navigation/new-window handlers restricted to the app origin; content protection hook; single-instance plugin/lock.
7. Write `docs/plaid-feasibility.md` from current official documentation:
   - Trial eligibility, Item limits, product access, and pricing caveats.
   - Transactions, balances, and Liabilities availability.
   - The actual desktop linking flow: HTTPS redirect rules, no custom URI schemes, localhost only in Sandbox, deprecated webview SDK. Evaluate Hosted Link + `/link/token/get` polling and a hosted HTTPS redirect page; document what is hosted and what data crosses it.
   - Redirect/OAuth/state requirements, reconnect behavior, and the short-lived token handoff to Rust.
   - Explicit verified vs unverified statements and concrete blockers. Do not write a flow that depends on `cardigan_money://` or a Production loopback callback.
8. Send the owner a short, concrete request for real CSV exports when available, plus the Plaid Trial signup checklist. Do not create the Plaid account.
9. Create the remaining documentation skeletons: `docs/implementation-status.md`, `docs/decisions.md`, `docs/test-plan.md`, and `SECURITY.md`; the threat model and security architecture are produced in step 4.
10. Establish build/supply-chain hygiene: lockfiles, `.gitignore` for DB/backups/`.env`, `cargo audit`/`cargo deny`/`pnpm audit`, no unapproved postinstall scripts, and a stable local signing identity decision (ad-hoc vs self-signed).

### Deliverables

- Runnable Tauri scaffold on the M1 Max.
- Version pins and lockfiles.
- Threat model, security architecture, and test vectors.
- Fail-closed SQLCipher spike with plain-SQLite rejection and plaintext-artifact tests.
- Plaid feasibility document with a verified desktop flow or a concrete blocker.
- Baseline CSP/capabilities/navigation/devtools configuration.
- Dependency audit output and signing-identity decision.
- Repository hygiene and documentation skeletons, including `SECURITY.md`.

### Verification

- App window launches from a clean install.
- `cargo test` runs the fail-closed SQLCipher spike and the plain-SQLite rejection test.
- Wrong-key behavior is confirmed, not assumed; DB/WAL/SHM are ciphertext.
- The release CSP contains no HTTPS source; capability files contain only the `main` window and no dangerous plugins.
- No secrets, database files, backups, or build artifacts are staged in Git.
- Plaid claims in the feasibility document are traceable to current official docs, including the redirect-scheme constraints.
- `cargo audit`/`cargo deny`/`pnpm audit` results are recorded.

### Exit gate

The scaffold runs natively, the encrypted-database path is proven fail-closed, the renderer baseline is locked down, and Plaid's linking/product situation is documented with a verified flow or a concrete blocker. If SQLCipher cannot be made to work cleanly, stop and report the concrete build failure before designing around it. If Plaid's only viable flow requires infrastructure the owner has not approved, record that and plan live linking as unavailable rather than improvising a callback.

---

## 5. Phase 1 — App shell and typed bridge

**Goal:** a native shell with a safe, typed boundary and a testable frontend foundation.

### Build steps

1. Configure the app identity exactly: display name `cardigan_money`, bundle identifier `com.awychoi.cardiganmoney`, window title `cardigan_money`, Apple Silicon target.
2. Define the shared command contract in Rust: request/response types, structured errors, money/date types, and a versioned envelope. Generate TypeScript bindings if a maintained solution is practical; otherwise mirror them with Zod validation and contract tests. Every request uses `deny_unknown_fields` and explicit bounds.
3. Implement the command gateway:
   - One `main` window only; every command checks the window label/origin.
   - Minimal commands: app info, health check, settings read/write stubs, and a no-op sync-state query. No generic SQL/file/URL/shell command.
   - Reject oversized, unknown, wrong-type, out-of-range, and path/URL-bearing arguments with sanitized error codes.
   - Add rate limits for sensitive commands (unlock, export, restore) and stop events on lock.
4. Implement the Tauri baseline from Phase 0 in the real app: CSP with no HTTPS source, one capability, no dangerous plugins, `freezePrototype`, `withGlobalTauri: false`, asset protocol off, devtools off in release, navigation/new-window blocking, drag-and-drop disabled.
5. Build a mock desktop bridge for browser and Playwright use. It must implement the same contract as Rust and fail loudly on unknown commands. Put it behind a compile-time test/dev feature so it cannot ship in a release build.
6. Build the UI shell: header, month navigation placeholder, sync-status area, theme tokens for light/dark/system, settings entry point, and an empty overview.
7. Add error boundaries, loading states, and a redacting logging helper. No financial payloads or secrets in logs. Add the secret-sentinel test harness now.
8. Set up Vitest, React Testing Library, Playwright, formatting, linting, IPC fuzz/contract tests, and the test scripts that later phases will extend.

### Deliverables

- Native shell matching the app identity.
- Typed command contract with a mock implementation compiled out of release.
- Command gateway with validation, window checks, rate limits, and sanitized errors.
- Tauri CSP/capability/navigation/devtools baseline applied.
- Theme-capable UI foundation.
- Redacting logger and secret-sentinel test.
- Test harness and baseline scripts.
- Provisional README commands, to be finalized in Phase 10.

### Verification

- A typed command round-trip succeeds through Rust and through the mock bridge.
- Invalid/unknown/oversized IPC input is rejected in Rust; no generic command exists.
- The release CSP has no HTTPS source; Playwright observes zero external requests.
- Navigation to a remote origin and `window.open` are blocked; devtools are absent in release.
- Theme switching works in the browser mock and the native shell.
- Navigating the UI makes no network request.
- Frontend, contract, and IPC fuzz tests pass.

### Exit gate

The app opens to a stable shell, the frontend can be developed and tested without Rust, and the IPC boundary is allowlisted, typed, bounded, and validated. The renderer is locked down before any real financial data exists. No screen yet claims real data.

---

## 6. Phase 2 — Encrypted vault and migrations

**Goal:** a persistent SQLCipher ledger with a human passphrase and a safe migration path.

### Build steps

1. Implement the native secure-input sheet (`NSSecureTextField` via `objc2-app-kit`) for passphrase creation, unlock, passphrase change, restore, export, and the Plaid client secret. The renderer receives only success/failure/cancelled.
2. Implement the vault envelope:
   - Generate a random 256-bit DEK; derive the KEK with Argon2id; wrap the DEK with XChaCha20-Poly1305; write an authenticated `vault.header` (0600, atomic, redundant) with KDF params/salt/wrapped DEK/vault ID/generation.
   - Open SQLCipher with the raw DEK. Verify `cipher_version`/`cipher_provider`, `cipher_status`, and `cipher_integrity_check`; fail closed on any mismatch.
   - Enforce 16 characters or ~60 bits estimated entropy for new passphrases; spaces and long passphrases allowed; no composition rules; clear no-recovery warning.
   - Wrong passphrase returns a non-destructive error and never renames, resets, or overwrites the database. Do not fall back to a passphrase-derived SQLCipher key.
3. Keep the DEK/KEK in Rust memory only. `mlock` where practical, zeroize on drop/lock, and never write to Keychain, preferences, logs, or temporary files. Disable core dumps.
4. Implement the lock lifecycle: manual Lock Now; lock on screen lock, sleep, fast user switching, logout, and quit; idle auto-lock default 15 minutes. On lock, close connections, zeroize, clear renderer caches, and stop sync safely.
5. Add a migration runner with a schema-version table, forward-only migrations, and a pre-migration encrypted snapshot primitive. A failed migration must leave the original database intact.
6. Configure SQLite `foreign_keys`, `journal_mode=WAL`, `synchronous=FULL`, `busy_timeout`, `secure_delete=ON`, `temp_store=MEMORY`, `trusted_schema=OFF`, defensive mode, `recursive_triggers=OFF`, and `cipher_memory_security=ON`. Dispatch blocking database work off the UI thread. Prove WAL/SHM are ciphertext.
7. Create the initial `settings`, `vault_metadata`, and `security_events` tables. Store theme, close behavior, onboarding completion, timezone, sync windows, auto-lock, and backup metadata there as those features land. KDF parameters/wrapped DEK stay in `vault.header`.
8. Enforce 0700 directories and 0600 files, reject symlinks, write atomically, and enforce single-instance before any DB open.
9. Add tests: create/reopen, native-input path, wrong passphrase non-destructiveness, ciphertext check with a plain SQLite client, plain-SQLite build rejection, header tamper/truncation, KDF bounds, migration success/failure, permission checks, lock/zeroize behavior, and a scan for plaintext temp artifacts.

### Deliverables

- Native passphrase creation/unlock/change flows; no HTML password field.
- Envelope vault with external authenticated header and fail-closed SQLCipher.
- Lock lifecycle and single-instance enforcement.
- SQLCipher-backed database with a migration runner.
- Pre-migration snapshot primitive.
- Settings, vault metadata, and security event schema.
- Vault, lock, and migration tests.

### Verification

- The database cannot be opened without the passphrase; a plain-SQLite build is rejected.
- A wrong passphrase leaves the data intact; no create/reset/overwrite occurs.
- Quitting and relaunching requires the passphrase and reopens the same data; locking zeroizes the session.
- Screen lock/sleep/manual lock close connections and clear renderer caches.
- A failed migration preserves the pre-migration database.
- No plaintext database, WAL, SHM, or decrypted copy exists in the app data directory.
- `vault.header` tampering/truncation and absurd KDF parameters fail safely.

### Exit gate

A user can create a vault through the native secure sheet, quit, relaunch, unlock, and reopen the same encrypted ledger. Fail-closed cipher, header integrity, non-destructive wrong-passphrase, lock lifecycle, and migration tests pass on the owner's Mac. No later phase starts if the vault can fail open or the passphrase has entered the renderer.

---

## 7. Phase 3 — Domain rules and calendar-month accounting

**Goal:** the financial core that every screen will trust.

### Build steps

1. Add the full core schema: accounts, account sources, balance snapshots, payment snapshots, manual payment metadata, transactions, transaction overrides, merchant rules, import batches/rows, sync runs/steps, and settings.
2. Implement money as signed integer cents with a decimal parser at provider/import boundaries. Reject binary-float parsing.
3. Implement date-only types and the reporting-date rule: authorization/purchase date when available, otherwise posted date. Preserve the chosen date and its provenance.
4. Implement `[first day of month, first day of next month)` selection in the configured timezone, defaulting to America/New_York.
5. Implement spend eligibility: posted purchases plus fees/interest minus posted refunds; exclude payments, transfers, and explicitly excluded rows; keep pending separate.
6. Implement the summary service: gross purchases, refunds, fees/interest, net spending, pending subtotal, supplier totals, category totals, and exact reconciliation to ledger rows.
7. Keep current balances and upcoming obligations on their own current-data path so month changes cannot alter them.
8. Build deterministic domain fixtures for boundaries, leap day, DST, pending/posted pairs, refunds, fees, transfers, exclusions, and ambiguous same-amount purchases.
9. Add all financial tests from the spec, including that a statement balance is never presented as confirmed remaining due.
10. Harden the repository/query layer: prepared statements only; sort/filter identifiers from closed enums; no string interpolation of values or identifiers; `LIKE` escaping; pagination caps; date-range caps; UUID/amount/string bounds. Test SQL-injection attempts through search, filters, sort, import mapping, and merchant aliases.
11. Add boundary validation for all provider/CSV/domain text: UTF-8, length caps, charset rules, control/bidi stripping for display, and rejection of NUL/binary content. Add property tests for the money/date parsers.

### Deliverables

- Full schema and repository interfaces.
- Money/date/domain types.
- Calendar-month and summary services.
- Domain fixture set.
- Parameterized query layer with allowlisted identifiers.
- Financial and input-validation test suite.

### Verification

- All financial/domain tests in spec §16 pass.
- Supplier and category totals reconcile exactly to eligible rows.
- A historical month change does not change current balances/obligations.
- Unknown metadata never becomes zero or "no payment due."
- DST, leap day, and year boundaries produce no date shifts.
- SQL-injection and malformed-input tests fail safely; no dynamic identifier interpolation.

### Exit gate

The domain layer is the only source of totals, its queries are parameterized, and its tests pass. No frontend total is required or accepted.

---

## 8. Phase 4 — Onboarding, accounts, settings, theme

**Goal:** a usable empty app that the owner can set up without any provider.

### Build steps

1. Build the first-launch wizard: passphrase creation (from Phase 2), optional first account, optional provider connection, finish. Every non-passphrase step is skippable and resumable.
2. Implement manual account creation/editing: issuer, display name, last four, account type, USD currency, optional credit limit, inclusion, and archive. Store `source_kind = manual`.
3. Add the empty overview and empty-state actions. The app remains useful with zero accounts.
4. Add settings: theme (light/dark/system), window-close behavior, timezone, sync windows, auto-lock timeout, demo mode entry point, quit, and a Security section (cipher/KDF status, last integrity check, last backup, recent security events, provider credential removal, delete-all-data, passphrase change). Persist in the encrypted database; never store secrets.
5. Implement the first-close dialog: hide versus quit, with "remember this choice," and a settings override. Explain that hiding keeps the process/scheduler alive but is not a lock, and that idle/screen-lock auto-lock still applies.
6. Add account list/management UI, the settings view, accessible form primitives, and native confirmations for destructive actions. Passphrase change uses the native secure sheet and the Phase 2 rekey/journal path.
7. Tests: wizard completion and skipping, account creation, restart/unlock, theme persistence, close-behavior persistence, auto-lock settings, security status, credential removal, delete-all confirmation, and empty states.

### Deliverables

- First-launch wizard.
- Manual account management.
- Settings, lifecycle preferences, and security settings.
- Native confirmations for destructive actions.
- Empty states.
- Onboarding and settings tests.

### Verification

- A new user can create a vault and add an account, or finish with none.
- Restarting requires the passphrase and preserves accounts, theme, close behavior, and auto-lock settings.
- The first close asks once; later closes follow the remembered choice; hiding does not bypass lock.
- Passphrase change succeeds through the native sheet, old passphrase fails, and old backups are labeled.
- No provider is required for any of this.

### Exit gate

The app is genuinely usable with manually entered accounts and no bank connection. The wizard never seeds demo data or requests provider credentials. Security settings expose the real cipher/KDF/backup state and destructive actions require native confirmation.

---

## 9. Phase 5 — Demo data and read-only dashboard

**Goal:** a convincing, deterministic demo and the core read-only views.

### Build steps

1. Implement demo mode as a separate database and code path selected from settings, with an unmistakable persistent label, a fixed app-managed key for synthetic data, and no owner passphrase requirement. Demo mode must be incapable of opening the real ledger.
2. Build a deterministic fixture generator: frozen clock, roughly ten cards across five fictionalized institutions, selected month plus previous two months, and at least 150 transactions covering every case in spec §15.
3. Include the non-USD synthetic case even though the owner's real data is USD-only.
4. Implement a mock provider that exposes deterministic accounts, transactions, balances, and capabilities. Demo mode does not link a real provider.
5. Build the overview: selected-month net spending, current total balances, upcoming reported payments, account freshness, and demo labels.
6. Build the cards view: issuer, chosen name, last four, balance/freshness, month spending, due date with amount type, statement date, optional closing date, manual-source label, and connection health. Support card-click filtering and a clear-filter action.
7. Build the ledger: date, merchant, card, category, status, amount; search, filters, sort, pagination, row count, eligible subtotal, pending subtotal, clear filters, and a read-only side panel.
8. Build the supplier view with drill-down and exact totals.
9. Add empty, pending-only, stale, unsupported-metadata, error, and demo states. Render all descriptions as text; use the shared no-HTML rule.
10. Disable CSV import, provider linking, backup creation, restore, and portable export in demo mode. Provide a "reset demo" action that recreates fixtures.
11. Apply content protection and the same renderer lockdown in demo mode; the demo label must be visible even when the window is captured.
12. Tests: fixture totals, month navigation, filtering, supplier drill-down, pending separation, demo label, no credential prompt, no real-data import/export, demo cannot open the real DB, and demo cannot read Keychain.

### Deliverables

- Demo database and fixture generator with an isolated code path.
- Mock provider.
- Overview, cards, ledger, and supplier views.
- Empty/error states.
- Demo isolation and UI fixture-expectation tests.

### Verification

- Demo totals match backend fixture expectations exactly.
- Demo data never touches the real ledger; demo cannot open the real DB or Keychain.
- All spec §15 scenarios appear in the demo.
- Real-data import/export/backup/provider actions are unavailable in demo mode.
- The app is usable at 1100 x 760 with graceful table scrolling.
- Month changes do not alter current balances or obligations.

### Exit gate

The owner can open demo mode and inspect a complete, trustworthy dashboard. Read-only workflows are fully navigable by keyboard.

---

## 10. Phase 6 — Edits, manual metadata, CSV import/export

**Goal:** the owner can correct data and bring in missing-card history without a provider.

### Build steps

1. Implement transaction overrides: merchant alias, category, type correction, exclusion, and notes. Keep them in `transaction_overrides`, separate from source fields.
2. Implement merchant rules with match type, text, canonical merchant, optional category, and priority. Normalization merges labels, never transaction records.
3. Build manual payment metadata: statement balance, minimum due, next due date, optional valid-through date, and source labels. Keep it independent from provider snapshots.
4. Implement CSV import as a Rust-owned job: native open dialog, file hash, bounded size/row/field caps, streaming parse, target account, generic column mapping, explicit date format, preview, validation, duplicate candidates, and confirmation. The renderer receives an opaque job ID, counts, and sanitized errors, never a path.
5. Record an import batch and its rows. Reimporting an identical file is a no-op. Overlapping files use source IDs where available and conservative fingerprints otherwise.
6. Show ambiguous duplicate candidates for review instead of silently merging or deleting.
7. Implement batch rollback that removes only that batch's unique contributions.
8. Implement filtered-ledger CSV export through a native save dialog: formula-injection escaping for `= + - @`, tab, CR, and leading whitespace; 0600 file; filename without account numbers; explicit plaintext warning; no auto-open.
9. Support large historical files without loading the entire dataset into the renderer; reject NUL/binary/invalid UTF-8 and cap parse time.
10. Tests: BOM, quoted commas, parentheses, debit/credit columns, ambiguous dates, duplicate review, rollback, formula-injection variants, invalid UTF-8/NUL, huge file/field, path traversal/symlink, 0600 export, and no auto-open.

### Deliverables

- Override and merchant-rule UI.
- Manual payment metadata UI.
- Rust-owned CSV import wizard, batch tracking, duplicate review, and rollback.
- Hardened CSV export.
- Import/export security tests.

### Verification

- Importing a real sample file produces the expected month totals.
- A second identical import changes nothing.
- Legitimate identical purchases are not deleted automatically.
- Rollback preserves shared and unrelated records.
- Overrides survive app restart; later, they must survive sync.
- Export resists formula injection, is 0600, and is clearly labeled plaintext.
- The renderer never receives an arbitrary file path; oversized/malformed files fail safely.

### Exit gate

The app supports a complete CSV-only workflow: import, review, correct, reconcile, export, and roll back.

---

## 11. Phase 7 — Encrypted backup, restore, and recovery

**Goal:** recoverability without weakening encryption.

### Build steps

1. Implement consistent encrypted snapshots via the SQLite backup API or `sqlcipher_export` into a destination keyed with the DEK; checkpoint WAL first; verify `cipher_integrity_check`/`integrity_check`; never copy only the main DB file during WAL activity.
2. Define the self-contained backup container: magic/format version, vault ID, cipher/provider, KDF params/salt, wrap nonce, wrapped DEK, generation, schema version, kind, created_at, DB length, DB ciphertext SHA-256, and DB bytes; bind header fields as AAD; write 0600 atomically.
3. Create a pre-migration backup and a daily rolling backup when the app runs. Retain 7 daily plus capped pre-migration/pre-restore backups; prune only strict-pattern, non-symlink, non-last-good files.
4. Automatic backups use the current DEK and embed the wrapped-DEK header, so no extra prompt is needed; restore always asks for the passphrase through the native sheet.
5. Build restore: native file picker, native passphrase sheet, size caps, header validation, DB length/SHA-256 verification, read-only candidate open with `trusted_schema=OFF`/defensive/`query_only`, vault-ID/integrity/schema checks, encrypted pre-restore backup, confirmation, exclusive lock, atomic replacement, stale WAL/SHM removal, fsync, reopen, and rollback on failure.
6. Implement passphrase change/rekey: new DEK, SQLCipher rekey API (`sqlite3_rekey`/`PRAGMA rekey`) using the new raw DEK, new header/generation, recovery journal, pre-change backup; warn that older backups need the old passphrase and never rewrite/delete them silently. Verify raw-key rekey support in Phase 0.
7. Implement portable export with a separate strong password, the same authenticated container/key-wrapping discipline (or an age outer layer over a self-contained container), no provider secrets, and an explicit full-history warning.
8. Implement delete-all/crypto-erase: lock/close the vault, destroy the wrapped DEK, header, and backups, and remove the associated Keychain provider credentials after confirmation; document SSD remanence limits.
9. Write the recovery runbook: corrupt database, failed rekey, failed migration, unreadable backup, lost header, lost passphrase, and schema incompatibility.
10. Tests: backup creation/verification, wrong-passphrase restore, tampered/truncated header, tampered DB page, wrong vault ID, schema downgrade, retention pruning, rekey crash recovery, pre-restore backup, nonempty-database restore, no provider tokens, and no plaintext temp.

### Deliverables

- Self-contained encrypted backup container and UI.
- Restore flow with read-only validation and atomic replacement.
- Passphrase change/rekey with recovery journal.
- Portable encrypted export with a separate password.
- Delete-all/crypto-erase.
- Recovery documentation and incident runbook.
- Backup/restore security tests.

### Verification

- A backup restores on a nonempty database; a failed restore leaves the original intact.
- Backup files cannot be opened as plaintext SQLite without the passphrase; WAL/SHM are ciphertext.
- Restore rejects truncated/tampered/wrong-vault/schema-downgrade files before replacing data.
- Rekey changes the passphrase, old passphrase fails, and old backups are clearly labeled.
- No backup contains provider tokens or Keychain material.
- No plaintext temp database exists during backup/restore/export/rekey.

### Exit gate

The owner can recover the ledger from an encrypted backup and understands the no-recovery consequence of losing the passphrase or the vault header. Tamper, wrong-passphrase, and failed-restore tests pass.

---

## 12. Phase 8 — Sync engine, scheduler, and mock provider

**Goal:** provider-neutral synchronization and lifecycle behavior, provable without live credentials.

### Build steps

1. Define the provider trait: start/repair, list accounts, fetch incremental transactions, fetch balances, fetch payment/liability metadata, capabilities, and disconnect.
2. Capabilities distinguish unsupported, temporarily unavailable, and available-but-null.
3. Build the sync service: one active sync per connection, bounded concurrency, staging before commit, cursor advancement only after commit, retry/backoff, and a finite retry budget. Stop safely on lock without advancing the cursor.
4. Process added, modified, and removed transactions; reconcile pending removal and posted addition across pages; preserve overrides only with reliable linkage.
5. Keep transaction, balance, and liability streams independent so partial failure is visible. Never clear old data after an empty or error response.
6. Build the network layer: Rust-only, TLS verification, HTTPS-only, redirects disabled, timeouts, response/decompression caps, exact Plaid host allowlist, typed response validation, no provider-supplied URLs, and no inbound listener/webhook.
7. Build the token path: Keychain-only access tokens/client secret, load per request, zeroize after use, no logging of headers/bodies/URLs, no token in renderer state; no blind retry of non-idempotent token exchange.
8. Build the scheduler: 08:00 and 20:00 in the configured timezone, launch/resume catch-up when stale, missed-window collapse, no wake, and skip when the vault is locked or the app is quitting. Record `skipped: vault locked`.
9. Build the sync UI: Sync now, per-connection status, per-stream results, last fetch, provider-as-of, sanitized errors, and reconnect-required state.
10. Implement local disconnect with explicit confirmation and Keychain deletion; no remote removal without confirmation.
11. Tests: idempotent repeat sync, pagination across pages, cursor rollback after failed write, concurrent triggers, one failed connection not blocking others, partial liability failure, reconnection without duplicate accounts, lock-during-sync, non-allowlisted host/redirect/TLS/oversize rejection, and token absence from logs/errors.

### Deliverables

- Provider trait and mock provider.
- Sync service, network layer, token path, and scheduler.
- Sync/reconnect UI.
- Sync and network security test suite.

### Verification

- Running the same mock sync twice yields the same ledger.
- A failed write does not advance the cursor; a lock mid-sync does not advance it either.
- A partial balance/liability failure preserves prior values and marks staleness.
- Concurrent manual and scheduled triggers do not duplicate work.
- Scheduled sync does nothing while locked or quitting and records why.
- Non-allowlisted hosts, redirects, plain HTTP, TLS failures, and oversized responses are rejected.
- No access token or client secret appears in logs, errors, renderer state, or URLs.
- The UI never equates last fetch with provider-as-of.

### Exit gate

The full sync lifecycle is deterministic and tested against the mock provider. The app remains fully usable if no provider ever connects.

---

## 13. Phase 9 — Plaid adapter and linking (conditional)

**Goal:** live transactions and balances where verified; Liabilities only where actually available.

### Build steps

1. Guide the owner through creating a Plaid Trial account. The owner accepts terms and completes eligibility checks. Record the plan's actual product access and limits.
2. Implement only the verified desktop linking flow from Phase 0:
   - Preferred: Hosted Link + `/link/token/get` polling. Create the link token in Rust, open the hosted URL through a Rust command that accepts only the exact verified Plaid host (never a renderer-supplied URL), poll with backoff, exchange the public token in Rust, and store the access token in Keychain.
   - Alternative: a documented hosted HTTPS redirect page that receives no access tokens.
   - No `cardigan_money://` scheme, no Production loopback callback, no local HTTP listener. If neither flow is verified, stop and mark live linking unavailable.
3. If a callback is used, enforce state/nonces/PKCE where supported, exact redirect-URI matching, origin validation, one-time consumption, short TTL, constant-time comparison, and replay rejection.
4. Keep the Plaid client secret and access tokens in Keychain with `WhenUnlockedThisDeviceOnly` and `Synchronizable=false`. The renderer never receives them. Link/public tokens live in Rust memory only and are zeroized after use; never log or persist them. The Hosted Link URL is never logged/persisted and is documented as a short-lived bearer.
5. If an embedded webview is ever approved: dedicated locked-down webview, no app IPC/preload/devtools, navigation only to exact Plaid origins, new windows denied, storage cleared after completion, and no vault/Keychain access.
6. Implement Transactions Sync with a persisted cursor: all pages, added/modified/removed, restart protocol when required.
7. Implement balance snapshots. Implement payment/liability metadata only if verified for the plan and institution.
8. Map Plaid accounts to local accounts without duplicates; support reconnect and re-mapping. Never create repeated connections during testing.
9. Handle rate limits, timeouts, reconnect-required states, and billable refresh endpoints conservatively. Do not auto-force bank refresh. Do not blindly retry non-idempotent token exchange.
10. Expose capabilities and provenance in the UI, including when Liabilities is unavailable.
11. Verify in Sandbox first, then with the owner's authorized accounts. Record exactly what was verified and by whom, including which hosting component (if any) sees data.
12. If the linking flow or product access is not practical, stop, document the concrete blocker, and keep live linking visibly unavailable. Do not disguise demo data or fabricate coverage.

### Deliverables

- Plaid linking flow matching the verified desktop constraints.
- Plaid Transactions Sync and balances adapter.
- Optional Liabilities adapter.
- Account mapping, reconnect, and capability UI.
- Provider setup guide content.
- Verified/unverified capability record and data-flow note for any hosted component.

### Verification

- Sandbox transaction/balance sync passes, including pagination and removals.
- The linking flow matches the verified desktop constraints: no custom scheme, no Production loopback, no local listener, no persisted/logged hosted URL or link token.
- State/nonce/replay/expiry tests pass if a callback exists.
- No token or secret appears in renderer state, logs, URLs, or localStorage; Keychain attributes are verified.
- A reconnection does not duplicate accounts or transactions.
- Live institutions are listed only if actually verified with authorized credentials.
- If blocked, the UI explains the blocker and the rest of the app still works.

### Exit gate

Either live transactions and balances are verified for at least one authorized account, or the phase is explicitly reported as blocked with a concrete cause and the offline/CSV app is complete.

---

## 14. Phase 10 — Hardening, performance, packaging, and handoff

**Goal:** a safe, measured, runnable local release with honest documentation.

### Build steps

1. Independent security review: a reviewer other than the implementer walks the threat model, the spec §13 checklist, and [security-review.md](security-review.md). Fix or explicitly accept every Critical/High finding in writing.
2. Adversarial verification: tamper the DB/header/backup, fuzz CSV/JSON/IPC, attempt path traversal/symlink, try a plain-SQLite build, test wrong passphrase/replay/OAuth misuse, assert CSP has no external requests, and scan logs/DOM/URLs for sentinel secrets.
3. Dependency/build audit: `cargo audit`, `cargo deny`, `pnpm audit`, lockfile/frozen install, postinstall allowlist, and no secret in the repository or environment files.
4. Accessibility review: full keyboard navigation for primary workflows, visible focus, descriptive labels, readable contrast, non-color-only status indicators, and native secure-input accessibility with VoiceOver.
5. Performance: generate a 50,000-transaction encrypted dataset; measure warm search and month change (target under 200 ms) and cached startup after unlock (target roughly two seconds). Profile before claiming any target.
6. Native smoke tests on the M1 Max: startup, native passphrase sheet, file dialogs, Keychain attributes/ACL, content protection/screen capture, screen-lock auto-lock, window close/quit behavior, state restoration, theme, devtools absence, and actual IPC. Document what could not be automated.
7. Build an Apple Silicon artifact, ad-hoc sign it, verify the signature, record its SHA-256, and verify it opens and runs. Do not call it distribution-ready.
8. Write the README: prerequisites, exact development/test/build commands, how to open the app, passphrase-loss/lost-header warning, lock lifecycle, backup/restore procedure, security limitations, and residual risks.
9. Write `SECURITY.md`, the provider guide (Trial setup, verified vs unverified products, linking flow/data flow, Liabilities caveats, limits/pricing, reconnect behavior), the data dictionary, calendar-month rules, backup/recovery runbook, and incident/troubleshooting guide.
10. Complete `docs/implementation-status.md`: completed work, tests actually run, measured performance, security findings and status, blockers, live institutions actually verified, and artifact hash.
11. Run the full acceptance pass against the spec's definition of done, including the security definition of done.

### Deliverables

- Independently reviewed, hardened build.
- Adversarial/security test report and residual-risk statement.
- Measured performance report.
- Native smoke-test record.
- Ad-hoc signed artifact with SHA-256.
- Complete handoff docs, including `SECURITY.md`, threat model, security architecture, and incident runbook.
- Final implementation-status.

### Verification

- All automated suites pass, or every exception is documented.
- No Critical/High security finding remains open without written owner acceptance.
- Performance numbers are measured, not estimated.
- The artifact launches on the owner's Mac and its hash/signature are recorded.
- No secret, database, backup, or personal financial data is committed.
- The handoff clearly states what is unverified, especially live provider coverage, and does not call the app "watertight."

### Exit gate

The owner can unlock the app, use demo and CSV workflows, back up and restore encrypted data, and inspect the measured limitations. The spec's definition of done is met for the offline/demo/CSV scope, the security acceptance checklist is evidenced, and live provider status is reported honestly.

## 15. Sequencing and critical path

```text
Phase 0  Foundation/feasibility
   |
Phase 1  Shell/contract
   |
Phase 2  Vault/migrations
   |
Phase 3  Domain/accounting
   |
Phase 4  Onboarding/settings
   |
Phase 5  Demo/dashboard (read-only)
   |
Phase 6  Edits/manual metadata/CSV
   |
Phase 7  Encrypted backup/restore
   |
Phase 8  Sync/scheduler/mock provider
   |
Phase 9  Plaid live (conditional)
   |
Phase 10 Hardening/release/handoff
```

Parallel work is limited and must not break the gates:

- Documentation skeletons and the Plaid feasibility document can progress alongside Phase 1.
- UI component work can proceed while domain tests are written, provided totals come from the backend contract.
- The fixture generator can grow during Phases 4-6, but demo isolation is tested in Phase 5.
- Plaid account setup can be requested early; implementation still waits for Phases 8 and 9.

The offline path (Phases 2-7) is the critical path. Phase 9 is deliberately late so a Plaid blocker cannot stop a usable app from existing.

## 16. Risk register

| Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- |
| SQLCipher links as plain SQLite / fail-open encryption | Medium | Critical | Phase 0 fail-closed spike; plain-SQLite negative test; runtime `cipher_version`/`cipher_status`; key-first connection init; refuse to run |
| Renderer compromise steals passphrase or tokens | Medium | High | Native secure-input sheet; renderer never receives secrets; strict CSP; no remote content; command gateway; supply-chain audit |
| Weak passphrase or weak KDF enables offline brute force | Medium | High | Argon2id calibrated + bounded; 16-char/60-bit minimum; strength estimate; encrypted backups; no false claims |
| Vault header lost/corrupted | Low | High | Atomic redundant header; self-contained backups; explicit no-recovery warning; tests |
| Backup theft / old backups readable with old passphrase | Medium | High | Strong passphrase, per-vault KDF, rotation on passphrase change, delete-old-backups action, documented residual risk |
| Plaid desktop OAuth flow is infeasible | High | High | Verified Hosted Link + polling or hosted HTTPS page; no custom scheme/loopback in Production; ship live linking unavailable if blocked |
| OAuth/callback hijack or replay | Medium | High | No custom scheme; state/nonce/PKCE where supported; exact redirect; one-time/TTL; no local listener |
| Provider token/client-secret leak via logs, renderer, env | Medium | High | Keychain-only; native input; Rust-only tokens; redaction; secret-sentinel tests; no `VITE_*` secrets |
| Supply-chain compromise (npm/crate) | Medium | High | Lockfiles, frozen installs, `cargo audit`/`cargo deny`/`pnpm audit`, official plugins only, no unapproved postinstall, artifact hash |
| Same-user malware reads unlocked memory | Medium | High | Cannot fully defend; `mlock`/zeroize, lock lifecycle, minimal session; documented residual risk |
| Screen capture / state restoration leaks ledger | Medium | Medium | `set_content_protected`; disable state restoration; no window titles/notifications; native smoke test |
| Unsigned build tampering / unstable Keychain ACL | Medium | Medium | Ad-hoc sign, SHA-256, stable local signing identity; no auto-update; not distribution-ready |
| CSV formula injection / parser DoS | Medium | Medium | Formula escaping; size/row/field caps; streaming; UTF-8 only; fuzz tests; 0600 export; no auto-open |
| Restore of malicious/corrupt backup | Low | High | Read-only validation; `trusted_schema=OFF`; defensive mode; header/DB ID/integrity/schema checks; pre-restore backup; atomic replace |
| Two instances corrupt the DB / race key changes | Low | High | Single-instance plugin/lock before DB open; tests |
| Crash during rekey loses access | Low | High | Pre-rekey backup; rotation journal; deterministic recovery test |
| Security deferred to the end / unverified claims | Medium | High | Security work in Phases 0–2; per-phase security gates; independent review; residual-risk disclosure |
| Auto-lock makes scheduled sync mostly unavailable | High | Medium | Document the tradeoff; owner-configurable timeout; no background unlock; sync remains best-effort |
| SQL injection / dynamic SQL | Low | High | Prepared statements only; enum allowlists; injection tests; `trusted_schema=OFF` |
| Demo and real data mix | Low | High | Separate DB/code paths; fixed demo key only for synthetic data; disable import/provider/backup/export in demo; tests |
| Performance degrades with encryption | Medium | Medium | Measure encrypted 50k dataset; index and paginate; avoid loading the ledger |
| Scope creep into budgets/notifications | Medium | Medium | Spec's out-of-scope list; ask only for missing decisions |
| Native behavior untested by Playwright | High | Medium | Documented native security smoke tests; no false claims of native automation |

## 17. Owner dependencies

These are tracked but do not block the offline path:

- Create the Plaid Trial account when Phase 9 begins; the agent supplies a checklist and docs, not enrollment.
- Approve the verified Plaid desktop flow, including any hosted component and its data flow. If no acceptable flow exists, live linking stays unavailable.
- Provide real CSV exports when available; until then, generic mapping and fixtures are used.
- Keep custody of the database passphrase. Never share it in chat, a repository, an environment file, or a support request.
- Enable FileVault and a short screen-lock timeout; these are environmental controls the app cannot enforce.
- Confirm the lock lifecycle (screen lock/sleep/manual/quit plus 15-minute idle default) is acceptable. Disabling idle auto-lock requires a recorded risk acceptance.
- Decide on ad-hoc signing vs a stable local self-signed identity for Keychain behavior.
- Confirm live verification personally before any "live connected" claim.
- Read the residual risks in [security-review.md](security-review.md) and the spec §13.14 before calling the app complete.
- Supply branding only if desired; otherwise a neutral placeholder icon is used.

## 18. Definition of done for the plan

The plan is complete when:

- The app unlocks with the owner's passphrase and never leaves a plaintext ledger; the plain-SQLite build is rejected.
- The passphrase never enters the renderer; native secure input is used for passphrase, backup/restore/export, and client-secret entry.
- The vault locks on screen lock, sleep, fast user switching, logout, quit, manual Lock Now, and the idle timeout; renderer caches are cleared and sync stops safely.
- The owner can add accounts, select a calendar month, inspect itemized spending, supplier totals, current balances, and clearly sourced due/statement metadata.
- CSV import/export, duplicate review, and batch rollback work without a provider, with formula-injection, size, path, and 0600 protections.
- Encrypted backup, restore, passphrase change, crypto-erase, and recovery documentation work; tamper/wrong-passphrase/nonempty-database tests pass.
- Sync, scheduling, reconnect, lock-during-sync, and network-allowlist behavior pass mock-provider tests.
- Plaid status is either honestly verified for named institutions or explicitly blocked with a concrete cause and a verified flow/no-flow record.
- Demo mode is complete, labeled, isolated, deterministic, and cannot import or expose real data.
- Release CSP/capabilities have no forbidden permissions or HTTPS sources; Playwright observes no external requests; IPC fuzz/validation tests pass.
- File permissions, symlink rejection, single-instance, redaction, and secret-sentinel tests pass.
- No Critical/High security finding remains open without written owner acceptance; independent review evidence is recorded.
- Tests actually run are recorded; performance is measured; native security smoke tests are documented.
- The ad-hoc signed local build runs on the owner's Mac, its SHA-256 is recorded, and the handoff documentation is complete, including residual risks.
- Nothing is described as "watertight," "unhackable," or distribution-ready.

## 19. Progress reporting

At the end of each phase, report:

1. What was completed.
2. Which tests actually ran and their results.
3. Any measured performance.
4. Security findings discovered, fixed, or still open; which spec §13 controls are now evidenced; and which are still unverified.
5. Blockers, including external dependencies.
6. Live institutions actually verified (usually none until Phase 9).
7. What the next phase will start, and whether its gate is met.

A phase is not "done" because code exists. It is done when its exit gate is satisfied, the relevant security checks are evidenced, and the implementation-status document reflects reality. Do not report a security control as implemented because the code exists; report what was tested.
