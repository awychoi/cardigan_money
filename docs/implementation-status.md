# Implementation status: cardigan_money

Last updated: Phase 0 exit and Phase 1 verification (2026-10-23, America/New_York).
This is the honesty document: it records what was built, what actually ran, and what remains unverified. It is updated at the end of every phase.

## 1. Phase tracker

| Phase | Outcome | Status |
| --- | --- | --- |
| 0 | Foundation, threat model, crypto design, SQLCipher and Plaid feasibility | **Complete** |
| 1 | Tauri shell, command gateway, renderer lockdown | **Complete** |
| 2 | Vault envelope, native secret input, lock lifecycle, migrations | Not started |
| 3 | Domain rules, input bounds, calendar-month accounting | Not started |
| 4 | Onboarding, accounts, settings, theme, security settings | Not started |
| 5 | Demo data and read-only dashboard | Not started |
| 6 | Edits, manual metadata, CSV import/export security | Not started |
| 7 | Encrypted backup, restore, rotation, recovery | Not started |
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
| SEC-02 passphrase in renderer | **Not yet implemented** (Phase 2). No HTML password input exists; the shell has no passphrase flow yet. |
| SEC-03 circular key hierarchy | **Mitigated**: external authenticated header; in-memory-only DEK. |
| SEC-04 backup vs. no-prompt | Designed (header-embedded wrapped DEK); implementation Phase 7. |
| SEC-05 lock lifecycle | Designed; implementation Phase 2. No lock screen yet. |
| SEC-06 Plaid desktop flow | **Documented blocker**: Hosted Link + polling, no credentials, unverified. |
| SEC-07 renderer privilege/network | **Mitigated for the shell**: CSP, one capability, gateway, nav/new-window blocking, no remote content. Native release verification pending Phase 10. |
| SEC-08 Keychain/signing | Design recorded; Keychain implementation Phase 8/9; signing ad-hoc in Phase 10. |
| SEC-09 security deferred | Phases 0–2 gates enforced; Phase 0/1 evidence recorded. |
| SEC-10 supply chain | Lockfiles, pinned toolchains, audits, explicit build-script allowlist (`esbuild`, `@tailwindcss/oxide`). |
| SEC-11 logs/crash/state restoration | Core dumps disabled; redacting logger off by default; state restoration disabled. Crash diagnostics in debug only. |
| SEC-12..20 | Corresponding features are not built yet; controls are designed in the spec and tracked per phase. |

Open Critical/High findings: none newly discovered. SEC-02 (native secure input) and SEC-05 (lock lifecycle) remain open until Phase 2; they are anticipated by the plan and do not allow a false claim because no vault or passphrase UI exists yet.

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

Phase 2 will implement the native secure-input sheet, the vault create/unlock/change flow, the lock lifecycle, single-instance-before-DB, migrations, and the encrypted settings/vault/security-event schema. Its exit gate requires a real create → quit → relaunch → unlock cycle on this Mac (native smoke where the sandbox permits) plus tests for wrong passphrase, header tamper, lock/zeroize, and migration failure.
