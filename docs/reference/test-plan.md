# Test plan: cardigan_money

This plan maps the specification's required tests (§16) to commands and phases. A test that has not been run is not evidence. `docs/reference/implementation-status.md` records actual runs and results.

## 1. Layers and commands

| Layer | Command | Notes |
| --- | --- | --- |
| Rust unit tests | `cd src-tauri && cargo test --lib` | Money, dates, summary, vault header, crypto, SQL parsing |
| Rust integration tests | `cd src-tauri && cargo test --test '*'` | Real temporary SQLCipher DBs, migrations, backups, sync mock, CSV |
| Plain-SQLite negative build | `./scripts/verify-plain-sqlite-reject.sh` | Builds a separate binary linking plain SQLite and proves the exact app check rejects it |
| Frontend unit/component | `pnpm test` | Vitest + React Testing Library |
| Full gate | `pnpm verify:all` | Typecheck, lint, Prettier, Vitest, build, Playwright, rustfmt, clippy, `cargo test` |
| Type checking | `pnpm typecheck` | `tsc -b` |
| Lint/format | `pnpm lint`, `pnpm format:check`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` | ESLint flat config + Prettier + rustfmt + clippy (warnings are errors) |
| Browser UI | `pnpm test:e2e` | Playwright against the dev-only mock bridge |
| CSP/capability assertions | `pnpm test:e2e --grep security` and Rust tests | No external requests, blocked navigation, no forbidden capability permissions |
| Dependency audit | `cargo audit`, `cargo deny check`, `pnpm audit` | Recorded in Phase 0 and Phase 10 |
| Native smoke | Manual checklist §4 | Not automated; Playwright is never proof of Tauri/native behavior |
| Performance | `cargo test --release --test perf -- --nocapture` and manual run | 50,000-transaction encrypted dataset; measured, not estimated |

## 2. What each phase must prove

### Phase 0
- SQLCipher 4.x active, provider recorded, key-first connection, wrong key rejected non-destructively, raw-key rekey works.
- DB/WAL/SHM contain no plaintext sentinel; `/usr/bin/sqlite3` cannot open the ledger.
- Plain-SQLite build rejected by the exact shared check.
- `vault.header` authentication: every byte flip, truncation, absurd KDF parameters fail closed.
- Release CSP has no HTTPS source; capability has only `main`; navigation/new windows blocked.
- Plaid flow documented with verified vs blocked status.

### Phase 1
- Gateway round-trip through Rust and through the mock bridge.
- Unknown command, unknown field, oversized payload, wrong type, bad window label/origin rejected.
- No generic SQL/file/URL/shell commands exist; command inventory is reviewed.
- Playwright observes zero external requests; `window.open` and remote navigation blocked.
- Devtools absent in release; `freezePrototype` on; `withGlobalTauri` off; asset protocol off.
- Theme switching works; no financial data in localStorage/sessionStorage/URL.

### Phase 2
- Create vault through native sheet (manual smoke), quit, relaunch, unlock same ledger.
- Wrong passphrase non-destructive; missing/tampered/truncated header fails.
- Header mirror recovery; KDF bounds; rotation journal recovery.
- Lock closes connections, zeroizes, clears renderer state; screen lock/sleep/idle/manual/quit triggers.
- Single instance before DB open.
- Migrations succeed/fail without destroying the original DB; 0700/0600; no plaintext temp artifacts.

### Phase 3
- All spec §16 financial/domain tests.
- Reconcile supplier/category totals to eligible rows exactly.
- Historical month change does not change current balances/obligations.
- Unknown is not zero; statement balance is not remaining due.
- DST, leap day, year boundary tests.
- SQL injection attempts through search/filter/sort/merchant alias fail safely; identifiers are enum-only.

### Phase 4
- Wizard completion/skipping, account create/edit/archive, restart persistence.
- Theme and close-behavior persistence; first-close prompt; hide is explained as not a lock.
- Auto-lock setting bounds; security status; credential removal; delete-all confirmation.
- No provider required.

### Phase 5
- Demo fixture expectations match backend totals exactly.
- Demo cannot open real DB, Keychain, import, export, or link.
- All spec §15 scenarios present; persistent demo label.
- Read-only workflows keyboard navigable; 1100×760 layout.

### Phase 6
- CSV: BOM, quoted commas/newlines, parentheses, debit/credit columns, explicit date format, invalid UTF-8/NUL, oversized file/row/field, path traversal/symlink.
- Identical reimport is a no-op; duplicate review; rollback preserves shared/unrelated rows.
- Formula injection variants; export 0600; no auto-open; renderer never receives a path.
- Overrides persist across restart and sync.

### Phase 7
- Backup creation/verification; wrong passphrase, tamper, truncation, wrong vault ID, wrong schema, nonempty restore, failed restore leaves original.
- Rekey changes passphrase/DEK; old passphrase fails; old backups labeled; crash recovery determinism.
- Portable export separate password; no provider tokens; no plaintext temp DB.

### Phase 8
- Idempotent repeat sync; pagination; pending/posted across pages; cursor rollback after failed write; lock mid-sync does not advance cursor.
- Concurrent triggers; one failed connection isolates; partial liabilities preserve prior values and mark stale.
- Scheduled skip while locked/quitting records why.
- Network allowlist/redirect/plain HTTP/TLS/oversize rejection; no token in logs/errors.

### Phase 9
- Sandbox link (hosted + polling) only if owner credentials exist; otherwise blocked with the concrete cause.
- No custom scheme, no loopback in Production, no host link URL in logs/persistence.
- Transaction sync/balance adapter pagination/removals; reconnect without duplicates.
- Capabilities/provenance shown; Liabilities unavailable is clearly labeled.

### Phase 10
- Independent review checklist; adversarial tests; dependency audit; accessibility; measured performance; signed artifact hash; native smoke record; README/SECURITY/handoff complete.

## 3. Required tests from spec §16 mapped to the suite

| Spec area | Where |
| --- | --- |
| Financial/domain | `src-tauri/src/domain/*` unit tests; `tests/ledger.rs`, `tests/domain_fixtures.rs`, `tests/properties.rs` |
| History/trend | `tests/ledger.rs` (`spend_history`), `src/mocks/data.test.ts`, `src/lib/bridge/bridge.test.ts` |
| Ledger presentation | `src/features/dashboard/ledgerPresentation.test.ts`, `e2e/dashboard.spec.ts` |
| Settings sections | `e2e/settings.spec.ts`, `e2e/helpers.ts` (`openSettingsSection`) |
| Vault/encryption | `src-tauri/tests/vault_cipher.rs`, `tests/vault_flow.rs`, `tests/backups.rs` |
| Database/sync | `tests/sync.rs`, `tests/vault_flow.rs` (migrations), `tests/demo.rs` |
| Import | `tests/imports.rs` |
| Security | `tests/vault_cipher.rs`, `tests/backups.rs`, `tests/plaid.rs`, `tests/plaid_report.rs`, Playwright `e2e/security.spec.ts`, capability/CSP assertions |
| UI | `src/**/*.test.tsx`, `e2e/*.spec.ts` |
| Performance | `tests/perf.rs` (ignored by default; run explicitly) |

## 4. Native macOS smoke checklist (manual, owner's M1 Max)

These cannot be proven by Playwright or CI. Each item records pass/fail and date in `docs/reference/implementation-status.md`.

1. App launches from the built `.app` and shows the exact title `cardigan_money`.
2. First launch offers vault creation; the native secure sheet appears (no HTML password field).
3. Passphrase with leading/trailing spaces is accepted exactly; wrong passphrase fails non-destructively; repeated failures delay.
4. Quit/relaunch requires the passphrase; same accounts and data return.
5. Lock Now (menu + Cmd+L), screen lock, sleep, fast user switching, logout, and idle timeout lock the vault and show only the lock screen.
6. While locked, no network request occurs and no ledger data is visible in the window.
7. Content protection: standard screen capture / screen sharing shows a blank/protected window.
8. State restoration: relaunch does not restore ledger content or a window snapshot.
9. Keychain: provider token item has `WhenUnlockedThisDeviceOnly`, non-synchronizable, data-protection attributes; deletion on disconnect.
10. File dialogs open natively; imports/exports never expose paths to the renderer.
11. Devtools cannot be opened in the release build.
12. No unexpected TCC prompts (Full Disk Access, Accessibility, Screen Recording, Camera, Microphone, Contacts, Calendar, Automation).
13. Ad-hoc signature verifies; recorded SHA-256 matches the artifact.
14. Core dump limit is zero for the running process (`launchctl limit core` / `ulimit -c` in the app is not authoritative; use process inspection).
