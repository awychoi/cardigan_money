# Build specification: cardigan_money (Personal Cards for macOS)

Status: revised 2026-10-23 with the owner's decisions and the security review. The owner decision record is §1.1; the security-review decisions that tighten or supersede earlier defaults are §1.2; dependencies the owner still controls are listed in §20. The review itself is [security-review.md](security-review.md).

## 1. Mission

Build a polished, local-first macOS application for one person's credit-card balances and spending. The initial user has an Apple Silicon M1 Max Mac. The owner will add their own accounts through the app's first-launch setup wizard; the exact institution and card list is deliberately not hardcoded. Demo fixtures and tests assume roughly five bank logins and ten cards, including representative Chase, Amazon Store Card (Synchrony), and Bilt shapes.

This is a personal dashboard, not a commercial fintech service, budgeting system, payment application, or statement-cycle tracker.

Deliver a working application, not merely a mockup, architecture document, or scaffold.

### 1.1 Recorded owner decisions

Confirmed by the owner on 2026-10-23. These decisions replace any contrary default elsewhere in this document.

| Topic | Decision |
| --- | --- |
| App identity | Display name `cardigan_money`; bundle identifier `com.awychoi.cardiganmoney`. The window title and in-app header use the same name; use the exact strings and do not title-case or prettify them. |
| Desktop stack | Tauri 2 with React, TypeScript, and Vite. |
| Build and signing | An unsigned local Apple Silicon build is sufficient; no notarization or distribution signing is required. The security review adds ad-hoc signing, a recorded SHA-256, and a stable local signing identity for Keychain reliability; see §1.2. |
| Accounts | First launch starts empty. The owner adds accounts through an in-app setup wizard. No real accounts are hardcoded. |
| Live provider | The owner will create a free Plaid Trial account with agent guidance. Enrollment, eligibility checks, and terms acceptance are the owner's actions. Live coverage remains gated by §6 verification. |
| Provider products | Transactions and balances first. Liabilities is requested only if the Trial plan and the linked institution actually support it; manual payment metadata covers gaps. |
| History | Start empty. The owner will export issuer CSV history later; provider history covers recent months. CSV import ships before any backfill is needed. |
| Encryption at rest | SQLCipher with an owner-chosen passphrase. The passphrase is never stored, logged, or transmitted. Losing it makes the ledger permanently unreadable. |
| Backups | Automatic backups and portable exports are password-encrypted. Retention is 7 daily backups plus pre-migration backups. |
| Currencies | Real data is USD-only. Non-USD handling remains a synthetic fixture for tests; no currency conversion. |
| Appearance | Light, dark, and follow-system modes with an in-app toggle. |
| Lifecycle | No macOS notifications in v1. No login item and no launch-at-login. The first window close asks whether to hide or quit, then remembers the choice. |
| Working style | Ask the owner only when a consequential decision is genuinely missing; otherwise proceed on documented defaults. |

### 1.2 Security-review decisions (2026-10-23)

These decisions were adopted after the architecture review in [security-review.md](security-review.md). Where they conflict with an earlier default in this document, these decisions win. They are not optional nice-to-haves: several are release blockers.

| Topic | Decision |
| --- | --- |
| Renderer trust | The React renderer is untrusted. It never receives the vault passphrase, the data encryption key, the key-encryption key, provider access tokens, the Plaid client secret, or raw provider payloads. Passphrases and the Plaid client secret are collected with a native macOS secure-input sheet; the renderer receives only success, failure, or cancellation. |
| Vault cryptography | Envelope encryption: a random 256-bit data encryption key (DEK) is the SQLCipher raw key; an Argon2id-derived key-encryption key (KEK) wraps the DEK with XChaCha20-Poly1305; KDF parameters, salt, wrapped DEK, vault ID, and generation live in an authenticated `vault.header` outside the encrypted database. The passphrase is never stored, logged, or sent off the Mac, and never placed in Keychain. |
| Fail closed | If SQLCipher is not actually active, the header is invalid, the KDF parameters are outside the allowed bounds, or integrity verification fails, the app refuses to open or create the ledger. There is no plaintext fallback and no automatic reset. |
| Backups and exports | Automatic backups are self-contained encrypted snapshots: the SQLCipher snapshot plus a header that carries the wrapped DEK. Automatic backup creation needs no extra prompt; restore always requires the passphrase. Passphrase change rotates the DEK and rekeys the live database, so older backups require the older passphrase and are labeled as such. Portable exports use a separate owner-chosen password and the same authenticated-container discipline. |
| Lock lifecycle | Manual Lock Now is required. The vault locks on macOS screen lock, sleep, fast user switching, logout, app quit, and after an idle timeout (default 15 minutes, configurable 1–240 minutes or disabled only with a visible warning). This supersedes the earlier "no auto-lock; locks on quit" decision. Hiding the window is not a lock. |
| Provider linking | No Production flow may depend on a custom URI scheme, a `cardigan_money://` deep link, or a loopback HTTP callback: Plaid currently requires HTTPS redirect URIs, does not support custom schemes, and allows localhost redirects only in Sandbox. The approved desktop flow must be verified in Phase 0. The preferred candidate is Hosted Link plus `/link/token/get` polling; a hosted HTTPS redirect page is the alternative. If neither is verified, live linking ships as unavailable with the concrete blocker. No server or cloud component is introduced silently. |
| Signing and Keychain | An unsigned local build remains acceptable for personal use, but it must be ad-hoc signed, its SHA-256 recorded, and its limits documented. A stable local signing identity (self-signed certificate) is the recommended way to get reliable Keychain ACL behavior during development; if the owner declines it, document the Keychain prompt behavior and accept that residual risk. Notarization/distribution signing remains out of scope; the build is not distribution-ready. |
| Demo mode | The demo database is synthetic-only, separate, and may use a fixed app-managed key. Demo mode disables CSV import, provider linking, backups, and portable exports so real data cannot be placed in an unprotected demo database. |
| Security gates | Security work is not deferred to Phase 10. The plan's Phase 0, 1, and 2 exit gates include the threat model, fail-closed vault, native secret input, renderer lockdown, and lock lifecycle. No phase is complete while a relevant Critical/High security finding is open. |

### Working style for the implementing agent

- Proceed using the decisions above and document consequential assumptions. Ask the owner only when a real decision is missing (for example, a provider capability turns out to be unavailable, or a new irreversible choice is required). Batch such questions and include a recommendation.
- Inspect the repository before changing anything. Preserve existing work; never overwrite unrelated files.
- Implement in small, tested increments. Maintain a short implementation checklist.
- Use current official documentation to verify framework APIs and provider requirements before implementing integrations.
- Never invent bank coverage, free-tier eligibility, available history, or supported provider fields.
- Never fabricate successful builds, live connections, tests, or sync results.
- Do not request, display, log, or commit the database passphrase or any credential. Credential-dependent verification may remain explicitly unverified.
- Do not enroll accounts, accept provider terms, incur charges, publish, or configure paid services on the owner's behalf. Plaid Trial enrollment is the owner's action; the agent may only provide instructions.
- Do not add speculative features. Finish the core ledger and data-integrity behavior first.

## 2. Product principles

1. Calendar months organize spending. Statement cycles do not.
2. Current balances and upcoming obligations remain current when the user selects an older spending month.
3. Purchases across all cards appear in one itemized ledger.
4. Pending amounts, posted spending, refunds, payments, and transfers must not be conflated.
5. Missing, stale, unsupported, and zero are different states.
6. The UI reads local data. Navigating the UI must not trigger bank API requests.
7. Financial correctness, understandable provenance, and recoverability outrank visual decoration.
8. The ledger is encrypted at rest and opens only with the owner's passphrase; automatic backups and portable exports are encrypted too.
9. Fully usable demo mode and CSV import are required even without live credentials.
10. The renderer is an untrusted presentation layer. Privilege, secrets, network access, and filesystem access stay in Rust; the passphrase never enters the webview.
11. Fail closed. A failed cipher check, integrity check, KDF validation, or authorization check stops the operation; the app never falls back to plaintext, a weaker cipher, an unverified host, or a silent reset.
12. Least privilege and data minimization. Request the narrowest Tauri capability and provider product that works; do not retain raw provider payloads or unnecessary PII.
13. Every security claim must be tested or clearly marked unverified. "Watertight" is a direction of travel, not a claim about an unsigned local app on a general-purpose OS.
14. Locking is part of the product, not an optional convenience: a hidden window, a sleeping Mac, or a locked screen must not become an unauthenticated ledger view.

## 3. Scope

### Required version one

- Installable/runnable macOS desktop app with persistent local storage.
- First-launch setup wizard for creating manual accounts, with a skippable optional provider connection step.
- Passphrase-protected, SQLCipher-encrypted local database with an unlock screen; no plaintext ledger at rest.
- Overview containing current balances, month spending, upcoming reported obligations, and account freshness.
- Calendar-month selector shared across spending views.
- Compact tiles/list for the owner's accounts (roughly ten expected, but not hardcoded).
- Light, dark, and follow-system appearance modes with an in-app toggle.
- Unified searchable and filterable transaction table.
- Supplier aggregation with drill-down to underlying transactions.
- Editable merchant aliases and categories, preserved across syncs.
- Pending/posted reconciliation and payment/transfer exclusion.
- CSV import with preview, validation, duplicate handling, and rollback by import batch.
- Generic CSV column mapping first; issuer-specific presets only once the owner supplies real sample files.
- Provider adapter architecture and a Plaid adapter, subject to actual account/product access.
- Manual statement/payment metadata when a provider does not supply it.
- Sync now, sync on launch/resume when stale, and scheduled attempts while running.
- Connection repair/status UI and partial-failure handling.
- Password-encrypted automatic backups, restore, portable export, tests, setup instructions, and build instructions.
- Native macOS secure-input sheet for passphrase creation/unlock/change, backup/restore/export passwords, and the Plaid client secret; the renderer never receives these values.
- Authenticated external `vault.header` with envelope key hierarchy (Argon2id KEK, random DEK, XChaCha20-Poly1305 wrap, SQLCipher raw key) and fail-closed cipher/integrity verification.
- Lock lifecycle: manual Lock Now; lock on screen lock, sleep, fast user switching, logout, and quit; configurable idle auto-lock defaulting to 15 minutes.
- Renderer lockdown: strict CSP with no HTTPS sources, minimal Tauri capabilities, command allowlist with bounded validation, blocked navigation/new windows, devtools off in release.
- Threat model, security test suite, dependency audit, security event trail, incident runbook, artifact hash, and documented residual risks.
- Window content protection on macOS, single-instance enforcement, restrictive file permissions, and no plaintext/crash/log artifacts.

### Explicitly out of scope initially

- Initiating payments, moving money, credit decisions, or financial advice.
- Multiple users, team permissions, public hosting, subscriptions, or commercial onboarding.
- Budget envelopes, forecasts, AI categorization, investments, receipt OCR, or item-level receipt extraction.
- Continuous real-time polling.
- macOS notifications, a login item, launch-at-login, automatic updates, and an always-on background helper that runs after the app quits.
- Native mobile applications or cross-device synchronization.
- Automatic currency conversion.
- Hardware-backed key storage, Touch ID/Face ID vault unlock, MFA, and multi-user vaults. The owner-passphrase model is deliberate; do not add a Keychain unlock path for the ledger.
- Distribution signing, notarization, App Sandbox, hardened runtime, and auto-update. These are documented as out of scope, not silently implied as present.

“Itemized” means one row per financial transaction, not individual products purchased within an Amazon order. Do not imply bank transaction data includes receipt line items.

## 4. Default technology choices

- Desktop: Tauri 2, native Apple Silicon target, bundle identifier `com.awychoi.cardiganmoney`, display/product name `cardigan_money`.
- Frontend: React, TypeScript, Vite.
- Styling/components: Tailwind CSS and shadcn/ui, with accessible primitives.
- Ledger: TanStack Table.
- Frontend validation: Zod.
- Backend: Rust through narrowly scoped Tauri commands.
- HTTP: reqwest with TLS verification, HTTPS-only, redirects disabled, connect/read timeouts, bounded retry with jitter, response-size limits, and a fixed provider host allowlist. All network calls occur in Rust; the renderer has no network source in its CSP.
- Database: SQLite through rusqlite with a maintained SQLCipher integration and versioned SQL migrations. Pin the exact SQLCipher provider/version and rusqlite feature flags. Encrypt the ledger; do not ship a plaintext database. Use the SQLite backup API or `sqlcipher_export` for snapshots, never a plain file copy during WAL activity.
- Vault: owner-chosen passphrase; Argon2id (RustCrypto `argon2`) derives a KEK; a random 256-bit DEK is wrapped with XChaCha20-Poly1305 (`chacha20poly1305`); SQLCipher opens with the raw DEK (`PRAGMA key = "x'...'"`). Store KDF parameters, salt, wrapped DEK, vault ID, and generation in an authenticated `vault.header` outside the database. Use `zeroize`/`secrecy`, `mlock` best-effort, `cipher_memory_security=ON`, `temp_store=MEMORY`, `secure_delete=ON`, `trusted_schema=OFF`, and SQLite defensive mode. Verify SQLCipher and rusqlite compatibility against current official documentation; fail closed if `cipher_version`, `cipher_provider`, or `cipher_status` is not what is expected.
- Native secure input: AppKit `NSSecureTextField` via `objc2-app-kit`/`objc2-foundation` for passphrases, backup/restore/export passwords, and the Plaid client secret. The value is returned only to Rust and zeroized after use.
- Credential storage: macOS Keychain through a maintained Rust integration (`security-framework`), for provider secrets and tokens only. Use `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`, `kSecAttrSynchronizable=false`, and the data-protection Keychain. One item per connection; never store the database passphrase, DEK, or KEK in Keychain or on disk.
- Backups and exports: self-contained authenticated snapshot container (SQLCipher DB + header carrying the wrapped DEK) for automatic backups and portable exports. Automatic backups need no extra prompt; restore always asks for the passphrase. Portable exports use a separate owner-chosen password. An age outer layer is optional but must not replace the inner key hierarchy. Verify the chosen libraries before implementing; do not invent cryptography.
- Tauri hardening: explicit CSP with no HTTPS sources; one capability scoped to the `main` window; no filesystem, shell, HTTP, opener, SQL, store, process, updater, websocket, upload, clipboard, or deep-link permissions; `freezePrototype: true`; `withGlobalTauri: false`; asset protocol disabled; devtools disabled in release; navigation/new-window handlers restricted to the app origin.
- Single instance: enforce one running instance before any database open (official single-instance plugin or an equivalent exclusive lock).
- Logging: redacting logger only; no persistent logs by default; no financial payloads, secrets, SQL parameters, file paths, or account numbers. Disable core dumps and third-party crash reporting.
- Frontend tests: Vitest and React Testing Library.
- Backend tests: cargo test, including temporary real encrypted SQLite databases and negative cipher tests.
- Browser-level UI tests: Playwright against a mock desktop bridge that is compiled out of release builds.
- Security tooling: `cargo audit`, `cargo deny` (advisories, sources, licenses, bans), `pnpm audit`, lockfile/frozen-install checks, secret scanning, CSV/JSON/IPC fuzz targets, and a Playwright CSP/network assertion.

Select compatible stable package versions, pin lockfiles, and document the resolved versions. Avoid adding a Next.js server, Electron runtime, Python service, Redis, PostgreSQL, or Docker requirement.

Use a shared typed command contract; generate TypeScript bindings from Rust if a maintained compatible solution is practical. Never rely on unchecked casts across the IPC boundary.

Money arithmetic belongs in the backend domain layer. The frontend formats amounts and requests typed summaries rather than reimplementing financial rules.

## 5. Architecture

```text
React UI (untrusted presentation layer; no secrets, no network, no filesystem)
  -> typed IPC command gateway (window/origin check, schema + bounds, allowlist, rate limit)
    -> application services
      -> vault session
        -> native secure-input passphrase
        -> Argon2id KEK -> unwrap DEK from authenticated vault.header
        -> SQLCipher opened with the raw DEK; keys mlock'd and zeroized on lock
      -> domain rules and repository layer
        -> encrypted SQLite (SQLCipher) + encrypted WAL
      -> provider adapters (Rust only)
        -> fixed HTTPS host allowlist, TLS, no redirects, response caps
      -> credential store
        -> macOS Keychain (provider tokens only; ThisDeviceOnly; non-synchronizable)
      -> native file jobs
        -> dialogs and file I/O owned by Rust; renderer receives opaque job IDs
```

Trust boundary: the renderer is untrusted even though it is locally bundled. Rust owns privilege, secrets, network, filesystem, and the database. Tauri capabilities are scoped to the `main` window; custom commands are allowlisted and validated, not a generic bridge.

Suggested layout:

```text
src/
  app/
  components/
  features/{onboarding,overview,cards,transactions,suppliers,connections,imports,settings}/
  lib/{bridge,formatting,validation}/
  mocks/
src-tauri/
  src/
    commands/
    domain/
    services/
    providers/{mod.rs,mock.rs,csv.rs,plaid.rs}
    storage/
    security/{mod.rs,keychain.rs,vault.rs}
    backups/
    scheduling/
  migrations/
  tests/
fixtures/
docs/
```

Keep provider DTOs out of the UI and domain schema. A provider must be replaceable without changing how monthly spending is calculated.

Use SQLite foreign keys and WAL mode on local disk. Keep database writes short. Never hold a database transaction open during a network request. Dispatch blocking SQLite work off the UI/event-loop thread; use a controlled connection strategy with explicit busy-timeout handling.

Open the real ledger only after a successful vault unlock. Parse and authenticate `vault.header`, derive the KEK with the native secure-input passphrase, unwrap the DEK, open SQLCipher with the raw DEK, verify `cipher_version`/`cipher_provider` and integrity, and only then run queries. The separate demo database in §15 is the only database that may use an app-managed key. The DEK and KEK live only in Rust memory, are `mlock`ed where practical, and are zeroized on lock or quit; they never appear in the renderer, localStorage, URLs, logs, or Keychain. A failed unlock must not create, reset, or overwrite the database. Do not extract a decrypted copy to a temporary plaintext file for querying or backup. Locking closes every database connection, clears renderer caches, and stops sync. Enforce a single running instance before opening the database.

Configure every connection with the key first, then `foreign_keys=ON`, `journal_mode=WAL`, `synchronous=FULL`, `busy_timeout`, `secure_delete=ON`, `temp_store=MEMORY`, `trusted_schema=OFF`, SQLite defensive mode, and `cipher_memory_security=ON`. Verify that WAL and SHM files are ciphertext, not plaintext. Use prepared statements exclusively; dynamic sort/filter identifiers come from enums, never from free-form input.

## 6. Initial provider integration gate

Before polishing live integration, perform a small feasibility spike using current official docs:

1. Confirm an eligible provider account can access Transactions, account balances, and, separately, Liabilities where available.
2. Verify the desktop linking flow against current official Plaid documentation. Record the actual rules in `docs/plaid-feasibility.md`, including that redirect URIs must be HTTPS, custom URI schemes are not supported, `localhost` redirects are allowed only in Sandbox, and the Link Webview SDK is deprecated. Do not design a Production flow around `cardigan_money://` or a loopback listener.
3. Evaluate **Hosted Link plus `/link/token/get` polling** as the preferred no-server candidate: create the link token in Rust with `hosted_link: {}`, open the returned `hosted_link_url` in the system browser, poll for session completion, exchange the public token in Rust, and store the access token in Keychain. Document that the hosted-link URL is a short-lived bearer URL that may appear in browser history, is never logged or persisted by the app, and is opened directly.
4. Evaluate a **hosted HTTPS redirect page** as the alternative. If used, document exactly what is hosted, who operates it, what data crosses it, its retention, and why it is necessary. The page must not receive or store access tokens, and no server or cloud component may be introduced silently.
5. If an embedded webview is ever proposed, require explicit Plaid approval for the flow and a dedicated locked-down webview: no application IPC, no preload, no devtools, navigation restricted to the exact Plaid origins, new windows denied, storage cleared after completion, and no access to the vault or Keychain. Do not assume WKWebView is supported.
6. If any callback exists, use state/nonces and PKCE where supported, exact redirect-URI matching, origin validation, one-time consumption, a short TTL, constant-time comparison, and replay rejection. Do not expose an unauthenticated permanent local server.
7. Keep the Plaid client secret, link tokens, public tokens, and access tokens in Rust/Keychain, never in the renderer, localStorage, URLs the app controls, or logs. Zeroize short-lived tokens after use.
8. Apply the network rules from §9: TLS verification, HTTPS-only, fixed host allowlist, redirects disabled, response-size caps, no automatic billable refresh, and typed validation of every response.
9. If the provider requires hosted components, explain exactly what is hosted and what data leaves the Mac. Do not silently introduce cloud infrastructure.
10. If no approved desktop flow is currently practical, ship the complete app with mock and CSV support; show live linking as unavailable with the concrete blocker. Do not disguise demo data as live data and do not fake a callback flow.

The security gate for Phase 0 is: a named, documented flow that Plaid currently supports (or an explicit blocker). "We will figure out OAuth later" is not an acceptable exit.

The owner will create the Plaid Trial account. Provide a setup checklist, but do not create the account, accept terms, enter credentials, or complete institution linking on the owner's behalf. Verify current Trial eligibility, Item limits, product access, and pricing in official documentation rather than hardcoding them.

Transactions and balances are the required live products. Liabilities is optional: request it only if the owner's plan and the linked institution expose it. When Liabilities is unavailable or incomplete, the owner can enter dated manual payment metadata; do not infer due dates or amounts.

Actual accounts are added in the app, so the institution list is not known in advance. Demo fixtures and expectations use representative Chase, Amazon Store Card/Synchrony, and Bilt shapes. Availability must be tested for each specific product and account. Do not equate generic Synchrony support with Amazon Store Card support, or generic Bilt support with verified card transactions and liability metadata.

Treat provider cost/trial limits as external configuration, not hardcoded product assumptions. Never upgrade a plan automatically. Do not repeatedly create duplicate connections during testing.

## 7. Data model

Use stable local UUIDs and UTC timestamps for events. Preserve provider calendar dates as YYYY-MM-DD strings, not midnight UTC timestamps. Store currency explicitly.

### Core tables

- `vault.header` (a file outside the encrypted database, not a table): format version, vault ID, cipher/provider version, KDF identifier and parameters, KDF salt, wrap nonce, wrapped DEK, generation, creation and rotation timestamps, and schema version. It is authenticated with AEAD, written atomically with a mirrored copy of the current generation, permission 0600, and contains nothing usable without the passphrase. Losing it makes the ledger unreadable unless a self-contained backup exists.
- `vault_metadata` (inside the encrypted database): vault ID, cipher/provider version, schema version, creation/rotation timestamps, generation, last integrity-check time, and audit-only key metadata. KDF parameters and the wrapped DEK belong in `vault.header`, never only inside the encrypted database. Never store the passphrase or a reversible key.
- `connections`: provider, environment, institution ID/name, opaque Keychain credential reference (never the token itself), status, last attempt/success, cursor, sanitized error, and last remote-revocation status.
- `accounts`: local ID, display name, issuer, last four digits, account type, currency, archived state, inclusion settings, and `source_kind` (`manual` or `provider`). Manual accounts do not require a provider source row.
- `account_sources`: provider account identity -> local account mapping and reconnection history. Optional for manual accounts.
- `balance_snapshots`: current balance, available credit, credit limit, currency, retrieved time, provider-as-of time if supplied, provenance.
- `payment_snapshots`: next due date, minimum due, last statement balance, last statement issue date, next closing date only if explicitly supplied, retrieved time and provenance.
- `manual_payment_metadata`: user-entered dated facts, optional valid-through date, creation/update timestamps. Keep independent from provider fields.
- `transactions`: source identity, local account ID, provider transaction ID, pending linkage, authorized date, posted date, effective reporting date and source, amount minor units, currency, raw description, normalized merchant, type, pending state, deleted/removed state, source update time.
- `transaction_overrides`: category, merchant override, type correction, excluded flag, notes, user edit timestamp; separate from source fields.
- `merchant_rules`: match type, match text, canonical merchant, optional category, priority.
- `import_batches`: file hash, mapping configuration, account, counts, timestamps, status.
- `import_rows`: batch/row identity and source provenance for duplicate tracking and rollback.
- `sync_runs` and `sync_steps`: per-connection and per-data-stream results, counters, timestamps, sanitized errors.
- `settings`: timezone, sync windows, theme (`light`/`dark`/`system`), window-close behavior (`unset`/`hide`/`quit`), onboarding completion, backup retention and last-backup status, display preferences, auto-lock timeout, lock-on-system-event preference, content-protection preference, and last integrity-check status. Never store secrets, tokens, passphrases, or wrapped keys here.
- `security_events`: event type (unlock success/failure, lock, backup created, restore started/succeeded/failed, export, rekey, credential added/removed, integrity failure, single-instance rejection), timestamp, outcome, and a small non-secret metadata object. This is an audit trail, not a financial log.

All provider- and CSV-derived text is untrusted. Enforce length caps and charset rules at the boundary, render as text only, and strip control/bidi characters for display. Use prepared statements exclusively; sort/filter identifiers come from closed enums. Prefer `STRICT` tables and `CHECK` constraints for amounts, dates, enums, and non-null source identifiers.

Add indexes for account/date, reporting date, canonical merchant, source identity, and connection status as needed. Enforce unique provider transaction identity within its actual documented scope.

### Money rules

- Store USD as signed integer cents. Preserve a positive amount for purchases/fees and a negative amount for refunds or repayments; transaction type controls spend eligibility.
- Parse decimal source amounts without binary floating-point rounding. Use a decimal parser at provider/import boundaries.
- Validate currency precision. Do not assume every currency has two decimal places.
- Do not sum currencies together. Default the dashboard to USD and real accounts are expected to be USD; still label any other currency separately.
- Treat non-USD data as a synthetic test fixture and a defensive code path, not part of the owner's real ledger in version one.
- Never merge two purchases solely because date, merchant, and amount match.

### Provenance and missing values

Every displayed payment/balance fact must be traceable to a provider snapshot or dated manual entry. `null` means unknown, not zero. Display whether data is provider-supplied or manual. Keep manual values independent so a sync cannot erase them; let the UI explicitly choose/show the source when values differ.

Do not retain unredacted provider payloads by default. Store normalized fields and minimal diagnostic metadata. Any optional raw-data troubleshooting mode must be explicit, local, time-limited, and exclude secrets.

## 8. Calendar-month accounting rules

Default reporting timezone: America/New_York, configurable.

1. Use authorization/purchase date when available; otherwise use posted date.
2. Preserve the selected date and its provenance on each transaction.
3. Month selection uses `[first day of month, first day of next month)` over date-only values.
4. Posted purchases plus fees/interest minus posted refunds form net monthly spending.
5. Payments, transfers, and explicitly excluded transactions never contribute to spending.
6. Pending purchases are shown separately and excluded from the posted headline total.
7. Refunds belong to the month received, even if the original purchase occurred earlier.
8. Show gross purchases, refunds, fees/interest, and net spending as explainable components.
9. Supplier totals and category totals must reconcile exactly to the same eligible ledger rows.
10. The current balance is a provider/manual snapshot, not a sum of imported purchases.
11. Changing the month must not change current balances or current upcoming obligations. Label those sections accordingly.
12. A later source correction can change a historical total; keep the changed transaction explainable.
13. Do not infer a statement due amount by subtracting unrelated balances or guessing payment allocation.
14. A recurring due-day hint is not a confirmed next due date and cannot appear as one.

Implement these rules once in tested backend functions. No separately maintained frontend totals.

## 9. Synchronization contract

Provide a provider-neutral interface with capabilities and operations for:

- Starting/repairing an approved connection.
- Listing accounts.
- Retrieving incremental transaction changes.
- Retrieving balance snapshots.
- Retrieving payment/liability metadata where supported.
- Disconnecting locally; remote removal only after explicit confirmation.

Capabilities must distinguish unsupported, temporarily unavailable, and available-but-null fields.

### Sync behavior

- One active sync at a time for a connection. Manual and scheduled triggers share the same lock/service.
- Bound concurrency across institutions.
- Timeouts, rate-limit-aware backoff, and a finite retry budget.
- Update healthy connections even if another fails.
- Track transactions, balances, and liabilities independently so a partial failure is visible.
- No destructive clearing of old data following an empty/error response.
- Preserve the last known data with its timestamps after a failure.
- Mark reconnect-required separately from transient network errors.
- Retries must not create duplicate records or connections.
- Sync requires an unlocked vault; if the vault is locked or the app has quit, skip the attempt and record why rather than starting a background unlock.
- If the vault locks mid-sync, stop at the next safe point, do not advance the cursor, and do not leave a partial commit.

### Network and credential handling

- All network calls occur in Rust. The renderer CSP has no HTTPS source, so the webview cannot contact a provider, analytics service, CDN, font host, or attacker endpoint.
- The HTTP client verifies TLS, refuses `http://`, disables redirects, sets connect/read timeouts, caps response and decompressed size, and uses an exact host allowlist (`production.plaid.com` and `sandbox.plaid.com`; no user-controlled origins). Do not disable certificate verification or add a debug "trust all" path in a release build.
- Never fetch URLs supplied by provider payloads, institutions, or imported files. Do not load remote logos, images, or fonts. There is no inbound listener, webhook endpoint, or local HTTP server in version one.
- Treat every provider response as untrusted: validate JSON types, required fields, string lengths, amount/date/currency ranges, and collection sizes. Ignore unknown fields; reject malformed records rather than storing them. Never retain raw provider payloads by default.
- Access tokens and the Plaid client secret live in Keychain. Load a token only for the duration of a request, zeroize it after use, and never place it in a URL, log, error, event payload, or renderer state. The client secret must never be a `VITE_*` variable or any value bundled into frontend JavaScript.
- Do not log request or response bodies, headers, authorization values, account numbers, or financial payloads. Request IDs are acceptable if they contain no account data.
- Respect rate limits with jittered backoff and a finite retry budget. Do not blindly retry token exchange, public-token exchange, or other non-idempotent creation calls; verify state before retrying. Do not call billable refresh endpoints automatically.
- On disconnect, delete the Keychain item and offer explicit remote Item removal. A local disconnect must not silently revoke a remote Item, and a remote failure must not delete local financial records.

### Plaid-specific correctness

Verify exact current endpoint behavior against official documentation.

- Use Transactions Sync with a persisted cursor.
- Process added, modified, and removed transactions.
- Retrieve all required pages before committing one consistent update and its cursor.
- If pagination mutation requires a restart, restart from the original cursor according to the documented protocol.
- Never advance the saved cursor before the related database changes commit.
- Do not hold a DB write transaction while fetching pages; stage in memory or a controlled local staging area.
- Reconcile pending removal and posted addition even when they appear on different pages.
- Transfer user overrides to the replacement transaction only when provider linkage or another sufficiently reliable relationship supports it.
- Never use fuzzy merchant/amount matching alone to delete apparently duplicated purchases.
- A removed transaction must stop contributing to totals without deleting relevant audit history.
- Distinguish cached provider retrieval from forcing a bank refresh. Do not invoke potentially billable refresh endpoints automatically by default.

## 10. CSV fallback

- Import locally without uploading files anywhere.
- Build the generic mapper and preview before any issuer-specific preset. Issuer presets are added only after the owner supplies real sample exports; never invent column layouts, sign conventions, or date formats.
- Choose the target account and map date, description, amount, debit/credit, optional category, and optional source ID.
- Support common CSV quoting, UTF-8 BOM, currency symbols, parentheses, and configurable debit/credit sign conventions.
- Require an explicit date format where ambiguous; never silently guess 03/04.
- Preview normalized rows, validation errors, duplicate candidates, and the resulting month totals before confirmation.
- Reimporting an identical file is a no-op.
- Detect overlapping files using source IDs where available and conservative fingerprints otherwise.
- For ambiguous duplicate candidates, show a review state rather than silently deleting legitimate identical purchases.
- Keep transactions from imports distinguishable from live provider transactions. Reconcile cross-source overlap through explicit review, not automatic fuzzy merging.
- Rollback by batch removes that batch's unique contributions but does not remove records retained by another batch/source or unrelated user work.
- Export UTF-8 CSV with spreadsheet-formula injection protection for text fields.
- Support later historical backfill from issuer exports without duplicating live transactions; reconciliation stays review-based.
- File selection and file I/O happen in Rust through a native open/save dialog; the renderer receives an opaque job ID, counts, and sanitized errors, never an arbitrary path it can read or write.
- Cap import size, row count, field length, and column count. Stream-parse; reject NUL bytes, binary content, invalid UTF-8, and malformed quoting with a bounded error. Do not load the whole file or the whole ledger into the renderer.
- Treat every field as untrusted text. No HTML or markdown rendering; strip control and bidi characters for display; cap description/merchant lengths.
- Export formula-injection protection must cover cells starting with `=`, `+`, `-`, `@`, tab, and carriage return, including after leading whitespace; quote correctly and do not auto-open the exported file.
- Export is plaintext by definition. Warn before writing, default to an owner-only file (0600), use a filename without account numbers, and explain that the export is not protected by SQLCipher.
- Demo mode never accepts a CSV import, provider connection, backup, or portable export of real data.

## 11. Interface specification

### First launch, unlock, and vault

- First launch is empty and shows the account setup wizard rather than demo data.
- Wizard steps: create the database passphrase, optionally add the first account (issuer, display name, last four, type, USD currency, optional credit limit), optionally connect a provider, then finish. Every non-passphrase step can be skipped, and the app remains usable with an empty state.
- The passphrase is collected by a native macOS secure-input sheet (`NSSecureTextField`), never by an HTML form. Creation requires two entries, allows spaces and long passphrases (up to 1,024 characters), and enforces at least 16 characters or an estimated 60 bits of entropy. Show a local strength estimate; do not impose composition rules. Explain before creation that there is no recovery if the passphrase is lost.
- The unlock screen contains a button that opens the same native secure-input sheet. The renderer receives only `unlocked`, `failed`, or `cancelled`; it never sees or stores the passphrase. A wrong passphrase shows a non-destructive error, applies a short exponential delay after repeated failures, and leaves the database untouched.
- Never email, sync, log, or store the passphrase. Do not offer a "remember passphrase" or Keychain-unlock option in version one. Do not put the passphrase in an environment variable, crash report, diagnostic bundle, or support request.
- Make the passphrase-loss warning visible in the wizard, settings, and README. Recovery is only from an encrypted backup whose passphrase is known.
- The app locks on launch, on manual Lock Now, on macOS screen lock, sleep, fast user switching, logout, and quit, and after the configured idle timeout (default 15 minutes). On lock, close database connections, zeroize the session, clear renderer caches and sensitive state, and show the lock screen.
- Apply macOS window content protection (exclude the window from screen capture/sharing where the OS supports it), disable window state restoration, and do not place financial data in window titles, notifications, menus, or saved application state.
- Provide a Security settings section: auto-lock timeout, lock-on-system-event status, content-protection status, provider-credential removal, delete-all-data (crypto-erase), backup/restore, passphrase change, and a plain-language explanation of what is and is not protected. Destructive actions require a native confirmation.
- Demo mode is opt-in from settings, not seeded during real onboarding. It uses a separate, unmistakable database and label, and disables real-data import/export and provider linking.

### Overall appearance

Desktop-first, calm, dense enough for the owner's accounts (roughly ten), with excellent typography and aligned monetary columns. Support light, dark, and follow-system modes with an explicit toggle in the header or settings; persist the choice. Use restrained colors and no decorative charts.

Initial window around 1440 x 960; remain usable at 1100 x 760. Smaller sizes should degrade gracefully with horizontal table scrolling rather than overlapping content.

### Header

- App name `cardigan_money`.
- Connection/sync status and Sync now button.
- Calendar-month navigation, including previous/next and This month.
- Explicit selected month date range.
- Settings entry point, including theme and quit actions.

### Headline metrics

- Selected-month posted net spending.
- Current total card balances, independent of selected month.
- Upcoming reported payments over the next 14 days.

Do not display an ambiguous all-in “amount due” total. Where available, show minimum payments separately from last statement balances; a statement balance is not necessarily the remaining unpaid statement amount. If the provider does not report remaining due, do not imply that it does.

### Cards

Compact grid or expandable list; no carousel hiding accounts. Show an empty state with an Add account action when none exist. Each card includes:

- Issuer, chosen name, last four digits.
- Current balance and balance freshness.
- Selected-month spending.
- Next reported payment due date and clearly identified amount type.
- Last statement issue date.
- Next closing date only if explicitly available.
- Manual-source label where applicable.
- Connection-health indicator.

Clicking a card filters the ledger while preserving the selected month. Provide an obvious clear-filter action.

Do not show “No payment due” merely because metadata is missing. Do not infer autopay status. Omit autopay unless a trusted source or dated manual entry supports it.

### Unified ledger

Columns: reporting date, merchant/description, card, category, status, amount.

Features: search, card/category/status/type filters, sort, pagination, row count, eligible spending subtotal, clear filters, CSV export of the filtered view. Include a separate pending subtotal. Default to newest reporting date first with deterministic tie-breaking.

Side panel: raw description, purchase/authorization date, posted date, reporting-date source, account, type, pending linkage, currency, provenance, merchant/category edits, exclusion, notes.

### Supplier view

Toggle between Transactions and By supplier without resetting month/filters. Show gross purchases, refunds, net spending, transaction count. Clicking a supplier opens its ledger. Merchant normalization merges labels, not transaction records.

### Connection/settings views

Manage connections, review data capabilities, reconnect, add/edit/archive an account, add manual metadata, import/export, encrypted backups/restore, change the passphrase, timezone, scheduling, theme, window-close behavior, auto-lock, provider-credential removal, delete-all-data, and demo mode. Explain what each provider product supplies and what is unknown. Destructive actions require explicit in-app confirmation, and passphrase/backup/export passwords are collected by the native secure-input sheet.

Changing the passphrase uses SQLCipher's supported rekey path with a new random DEK, a pre-change encrypted backup, and a crash-recovery journal. It warns that older encrypted backups still require the old passphrase, never rewrites or deletes them silently, and never leaves a plaintext intermediate file. Verify the old passphrase fails and the new one succeeds before reporting success.

The Settings view must also expose: whether SQLCipher is active (`cipher_version`), the KDF parameters in human-readable form, the last integrity-check result, the last backup result, the last security events, and a plain-language residual-risk statement. Do not show secrets.

All provider/CSV text is rendered as text. Do not use `dangerouslySetInnerHTML`, `innerHTML`, `eval`, `new Function`, or a markdown renderer that emits HTML. Strip control and bidi characters for display. External links are opened only through a Rust allowlist command, never by the webview.

### Empty/error states

Distinct states for locked vault, no accounts, no transactions in month, only pending data, offline, stale account, unsupported metadata, expired credentials, failed import, failed unlock, and demo mode.

Keyboard accessibility, visible focus, descriptive labels, readable contrast, and non-color-only status indicators are required.

## 12. Scheduling and lifecycle

- Default attempted sync windows: 08:00 and 20:00 in the configured timezone. These remain configurable.
- These are best-effort while the app process is running and the vault is unlocked, not guaranteed bank refresh times.
- On launch/resume, sync once if the most recent attempt/success is stale and retry backoff permits it.
- Collapse missed windows into one catch-up attempt.
- Never wake a sleeping Mac merely to sync.
- On the first window close, ask whether to hide the window (keep the process and scheduler running) or quit. Remember the answer, allow changing it in settings, and explain the consequence. Hiding is not a lock: the idle timer keeps running, and screen lock/sleep still lock the vault.
- A normal Quit command must always be available. Full Quit stops scheduled syncing, closes the database, zeroizes the session, and requires the passphrase again at next launch.
- Do not install a LaunchAgent, login item, or persistent helper in version one. The app must not launch at login.
- Do not send macOS notifications in version one. Surface sync and due-date information in-app.
- Use application lifecycle notifications where practical and a coarse clock check as a fallback; no aggressive polling.
- UI displays both the last local fetch and provider-as-of timestamp when available; it does not equate them.
- Mandatory lock events: manual Lock Now (menu and keyboard shortcut), macOS screen lock, sleep, fast user switching, logout, and quit. The app subscribes to the relevant workspace/session notifications and treats a failed subscription as an error shown in Settings, not as a silent pass.
- Configurable idle auto-lock: default 15 minutes, allowed range 1–240 minutes. Disabling it requires a visible warning that the ledger remains available to anyone at the Mac. This setting is stored in the encrypted database and applied by Rust, not by a renderer timer alone.
- On lock: cancel or safely stop in-flight sync, close all DB connections, zeroize DEK/KEK, clear renderer query caches and sensitive UI state, and show the lock screen. Do not advance a sync cursor after a lock.
- Scheduled syncs run only while the vault is unlocked. If the vault is locked, record `skipped: vault locked` and do not attempt a background unlock. This is a deliberate security/availability tradeoff: a short auto-lock makes scheduled sync best-effort.
- Disable macOS window state restoration and enable secure restorable state. Do not persist window screenshots or ledger content in saved application state.
- Document plainly that an open, unlocked window exposes ledger data, that screen locking now locks the vault, and that the idle timeout is the remaining exposure window.

## 13. Security architecture and controls

This section is normative. The review that produced it is [security-review.md](security-review.md). "Must" means release-blocking; "should" requires a documented reason if skipped.

### 13.1 Threat model and trust boundaries

- Protect the ledger at rest, backups, portable exports, provider credentials, and the renderer boundary. The primary adversaries are an opportunistic local user, a stolen disk/backup, malware running as the owner, a malicious website/local server, a malicious imported file or provider payload, a compromised dependency, and a network attacker.
- A compromised macOS account, root/kernel attacker, physical DRAM attack, coerced owner, and compromised Plaid/bank are out of scope. Do not claim protection against them. State this in the README and Security settings. Within that limit, reduce exposure with `mlock`/zeroize, short unlock sessions, and the lock lifecycle; do not claim that these defeat same-user malware.
- The React renderer is untrusted. It never receives the passphrase, DEK, KEK, access tokens, client secret, or raw provider payloads. It has no network, filesystem, shell, or arbitrary URL capability. All privileged work happens in Rust.
- The passphrase, backup/export passwords, and Plaid client secret are collected only by a native secure-input sheet. The renderer receives only success/failure/cancellation.
- The Tauri IPC layer is a validated command gateway, not a generic bridge. Capabilities are scoped to the `main` window; custom commands are allowlisted, typed, bounded, and window/origin checked.
- The app is single-instance. A second instance must not open the database.
- Assume macOS and FileVault are enabled and patched. FileVault protects the disk; SQLCipher protects the ledger independently. Keychain protects tokens, not the ledger.

### 13.2 Cryptographic architecture and fail-closed vault

- Use envelope encryption. At vault creation, generate a random 256-bit DEK. Derive a 32-byte KEK from the passphrase with Argon2id (default 64 MiB, 3 iterations, 1 lane; calibrate to roughly 0.5–1 s on the M1 Max, hard-bound memory/time to prevent a tampered header from causing a DoS). Wrap the DEK with XChaCha20-Poly1305 using a fresh 24-byte nonce. Bind the format version, vault ID, KDF identifier/parameters, salt, nonce, generation, and schema version as AAD.
- Store the KDF parameters, 16-byte salt, wrap nonce, wrapped DEK, vault ID, generation, and timestamps in `vault.header` **outside** the encrypted database. It is 0600, written atomically (temp + fsync + rename + directory fsync), and keeps a mirrored copy of the current generation for corruption recovery. It contains nothing usable without the passphrase, but it is integrity-protected and rollback-detected. Losing every copy of the header makes the ledger unreadable; self-contained backups mitigate this.
- Open SQLCipher with the raw DEK (`PRAGMA key = "x'<64 hex chars>'"`), never with the passphrase. Do not enable `cipher_plaintext_header_size`, do not downgrade `cipher_compatibility`, and do not auto-run `cipher_migrate` or fall back to a legacy KDF on open. Verify the file is not a plain SQLite database.
- Fail closed at startup and on every connection:
  - `PRAGMA cipher_version` and `PRAGMA cipher_provider` are non-empty and match the expected SQLCipher.
  - `PRAGMA cipher_status` is `1` (where supported) and a keyed read of `sqlite_master` succeeds.
  - `PRAGMA key` is the first statement; no schema query, pragma, or pool checkout may run before it.
  - A plain SQLite client cannot open the DB, and a test-only plain-SQLite build is rejected by the app.
  - Run the full `PRAGMA cipher_integrity_check` at unlock, after restore, and after backup creation; it is not a substitute for the key-first check on every connection.
  - Any mismatch closes the connection, zeroizes the key, and reports a sanitized error. Never create a new empty DB at an existing path, reset it, or fall back to plaintext.
- Set `foreign_keys=ON`, `journal_mode=WAL`, `synchronous=FULL`, a finite `busy_timeout`, `secure_delete=ON`, `temp_store=MEMORY`, `trusted_schema=OFF`, SQLite defensive mode, `recursive_triggers=OFF`, and `cipher_memory_security=ON`. Keep extension loading disabled and do not register user-defined SQL functions that can execute commands. Prove by test that WAL and SHM content is ciphertext. Do not assume SQLCipher encrypts SQLite temp files; keep temp storage in memory or prove otherwise.
- The DB stores only audit metadata (`vault_metadata`): vault ID, cipher/provider version, schema version, generation, creation/rotation timestamps, last integrity check. KDF parameters/wrapped DEK live in the external header.
- Wrong passphrase or corrupt header must be non-destructive: no create, rename, reset, overwrite, or plaintext sidecar. A failed unlock may create encrypted SQLite sidecar files; test that the main DB data is unchanged and report the sidecar behavior honestly.
- Passphrase change: make a pre-change encrypted backup; generate a new DEK; rekey the live DB with the SQLCipher rekey API (`sqlite3_rekey`/`PRAGMA rekey`) using the new raw DEK; write a new header/generation; record a recovery journal so a crash between rekey and header replacement is recoverable; verify the old passphrase fails and the new one succeeds. Verify raw-key rekey support in Phase 0. Do not leave a plaintext intermediate.
- Keep the DEK/KEK in Rust memory only while unlocked. `mlock` where practical, zeroize on drop/lock, and never place them in Keychain, the DB, preferences, logs, crash reports, URLs, or the renderer.
- Use only maintained, reviewed cryptographic crates (`argon2`, `chacha20poly1305`, `zeroize`, `secrecy`, `getrandom`, `subtle`). Do not invent a cipher, a KDF, or a MAC. Record test vectors for the header format and key wrapping.

### 13.3 Secret handling, memory, and lock lifecycle

- Passphrase creation, unlock, passphrase change, backup restore, portable export, and Plaid client-secret entry use the native secure-input sheet. The passphrase value is never sent over IPC or off the Mac. The renderer never contains an HTML password field for these values. The sheet's title/explanation are fixed by Rust; the renderer cannot supply or overlay them. Apply content protection to the sheet as well. If the native sheet cannot be built, stop and obtain a written owner risk acceptance; do not silently fall back.
- Preserve the passphrase exactly as typed: do not trim whitespace, change case, or silently Unicode-normalize it. Warn that it is case- and space-sensitive. Enforce at least 16 characters or an estimated 60 bits of entropy for new passphrases, allow spaces and up to 1,024 characters, do not impose composition rules, and show a local strength estimate. No hints, security questions, or "remember me."
- Apply a short exponential delay after repeated wrong passphrases. Do not implement a permanent lockout that enables denial of service or data loss.
- Zeroize passphrases, KEKs, DEKs, tokens, and temporary plaintext buffers. Use secret wrapper types without `Debug`/`Clone`/`Serialize`. Do not format secrets into error strings.
- Disable core dumps (`setrlimit(RLIMIT_CORE, 0)`), install a panic hook that emits no sensitive data, do not ship debug symbols or `RUST_BACKTRACE` behavior, and do not use third-party crash reporting. Document that macOS Crash Reporter may still record a backtrace and that no heap contents are intentionally included.
- Keychain: provider tokens and the Plaid client secret only; `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`, `kSecAttrSynchronizable=false`, data-protection Keychain, one item per connection, labels containing only the local connection UUID (no account numbers), no file fallback. Delete on disconnect; provide "remove all provider credentials"; never include Keychain values in backups/exports. If Keychain is unavailable or the ACL is denied, fail closed for that provider operation and explain; the local ledger remains usable offline.
- Lock on launch, manual Lock Now, macOS screen lock, sleep, fast user switching, logout, quit, and the configured idle timeout (default 15 minutes; 1–240 minutes; disabling requires a visible warning). On lock: stop sync safely without advancing the cursor, close all DB connections, zeroize keys, clear renderer query caches and sensitive UI state, and show the lock screen.
- Hiding the window is not a lock. The idle timer continues while hidden. Scheduled sync requires an unlocked vault and records a skip reason; there is no background unlock and no key in Keychain. Use a native activity/session source (or a Rust timer reset only by validated activity) for idle detection; do not rely solely on renderer-reported activity.
- Disable macOS window state restoration and enable secure restorable state. Use `set_content_protected(true)` on the main window and native secure-input sheet where supported, and document that cameras, some remote-support tools, or a compromised window server may still capture the screen.

### 13.4 Renderer and IPC hardening

- Maintain an explicit command inventory. Every command has a typed request/response, `deny_unknown_fields`, length/range/enum bounds, a window-label/origin guard, and a rate limit where abuse is possible. No generic `query`, `execute`, `read_file`, `write_file`, `open_url`, `http_request`, `shell`, or "run this SQL" command.
- File import/export, backup, restore, and portable export are Rust-owned jobs: the native dialog and file I/O happen in Rust; the renderer receives an opaque job ID, counts, progress, and sanitized errors, never an arbitrary path.
- Validate every argument in Rust even when Zod validates in the frontend. Reject oversized payloads (search, notes, descriptions, page sizes, date ranges, IDs) and unknown fields. Money is an integer minor-unit type, never `f64`; dates are validated date-only types; identifiers are UUIDs.
- Errors crossing IPC are stable codes plus a safe message. Do not include SQL, paths, tokens, account numbers, or provider payloads. Log full details only in a redacted debug path that is disabled in release.
- Events go only to the `main` window, contain only the minimum data needed, and stop on lock. Do not broadcast financial payloads.
- The renderer must not store financial data or secrets in `localStorage`, `sessionStorage`, IndexedDB, Cache API, service workers, or URL query strings. Query caches are memory-only and cleared on lock. On lock, unmount sensitive views and reset filters/search.
- No `dangerouslySetInnerHTML`, `innerHTML`, `eval`, `new Function`, remote scripts, remote styles, remote images, remote fonts, iframes, or markdown-to-HTML. Render provider/CSV text as text; strip control and bidi characters for display and cap display length.
- Block navigation to anything other than the app origin; deny new windows; disable drag-and-drop file handling; disable devtools and context-menu devtools in release.
- The mock bridge is development/test only, behind a compile-time feature, and cannot be selected in a release build. The demo provider is explicit and visibly labeled.
- No print, "save as HTML," or hidden export path may bypass the reviewed export flow.

### 13.5 Tauri configuration baseline

- CSP (release) must have no HTTPS source and no `unsafe-eval`: `default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; frame-src 'none'; worker-src 'none'; manifest-src 'none'; media-src 'none'`. Replace `'unsafe-inline'` styles with nonces/hashes if the frontend permits. Tauri's injected nonces/hashes must remain enabled; never set `dangerousDisableAssetCspModification`.
- Use a separate, equally strict `devCsp`; do not reuse the production policy for the Vite dev server or allow remote origins in dev. Development servers are local-only and must not be exposed to untrusted networks.
- One capability file for the `main` window only. Do not grant `fs:*`, `shell:*`, `http:*`, `opener:*`, `sql:*`, `store:*`, `process:*`, `updater:*`, `websocket:*`, `upload:*`, `clipboard:*`, `notification:*`, `deep-link:*`, `autostart:*`, `global-shortcut:*`, or renderer-side `dialog:*` permissions. Use only the specific core event/window permissions the shell needs; native dialogs and file I/O are opened from Rust, so the renderer never receives a path. Never enable remote capabilities or remote IPC access.
- `freezePrototype: true`, `withGlobalTauri: false`, asset protocol disabled, no updater plugin, no shell plugin, no HTTP plugin, no SQL/store plugin. No `cardigan_money://` scheme and no deep-link plugin unless a verified provider flow requires it (Plaid does not support custom schemes).
- Single window label `main`; navigation allowlist only the app origin; new windows denied. Disable devtools in release and do not ship a debug build.
- Enforce single-instance before opening the database. On a second launch, focus the existing window and exit without touching the DB.
- Call `set_content_protected(true)` on the main window at setup and re-apply after theme/window recreation.
- Disable state restoration and saved application state. Do not persist ledger content in window titles, menus, tooltips, notifications, or restore snapshots.
- Capability and CSP files are tested: parse them, assert no forbidden permissions, and assert the release CSP contains no `https:`/`http:` source other than the IPC endpoint.

### 13.6 Network and provider security

- All network calls are in Rust. The renderer has no HTTPS source in its CSP, so it cannot exfiltrate data even if an XSS is found. There are no remote images, fonts, scripts, analytics, or telemetry.
- `reqwest` (or the chosen maintained client) verifies TLS, refuses plain HTTP, disables redirects, sets connect/read timeouts, caps response and decompressed size, and uses an exact host allowlist. Do not disable certificate verification or add a release "trust all" mode. Do not fetch URLs supplied by provider payloads, institutions, or imported files.
- Provider responses are untrusted: typed parsing, required-field checks, string/collection/amount/date/currency bounds, no raw payload retention, and no provider-supplied URLs. Reject malformed data rather than coercing it.
- No inbound HTTP listener, webhook endpoint, or local server in version one. If a future feature requires one, it needs a new security review.
- Plaid linking must use a currently documented desktop flow. Current documented constraints include HTTPS redirect URIs, no custom URI schemes, localhost redirects only in Sandbox, and a deprecated Link Webview SDK. The preferred no-server flow is Hosted Link with `/link/token/get` polling; the alternative is a documented hosted HTTPS page. If neither is verified, live linking is unavailable.
- Link tokens, public tokens, access tokens, and the client secret stay in Rust/Keychain, are zeroized after use, and never appear in renderer state, logs, URLs the app controls, or error messages. The Hosted Link URL is a short-lived bearer opened through a Rust command that accepts only the exact verified Plaid host; the renderer never supplies the URL. Never log/persist it, minimize its lifetime, and document browser-history exposure.
- If an embedded webview is ever approved: dedicated locked-down webview, no app IPC, no preload, no devtools, navigation only to exact Plaid origins, new windows denied, storage cleared after completion, and no vault/Keychain access. Do not assume WKWebView is supported.
- If any callback exists: state/nonces and PKCE where supported, exact redirect-URI match, origin validation, one-time consumption, short TTL, constant-time comparison, and replay rejection. No unauthenticated permanent local server.
- Rate limits: jittered backoff, finite retry budget, respect 429, no blind retry of non-idempotent token exchange, no automatic billable refresh, no duplicate connections during testing.
- Logging: never log request/response bodies, authorization headers, tokens, account numbers, or financial payloads. Request IDs without account data are acceptable.
- On disconnect, delete the Keychain item and offer explicit remote Item removal. Do not silently revoke remotely; do not delete local records on a remote failure.

### 13.7 Local storage, files, logging, and crash artifacts

- App data lives under the Tauri application-data directory, not in the repository or a synced folder. Create directories 0700 and files 0600; verify at startup and fail closed if the expected path is a symlink or not owned by the owner. Use `O_NOFOLLOW`/`symlink_metadata` and reject path traversal.
- Write atomically: temp file in the app data directory, fsync, rename, fsync the directory. Handle disk-full and partial-write errors without destroying the previous good file.
- Temp files must be 0600, randomly named, in the app data directory, and cleaned on startup. Never create a plaintext database, a plaintext decrypted export, or a debug dump. A temp file containing SQLCipher ciphertext is acceptable; a temp file containing plaintext ledger rows is not.
- Real CSV exports, database copies, backups, and screenshots must never be committed. Keep them outside the repository, add them to `.gitignore`, and run a secret/PII scan before handoff. Demo fixtures are synthetic only.
- The app must not request macOS TCC permissions such as Full Disk Access, Accessibility, Screen Recording, Camera, Microphone, Contacts, Calendar, or Automation. Version one needs none; file access is user-initiated through native dialogs. Any future TCC request requires a security review.
- Keep logs off by default. If diagnostics are enabled, use a redacting logger with no financial data, secrets, paths, or SQL parameters, rotate and cap the file, and make the mode explicit and local. Never enable SQLCipher `cipher_log` at DEBUG/TRACE in a release.
- Disable core dumps, avoid backtrace-rich panic output, and do not add Sentry/Crashlytics/analytics. Document the macOS Crash Reporter limitation.
- Disable state restoration and secure restorable state as described in §13.5. Consider excluding the DB/backup directory from Spotlight indexing; do not exclude it from Time Machine if encrypted backups are desired. Warn against placing exports in iCloud Drive or other synced folders.
- Use one process. A second instance focuses the first and exits. Do not open the DB before the single-instance lock is held.
- No plaintext ledger page cache is written by the app. SQLite temp store is memory; verify WAL/SHM ciphertext; `secure_delete=ON` reduces recoverable deleted plaintext inside the DB, though SSD remanence remains a documented limit.

### 13.8 Backup, restore, and key rotation security

- Use a consistent SQLCipher snapshot via the SQLite backup API or `sqlcipher_export` into a destination encrypted with the DEK. Checkpoint WAL first; never copy only the main DB file during WAL activity. Verify the snapshot's integrity before declaring success.
- The backup container is authenticated and self-contained: magic/format version, vault ID, cipher/provider, KDF parameters and salt, wrap nonce, wrapped DEK, generation, schema version, backup kind, created_at, DB length, DB ciphertext SHA-256, and the SQLCipher DB bytes. Bind the header fields as AAD. The header is not secret, but tampering, truncation, or DB-swapping must fail closed.
- Automatic backups need no extra passphrase prompt because the header already wraps the DEK. Restore always requires the passphrase. Do not retain the passphrase for the session solely to make backups work.
- Restore: native file picker and native passphrase sheet; cap header/DB sizes; parse and validate the header; verify `db_len` and `db_sha256` against the bytes before opening; derive/unwrap; open the candidate read-only with `trusted_schema=OFF`, defensive mode, and `query_only`; verify vault ID, `cipher_integrity_check`, `integrity_check`, schema compatibility, and `foreign_key_check`; make an encrypted pre-restore backup; acquire exclusive access; atomically replace the DB and remove stale WAL/SHM; fsync; reopen; on any failure keep the old DB.
- Passphrase change rotates the DEK and rekeys the DB. Old backups keep their old header/wrapped DEK and require the old passphrase. Warn before rotation; do not rewrite or delete old backups silently. Provide an explicit "delete old backups" action with confirmation, and explain that a compromised old passphrase compromises them.
- Crypto-erasure/delete-all: destroy the wrapped DEK, the vault header, and every backup; document that SSD wear-leveling and copies outside the app are outside the app's control. Do not claim physical secure erase.
- Portable export uses a separate owner-chosen passphrase, the same authenticated container/key-wrapping discipline (or an age outer layer over a self-contained container), and no provider tokens. Warn that it is a full financial history; enforce a strong export passphrase; never write a plaintext SQLite file.
- Retention: 7 daily backups plus pre-migration/pre-restore backups with a documented cap. Prune only files matching the app's strict naming pattern, never symlinks, never the last verified backup. Verify after creation; delete a failed backup and keep the previous.
- Tests must cover wrong passphrase, truncated/tampered header, tampered DB page, wrong vault ID, schema downgrade, nonempty restore, pre-restore rollback, rekey crash recovery, and absence of plaintext temp files.

### 13.9 Import/export security

- Import and export are Rust-owned native file jobs. The renderer never receives an arbitrary path and cannot read or write files directly.
- Import caps: file size, row count, column count, field length, and parse time. Stream-parse; reject NUL/binary/invalid UTF-8; handle quoted commas/newlines and BOM; fail with a bounded error. Do not load the entire file or ledger into the renderer.
- Treat all fields as text. No HTML/markdown rendering; strip control/bidi characters for display; cap lengths. Ambiguous duplicates go to review, never automatic deletion.
- Export is plaintext. Warn before writing; default 0600; use a filename without account numbers; explain that SQLCipher does not protect it and that cloud-synced folders are outside the app's control.
- Formula-injection protection covers cells beginning with `=`, `+`, `-`, `@`, tab, or carriage return (including after leading whitespace), with correct quoting. Do not auto-open the exported file.
- Demo mode never imports real CSV, links a provider, creates backups, or exports real data.

### 13.10 Supply chain, build integrity, and signing

- Commit lockfiles; use `pnpm install --frozen-lockfile` and `cargo build --locked`; pin the Node/pnpm/Rust toolchain (`packageManager`, `rust-toolchain.toml`). Document resolved versions.
- Run `cargo audit`, `cargo deny check` (advisories, bans, sources, licenses), and `pnpm audit` at Phase 0 and before release. Only official Tauri plugins; minimize dependencies; no git/path dependencies except the workspace; no unvetted postinstall scripts (`ignore-scripts` with an explicit allowlist).
- No remote assets/CDNs/fonts. The frontend bundle must be reproducible from the lockfile; disable production source maps and debug symbols unless needed for local diagnosis.
- Build on a trusted, patched Mac from a clean checkout, in release mode, with devtools disabled and the mock bridge compiled out. Do not use production provider secrets during development. Record the artifact SHA-256 and the SQLCipher provider/version. Verify the signature/hash before running.
- The unsigned build is acceptable only for personal use. Ad-hoc sign the app and nested binaries (`codesign --force --sign -`), verify with `codesign --verify --deep --strict`, and document Gatekeeper/tamper limitations. Use a stable local self-signed identity for development so Keychain ACL behavior is predictable. Do not claim notarization, hardened runtime, or distribution readiness.
- Keep entitlements minimal. Do not add `get-task-allow`, `allow-unsigned-executable-memory`, `disable-library-validation`, `allow-dyld-environment-variables`, or `allow-jit` to a release build. Do not request TCC permissions.
- No auto-update in version one. Rebuilding is the update path; verify source and hash. If a future updater is added, it needs its own signature/verification design.
- Never place secrets in `.env`, `VITE_*` variables, build scripts, shell history, or the repository. The Plaid client secret is entered natively and stored in Keychain. `.env.example` contains placeholders only, and the README warns that `VITE_*` values are public.
- Provide an SBOM or dependency manifest with versions and hashes if practical; at minimum, record the locked dependency set.

### 13.11 Demo isolation

- Demo mode uses a separate database path and a distinct code path; it never opens, migrates, backs up over, restores over, or writes to the real ledger. The real DB path is not reachable from demo mode.
- The demo key is app-managed and may be fixed/hard-coded because the data is synthetic; document that this is not protection for real data. Never put real data in the demo DB.
- Demo mode disables CSV import, provider linking, backup creation, restore, and portable export. It may provide a "reset demo" action that recreates the synthetic fixtures.
- Show a persistent, unmistakable demo label and window title suffix. Demo data must never be presented as live.
- Tests: demo cannot open the real DB, cannot import a real CSV, cannot read Keychain, and cannot write outside its directory.

### 13.12 Security testing and assurance

- Negative vault tests: plain-SQLite build rejected; wrong passphrase; tampered header; tampered DB page; truncated header; invalid KDF parameters; header/DB vault-ID mismatch; missing header; ciphertext WAL; no plaintext temp; core-dump policy.
- Renderer tests: CSP has no HTTPS source; Playwright observes zero external requests; navigation/new-window blocked; devtools disabled in release; unknown IPC command rejected; oversized/malformed payloads rejected; window-label check; rate limits; no secret in DOM/localStorage/sessionStorage/URL/error.
- Network tests: non-allowlisted host rejected; redirect rejected; TLS failure rejected; oversized response rejected; token never logged; sync skipped while locked; no network while locked.
- Backup tests: tamper/truncate/wrong passphrase/wrong vault ID/schema downgrade; nonempty restore; pre-restore rollback; rekey crash recovery; no provider tokens; no plaintext.
- Import/export tests: formula-injection variants; BOM/quotes/parentheses; invalid UTF-8/NUL; huge file; path traversal/symlink; 0600; no auto-open.
- Fuzz/property tests for CSV, provider JSON, and IPC structs. Use bounded, deterministic inputs; do not fuzz against live provider credentials.
- Native smoke tests on the M1 Max: native passphrase sheet, Keychain attributes/ACL, screen-capture protection, screen-lock auto-lock, state restoration, crash logs, file dialogs, and devtools absence. Document what cannot be automated.
- Dependency audit output, capability/CSP assertions, and artifact SHA-256 are release evidence.
- Independent review before release: a reviewer other than the implementer walks the threat model and this checklist; unresolved High findings require written owner acceptance.

### 13.13 Security events and incident response

- Record minimal security events in the encrypted DB: unlock success/failure counts, lock, backup created/verified/failed, restore started/succeeded/failed, export, rekey, credential added/removed, integrity failure, and single-instance rejection. No secrets, tokens, account numbers, or financial payloads. Show recent events in Settings.
- Incident runbook, documented in `SECURITY.md` and the README:
  - Suspected passphrase compromise: lock, change the passphrase (rotate DEK), revoke/relink Plaid Items, delete old backups that used the old passphrase, review security events, rotate the Plaid client secret.
  - Suspected access-token leak: delete the Keychain item, revoke the Item in Plaid, relink; do not reuse the old token.
  - Lost/stolen Mac: rely on FileVault and the vault passphrase; revoke Plaid Items from another device; do not claim remote wipe.
  - Lost/stolen backup: assume offline attack; rotate credentials; delete old backups; change the passphrase; monitor accounts.
  - Corrupt DB: preserve a copy, restore from a verified encrypted backup, do not run repair tools on the only copy.
  - Compromised dependency: remove/rebuild, rotate all secrets, review network/export events, verify artifact hash.
- Do not promise recovery the design cannot provide. If the passphrase and every backup passphrase are lost, the data is unrecoverable.

### 13.14 Residual risks and non-guarantees

- A compromised macOS account or root/kernel attacker can read the unlocked process; `mlock`/zeroize are best-effort, not a defense against a hostile OS.
- Screen capture is not fully preventable. Content protection blocks the standard path; cameras, some remote-support tools, and a compromised window server may not be blocked.
- The passphrase is a single point of failure. Weak/reused passphrases defeat Argon2id; lost passphrases are unrecoverable.
- Old backups remain decryptable with the old passphrase by design. A compromised old passphrase compromises them until they are deleted.
- Exported CSV is plaintext and outside the app's control after writing. Cloud sync, sharing, and later copying are the owner's responsibility.
- Hosted Link places a short-lived bearer URL in the system browser. The app minimizes lifetime and avoids logging, but browser history/extensions are outside its control.
- Unsigned/ad-hoc builds can be tampered with. There is no auto-update, no notarization, and no hardened runtime; verify source and artifact hash.
- A compromised renderer can still invoke allowed commands and ask the user to unlock. Native input prevents silent passphrase capture, not all post-unlock abuse; CSP, no remote content, and the command allowlist are the defenses.
- TLS depends on the macOS trust store and proxy configuration. The app does not implement certificate pinning; a compromised trust store or privileged proxy can intercept provider traffic even with TLS verification.
- An attacker who can replace both `vault.header` and the database with an older consistent pair can roll the ledger back. The app detects header/DB mismatches, not a full rollback without external monotonic state.
- Swap/hibernation is protected by FileVault, not by the app. `mlock` is best-effort and can fail under memory pressure.
- Security events are encrypted but are not tamper-evident to an attacker who has the vault key and write access.
- SQLCipher, SQLite, WebKit, macOS, and dependency vulnerabilities are inherited. Patching is part of operation.
- Crypto-erasure is logical, not physical; SSD remanence and copies outside the app are not controllable.
- Auto-lock and scheduled sync trade off: a locked vault cannot sync. The owner chooses the idle timeout and accepts the availability consequence.
- No hardware-backed key, Touch ID/Face ID vault unlock, MFA, multi-user separation, or App Sandbox. These are out of scope, not implicit protections.

## 14. Backup and recovery

- Use SQLite's supported backup mechanism or SQLCipher's supported export path to obtain a consistent encrypted snapshot; never copy only the main DB file during WAL activity. Checkpoint WAL first where practical, and verify the snapshot with `cipher_integrity_check`/`integrity_check` before reporting success.
- Make a pre-migration backup and a daily rolling backup when the app runs. Retain 7 daily backups plus pre-migration and pre-restore backups, with a documented cap. Prune only app-created files that match the strict naming pattern, never symlinks, and never the last verified backup.
- Each backup is a self-contained authenticated container: format version, vault ID, cipher/provider version, KDF identifier/parameters and salt, wrap nonce, wrapped DEK, generation, schema version, backup kind, creation time, DB length, DB ciphertext SHA-256, and the SQLCipher DB bytes. Header fields are bound as AAD. A backup is useless without its header, so header and DB bytes travel together; restore verifies the length and hash before opening.
- Encrypt automatic backups, including pre-migration and pre-restore backups, under the current key hierarchy. The DB snapshot is encrypted with the current DEK; the header carries the wrapped DEK copy. Because the app is unlocked while running, do not add a second prompt for automatic backup creation. Restore always asks for the passphrase.
- Store backups in the application data directory with 0600 files and 0700 directories; never in the source repository. Do not include provider secrets or Keychain material. Restored connections may require relinking.
- On passphrase change, generate a new DEK, rekey the live database, write a new header/generation, and keep a recovery journal so a crash between rekey and header replacement is recoverable. Future backups use the new DEK. Do not rewrite or delete older backups silently; clearly label that they require the old passphrase. Provide explicit deletion with confirmation for compromised old backups.
- Portable export (separate from automatic backup) uses an owner-chosen export passphrase collected by the native secure sheet and the same authenticated-container/key-wrapping discipline, or an age outer layer over a self-contained container. Enforce at least the same 16-character/60-bit policy, never store the password, and allow it to differ from the ledger passphrase. Never write a plaintext SQLite file, and never include provider secrets. Warn that the export is a full financial history and that a lost export password is unrecoverable.
- Restore: native file picker and native passphrase sheet; cap sizes; parse/validate the header; derive/unwrap; open the candidate read-only with `trusted_schema=OFF`, SQLite defensive mode, and `query_only`; verify vault ID, `cipher_integrity_check`, `integrity_check`, schema compatibility, and `foreign_key_check`; create an encrypted pre-restore backup; acquire exclusive access; atomically replace the DB and remove stale WAL/SHM; fsync; reopen. If any step fails, keep the current database and report a sanitized error.
- A restore must never run triggers, views, or user-defined functions from the candidate database. It may read only the expected tables. Reject a schema newer than the app supports and require an upgrade path.
- Crypto-erasure/delete-all locks and closes the vault first, then destroys the wrapped DEK, the vault header, every backup, and the associated Keychain provider credentials after explicit confirmation. Document that SSD wear-leveling and copies outside the app are outside the app's control. Do not promise physical secure erase.
- Document recovery from a corrupt database, a failed migration, a lost header, a lost passphrase, and an unreadable backup. Do not promise recovery that the encryption design cannot provide.
- Tests must prove that backup files and WAL/SHM are not readable as plaintext SQLite without the passphrase; wrong-passphrase restore fails safely; tampered/truncated headers and DB pages are rejected; a nonempty database restores; a failed restore leaves the original intact; and rekey crash recovery is deterministic.

## 15. Demo fixtures

Demo mode is opt-in from settings and never runs during real onboarding. Provide a reproducible demo mode with an unmistakable persistent label. Seed approximately ten cards across five fictionalized institutions/accounts, including representative Chase, Synchrony/Amazon, and Bilt labels without implying verified live coverage.

Demo data lives in a separate database from the real ledger. It contains no personal data and is never seeded into, merged with, or restored over a real ledger. Because it is synthetic, the demo database may use a fixed app-managed key and skip the owner passphrase; document that this is not protection for real data. Never put real data in the demo database. Demo mode disables CSV import, provider linking, backup creation, restore, and portable export. It may offer a "reset demo" action that recreates the synthetic fixtures. The demo database path and real ledger path are distinct code paths; demo mode must be incapable of opening the real ledger.

Use a configurable/frozen clock for deterministic tests. Seed the selected month and previous two months, with at least 150 transactions covering:

- Multiple cards under one institution login.
- Month-boundary authorization versus posting dates.
- Pending transactions and linked posted replacements.
- Same-amount, same-merchant purchases that are legitimately distinct.
- Refunds received in a later month.
- Payments and transfers.
- Interest and fees.
- Merchant aliases across cards.
- Missing statement/due fields and dated manual values.
- A stale account, a reconnect-required account, and a partial sync failure.
- An account with no payment due explicitly reported, distinct from unknown metadata.
- A non-USD transaction/account displayed separately, even though the owner's real data is USD-only.

Never seed demo data into a real ledger. Demo mode must not request credentials or connect to a provider.

## 16. Required tests and acceptance criteria

### Financial/domain tests

- Calendar-month boundaries, year changes, leap day, and New York DST without date shifts.
- Pending exclusion and posted replacement without double-counting.
- Payments/transfers excluded; fees included and identified.
- Refunds attributed to the received month.
- Supplier/category totals reconcile to eligible ledger rows.
- Exact cent arithmetic and source sign normalization.
- Separate currencies remain separate.
- Historical month changes do not alter current balances/obligations.
- Unknown metadata never becomes zero/no-payment-due.
- Statement balance is never presented as confirmed remaining due.

### Vault/encryption tests

- Creating a vault, reopening it with the correct passphrase, and rejecting a wrong passphrase without modifying the file.
- The database file, WAL, SHM, and backups are not readable as plaintext SQLite and are not opened by a normal SQLite client without the key.
- A test-only plain-SQLite build is rejected: `cipher_version`/`cipher_provider` fail closed and the app refuses to create or open a ledger.
- `PRAGMA key` is first on every connection; a connection that queries before keying is impossible by construction and covered by a test.
- `vault.header` is 0600, atomic, redundant, authenticated, rollback-detected, and required for unlock; a missing/tampered/truncated header fails safely.
- KDF parameter bounds: absurd memory/time parameters are rejected before a long derivation (DoS resistance); unknown/legacy KDF identifiers are rejected rather than silently downgraded; a weaker but still-supported Argon2id parameter set may be upgraded only after a successful unlock. No automatic `cipher_migrate`; extension loading is disabled.
- Header/DB binding: a header that does not match the database's vault ID/generation fails safely. A full rollback to an older consistent header+DB pair is documented as a residual risk, not claimed as detected.
- Correct passphrase enters through the native secure-input path; the renderer never receives it. Wrong passphrase applies the configured delay and leaves data intact.
- Passphrase change/rekey: the old passphrase fails, the new one succeeds, old backups require the old passphrase, and a crash during rekey recovers deterministically.
- Encrypted backup creation, retention pruning, and restore with the correct passphrase; restore with a wrong passphrase, wrong vault ID, tampered header, truncated DB, or newer schema fails safely.
- No plaintext temporary database, decrypted export, or debug dump is produced during normal backup, restore, migration, or query paths.
- Demo and real databases cannot be mixed, and demo mode never touches the real ledger, Keychain, or real-data import/export.
- `cipher_memory_security`, `temp_store=MEMORY`, `secure_delete`, `trusted_schema=OFF`, defensive mode, and WAL encryption are configured and verified.
- Lock closes all connections, zeroizes session material, clears renderer caches, and leaves no usable key behind.

### Database/sync tests

- Repeating a sync yields the same result.
- Pagination and pending/posted records across pages.
- Cursor rollback after a failed write.
- Add/modify/remove behavior and retained user overrides.
- Concurrent manual/scheduled triggers do not duplicate work.
- One failed connection does not block healthy connections.
- Partial liability failure preserves previous values and marks staleness.
- Reconnection/account mapping does not silently duplicate accounts.
- Migration and encrypted backup/restore on a nonempty database.
- Scheduled sync skips safely when the vault is locked or the app is quitting.

### Import tests

- BOM, quoted commas, parentheses, split debit/credit columns, and ambiguous dates.
- Exact-file reimport is a no-op.
- Overlap detection does not delete legitimate identical transactions.
- Batch rollback preserves shared/unrelated records.
- Spreadsheet-formula injection protection in exports.
- Large historical files can be previewed and imported without loading everything into the renderer.

### Security tests

- Release CSP contains no `https:`/`http:` source except the IPC endpoint; Playwright observes zero external requests; navigation to a remote origin and `window.open` are blocked; devtools are absent in release; `freezePrototype` is on; `withGlobalTauri` is off; asset protocol is disabled.
- Capability files parse and contain only the `main` window plus the minimum core/dialog permissions; no fs/shell/http/opener/sql/store/process/updater/websocket/upload/clipboard/deep-link/notification permissions.
- IPC fuzz: unknown command, oversized payload, wrong type, extra fields, path/URL arguments, out-of-range IDs/dates/amounts, and repeated calls are rejected with sanitized errors. Window-label/origin guard is exercised.
- Secret sentinel test: a known fake passphrase and token never appear in the DOM, `localStorage`, `sessionStorage`, URL, logs, error messages, or crash artifacts. The renderer cannot read a passphrase field because none exists.
- Network tests: non-allowlisted host, redirect, plain HTTP, TLS failure, and oversized/decompression-bomb response are rejected. No network occurs while locked. Tokens are absent from logs and errors.
- Provider-link tests: state/nonce mismatch, replay, expiry, and wrong redirect URI are rejected; no custom scheme or loopback listener exists in the Production path; Hosted Link URL/link token is never logged or persisted.
- CSV tests: formula-injection variants (`=`, `+`, `-`, `@`, tab, CR, leading whitespace), invalid UTF-8, NUL, huge row/field, quoted newlines, path traversal/symlink, and export 0600; the file is not auto-opened.
- Filesystem tests: 0700/0600 enforcement, symlink rejection, atomic write/rename, disk-full behavior, temp cleanup, and single-instance rejection without DB access.
- Lock tests: manual, screen lock, sleep, fast user switching, idle timeout, and quit close connections and clear renderer caches; in-flight sync does not advance the cursor; scheduled sync records `skipped: vault locked`.
- Native smoke tests: content protection against screen capture/sharing for both the main window and the secure-input sheet; Keychain accessibility/ACL; state restoration; core-dump setting; devtools absence; native secure-input accessibility with VoiceOver; exact passphrase handling (leading/trailing spaces preserved); and confirmation that no unexpected TCC/entitlement is requested.
- Build/entitlement tests: inspect the signed bundle's entitlements and verify no `get-task-allow`, unsigned-executable-memory, library-validation bypass, dyld-environment, or JIT entitlement is present in release; verify the ad-hoc signature and recorded hash.
- Dependency/security gates: `cargo audit`, `cargo deny`, `pnpm audit`, lockfile/frozen-install, no unapproved postinstall scripts, and no secret in the repository.
- Fuzz/property tests for CSV, provider JSON, and IPC structs; bounded inputs, no live credentials.

### UI tests

- First-launch wizard, native passphrase creation/confirmation, unlock success/failure/cancel states, and confirmation that the renderer never receives the passphrase.
- Account creation with no provider, and an empty state before any account exists.
- Month navigation, search, card filtering, supplier drill-down, edits, and persistence.
- Empty/pending-only/stale/error states.
- Summary values match backend-provided fixture expectations.
- Theme toggle persists light/dark/system choices.
- First-close prompt remembers hide/quit and can be changed in settings; hiding is explained as not a lock.
- Lock Now, auto-lock settings, security status (cipher/KDF/backup), credential removal, and delete-all-data confirmation.
- Full keyboard navigation for primary workflows, including the native passphrase sheet.
- Distinct demo label; no credentials requested, no real-data import/export, and no provider connection in demo mode.
- No HTML injection: a transaction description/merchant containing markup is displayed as text, not rendered.

Do not assume browser Playwright tests prove native Tauri functionality. Run browser tests through a mock bridge and perform/document native macOS smoke tests for startup, passphrase unlock, file dialogs, Keychain, window lifecycle, theme application, and actual IPC. Clearly state any native automation limitations.

### Performance targets

On the user's Mac, aim for warm local searches/month changes under 200 ms and startup showing cached data within roughly two seconds after unlock, using a 50,000-transaction synthetic dataset. Measure and report results rather than claiming the targets were met without profiling. Avoid loading the entire ledger into the renderer. Since SQLCipher adds overhead, measure with encryption enabled rather than an unencrypted prototype.

## 17. Suggested delivery milestones

1. Repository inspection, dependency selection, threat model, crypto design, SQLCipher fail-closed spike, Tauri CSP/capability baseline, dependency audit, Plaid desktop-flow verification, and a request for real CSV samples.
2. Tauri app boots; typed command gateway, native secure input, envelope vault, passphrase creation/unlock, lock lifecycle, migrations, onboarding wizard, and demo isolation work.
3. Calendar-month accounting, query/input bounds, and domain tests pass.
4. Overview/cards, transaction table, supplier drill-down, edits, theme toggle, content protection, and security settings are functional.
5. Generic CSV import/export with security controls, encrypted backup/restore, rotation/crypto-erase, and retention behavior are functional; security tests pass.
6. Plaid transaction/balance adapter and the verified linking flow implemented where credentials/access permit; Liabilities only where verified.
7. Scheduling, reconnect flow, security hardening, dependency audit, and accessibility review.
8. Independent security review, adversarial tests, release build, native smoke tests, artifact hash, and complete handoff documentation.

Do not allow a live-provider blocker to prevent delivery of the functioning offline/demo/CSV application.

## 18. Required handoff

Deliver:

- Complete source, migrations, deterministic fixtures, and lockfiles.
- README with prerequisites, exact development commands, tests, Apple Silicon build commands, and how to open the app. Include the passphrase-loss warning, the exact backup/restore procedure, the lock lifecycle, and the security limitations.
- `.env.example` containing placeholders only if environment configuration is needed; provider secrets should ultimately be enrolled into Keychain. Never put the ledger passphrase or provider secrets in environment files. Warn that `VITE_*` values are public and must never contain secrets.
- `SECURITY.md` with the threat model summary, security controls, incident runbook, residual risks, and reporting contact.
- `docs/threat-model.md`, `docs/security-architecture.md` (key hierarchy, header format, KDF parameters, backup container, AAD, rotation journal, test vectors), and `docs/security-test-report.md`.
- Provider setup guide covering the Plaid Trial signup checklist, required products, the verified desktop linking flow, verified vs unverified capabilities, Liabilities caveats, trial/billing limits, and reconnect behavior.
- Data dictionary and the calendar-month accounting rules, including encryption, vault, header, and backup fields.
- Backup/restore and troubleshooting instructions, including lost passphrase, lost header, corrupt database, failed rekey, failed migration, and schema incompatibility.
- `docs/implementation-status.md` listing completed work, tests actually run, measured performance, security findings and their status, outstanding blockers, and live institutions actually verified.
- A dependency manifest/SBOM or locked dependency list, the SQLCipher provider/version, and the artifact SHA-256. If an artifact is built, ad-hoc sign it and verify the signature.
- A local development/release artifact when the environment permits it. Signing/notarization for distribution is separate; do not claim the ad-hoc signed local build is distribution-ready.
- Native smoke-test evidence for the secure-input sheet, Keychain attributes/ACL, content protection, screen-lock auto-lock, state restoration, and devtools absence.

At completion, give a concise summary with run instructions, tests actually executed, measured performance, security limitations, and remaining limitations. Never report the application as fully live-connected unless it has been verified with authorized real credentials. Never describe the app as "watertight," "unhackable," or "military-grade"; state the residual risks.

## 19. Official documentation starting points

Verify current documentation while implementing; these are starting points, not a guarantee of current provider behavior or coverage.

- Tauri: https://v2.tauri.app/
- Tauri security: https://v2.tauri.app/security/
- Tauri CSP: https://v2.tauri.app/security/csp/
- Tauri capabilities: https://v2.tauri.app/security/capabilities/
- Tauri lifecycle threats: https://v2.tauri.app/security/lifecycle/
- Tauri configuration reference: https://v2.tauri.app/reference/config/
- SQLite WAL: https://sqlite.org/wal.html
- SQLite backup API: https://sqlite.org/backup.html
- SQLCipher: https://www.zetetic.net/sqlcipher/
- SQLCipher API: https://www.zetetic.net/sqlcipher/sqlcipher-api/
- SQLCipher source and documentation: https://github.com/sqlcipher/sqlcipher
- rusqlite: https://docs.rs/rusqlite
- Argon2 (RFC 9106): https://www.rfc-editor.org/rfc/rfc9106
- XChaCha20-Poly1305: https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-xchacha
- age encryption: https://age-encryption.org/
- Apple Keychain Services: https://developer.apple.com/documentation/security/keychain_services
- OWASP ASVS: https://owasp.org/www-project-application-security-verification-standard/
- Plaid Trial signup: https://dashboard.plaid.com/signup
- Plaid Transactions Sync: https://plaid.com/docs/api/products/transactions/
- Plaid transaction states: https://plaid.com/docs/transactions/transactions-data/
- Plaid Liabilities: https://plaid.com/docs/api/products/liabilities/
- Plaid OAuth: https://plaid.com/docs/link/oauth/
- Plaid Hosted Link: https://plaid.com/docs/link/hosted-link/
- Plaid Link Webview SDK (deprecated): https://plaid.com/docs/link/webview/

## 20. Owner dependencies and open items

These are not blockers to the offline/demo/CSV application, but they must be tracked in `docs/implementation-status.md` and not invented or silently assumed.

- **Plaid Trial account:** the owner creates it; the agent supplies a checklist and verified-doc guidance. Do not enroll or accept terms on the owner's behalf.
- **Plaid product access:** confirm Transactions, balances, and Liabilities for the actual Trial plan and institutions. If Liabilities is unavailable, fall back to manual payment metadata and label it clearly.
- **Real account list:** the owner adds accounts in the app. No card list is known or hardcoded, and no live coverage may be claimed until linking is verified.
- **CSV samples:** the owner will export issuer history later. Until real samples arrive, ship the generic mapper and preview only; do not invent issuer presets.
- **Passphrase custody:** the owner controls the SQLCipher passphrase and accepts that there is no recovery without it. Never ask the owner to share it in chat. Recommend a long, unique passphrase stored in the owner's password manager; never store it in the repository, notes, or environment.
- **Lock lifecycle acceptance:** the security review supersedes the earlier "no auto-lock" decision. The owner must confirm that locking on screen lock/sleep/manual/quit and a 15-minute default idle timeout are acceptable. Disabling idle auto-lock is allowed only with a visible in-app warning and a recorded risk acceptance.
- **FileVault and screen lock:** the owner should enable FileVault and a short screen-lock timeout. These are environmental controls; the app cannot enforce them.
- **Signing identity:** the owner decides whether to use ad-hoc signing only or create a local self-signed code-signing certificate for stable Keychain behavior. Notarization/distribution signing stays out of scope.
- **Plaid desktop flow:** the owner must approve the verified flow, including any hosted component and its data flow. If the flow requires infrastructure the owner does not want, live linking is marked unavailable rather than improvised.
- **Plaid client secret entry:** the owner enters the secret through the native secure-input sheet; it is stored only in Keychain. Never paste it into chat, a repository, an environment file, or the renderer.
- **No real data in demo mode:** the owner must not import real CSVs into the demo database. The app disables it, but the README must state it plainly.
- **Native smoke testing:** perform on the owner's M1 Max Mac, including the native passphrase sheet, Keychain ACL/accessibility, content protection, screen-lock auto-lock, and state restoration; document what could not be automated.
- **Security review acceptance:** the owner should read [security-review.md](security-review.md), including the residual risks, before calling the app complete.
- **Branding assets:** no app icon or logo has been supplied. Use a neutral placeholder icon and do not spend time on branding beyond a clean, readable default.
- **Signing/notarization:** deferred by the owner. Do not claim distribution readiness.
- **Notifications, login item, launch-at-login, auto-update:** out of scope for version one.
- **Live verification status:** initially unverified. The handoff must say so plainly.

## Definition of done

The owner can unlock the app with their passphrase, add their accounts, select a calendar month, see itemized spending across all cards, inspect supplier totals, see current balances and clearly sourced due/statement metadata, import missing-card data, back up and restore encrypted data, and recover from a failed sync without losing or double-counting financial records. Everything works with demo data and CSV even before a live bank connection is approved, and the app never ships or creates a plaintext ledger.

Security is part of done, not a follow-up: the fail-closed SQLCipher check and plain-SQLite rejection are demonstrated; the passphrase never enters the renderer; the lock lifecycle works on screen lock, sleep, manual lock, and quit; the renderer has no network/secret/arbitrary-file capability; CSP and capability tests pass; backups and restore pass tamper/wrong-passphrase/nonempty-database tests; the rekey and crypto-erase paths are tested; no Critical/High security finding remains open without a written owner acceptance; the residual risks in §13.14 are disclosed in the README; and the artifact hash/signature status is recorded. Do not call the result "watertight," "unhackable," or "military-grade"; call it what it is: a local, encrypted, single-user app with documented limits.
