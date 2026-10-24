# Implementation status: cardigan_money

Last updated: Phase 10 hardening (2026-10-23, America/New_York).
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
| 8 | Sync engine, scheduler, mock provider | Not started |
| 9 | Plaid adapter and verified linking (conditional) | Not started |
| 10 | Independent security review, performance, packaging, handoff | Not started |

## 2. Phase 0 — foundation and feasibility

### Completed
- Git repository initialized; `.gitignore` excludes toolchain, Node modules, Rust targets, databases, backups, exports, `.env`, logs, and build output.
- Toolchain pinned and recorded: Rust 1.99.1 stable (`rust-toolchain.toml`), Node 22.23.3, pnpm 12.11.2 (`packageManager`), Tauri CLI 2.12.4, Tauri 2.12.5. Lockfiles committed (`pnpm-lock.yaml`, `src-tauri/Cargo.lock`).
- Tauri 2 + React 19 + TypeScript 5.9 + Vite 7 shell; a native window launches on the M1 Max under `pnpm tauri dev`.
- `docs/threat-model.md`: assets, adversaries, trust boundaries, controls, out-of-scope.
- `docs/security-architecture.md`: envelope key hierarchy, 152-byte authenticated header layout, KDF bounds, rotation journal, backup container, SQLCipher fail-closed order, lock lifecycle, Keychain, network, IPC.
- `docs/plaid-feasibility.md`: current-doc research; Hosted Link + `/link/token/get` polling is the only viable no-server flow; live linking blocked on owner enrollment. No Plaid account or credentials were created.
- SQLCipher fail-closed spike implemented and tested with real encrypted databases.
- Test-only plain-SQLite probe builds a separate binary linking plain SQLite and rejects the app's exact verification code.
- Tauri baseline: one `main` window built in Rust; one capability (`core:event:default`); CSP with no HTTPS source; `freezePrototype` on; `withGlobalTauri` off; asset protocol off; devtools off; drag-and-drop disabled; navigation allowlist; new windows denied; single-instance plugin; content protection applied; state restoration disabled; core dumps disabled.
- Documentation skeletons: this file, `docs/decisions.md`, `docs/test-plan.md`, `SECURITY.md`, provisional README.
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
- Post-unlock onboarding wizard: optional first account, provider step that explains live linking is unavailable and never requests credentials, finish; every non-passphrase step skippable; completion persisted in encrypted settings. No demo data is seeded.
- Manual account management: list, create, edit, archive/restore, include-in-overview, credit limit parsed in Rust from a decimal string, validation and text cleaning, archival keeps history.
- Empty account state with an Add account action; account list in Settings with source labels and reversible archive confirmation.
- Settings: theme (DB-persisted), timezone, idle auto-lock, close behavior, quit, security section with cipher/KDF/generation/vault prefix/last integrity, recent events, passphrase change.
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
- Self-contained backup container v1 (256-byte header + SQLCipher DB): embeds the vault header, DB length/SHA-256, kind, timestamps, and a DEK-derived AEAD seal over the manifest. Full format in `docs/security-architecture.md` §7.
- Backup creation via consistent SQLite backup-API snapshot, checkpointed, verified (header decode, seal, hash), owner-only 0600, atomic write, strict filename, no plaintext temp.
- Kinds: daily, manual, pre-migration, pre-restore, pre-change, portable. Automatic daily backup after unlock when >24 h old; pre-migration before pending migrations; pre-restore immediately before replacement; pre-change during passphrase rotation. Backup failures never prevent unlock and are recorded.
- Retention: 7 daily, 10 manual, 5 each pre-migration/pre-restore/pre-change; strict pattern only; never the last backup.
- Restore: native `.cmbak` picker + native passphrase sheet; full validation before install (header, seal, length, DB hash, vault ID, cipher/SQLite integrity, schema/migration compatibility); staged database rekeyed to the current DEK; metadata generation/schema reconciled; pre-restore backup; atomic file swap; post-swap verification and rollback attempt; current passphrase retained. Backup bytes are never modified. Restore requires an unlocked vault; empty-install restore is documented as unsupported.
- Portable encrypted export: separate passphrase (native two-field sheet), fresh export DEK, snapshot rekeyed to it, embedded export header, arbitrary destination, 0600, no provider credentials. Tested restore with its own passphrase.
- Crypto-erase: typed confirmation + native confirmation; locks the session, best-effort overwrites and deletes vault/header/journal/backups/demo/staged files, fsyncs, and returns the no-vault state. External copies are explicitly out of scope.
- Security events for backup creation/verification/failure, restore success, export, and lock-before-erase.
- UI: Backups & recovery panel (back up now, list, delete, restore warning, portable export, crypto-erase danger zone, last-backup status), hidden in demo.
- Recovery guide: `docs/backup-recovery.md`.

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

Known limits: native file pickers/sheets/confirmations are not automated; empty-install restore is not implemented; a crash between the two restore renames is a documented window mitigated by the pre-restore backup/`.restore-old`; backups exclude Keychain material by design.

## 3g. Phase 8 — sync engine, scheduler, mock provider, network boundary

### Completed
- Provider abstraction/capabilities and a deterministic mock provider with paged changes, pending/posted transitions, balances, liabilities capability shapes, and injected transient failure.
- Sync engine: account mapping/creation from provider sources; page loop; per-page transactional upsert + cursor advancement in the same transaction; idempotent replays; pending→posted linking with the pending row retired (no double count); tombstoning removals without deleting local edits; balances as append-only snapshots; liabilities never delete prior values on `available_but_null`/unsupported/partial; rule reapplication; per-connection isolation; run/step records with sanitized errors; audit events; stale `running` runs marked interrupted on the next attempt.
- Cursor safety: provider failure leaves the cursor at the last committed page; vault lock mid-sync stops without advancing and can resume; a lock before start returns a skip without writes.
- Concurrency: one global `try_lock` gate; concurrent triggers return `already_running` without network or writes.
- Scheduler: 08:00/20:00 local two-window loop, launch sync, 5-minute window, per-window in-memory de-duplication, skip when vault locked/quitting, retry on the next poll while the window is open. Settings windows/launch flag are respected.
- Network boundary: Rust-only `reqwest::blocking` client, HTTPS-only, exact host allowlist (`sandbox.plaid.com`, `production.plaid.com`), no redirects, TLS verification, connect/total timeouts, 4 MiB bounded response reader; renderer has no network source. Unit tests cover scheme/host/port/userinfo rejection and the response cap.
- UI: header sync status (provider count/last success/error), manual Sync now when connections exist, and Settings → Connections & sync with connection/status/account counts and recent run/step history. Provider-free state is explicit; CSV remains available.
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

## 3h. Phase 9 — Plaid adapter (conditional) — BLOCKED

No owner Plaid Trial account, client ID/secret, or authorized Item exists in this environment. The verified desktop Hosted Link + `/link/token/get` flow is documented in `docs/plaid-feasibility.md` but **not implemented as a live adapter**, and no live institution/product coverage is claimed. The app shows the blocker in Settings, fails any existing `plaid` connection closed with “live Plaid adapter is not configured”, and keeps CSV/manual/demo/sync/mock workflows usable. No link URL, token, or secret is exposed to the renderer. Unblock criteria: owner enrollment/terms, product access record, non-repository credentials, Sandbox verification with an authorized account, then end-to-end transactions/balances (and optional Liabilities) verification.

## 3i. Phase 10 — hardening, performance, packaging, handoff (in progress)

### Completed evidence
- **Performance:** 50k encrypted transactions measured in release (`cargo test --release --test perf -- --ignored --nocapture`): warm search **13.1 ms**, month change **32.3 ms**, dashboard **40.5 ms**, cached dashboard after unlock **41.6 ms**, cold unlock **2.08 s**. Targets met/approximately met; details and debug comparison in `docs/performance.md`.
- **Adversarial verification:** documented matrix in `docs/adversarial-verification.md`: plain-SQLite rejection, wrong passphrase, DB/header/backup tamper, wrong-vault restore, symlink/path rejection, malformed/oversized CSV, formula injection, extra/oversized IPC fields, cursor failure, lock mid-sync, pending double-count, network URL/body caps, locked restore, concurrent sync, renderer sinks/localStorage/dev mock.
- **Dependency/build audit:** `cargo audit` shows **0 vulnerabilities**; 5 allowed Linux/unmaintained warnings (glib family/GTK, not used on macOS). `cargo deny check` all ok. `pnpm audit` → no known vulnerabilities. `pnpm install --frozen-lockfile` is the documented install. No repository secrets were added.
- **Accessibility:** labels, focusable controls, keyboard-activatable primary navigation test added (34 Playwright). VoiceOver/native sheet accessibility remains an owner smoke item.
- **Artifact:** release bundle built with `pnpm tauri build --bundles app`; ad-hoc signed (`codesign --force --deep --sign -`), `codesign --verify --deep --strict` passes, Identifier `com.awychoi.cardiganmoney`, Mach-O arm64. Binary SHA-256: **4073844017c4f21a22ee0317fdc879668ae19c9c7eadacd735373472f4037931** (`Contents/MacOS/cardigan-money`). Artifact scan passed. Not notarized, hardened runtime, or App Sandboxed; not distribution-ready.
- **Docs:** README updated; `docs/backup-recovery.md`, `docs/performance.md`, `docs/adversarial-verification.md`, `docs/native-smoke.md`, `docs/data-dictionary.md`, `docs/incident-runbook.md`, and the Plaid blocker section.
- **Automated suites:** 141 Rust (plus ignored release perf test), 23 Vitest, 34 Playwright; `pnpm build` + artifact scan passed.

### Still required before Phase 10 completion
- Independent reviewer report and resolution/written acceptance of Critical/High findings.
- Owner native smoke on the signed artifact: native passphrase sheet, Keychain ACL, screen capture/content protection, screen-lock/sleep auto-lock, close/quit, state restoration, file dialogs, VoiceOver. These are **not claimed verified**.
- Final implementation-status update with the review outcome and any accepted residual risks.

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
| SEC-04 backup vs. no-prompt | Designed (header-embedded wrapped DEK); implementation Phase 7. |
| SEC-05 lock lifecycle | **Implemented** in Rust: manual, screen lock, sleep, session switch, quit, idle timeout; connection close + DEK drop + renderer clear + lock event. OS-trigger verification pending owner smoke. |
| SEC-06 Plaid desktop flow | **Documented blocker**: Hosted Link + polling, no credentials, unverified. |
| SEC-07 renderer privilege/network | **Mitigated for the shell**: CSP, one capability, gateway, nav/new-window blocking, no remote content. Native release verification pending Phase 10. |
| SEC-08 Keychain/signing | Design recorded; Keychain implementation Phase 8/9; signing ad-hoc in Phase 10. |
| SEC-09 security deferred | Phases 0–2 gates enforced; Phase 0–2 evidence recorded here. |
| SEC-10 supply chain | Lockfiles, pinned toolchains, audits, explicit build-script allowlist (`esbuild`, `@tailwindcss/oxide`). |
| SEC-11 logs/crash/state restoration | Core dumps disabled; redacting logger off by default; state restoration disabled. Crash diagnostics in debug only. |
| SEC-12..20 | Corresponding features are not built yet; controls are designed in the spec and tracked per phase. |

Open Critical/High findings: none. SEC-02 and SEC-05 are implemented and tested to the extent this environment allows; their native OS-trigger portions remain owner-smoke items and are not claimed as verified.

## 6. Blockers and external dependencies

- **Plaid:** no owner account or credentials. Live linking is unavailable and unverified. Hosted Link flow requires a real Sandbox test with Hosted Link enabled and a redirect URI decision; see `docs/plaid-feasibility.md`.
- **Real CSV samples:** none supplied. Only a generic mapper will be built; no issuer presets will be invented.
- **Native smoke tests:** the DSH build sandbox blocks writes to `~/Library/Application Support`, `~/Library/WebKit`, and the login Keychain. Native development runs use a debug-only app-data override. Standard-path launch, Keychain ACL, screen-capture protection, and screen-lock notifications must be verified by the owner on the built `.app` outside this sandbox (checklist in `docs/test-plan.md`).
- **Signing:** ad-hoc only; no stable local signing identity has been created (owner may choose one later). Note that Keychain ACL prompts may change across rebuilds.
- **Screen capture verification:** cannot be observed from this environment without Screen Recording permission; not claimed.

## 7. Live institutions verified

**None.** No Plaid account, Sandbox link, or institution has been tested. No institution or product coverage is claimed. Demo fixtures are synthetic and will be labeled as demo.

## 8. Performance

No performance target has been measured yet (no ledger/dashboard). Do not treat any target as met. The 50,000-transaction encrypted benchmark is Phase 10 work.

## 9. Next phase

Phase 10 is completing independent-review disposition and owner native smoke. Phase 9 (conditional) requires owner Plaid credentials and a live/sandbox Item. No account or credentials are available in this environment, so the adapter, Keychain credential handoff, Hosted Link polling, and live verification are **not claimed complete**. Phase 10 hardening continues in parallel.

## 10. Superseded next-phase note

Phase 8 will add the sync engine, scheduler, mock-provider integration, cursor safety/idempotency, concurrent triggers, partial failure handling, lock-during-sync, and the network allowlist/TLS/response-cap layer.

## 10. Superseded next-phase note

Phase 7 will implement the self-contained authenticated backup container, automatic retention, native restore with read-only validation, portable encrypted exports with a separate password, passphrase rotation using the journal, crypto-erase, and recovery documentation/tests.

## 10. Superseded next-phase note

Phase 6 will add transaction overrides, merchant rules, manual payment metadata, and the Rust-owned CSV import/export workflow with batch tracking, duplicate review, rollback, formula-injection defenses, and 0600 exports.

## 10. Superseded next-phase note

Phase 5 will build demo mode (separate database, fixed synthetic key, deterministic 150+ transaction fixture generator, mock provider), and the read-only overview/cards/ledger/supplier views backed by the Phase 3 services, with persistent demo labeling and demo isolation tests.

## 10. Superseded next-phase note

Phase 3 will add the financial schema (accounts, snapshots, transactions, overrides, merchant rules, imports, sync runs), integer-cent money and date-only types, calendar-month eligibility and summary functions, parameterized repositories with enum-only identifiers, and the domain test suite including reconciliation, DST/leap-day, pending/posted, refunds, exclusions, and SQL-injection attempts.
