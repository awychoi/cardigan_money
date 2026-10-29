# Implementation status: cardigan_money

Last updated: 2026-10-29. Phase 10 is a local release candidate for the offline/demo/CSV scope; the UI overhaul (Phases 0–6) and the backup/restore overhaul are complete. Owner native smoke, Keychain attribute inspection, and live reconnect remain external checks. Plaid Trial/Production linking is wired and available in the app; a real Production Item has not yet been verified. Restore reuses a Plaid Item when this Mac still holds its Keychain token; connections without a token require a new link.
This is the honesty document: it records what was built, what actually ran, and what remains unverified. It is updated at the end of every phase.

## 1. Phase tracker

| Phase | Outcome | Status |
| --- | --- | --- |
| 0 | Foundation, threat model, crypto design, SQLCipher and Plaid feasibility | **Complete** |
| 1 | Tauri shell, command gateway, renderer lockdown | **Complete** |
| 2 | Vault envelope, native secret input, lock lifecycle, migrations | **Complete** (implementation + automated evidence; owner passphrase-entry smoke pending) |
| 3 | Domain rules, input bounds, calendar-month accounting | **Complete** |
| 4 | Onboarding, accounts, settings, theme, security settings | **Complete** (native close/passphrase smoke pending owner) |
| 5 | Demo data and read-only dashboard | **Complete** |
| 6 | Edits, manual metadata, CSV import/export security | **Complete** |
| 7 | Encrypted backup, restore, rotation, recovery | **Complete** |
| 8 | Sync engine, scheduler, mock provider | **Complete** (mock provider; no live provider call) |
| 9 | Plaid adapter and verified linking (conditional) | **Sandbox verified**; Trial/Production wired but **unverified with a real Item**; live reconnect and Keychain ACL inspection unverified |
| 10 | Independent security review, performance, packaging, handoff | **In progress** — automated evidence complete; owner native smoke and distribution signing outstanding |
| UI | UI overhaul: spend_history, charts, Overview, Transactions, Settings | **Complete** (Phases 0–6) |

## 2. Phase 0 — foundation and feasibility

### Completed
- Git repository initialized; `.gitignore` excludes toolchain, Node modules, Rust targets, databases, backups, exports, `.env`, logs, and build output.
- Toolchain pinned and recorded: Rust 1.99.1 stable (`rust-toolchain.toml`), Node 22.23.3, pnpm 12.11.2 (`packageManager`), Tauri CLI 2.12.4, Tauri 2.12.5. Lockfiles committed (`pnpm-lock.yaml`, `src-tauri/Cargo.lock`).
- Tauri 2 + React 19 + TypeScript 5.9 + Vite 7 shell; a native window launches on the M1 Max under `pnpm tauri dev`.
- `docs/security/threat-model.md`: assets, adversaries, trust boundaries, controls, out-of-scope.
- `docs/security/architecture.md`: envelope key hierarchy, 152-byte authenticated header layout, KDF bounds, rotation journal, backup container, SQLCipher fail-closed order, lock lifecycle, Keychain, network, IPC.
- `docs/integrations/plaid-feasibility.md`: current-doc research; Hosted Link + `/link/token/get` polling is the only viable no-server flow. Sandbox was subsequently verified; Trial/Production wiring now exists but has not been verified with a real Item. No Plaid account or credentials were created by the agent.
- SQLCipher fail-closed spike implemented and tested with real encrypted databases.
- Test-only plain-SQLite probe builds a separate binary linking plain SQLite and rejects the app's exact verification code.
- Tauri baseline: one `main` window built in Rust; one capability (`core:event:default`); CSP with no HTTPS source; `freezePrototype` on; `withGlobalTauri` off; asset protocol off; devtools off; drag-and-drop disabled; navigation allowlist; new windows denied; single-instance plugin; content protection applied; state restoration disabled; core dumps disabled.
- Documentation skeletons: this file, `docs/reference/decisions.md`, `docs/reference/test-plan.md`, `SECURITY.md`, provisional README.
- Dependency audits run (see §5); `cargo-deny` policy added; signing decision recorded as ad-hoc.

### Actual test runs

| Command | Result |
| --- | --- |
| `cargo test` (debug) | 20 lib tests + 7 `vault_cipher` integration tests passed |
| `./scripts/verify-plain-sqlite-reject.sh` | `/usr/bin/sqlite3` refused the DB (`file is not a database`); the plain-SQLite build of the app check exited 2 (`MissingVersion`) |
| `cargo audit` | 554 crates scanned; 0 vulnerabilities; 5 unmaintained/unsound warnings, all Linux-only Tauri transitive crates excluded by the macOS target filter. Exit 0. |
| `cargo deny check` | advisories ok, bans ok, licenses ok, sources ok |
| `pnpm audit --audit-level high` | No known vulnerabilities (after upgrading Vite 7.3.7, Vitest 4.1.12, Zod 4.6.7) |
| `pnpm test` | 4 files, 18 tests passed |
| `pnpm test:e2e` | 8 Playwright tests passed |
| `pnpm build` + `node scripts/check-release-artifacts.mjs` | Build succeeded; dev mock sentinel absent; no source maps; checks passed |
| `pnpm tauri dev` | Native window launched; WebKit/React renderer loaded; 4 IPC gateway requests accepted (React StrictMode double-invoked two bootstrap commands) |

### SQLCipher evidence (measured, not assumed)
- `PRAGMA cipher_version` → `4.6.1 community`
- `PRAGMA cipher_provider` → `openssl`; provider version `OpenSSL 3.6.3`
- `PRAGMA cipher_status` → unsupported in 4.6.1 (added in 4.12); the code treats unsupported as non-fatal and keeps the keyed read + full integrity check mandatory.
- Raw-key `PRAGMA rekey` verified: old raw key fails, new raw key opens the DB, and `cipher_integrity_check` passes after a WAL checkpoint.
- Wrong key leaves the database bytes unchanged.
- DB/WAL/SHM contain no plaintext sentinel; a plain SQLite client cannot open the database.
- Absurd KDF parameters (`u32::MAX` memory) are rejected before Argon2 runs (<50 ms in the unit test).
- Every one of the 152 header bytes is flipped in a test; each tamper is rejected. Truncation/extension is rejected.
- Header mirror fallback works; removing both copies returns `vault_missing`.

### Phase 0 exit gate check
- Scaffold runs natively: **yes** (`pnpm tauri dev`).
- Encrypted path proven fail-closed on this Mac: **yes** (real SQLCipher tests + plain-SQLite probe).
- Renderer baseline locked down: **yes** (config assertions + tests; native release verification is repeated in Phase 10).
- Plaid linking documented with a verified flow or concrete blocker: **yes** — flow documented from official docs; live implementation blocked on owner enrollment and unverified until Sandbox testing.
- Dependency audit and signing identity recorded: **yes**.

## 3. Phase 1 — app shell and typed bridge

### Completed
- App identity: display name `cardigan_money`, bundle identifier `com.awychoi.cardiganmoney`, window title `cardigan_money`, Apple Silicon target.
- One custom IPC command `gateway` with a versioned envelope, per-command `deny_unknown_fields` payloads, object-only payloads, 256 KiB request cap, command-name allowlist, and sanitized `{code, message}` errors.
- Window/origin guard: only the `main` window at `tauri://localhost` (or the loopback Vite origin in debug builds) may invoke commands.
- Commands implemented: `app_info`, `health_check`, `vault_status`. No generic SQL/file/URL/shell/HTTP command exists.
- Navigation allowlist and new-window denial installed on the window builder before first load.
- Browser mock bridge with the same contract, dev-only via `import.meta.env.DEV` + `VITE_MOCK_BRIDGE`, aliased to a disabled stub in production.
- Zod request/response validation at the renderer boundary; contract errors fail loudly.
- UI shell: header, disabled month navigation placeholder, honest "No provider connected" status, theme toggle (light/dark/system), settings/nav shell, empty overview, error boundary, loading state, redacting logger.
- Tests: Vitest component/contract/security-config tests; Playwright mock-bridge workflows; release artifact scan; Rust IPC malformed-input corpus.

### Actual test runs
- `cargo test`: 20 lib + 7 integration passed.
- `pnpm test`: 18 passed.
- `pnpm test:e2e`: 8 passed; zero requests outside the loopback origin; no password input; only `cm.theme` in localStorage.
- `pnpm build` + artifact scan: passed; mock bridge sentinel absent from `dist`.
- Native `pnpm tauri dev`: process stays up and accepts gateway requests. WebKit emitted sandbox-related "could not create directory" warnings for `~/Library/WebKit/...`; this is a limitation of the DSH build sandbox, not app code. The app data path is redirected only in debug builds for this environment.

### Phase 1 exit gate check
- Typed round-trip through Rust and mock: **yes**.
- Invalid/unknown/oversized IPC rejected; no generic command: **yes** (unit tests + inventory review).
- Release CSP has no HTTPS source; Playwright observes no external requests: **yes**.
- Navigation/new windows blocked; devtools absent in release: **partially verified** — handlers and config are tested; release artifact inspection is performed in Phase 10 and the native smoke checklist.
- Theme switching works: **yes**.
- Frontend/contract/IPC tests pass: **yes**.

## 3a. Phase 2 — encrypted vault, native secret input, lock lifecycle

### Completed
- Native AppKit `NSSecureTextField` sheet (`security/secure_input.rs`) for vault creation (two fields + match), unlock, current/new passphrase, restore, portable export, and the Plaid client secret. Fixed Rust-owned titles/messages; renderer receives only success/failure/cancel. Sheet window sets `NSWindowSharingNone`. No HTML password field exists anywhere.
- Vault envelope: random 256-bit DEK, Argon2id KEK, XChaCha20-Poly1305 wrap, authenticated 152-byte `vault.header` + mirror, generation, vault ID, schema version, timestamps, rotation journal.
- `services/vault.rs`: create, unlock (with journal recovery and mirror repair), lock, passphrase rotation (new DEK, pre-rekey encrypted snapshot + old header copy, journal, raw-key rekey, verification), status, security summary/events.
- SQLCipher open/create is fail-closed; creation pre-creates the ledger 0600 and never opens an existing path with CREATE. `umask(0077)` plus explicit 0600 enforcement for DB/WAL/SHM.
- Memory: `LockedKey` mlock/zeroize; passphrases in zeroizing `SecretString`; drop on lock/decrypt; core dumps disabled; debug-only panic detail, release fixed message.
- Lock lifecycle: Rust monitor every 2 s for screen lock and console session state, sleep/resume wall-vs-monotonic gap, and native idle seconds. Manual lock via header button, menu item, and Cmd+L. Quit/exit locks. Lock closes the connection, drops DEK/header/settings, records a `lock` event, and emits one `vault:locked` event; renderer unmounts sensitive views. Hiding is not a lock.
- Single-instance plugin registered before setup; second instance focuses the first and exits without touching the database.
- Migrations: `schema_migrations` with checksums, forward-only application in transactions, pre-migration encrypted snapshot for non-empty DBs, fail-closed on edited checksum or newer schema.
- Schema (migration 1): `vault_metadata`, `settings`, `security_events` with STRICT tables/CHECK constraints and indexes.
- Settings repository: validated timezone, theme, close behavior, onboarding, auto-lock (1–240), sync windows, retention; secrets are never stored. Lock-on-system-events and content protection are pinned on.
- Security events: allowlisted types, outcome, capped metadata with secret/path/SQL-shaped keys dropped; recent events shown in Settings.
- UI: locked gate, vault creation, unlock, lock-again, real security/KDF/cipher/event display, theme through encrypted settings, timezone/auto-lock controls.
- Native smoke harness (debug only) that presents the AppKit secure sheet and auto-cancels it after 1.5 s, used to prove the native path on this Mac without inventing automation.

### Actual test runs
| Command | Result |
| --- | --- |
| `cargo test` | 37 lib + 7 `vault_cipher` + 9 `vault_flow` = 53 passed, 0 failed |
| `./scripts/native-sheet-smoke.sh` (via equivalent run) | `NATIVE_SHEET_SMOKE=passed: sheet presented and cancelled` |
| Native single-instance probe | First process stayed up; second exited 0 in the same second; exactly one process remained |
| `pnpm test` | 21 passed |
| `pnpm test:e2e` | 11 passed |
| `pnpm build` + artifact scan | Passed; dev mock sentinel absent; no source maps |

### Phase 2 exit gate check
- Create → quit → relaunch → unlock same ledger: **partially verified.** The full lifecycle is exercised in Rust with real encrypted DBs (create, lock/clear, reopen, unlock, wrong passphrase, rotation, crash-window journal recovery). The native sheet presentation is verified. Entering a passphrase into the native sheet and relaunching the app is an owner smoke item because this environment cannot safely drive macOS secure input (TCC/accessibility automation was refused; a probe was stopped without sending input).
- Wrong passphrase non-destructive: **verified** (DB and header bytes unchanged).
- Header missing/tampered/truncated, absurd KDF, mirror recovery: **verified**.
- Lock closes connections and drops the DEK: **verified**; OS screen-lock/sleep/fast-user-switch triggers are implemented but **not automated here**; owner smoke required.
- Migrations success/failure, checksum tamper: **verified**; pre-migration snapshot primitive implemented.
- 0700/0600, symlink rejection, single instance: **verified**.
- Passphrase never enters the renderer: **verified by construction and tests** (no password input; native sheet only; no secret in DOM/storage/URL).
- No plaintext temp artifacts: snapshots are SQLCipher-encrypted with 0600; no plaintext DB copy is created.

### Open Phase 2 items (blockers)
- Owner must complete the native passphrase-entry smoke: create a vault, quit, relaunch, unlock, Lock Now, screen-lock lock, and sleep lock on the M1 Max outside the DSH sandbox.
- Screen lock/sleep/fast-user-switching/logout could not be induced or observed in this environment. The code paths are implemented; do not describe them as verified.
- Keychain is not touched yet (Phase 8/9).

## 3b. Phase 3 — domain rules and calendar-month accounting

### Completed
- Migration 2 adds the full financial schema: `accounts`, `connections`, `account_sources`, `balance_snapshots`, `payment_snapshots`, `manual_payment_metadata`, `transactions`, `transaction_overrides`, `merchant_rules`, `import_batches`, `import_rows`, `sync_runs`, `sync_steps`. STRICT tables, CHECK constraints, foreign keys, partial unique provider identity, and query indexes.
- Money: signed integer minor units; decimal-string parser (currency symbols, validated thousands grouping, parentheses, exact precision, no binary floats), supported currency exponents, ±1e12 bound, property tests.
- Dates: strict date-only parsing, calendar `Month` ranges `[start, first day of next month)`, timezone-aware current month (default America/New_York), leap day/year-boundary/DST tests, reporting-date rule (authorization → posted → explicit manual) with provenance.
- Text: NUL/control/bidi rejection, length caps, single-line collapsing, NFKC merchant normalization, display-safe sanitization, LIKE escaping.
- Ledger query layer: closed enums for status/type/sort, parameterized queries only, bounded date range (731 days), account filter cap (20), page cap (200/10 000), search length cap, escaped LIKE, deterministic tie-breaking, eligible/pending subtotals computed by SQL.
- Calendar-month summary service: gross purchases, negative refunds, fees/interest, net, pending subtotal, counts, per-supplier and per-category breakdowns, exact reconciliation, other-currency row count.
- Overview service: latest balances, current known total with missing count (never zero-filled), provider and manual payment facts kept independent, valid-through expiry, upcoming 14-day window with unknown amounts not summed, statement balance never used as remaining due.
- Account repository: validated manual account create/update/archive/list; account commands and tests.
- Deterministic domain fixtures covering leap day, DST dates, year boundary, pending/posted pair, later-month refund, fees/interest, payment/transfer/exclusion, identical same-amount purchases, non-USD row, unknown vs zero payment metadata.

### Actual test runs
| Suite | Result |
| --- | --- |
| `cargo test` | 59 lib + 3 accounts + 2 domain_fixtures + 8 ledger + 4 properties + 7 vault_cipher + 11 vault_flow = **94 passed**, 0 failed |
| `pnpm test` | 23 passed |
| `pnpm test:e2e` | 14 passed |
| `pnpm build` + artifact scan | Passed |

Notable assertions: October fixture gross 11 500 / refunds −2 500 / fees 700 / net 9 700 / pending 7 500 with exact supplier and category reconciliation; September purchase 9 000 with October refund −2 500; leap-day 1 111; DST months 2 222 and 3 333; Dec 31 4 444 not January; two identical Café purchases counted twice; injection-shaped search/ sort rejected with the accounts table intact.

### Phase 3 exit gate check
- All financial/domain tests in spec §16: **covered** (see suite and fixture assertions).
- Supplier/category totals reconcile exactly: **yes** (`reconciliation.exact` asserted).
- Historical month does not change current balances/obligations: **yes** (overview has no month input; test compares months).
- Unknown metadata never becomes zero / no payment due: **yes**.
- DST, leap day, year boundaries: **yes**.
- SQL injection/malformed input fails safely; no dynamic identifier interpolation: **yes** (closed enums + parameters; tests).
- Domain layer is the only source of totals: **yes** — no frontend financial calculation exists.

## 3c. Phase 4 — onboarding, accounts, settings, theme

### Completed
- Post-unlock onboarding wizard: an optional first step configures Plaid Sandbox or Trial/Production credentials in the native AppKit sheet (client ID + secret go to the macOS Keychain; the renderer never receives them), with a manual-card branch and a skip path; the next step auto-detects the Mac's IANA timezone, shows the friendly zone/city label, and lets the owner change it (fallback `America/New_York`); a final summary points to where each optional item can be finished in Settings. Every non-passphrase step is skippable, and completion is persisted in encrypted settings. No demo data is seeded.
- Account management: one source-labeled list for manual, provider, and demo cards; create, edit, archive/restore, include-in-overview; credit limit parsed in Rust from a decimal string; validation and text cleaning; archival keeps history. Manual payment details are opened per account from its row instead of living in a separate panel.
- Empty account state with an Add account action that opens **Settings → Accounts → All accounts**; the account list carries source labels and reversible archive confirmation.
- Settings: theme (DB-persisted), timezone (grouped, friendly-label dropdown, with this Mac's detected zone and current local time), idle auto-lock, close behavior, quit, security section with cipher/KDF/generation/vault prefix/last integrity, recent events, passphrase change.
- Settings information architecture: **Accounts** groups three tabs — **All accounts** (list/add/edit/payment details), **Bank connections** (Plaid credentials, links, status, sync runs), and **CSV import** (mapping, preview, history). The old top-level Connections & sync and Import & export sections are gone.
- First-close behavior: native AppKit dialog with Hide / Quit / Cancel and a “remember this choice” checkbox; stored in encrypted settings; hide keeps scheduler/idle timer; reopen shows the window; quit locks and exits. Decision logic unit-tested; native dialog presentation is compiled and awaits owner smoke.
- `app_quit` command; theme no longer stored in localStorage (only encrypted settings).
- Account and settings restrictions on locked vault; no provider required.

### Actual test runs
- `cargo test --test accounts`: 3 passed (create/update/archive/validation/cleaning).
- `cargo test --test vault_flow settings`: 2 passed (settings survive lock/unlock; invalid settings rejected).
- `pnpm test`: 23 passed (wizard skip, account add, lock/unlock, theme, settings status).
- `pnpm test:e2e`: 14 passed (wizard with account, archive/restore, close-behavior persistence, locked gate).

### Phase 4 exit gate check
- New user can create a vault and add an account or finish with none: **backend and mock flow verified**; native passphrase entry still owner smoke.
- Restart preserves accounts/theme/close behavior/auto-lock: **Rust persistence tests pass**; native restart owner smoke.
- First close asks once/later follows: **decision + persistence tested**; native dialog presentation owner smoke.
- Passphrase change via native sheet: **implemented and Rust-tested**; native entry owner smoke.
- No provider required: **yes**.

## 3d. Phase 5 — demo mode and read-only dashboard

### Completed
- Demo mode: separate SQLCipher database at `<app-data>/demo/demo.cmdb`, fixed app-managed synthetic key (SHA-256 of a documented string), no owner passphrase, no vault header. Opening demo closes any real session; the real DB remains separate. Demo reset deletes/recreates only the demo DB. Window title gains `— Demo (synthetic data)`; renderer shows a persistent demo banner with Reset/Exit.
- Deterministic demo fixture generator: frozen clock 2026-10-23, 11 accounts across 5 fictionalized institutions (Chase, Synchrony, Bilt, Amex, Citi) plus a EUR synthetic card; 176 transactions across Aug–Oct; pending/posted pair; month-boundary authorization/posting; later-month refunds; payments, transfers, fees, interest, exclusions; two identical Café purchases; merchant aliases/rules; missing/zero/unknown due metadata; valid and expired manual metadata; stale/reconnect/error connections; partial and failed sync history; non-USD rows kept separate.
- Mock provider module with capabilities (transactions/balances supported, liabilities unsupported), deterministic accounts/transactions/balances, and no credentials.
- Read-only dashboard backed entirely by Rust services: overview headline metrics (net spending, current known balances with missing count, upcoming 14-day reported payments, pending), account freshness, cards view (balance/freshness, month spend, due-date with amount-kind, statement date labeled as not remaining due, next closing only if supplied, manual labels, connection health), ledger (search, card/category/status/type filters, sort, pagination, row count, eligible/pending subtotals, clear filters, read-only detail panel), supplier view with drill-down and exact reconciliation. Transactions/By supplier toggle preserves month and filters; card click filters the ledger while preserving month.
- Empty, pending-only, stale, connection-error, unsupported-metadata note, and demo states; keyboard-reachable controls and focusable rows; usable at 1100×760 with table scrolling.
- Demo isolation guards: account mutations, passphrase change, and real unlock/create are blocked in demo; demo never touches Keychain (no Keychain code path yet), backups/exports/imports do not exist yet and must be guarded when added.
- Month navigation is backend-computed (`previousMonth`/`nextMonth` from the dashboard service); current balances/upcoming obligations do not change with the selected month.
- Debug-only native demo harness (`CARDIGAN_MONEY_OPEN_DEMO=1`) verifies native demo DB opening and dashboard assembly without simulating input.

### Actual test runs
| Suite | Result |
| --- | --- |
| `cargo test` | 65 lib + 3 accounts + 5 demo + 2 domain_fixtures + 8 ledger + 4 properties + 7 vault_cipher + 11 vault_flow = **105 passed**, 0 failed |
| `pnpm test` | 23 passed |
| `pnpm test:e2e` | 22 passed (dashboard, demo mode, card filtering, supplier drill-down, month independence, 1100×760, security) |
| `pnpm build` + artifact scan | Passed |
| Native demo harness | `DEMO_NATIVE_OPEN=passed accounts=11 rows=67 net=446652 mode=demo`; renderer issued dashboard gateway requests |

### Phase 5 exit gate check
- Demo totals match backend fixture expectations exactly: **Rust demo tests compare deterministic summaries across reopen/reset**; UI renders backend-provided values only.
- Demo never touches the real ledger; cannot open the real DB or Keychain: **isolation tests pass**; the fixed demo key cannot open the real DB; demo DB path is distinct; reset leaves the real vault usable.
- All spec §15 scenarios appear: **covered by fixture and tests** (see list above).
- Real-data import/export/backup/provider actions unavailable in demo: **no such features exist yet**; account/passphrase mutations are blocked and features must call `require_real` when added.
- Usable at 1100×760 with scrolling: **Playwright viewport test passes**.
- Month changes do not alter current balances/obligations: **tested**.
- Read-only workflows navigable by keyboard: **card tiles/rows/controls are focusable**; a full assistive audit remains Phase 10.

## 3e. Phase 6 — edits, manual metadata, CSV import/export

### Completed
- Migration 3 adds separate rule-derived display columns (`rule_merchant`, `rule_category`, `rule_id`) and import fingerprints, so user rules never rewrite provider/CSV source fields.
- Editable transaction overrides: merchant alias, category, type correction, exclusion, notes; clear-edits; source fields preserved; source type/date/provenance shown. Overrides survive reload and take precedence over merchant rules. Ledger/summary reflect them.
- Merchant rules: CRUD with closed match types, priority, enabled, canonical merchant, optional category; reapplication recomputes derived display fields for all transactions; user override still wins; deleting a rule reverts rule-derived labels. Lead (virtual) empty states for no rules.
- Manual payment metadata: dated statement balance, minimum due, next due date, valid-through; parsed and validated in Rust; independent of provider snapshots; expiry handled; clear supported; overview/Cards show manual source.
- CSV import: Rust-owned job, native file picker, bounded UTF-8 streaming parse (20 MiB, 100,000 rows, 40 columns, 4,096 field chars, 30 s cap), optional BOM, explicit date format, signed or debit/credit mapping, optional category/source ID columns, formula-safe display, per-row rejection, duplicate classification (source ID, fingerprint), review-not-delete for ambiguous overlap, identical-file no-op, transactional batch commit, import history, rollback of unique contributions only, and fingerprint/hash verification to detect a changed file.
- CSV export: kernel-owned filtered query, plaintext warning, native save dialog, owner-only (0600) atomic file, formula-injection escaping for text fields after leading whitespace, numeric amount cells without float rounding, no auto-open, no account numbers in the filename, and an `export` security event.
- UI: transaction editor, import wizard with mapping/preview/month totals/review confirmation, batch history/rollback, merchant-rule manager, manual-payment editor, export warning; demo mode hides/disables all of these.
- Demo guards: transaction edits, rules, manual metadata, imports, exports, and rollback are blocked in demo mode.

### Actual test runs
| Suite | Result |
| --- | --- |
| `cargo test` | 74 lib + 3 accounts + 5 demo + 2 domain_fixtures + 8 ledger + 6 imports + 4 properties + 7 vault_cipher + 11 vault_flow = **120 passed**, 0 failed |
| `pnpm test` | 23 passed |
| `pnpm test:e2e` | 28 passed (edits/rules/manual/import/export/demo read-only) |
| `pnpm build` + artifact scan | Passed |

Specific import/export assertions: BOM + quoted comma + parentheses import; identical file no-op; overlapping row review not inserted; accepted review inserts; rollback leaves the other batch untouched; malformed/oversized/symlink files rejected; formula-shaped text escaped (`'=SUM…`) while amounts stay numeric; 0600 mode; symlink export destination rejected without writing through it.

### Phase 6 exit gate check
- Edits persist across restart and sync: **persist across lock/unlock and reload; sync interaction is Phase 8**.
- Manual metadata independent/dated/stale/expired: **tested**.
- CSV edge cases, formula injection, 0600, no auto-open, review/rollback: **tested** at the service level.
- Renderer never receives a file path: **by construction**; import/export results carry counts and opaque IDs only.
- Demo isolation: **tested/blocked**.

Known limits to carry forward: native file dialogs are exercised by the owner smoke, not Playwright; provider-row overlap detection uses imported fingerprints only (provider rows are reviewed at sync time in Phase 8); rule reapplication is a full-table pass and will be measured at 50k rows in Phase 10.

## 3f. Phase 7 — encrypted backups, restore, rotation, recovery

### Completed
- Self-contained backup container v1 (256-byte header + SQLCipher DB): embeds the vault header, DB length/SHA-256, kind, timestamps, and a DEK-derived AEAD seal over the manifest. Full format in `docs/security/architecture.md` §7.
- Backup creation via consistent SQLite backup-API snapshot, checkpointed, verified (header decode, seal, hash), owner-only 0600, atomic write, strict filename, no plaintext temp.
- Kinds: daily, manual, pre-migration, pre-restore, pre-change, portable. Automatic daily backup after unlock when >24 h old; pre-migration before pending migrations; pre-restore immediately before replacement; pre-change during passphrase rotation. Backup failures never prevent unlock and are recorded.
- Retention: 7 daily, 10 manual, 5 each pre-migration/pre-restore/pre-change; strict pattern only; never the last backup.
- Restore (superseded by the 2026-10-24 overhaul below): native `.cmbak` picker + native passphrase sheet; full validation before install; staged database rekeyed to the current DEK; pre-restore backup; atomic file swap and rollback attempt. The overhaul adds cross-vault, fresh-install, selective-scope, locked, and destructive paths; see §10.
- Legacy portable container (kind 6): separate passphrase, fresh export DEK, embedded header. The separate UI action is removed; existing portable containers remain restorable through the unified flow.
- Crypto-erase: typed confirmation + native confirmation; locks the session, best-effort overwrites and deletes vault/header/journal/backups/demo/staged files, fsyncs, and returns the no-vault state. External copies are explicitly out of scope.
- Security events for backup creation/verification/failure, restore success, export, and lock-before-erase.
- UI (superseded by the 2026-10-24 overhaul): Backups & recovery panel, hidden in demo.
- Recovery guide: `docs/operations/backup-recovery.md`.

### Actual test runs
| Suite | Result |
| --- | --- |
| `cargo test` | 76 lib + 3 accounts + 8 backups + 5 demo + 2 domain_fixtures + 6 imports + 8 ledger + 4 properties + 7 vault_cipher + 11 vault_flow = **130 passed**, 0 failed |
| `pnpm test` | 23 passed |
| `pnpm test:e2e` | 32 passed (backup create/list/restore/delete, portable export, crypto-erase gate, demo hiding) |
| `cargo test --test backups` | 8 passed in ~43 s (tamper, wrong passphrase, wrong vault, retention 9→7, portable, pre-change, erase, locked restore) |
| `pnpm build` + artifact scan | Passed |

### Phase 7 exit gate check
- Backup/restore/tamper/wrong-passphrase/nonempty-database tests pass: **yes** at the service level with real encrypted containers.
- Restore leaves original on failure and creates a pre-restore backup: **yes** for validation failures and successful restore; post-swap rollback path implemented but not crash-injected.
- Passphrase rotation journal and pre-change backup: **tested** (new passphrase works, old fails, journal recovery from earlier Phase 2 tests).
- Old backups remain labeled/decryptable with the old passphrase and are never rewritten: **by design**; deletion is explicit.
- Portable export separate password: **tested**.
- Crypto-erase removes app data and cannot reopen: **tested**.
- Admin actions audited: **yes** (backup, restore, export, lock); native dialogs are owner-smoke items.

Known limits (updated by §10): native file pickers/sheets/confirmations are not automated; a crash between the two restore renames is a documented window mitigated by the pre-restore backup/`.restore-old`; backups exclude Keychain material by design.

## 3g. Phase 8 — sync engine, scheduler, mock provider, network boundary

### Completed
- Provider abstraction/capabilities and a deterministic mock provider with paged changes, pending/posted transitions, balances, liabilities capability shapes, and injected transient failure.
- Sync engine: account mapping/creation from provider sources; page loop; per-page transactional upsert + cursor advancement in the same transaction; idempotent replays; pending→posted linking with the pending row retired (no double count); tombstoning removals without deleting local edits; balances as append-only snapshots; liabilities never delete prior values on `available_but_null`/unsupported/partial; rule reapplication; per-connection isolation; run/step records with sanitized errors; audit events; stale `running` runs marked interrupted on the next attempt.
- Cursor safety: provider failure leaves the cursor at the last committed page; vault lock mid-sync stops without advancing and can resume; a lock before start returns a skip without writes.
- Concurrency: one global `try_lock` gate; concurrent triggers return `already_running` without network or writes.
- Scheduler: 08:00/20:00 local two-window loop, launch sync, 5-minute window, per-window in-memory de-duplication, skip when vault locked/quitting, retry on the next poll while the window is open. Settings windows/launch flag are respected.
- Network boundary: Rust-only `reqwest::blocking` client, HTTPS-only, exact host allowlist (`sandbox.plaid.com`, `production.plaid.com`), no redirects, TLS verification, connect/total timeouts, 4 MiB bounded response reader; renderer has no network source. Unit tests cover scheme/host/port/userinfo rejection and the response cap.
- UI: header sync status (provider count/last success/error), manual Sync now when connections exist, and **Settings → Accounts → Bank connections** with connection/status/account counts and recent run/step history. Provider-free state is explicit; CSV remains available.
- Commands: `sync_status`, `sync_now`, `sync_runs_list`, `connection_disconnect`; demo sync is blocked, status remains readable.

### Actual test runs
| Suite | Result |
| --- | --- |
| `cargo test` | 79 lib + 3 accounts + 8 backups + 5 demo + 2 domain_fixtures + 6 imports + 8 ledger + 4 properties + 8 sync + 7 vault_cipher + 11 vault_flow = **141 passed**, 0 failed |
| `pnpm test` | 23 passed |
| `pnpm test:e2e` | 33 passed (provider-free status, disabled manual sync, prior phases) |
| `pnpm build` + artifact scan | Passed |

Sync-specific assertions: three pages/three transactions, two accounts, two balances; replay no-op; provider failure at page 2 leaves cursor `page-2`; lock after page 1 leaves cursor `page-2` and resumes without duplication; pending→posted leaves one visible posted transaction and retires the pending row; `available_but_null` preserves a prior liability snapshot; two connections (mock + unconfigured plaid) isolate failure; concurrent and locked triggers skip.

### Phase 8 exit gate check
- Incremental/idempotent/paged sync, cursor safety, lock-mid-sync, pending/posted, removals: **tested**.
- Partial liability preservation and per-stream partial status: **tested**.
- One connection failure does not block another: **tested**.
- Manual/scheduled/launch triggers and concurrent suppression: **implemented/tested at the gate/window level**; no sleep/wake or multi-hour wall-clock test.
- Network allowlist/TLS/redirect/cap: **policy-tested**; no live network call was made (no credentials).
- No tokens in renderer/logs/errors: **by construction**; Plaid token storage is Phase 9 and not yet implemented.

Known limits: only the mock provider is wired; an existing `plaid` connection fails closed until the Phase 9 adapter is configured. Scheduled runs are tested through the window matcher, not by waiting for wall-clock windows. The scheduler thread does not yet observe sleep/wake explicitly; on wake the window matcher still applies.

## 3h. Phase 9 — Plaid adapter (conditional) — SANDBOX VERIFIED

### Live Sandbox evidence (2026-10-24)
Owner-supplied reports from build `2ef8b68...` (see `docs/integrations/plaid-verification-2026-10-24.md`):

| Connection | Status | Accounts | Provider transactions | Cursor | Liabilities |
| --- | --- | ---: | ---: | --- | --- |
| Charles Schwab (Sandbox) | healthy | 2 | 18 | present | available_but_null (partial) |
| Chase (Sandbox) | healthy | 0 | 0 | present | available_but_null (partial) |

A full quit/reopen + sync kept Charles Schwab at 2 accounts / 18 transactions with `+0` new transactions, demonstrating persisted-cursor continuity and no duplication. Balances added 2 snapshots per run (append-only). The Chase Item had no credit-card accounts; the app intentionally imports only Plaid `credit` accounts, so it remains a healthy 0-account connection.

The recorded link/sync evidence is **Sandbox-only**. No live bank/real financial data was used. Keychain attribute inspection and reconnect/update-mode live behavior remain owner smoke items; the functional link/token round trip and initial sync are verified. Trial/Production uses the same code path with per-environment credentials and `production.plaid.com`, but no real Item has been linked yet.

### Implemented
- macOS Keychain store (generic password, `WhenUnlockedThisDeviceOnly`, non-synchronizable) with **separate Sandbox and Trial/Production client ID/secret slots** plus per-connection access tokens; native AppKit setup sheet for each environment; renderer never receives credentials.
- Rust Plaid client through the HTTPS exact-host allowlist, no redirects, timeouts, bounded responses; static sanitized provider errors; no raw Plaid messages/tokens in errors or logs.
- Hosted Link: create token with `hosted_link: {}` against the selected environment's host, open only `https://secure.plaid.com/...` via the Tauri opener, poll `/link/token/get` with backoff/expiry/cancel, exchange public token in Rust, store access token, insert connection with its environment, initial sync. Hosted URL/link/public tokens are memory-only. Trial/Production uses `production.plaid.com`; Sandbox uses `sandbox.plaid.com`.
- Duplicate prevention: before `/item/public_token/exchange`, compare the Link institution (id, with a name fallback) with active connections in the same environment. A match stops the flow, records `plaid_duplicate_blocked`, and does **not** request an access token — per Plaid's duplicate-Item guidance this avoids creating/billing a duplicate Item. `disconnected` connections and restored `newlink-…` connections without local tokens are exempt so re-linking/recovery still works.
- Plaid Transactions Sync/balances/optional Liabilities mapping; credit-card account mapping to local accounts without duplicates; pending/removed handling; cursor through the existing sync engine.
- Reconnect/update mode (existing access token, no products, no new exchange) and disconnect with `/item/remove` attempt plus local token deletion. Healthy connections expose **Update…**; unhealthy ones expose **Reconnect…**; both reuse the existing Item and account mapping.
- UI: environment picker (Sandbox or Trial/Production), per-environment setup/replace/test, real-data and Trial-limit warning, link/cancel/waiting/linked/duplicate status, environment column, update/reconnect action, one-Item-per-institution notice, capabilities copy; demo disabled.
- Tests: Keychain memory store/credential scoping per environment, link-session parsing, duplicate-institution lookup (environment/name/disconnected/newlink cases), mapping fixtures, URL validation, mock duplicate/update UI flows, and production/Sandbox separation. Sandbox live verification is recorded above; real Trial/Production verification is **not yet recorded**.

### Explicitly not verified
- Real Trial/Production Item linking and live institution coverage. The code path is enabled, but no real Item has been linked by the maintainer.
- Keychain attribute/ACL inspection in Keychain Access (functional round trip verified).
- Reconnect/update-mode live repair (implemented and mocked; connection stayed healthy).
- Browser/network-level inspection of Hosted Link handling.
See `docs/integrations/plaid-verification-2026-10-24.md`.

## 3i. Phase 10 — hardening, performance, packaging, handoff (in progress)

### Completed evidence
- **Performance:** 50k encrypted transactions measured in release (`cargo test --release --test perf -- --ignored --nocapture`): warm search **13.1 ms**, month change **32.3 ms**, dashboard **40.5 ms**, cached dashboard after unlock **41.6 ms**, cold unlock **2.08 s**. Targets met/approximately met; details and debug comparison in `docs/operations/performance.md`.
- **Adversarial verification:** documented matrix in `docs/operations/adversarial-verification.md`: plain-SQLite rejection, wrong passphrase, DB/header/backup tamper, wrong-vault restore, symlink/path rejection, malformed/oversized CSV, formula injection, extra/oversized IPC fields, cursor failure, lock mid-sync, pending double-count, network URL/body caps, locked restore, concurrent sync, renderer sinks/localStorage/dev mock.
- **Dependency/build audit:** `cargo audit` shows **0 vulnerabilities**; 5 allowed Linux/unmaintained warnings (glib family/GTK, not used on macOS). `cargo deny check` all ok. `pnpm audit` → no known vulnerabilities. `pnpm install --frozen-lockfile` is the documented install. No repository secrets were added.
- **Accessibility:** labels, focusable controls, and keyboard-activatable primary navigation test added. VoiceOver/native sheet accessibility remains an owner smoke item.
- **Artifact:** release bundle rebuilt 2026-10-29 after the placket mark and its four theme colorways were installed, with `pnpm tauri build` (repo-local toolchain via `scripts/dev-env.sh`); ad-hoc signed (`codesign --force --deep --sign -`), `codesign --verify --deep --strict` passes, Identifier `com.awychoi.cardiganmoney`, Mach-O arm64. Binary SHA-256: **a81bc6e8c551312c5dee952b04b8cbb2f1d8a555aae36a3fc3e25c437c1a7162** (`Contents/MacOS/cardigan-money`). Artifact scan passed, the embedded assets contain the placket colorways, and the bundled `icon.icns` matches the regenerated Blueprint icon. Not notarized, hardened runtime, or App Sandboxed; not distribution-ready. This hash is historical: rebuild and record a new hash after any source change.
- **Docs:** README updated; `docs/operations/backup-recovery.md`, `docs/operations/performance.md`, `docs/operations/adversarial-verification.md`, `docs/operations/native-smoke.md`, `docs/reference/data-dictionary.md`, `docs/security/incident-runbook.md`, and the Plaid blocker section.
- **Automated suites (current):** 200 Rust passed (1 ignored release perf test), 90 Vitest, 51 Playwright; `pnpm build` + artifact scan passed.

### Independent review (completed)
A separate read-only review agent examined backup/restore, sync, gateway validation, network policy, native confirmations, and session races. It reported **no Critical or High findings**, 2 Medium, 1 Low–Medium, 2 Low. All five were fixed: demo-mode crypto-erase/passphrase guards; restore mode/vault re-check after the native prompt; settings lock/session race; no chmod of arbitrary selected backups; `O_NOFOLLOW` backup reads with owner-only mode enforcement. Regression tests were added. Full disposition and accepted residual risks: [`docs/security/independent-review-2026-10-23.md`](../security/independent-review-2026-10-23.md).

### Post-review regression fix (demo theme)
The review's settings hardening initially required a vault header even in demo mode. Saving a demo theme wrote SQLite but then returned “vault is locked” after the save, so the UI did not update until re-open. Fixed by capturing mode-specific session identity (demo has no header) before the write and updating in-memory settings for the still-current demo session. Regression tests: Rust `commands::tests::demo_theme_update_is_applied_and_kept_in_memory` and Playwright demo theme toggle. This regression was introduced by the review fix and is now covered.

### Phase 10 exit-gate status
- Automated suites: **194 Rust passed** (1 ignored 50k perf), **76 Vitest**, **43 Playwright**; release perf met search/month targets; artifact scan passed.
- Audits: `cargo audit` 0 vulnerabilities/5 allowed warnings, `cargo deny` all ok, `pnpm audit` clean.
- Ad-hoc signed artifact: **28b75a16182683b4e961e994b4e5adff65708eaea894ef51a93b7fdf59270998**, `codesign --verify --deep --strict` passes, arm64.
- Honest gaps: owner native smoke (Keychain ACL, native sheet/dialogs, screen-capture protection, screen-lock/sleep, close/quit, VoiceOver) is **not performed/claimed**; Plaid Trial/Production is **wired but unverified with a real Item** (Sandbox is verified); live reconnect/update mode and Keychain ACL inspection are unverified; the post-restore native prompt is not automated; ad-hoc/non-notarized.
- The offline/demo/CSV scope is implemented and evidenced. The project is not represented as distribution-ready or watertight.

## 3j. UI overhaul — Phase 0 `spend_history` (2026-10-24)

### Implemented
- Gateway command `spend_history` with `{ months?: 1..24, endMonth?: YYYY-MM }`; defaults are 12 months and the current month. Out-of-range or malformed values are rejected, not clamped.
- `storage/ledger.rs::spend_history` runs one grouped query over posted, non-excluded spend rows for one currency, zero-fills the window from a `BTreeMap`, returns refunds as a positive magnitude, and marks the current and any future month `complete: false`. `net = gross − refunds` and the current-month total reconciles with `month_summary`.
- `services/dashboard.rs::spend_history` resolves "today" from `demo_today()` in demo mode or the settings timezone otherwise, so the history window matches the dashboard.
- Typed contract (`zSpendHistory`, request validation), mock bridge parity through `summaryFor`, and no new dependency.

### Actual test runs

| Command | Result |
| --- | --- |
| `cargo test --test ledger` | 14 passed (4 history tests, 1 service validation test, 1 cross-currency subtotal test) |
| `pnpm test` | 35 passed, including 3 mock-history tests and 2 bridge contract tests |
| `pnpm verify:all` | typecheck, lint, Prettier, Vitest, build, 36 Playwright, rustfmt, clippy, `cargo test` all pass; 161 Rust tests |

### Phase 0 exit gate
- Reconciliation guard: **yes** (current-month `net` equals `month_summary` and the mock dashboard).
- Zero-fill, ordering, window rejection, currency isolation: **yes**.
- Locked session rejected with `vault_locked`: **yes** (`commands::tests`).
- Mock parity through the real `command()` path: **yes**.
- Chart UI: not part of this phase; Phase 2–3.

## 3k. UI overhaul — Phases 1–3: theme, charts, Overview (2026-10-24)

### Implemented
- **Theme contract:** `options.overview` (composition, trend, topCategories, trendMonths), a 3–6 color chart ramp with a sixth color in every theme/mode, `--chart-6` fallback, and overview data attributes in `applyTheme`. JSON Schema and the theming guide are updated.
- **Charts:** `chartMath` (largest-remainder shares, top-N + neutral Other, donut offsets, signed bar scale), `CategoryRing`, `RankedBars`, `TrendBars`, `ChartLegend`, `ChartTable`, and `charts.css`. No chart library or new runtime dependency.
- **Overview:** hero with net spending, arithmetic, pending label, eligible/pending/total rows, and an Open ledger action; 14-day due rail with four nearest payments and an unknown-amount pip; theme-selected composition; `spend_history` trend with loading/error states; balances with state chips; a three-item attention card; one `current as of` footnote plus the currency notice. The old freshness panel and notice bars are gone.
- **Preserved hooks:** `h1 Overview`, the three metric labels, `.metric`/`.metric-value` on balances, `.month-label`, the currency notice, and the demo label.

### Actual test runs

| Command | Result |
| --- | --- |
| `pnpm test` | 64 passed (12 chart math, 8 chart component, 5 Overview, 39 pre-existing/other) |
| `pnpm test:e2e` | 37 passed, including chart presence, theme-swapped composition, month-driven ring/trend, and 1100×760 Overview |
| `pnpm verify:all` | typecheck, lint, Prettier, Vitest, build, Playwright, rustfmt, clippy, 161 Rust tests all pass |

### Exit gates
- Ring shares sum to 100 within rounding; the ring's gross total and refunds reconcile with the hero net: **yes** (math tests + Overview test).
- Trend ends at the selected month and marks current/future months incomplete: **yes**.
- Theme switch changes ring ↔ ranked bars without a reload: **yes** (Playwright).
- 1100×760 Overview has no horizontal overflow: **yes** (Playwright).
- No new runtime dependency: **yes**.

### Follow-ups for Phase 4
- Day subtotals must respect pagination: the ledger loads 50 rows per request, so full-month day totals need backend grouping or page-local labeling.
- `eligibleCount` and per-currency subtotals are already on `LedgerPage`; use them in the new meta line.

## 3l. UI overhaul — Phase 4: Transactions re-grade (2026-10-24)

### Implemented
- Date-sorted views group rows by reporting date with a sticky day header and a day subtotal. The subtotal is labelled **page subtotal** because the table is paginated; amount/merchant sorts stay flat and keep the Date column. A scroll observer hides group headers after their whole group has passed the sticky column header, preventing label pile-up.
- Merchant monograms, category chips, status dot + word, and amount tones: refunds/payments use the success/credit tone, transfers are neutral, and danger/rust is reserved for excluded rows. Supplier-view refunds use the same credit tone.
- Meta line keeps `N rows` as its own node for the existing e2e selector and adds `N eligible`, then eligible/pending subtotals per currency with explicit codes.
- Sticky filter bar and a bounded, internally scrolling transaction table with a sticky column header.
- Preserved: every filter label, `Clear filters`, `By supplier`, the detail-panel flow, and the `N rows` hook.

### Actual test runs

| Command | Result |
| --- | --- |
| `pnpm test` | 71 passed, including 7 `ledgerPresentation` tests |
| `pnpm test:e2e` | 38 passed, including day grouping, page subtotals, sticky classes, credit styling, and scroll-hiding |
| `pnpm verify:all` | typecheck, lint, Prettier, Vitest, build, Playwright, rustfmt, clippy, 161 Rust tests all pass |

### Exit gate
- Ledger e2e green: **yes**.
- Subtotal reconciles per currency against the filtered rows: **yes** (Rust and mock tests from the earlier fix).
- Day totals are internally consistent with the rows shown and explicitly page-local: **yes**.
- Credits are no longer rendered as errors; rust is reserved for excluded rows: **yes**.

## 3m. UI overhaul — Phase 5: Settings sectioning (2026-10-24; Accounts IA refreshed 2026-10-25)

### Implemented
- Left section list with one section rendered at a time: Appearance, Lock & idle, Accounts, Backups & recovery, Merchant rules, Security events, About.
- **Accounts** contains the tabs **All accounts**, **Bank connections**, and **CSV import**, so account entry, sync, and import are one section without one very long pane. Demo mode keeps Accounts read-only with only the All accounts tab and hides Bank connections, CSV import, Backups & recovery, and Merchant rules; Security events and About remain available.
- Control placement: idle auto-lock, system-event locking, window close behavior, reporting timezone, and passphrase change live in **Lock & idle**; account entry and per-account payment details live in **Accounts → All accounts**; Plaid credentials, linking, and sync live in **Accounts → Bank connections**; CSV mapping/history lives in **Accounts → CSV import**; security facts, demo mode, and quit live in **About**; crypto-erase stays in **Backups & recovery** with its typed confirmation.
- `openSettingsSection(page, name)` and `openAccountsTab(page, tab)` were added to `e2e/helpers.ts`; the backup, edit, shell, and settings specs use them.
- The Overview empty-state **Add account** action opens **Settings → Accounts → All accounts** instead of the default Appearance tab.

### Actual test runs

| Command | Result |
| --- | --- |
| `pnpm test` | 82 passed, including App onboarding, per-account payment details, and timezone-helper tests |
| `pnpm test:e2e` | 49 passed, including Plaid credentials/timezone onboarding, empty-Overview Add account routing, Accounts tabs, timezone persistence, per-account payment details, and Plaid update/duplicate-refusal flows |
| `pnpm typecheck && pnpm lint && pnpm test && pnpm build` | pass; Rust `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` also pass |
| `pnpm format:check` | pass |

### Exit gate
- All settings-driven specs green: **yes**.
- No control unreachable: **yes** (each old control is in a named section; the account, backup, rule, import, close-behavior, and security tests exercise them).
- One section at a time: **yes** (new Playwright test).
- 1100 px clean: **yes** (visual check; no horizontal overflow).

## 3n. UI overhaul — Phase 6: polish (2026-10-24)

### Implemented
- `OverviewSkeleton` mirrors the 12-column Overview while `dashboard_get` is in flight, announces itself with `role="status"`, and disables shimmer for calm/reduced-motion.
- The trend panel has a 12-bar loading skeleton; ring, ranked bars, and trend keep explicit empty states and hidden tables.
- Visual QA covered Ledger, Graphite, and Editorial in light and dark at 1440×960 and 1100×760: composition variants, 6/12-month trends, due rail, attention card, balances, and sectioned Settings. No horizontal overflow or illegible text; the theme contrast matrix remains authoritative.
- No screenshots are committed; the existing proposal images remain the visual reference and the theming guide records the QA.

### Actual test runs

| Command | Result |
| --- | --- |
| `pnpm test` | 72 passed, including `OverviewSkeleton` |
| `pnpm test:e2e` | 40 passed |
| `pnpm verify:all` | typecheck, lint, Prettier, Vitest, build, Playwright, rustfmt, clippy, 161 Rust tests all pass |

### Exit gate
- Empty/loading states: **yes**.
- Dark-mode QA for every theme: **yes** (six theme/mode combinations).
- Final full gate: **yes**.
- Goal: the UI and backup/restore overhaul plans are complete; remaining work is release-level (native smoke, a real Trial/Production verification pass, signing).

## 4. Toolchain and environment

| Component | Version / note |
| --- | --- |
| macOS | 27.0.1, arm64 (M1 Max, 64 GiB) |
| Xcode / clang | /Applications/Xcode.app, Apple clang 21.0.0 |
| Rust | 1.99.1 stable, workspace-local toolchain due to sandbox |
| Node / pnpm | 22.23.3 / 12.11.2 |
| Tauri | CLI 2.12.4, runtime 2.12.5 |
| SQLCipher | 4.6.1 community, OpenSSL 3.6.3, via `rusqlite 0.37` + `bundled-sqlcipher-vendored-openssl` |
| Frontend | React 19.1.1, Vite 7.3.7, TypeScript 5.9.3, Vitest 4.1.12, Zod 4.6.7 |

## 5. Security findings status

| ID | Status in this build |
| --- | --- |
| SEC-01 fail-open encryption | **Mitigated and tested**: key-first, cipher/provider checks, plaintext rejection, plain-SQLite negative build, wrong-key non-destructiveness. `cipher_status` unsupported on 4.6.1 and handled explicitly. |
| SEC-02 passphrase in renderer | **Implemented**: native AppKit secure input for create/unlock/change; no HTML password field; renderer receives only outcomes. Native passphrase entry itself awaits the owner smoke. |
| SEC-03 circular key hierarchy | **Mitigated**: external authenticated header; in-memory-only DEK. |
| SEC-04 backup vs. no-prompt | **Implemented**: self-contained authenticated containers carry their own wrapped DEK; every restore asks for the backup passphrase in a native sheet. Same-vault, cross-vault, fresh-install, selective-scope, locked, and destructive paths are implemented; see §10. |
| SEC-05 lock lifecycle | **Implemented** in Rust: manual, screen lock, sleep, session switch, quit, idle timeout; connection close + DEK drop + renderer clear + lock event. OS-trigger verification pending owner smoke. |
| SEC-06 Plaid desktop flow | **Sandbox verified**: Hosted Link + polling + token exchange + Transactions Sync against Sandbox Items. Trial/Production is wired with separate credentials and host, but no real Item has been verified. |
| SEC-07 renderer privilege/network | **Mitigated for the shell**: CSP, one capability, gateway, nav/new-window blocking, no remote content. Native release verification pending owner smoke. |
| SEC-08 Keychain/signing | **Keychain implemented** for provider credentials (`WhenUnlockedThisDeviceOnly`, non-synchronizable). Signing is ad-hoc; ACL inspection and notarization are not done. |
| SEC-09 security deferred | Phases 0–2 gates enforced; evidence recorded here. |
| SEC-10 supply chain | Lockfiles, pinned toolchains, audits, explicit build-script allowlist (`esbuild`, `@tailwindcss/oxide`). |
| SEC-11 logs/crash/state restoration | Core dumps disabled; redacting logger off by default; state restoration disabled. Crash diagnostics in debug only. |
| SEC-12..20 | Retired mapping: the original specification rows no longer map one-to-one onto this build. Current controls and evidence are the per-phase sections above and the dated reviews. |

Open Critical/High findings: none. SEC-02 and SEC-05 are implemented and tested to the extent this environment allows; their native OS-trigger portions remain owner-smoke items and are not claimed as verified.

## 6. Blockers and external dependencies

- **Plaid:** Sandbox linking is implemented and verified against owner-supplied Items. Trial/Production linking is implemented and available with separate Production credentials; it needs an owner-supplied real Item for verification, and the owner must accept Trial billing/Item-limit implications. Live reconnect/update mode and Keychain ACL inspection remain unverified.
- **Fresh-install restore:** supported through **Restore from backup…** on the locked/fresh gate. The backup's vault identity and passphrase become the new vault. See [backup/recovery](../operations/backup-recovery.md).
- **Real CSV samples:** none supplied. The app ships a generic mapping flow; no issuer presets are invented.
- **Native smoke tests:** the DSH build sandbox blocks writes to `~/Library/Application Support`, `~/Library/WebKit`, and the login Keychain. Native development runs use a debug-only app-data override. Standard-path launch, Keychain ACL, screen-capture protection, and screen-lock notifications must be verified by the owner on the built `.app` outside this sandbox (checklist in [native smoke](../operations/native-smoke.md)).
- **Signing:** ad-hoc only; no stable local signing identity has been created (owner may choose one later). Note that Keychain ACL prompts may change across rebuilds.
- **Screen capture verification:** cannot be observed from this environment without Screen Recording permission; not claimed.

## 7. Live institutions verified

**Sandbox only.** Charles Schwab (2 credit accounts, 18 provider transactions) and Chase (0 credit accounts; depository-only Item) were linked in Plaid Sandbox. No Production Item, real institution, or real financial data has been used by the maintainer. Trial/Production linking is available in the app but remains unverified. See [the verification record](../integrations/plaid-verification-2026-10-24.md).

## 8. Performance

Measured 2026-10-23 in release on a 50,000-transaction encrypted dataset (`cargo test --release --test perf -- --ignored --nocapture`): warm search **13.1 ms**, month change **32.3 ms**, dashboard **40.5 ms**, cached dashboard after unlock **41.6 ms**, cold unlock **2.08 s**. Targets met or approximately met; methodology and debug comparison are in [performance](../operations/performance.md).

## 9. Next phase

The initial build is complete for the offline/demo/CSV scope, and the [UI overhaul](../plans/ui-overhaul-2026-10.md) (Phases 0–6) is complete.

Release blockers are not UI work: owner native smoke, a real Trial/Production verification pass, and distribution signing remain outstanding.

## 10. Backup and restore overhaul (2026-10-24)

### Implemented
- **One manual backup action.** **Back up now** always presents the native passphrase sheet. A blank passphrase uses the current header/DEK when unlocked; a non-blank one creates a fresh header/DEK for that file with the current KDF parameters and the same `vault_id`. The native save dialog defaults to the app backup directory and accepts a different destination. Backups written elsewhere are not listed or pruned. While locked, the sheet also requires the current vault passphrase; the app opens the ledger only to create and verify the container, keeps no UI session, and never renders data.
- **Backup list and retention.** The normal list shows only `cm-backup-*` containers for the current `vault_id`; retention counts only those files and never removes the last one. Retired `cm-retired-*` containers are listed separately, excluded from retention, and delete-only.
- **Restore matrix.** `services/restore.rs` implements same-vault full (keep identity), cross-vault full (adopt backup identity with a fresh header/DEK), fresh-install full (atomic temporary-directory install), selective scopes on a staged candidate, and the destructive full-only path for a locked vault without the current passphrase. Replacing an existing vault requires the owner to type `replace`; cross-vault and destructive paths add explicit warnings.
- **Scoped restore.** Full database; financial data + settings; financial data only. The last two reset the documented table sets, write default settings where required, delete connections/sync history/account sources, null `source_connection_id`, and null `import_batch_id` when import history is reset. Existing targets keep the current `vault_id` and passphrase; fresh targets adopt the backup identity.
- **Provider gate.** All restores mark provider balance/payment snapshots `restored = 1` (schema 4) and clear restored connections' cursors and last-success timestamps, so the next sync is a full resync; the overview hides restored live balances, available credit, limits, and due metadata while keeping restored transaction history.
- **Item reuse on restore (2026-10-26).** A restored Plaid connection whose `credential_ref` still has a Keychain token keeps both, stays `healthy` with no cursor, and resyncs without creating a Plaid Item. Reuse is automatic for same-vault and fresh-install restores and an explicit **reuse local Items** choice for cross-vault and destructive restores; it preserves the 10-Item Plaid Trial allowance, which `/item/remove` does not restore. Reused connections sync in the background after an unlocked restore and on unlock after a locked restore; a revoked token demotes to `reconnect_required`. Connections without a local token rotate to `newlink-…`, start `reconnect_required`, and offer **Link again…**. `ConnectionView.hasToken` probes the Keychain for non-healthy connections so a tokenless same-vault restore offers a new link rather than a failing reconnect.
- **Token and Item choices.** Same-vault full keeps Keychain tokens and credential references; fresh-install restore reuses matching local tokens automatically. Cross-vault full offers reuse-local, delete-local, delete-and-revoke, or keep; partial scopes delete old tokens because no connection remains. The destructive path offers reuse-local, delete-all-app-managed-tokens, or leave. Cross-vault restores rotate references only for connections that cannot reuse a local token.
- **Retired backups.** A full cross-vault restore (and a destructive restore whose `vault_id` changes) renames every normal previous-vault backup, including the pre-restore backup, with `cm-retired-`. They are never auto-pruned and remain until deleted from the retired view.
- **Non-private acceptance.** Inspect reports the file mode without enforcing it; apply accepts a non-private file only with an explicit acknowledgement. The acknowledgement bypasses only the mode check; owner, regular-file, no-symlink, size, header, hash, and seal checks remain.
- **Validation and install.** Header/seal/hash/length, `vault_id` consistency, SQLCipher/SQLite integrity, foreign keys, schema compatibility, migration, and a schema-object allowlist run before install. Same-vault/partial restore keeps the current header/DEK; cross-vault/fresh create a fresh header/DEK under the backup passphrase and never adopt the backup's DEK. Existing installs keep the old files until the new vault verifies; fresh installs are renamed into place.
- **Post-restore prompt.** A native acknowledgement offers **Change passphrase now** / **Change later**. A cancelled or failed sheet never rolls back restored data. The locked sessionless rotation path is implemented for restores that leave the vault locked.
- **UI and bridge.** `backup_restore_prepare` → renderer choices (scope, acknowledgement, Plaid choice, typed word) → `backup_restore_apply`; `backup_restore_cancel` and `backup_list_retired` are separate commands. The preview/result carry no path or passphrase/secret field. Restore entry points exist on both the locked and fresh gate. The old `portable_export` and uncontrolled `backup_restore` commands are removed.
- **Rate limits and demo guards.** Prepare/apply are rate-limited; restore, backup creation, retired listing, and deletion are real-mode only.

### Actual test runs (2026-10-24)
| Suite | Result |
| --- | --- |
| `cargo test` | **191 passed**, 0 failed, 1 ignored (50k perf) — includes 24 restore-overhaul, 4 backup-now, 9 backup, 90 lib |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo fmt --all --check` | clean |
| `pnpm test` | **75 passed** (14 files), including restore preview/result redaction and command-removal contract tests |
| `pnpm test:e2e` | **42 passed**, including typed-confirmation restore, locked-gate backup, and fresh-gate restore |
| `pnpm verify` | typecheck, lint, Vitest, production build, and release-artifact scan passed |

### Exit gates and honest gaps
- Matrix coverage, identity/passphrase retention/adoption, scoped reset semantics, FK cleanup, pre-restore abort, retired naming/pruning exemption, provider gate, non-private acknowledgement, and post-restore locked rotation: **covered by Rust tests**.
- Renderer path/secret exclusion and fresh/locked restore entry points: **covered by Vitest and Playwright**.
- Native file pickers, native passphrase sheets, and the post-restore dialog: **owner smoke only** (this environment cannot drive secure input).
- Existing-vault installs use old-file rename/rollback; a crash between renames remains a narrow documented window mitigated by the pre-restore backup and `.cmdb.restore-old`. Fresh installs are assembled in a temporary directory and renamed into place.
- Backups from schema 3 are migrated to schema 4 during staged validation. The schema-object allowlist rejects unknown tables, triggers, views, attached databases, and foreign-key violations.

## 11. Theme colorways for the app mark (2026-10-29)

### Implemented
- The app mark is the **04 Placket** from the [abacus exploration](../design/brand/abacus/README.md), replacing the Forest C button in the header, vault gate, onboarding and boot screen. The placket was chosen for its story and its colorable form; the theme picker shows every colorway.
- **Four colorways ship**: Blueprint, Editorial, Graphite and Ledger. `src/brand/marks.ts` maps `theme.id` to `src/assets/brand/placket-<id>.svg`; `brandMarkFor()` falls back to Blueprint for a missing or unknown id. Assets are generated by `scripts/build-brand-marks.mjs`, which refuses to run when a bundled theme has no palette.
- **Blueprint is the fixed default.** The Tauri icon set was regenerated from `placket-blueprint-icon.svg`; the boot screen and gate follow the resolved theme, while the Dock icon cannot. The README notes that its Knit Graph banner is historical.
- The theme picker renders each placket at 20 px next to its theme name. `BrandMark` tags the wrapper with `data-brand-theme`, so the tests assert the shown colorway instead of a hashed asset URL.
- Legibility was checked at 16/32/64/128/512 px, and the four colorways were checked in the mock app in light and dark modes.

### Actual test runs
| Suite | Result |
| --- | --- |
| `pnpm verify` | typecheck, lint, **90 Vitest**, production build, and release-artifact scan passed |
| `pnpm test:e2e` | **49 Playwright passed**, including the theme switch asserting the Ledger then Graphite mark |
| `cargo test` | not rerun; Rust and SQLCipher were untouched |

## 12. Navigation setting and maker branding (2026-10-29)

### Implemented
- **Navigation is a user setting, not a theme option.** `settings.navigation` (`sidebar` or `topbar`) is stored in the encrypted settings table, defaults to sidebar, and is exposed in Settings → Appearance. The theme picker previews the chosen layout. Any theme can use either layout.
- `options.navigation`, the `data-nav` theme attribute, and the navigation field in the theme schema and all four theme JSONs were removed; `theme.schema.json` was regenerated. Existing settings rows without the key load as sidebar.
- **Maker credit.** Settings → About, the locked gate, and the app footer show **Wai Yin Choi**, `wychoi.com`, and the personal abacus mark (`src/assets/wychoi-abacus.svg`). The website opens through the new `open_website` command, which hard-codes `https://wychoi.com`, accepts no URL, and uses the existing Rust opener plugin. The renderer still has no opener capability.
- **Cleanup.** Removed the superseded Forest C brand assets (icons, heroes, previews, build scripts, index) and six unreferenced app screenshots. Rewrote `docs/design/brand/README.md`; kept the current abacus/colorway docs and the historical README banner.
- Updated the theming contract, decision log (D-027–D-029), security architecture and threat model, README, and docs index.

### Actual test runs
| Suite | Result |
| --- | --- |
| `pnpm verify` | typecheck, lint, **90 Vitest**, production build, and release-artifact scan passed |
| `pnpm test:e2e` | **51 Playwright passed**, including navigation/theme mix-and-match and maker credit on the gate, About and footer |
| `cargo test` | **200 passed**, 1 ignored (50k perf) |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo fmt --all --check` | clean |
