# Decision log: cardigan_money

Decisions that shape the build. The owner and security decisions are normative; the implementation log records concrete choices the implementing agent made within them, and the reason. Both decision tables below were extracted verbatim from the retired 2026-10 build specification when it was removed from the tree (see `docs/README.md`).

## Owner decisions (2026-10-23)

Recorded by the owner before implementation. Still normative unless a later decision supersedes them.

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
| Backups | Automatic backups and user backups are password-encrypted; legacy portable containers remain restorable. Retention is 7 daily, 10 manual, and 5 each pre-migration/pre-restore/pre-change, never the last one. |
| Currencies | Real data is USD-only. Non-USD handling remains a synthetic fixture for tests; no currency conversion. |
| Appearance | Light, dark, and follow-system modes with an in-app toggle. |
| Lifecycle | No macOS notifications in v1. No login item and no launch-at-login. The first window close asks whether to hide or quit, then remembers the choice. |
| Working style | Ask the owner only when a consequential decision is genuinely missing; otherwise proceed on documented defaults. |

## Security decisions (2026-10-23)

Adopted after the pre-implementation design review. Still normative; several were release blockers.

| Topic | Decision |
| --- | --- |
| Renderer trust | The React renderer is untrusted. It never receives the vault passphrase, the data encryption key, the key-encryption key, provider access tokens, the Plaid client secret, or raw provider payloads. Passphrases and the Plaid client secret are collected with a native macOS secure-input sheet; the renderer receives only success, failure, or cancellation. |
| Vault cryptography | Envelope encryption: a random 256-bit data encryption key (DEK) is the SQLCipher raw key; an Argon2id-derived key-encryption key (KEK) wraps the DEK with XChaCha20-Poly1305; KDF parameters, salt, wrapped DEK, vault ID, and generation live in an authenticated `vault.header` outside the encrypted database. The passphrase is never stored, logged, or sent off the Mac, and never placed in Keychain. |
| Fail closed | If SQLCipher is not actually active, the header is invalid, the KDF parameters are outside the allowed bounds, or integrity verification fails, the app refuses to open or create the ledger. There is no plaintext fallback and no automatic reset. |
| Backups and exports | Automatic backups are self-contained encrypted snapshots: the SQLCipher snapshot plus a header that carries the wrapped DEK. Automatic backup creation needs no extra prompt; restore always requires the passphrase. Passphrase change rotates the DEK and rekeys the live database, so older backups require the older passphrase and are labeled as such. Portable exports use a separate owner-chosen password and the same authenticated-container discipline. |
| Lock lifecycle | Manual Lock Now is required. The vault locks on macOS screen lock, sleep, fast user switching, logout, app quit, and after an idle timeout (default 15 minutes, configurable 1–240 minutes or disabled only with a visible warning). This supersedes the earlier "no auto-lock; locks on quit" decision. Hiding the window is not a lock. |
| Provider linking | No Production flow may depend on a custom URI scheme, a `cardigan_money://` deep link, or a loopback HTTP callback: Plaid currently requires HTTPS redirect URIs, does not support custom schemes, and allows localhost redirects only in Sandbox. The approved desktop flow must be verified in Phase 0. The preferred candidate is Hosted Link plus `/link/token/get` polling; a hosted HTTPS redirect page is the alternative. If neither is verified, live linking ships as unavailable with the concrete blocker. No server or cloud component is introduced silently. |
| Signing and Keychain | An unsigned local build remains acceptable for personal use, but it must be ad-hoc signed, its SHA-256 recorded, and its limits documented. A stable local signing identity (self-signed certificate) is the recommended way to get reliable Keychain ACL behavior during development; if the owner declines it, document the Keychain prompt behavior and accept that residual risk. Notarization/distribution signing remains out of scope; the build is not distribution-ready. |
| Demo mode | The demo database is synthetic-only, separate, and may use a fixed app-managed key. Demo mode disables CSV import, provider linking, backups, and exports so real data cannot be placed in an unprotected demo database. |
| Security gates | Security work is not deferred to Phase 10. The plan's Phase 0, 1, and 2 exit gates include the threat model, fail-closed vault, native secret input, renderer lockdown, and lock lifecycle. No phase is complete while a relevant Critical/High security finding is open. |

## Implementation log

Choices the implementing agent made inside those constraints.

| ID | Decision | Reason | Reversible? |
| --- | --- | --- | --- |
| D-001 | Workspace-local Rust toolchain and pnpm store under `.toolchain/` and `.cache/` | The build sandbox permits writes only below the repository root. This is an environment constraint, not an app requirement. The README documents normal global commands for the owner. | Yes |
| D-002 | Tauri 2.12, React 19.1, Vite 7.1, TypeScript 5.9, Tailwind 4.1, TanStack Table 8.21, Zod 4.1 | Pinned, compatible, maintained versions at build time. Avoids TS 7/Vite 8 immediately after major releases. | Yes |
| D-003 | SQLCipher via `rusqlite 0.37` + `bundled-sqlcipher-vendored-openssl` (SQLCipher 4.6.1 community, OpenSSL 3.6.3) | No system SQLCipher exists on the owner's Mac; vendored build is reproducible and avoids a plain-SQLite fallback. Verified `PRAGMA cipher_version` non-empty and raw-key rekey works. | Provider can change with a new tested build |
| D-004 | Envelope encryption: Argon2id KEK wraps a random 256-bit DEK with XChaCha20-Poly1305; SQLCipher receives only the raw DEK | Resolves the circular key-hierarchy finding (SEC-03) and lets automatic backups avoid retaining the passphrase (SEC-04). | Locked by spec §13.2 |
| D-005 | Passphrases and the Plaid client secret are collected only via a native AppKit `NSSecureTextField` sheet | Renderer compromise must not disclose the permanent passphrase (SEC-02). No HTML password field exists. | Locked by spec §1.2 |
| D-006 | External 152-byte authenticated `vault.header` plus mirror; AAD covers all public fields, including the nonce | Fail-closed integrity and rollback detection without a secret in the header. | Format version bump required |
| D-007 | Raw-key `PRAGMA rekey` is the rotation mechanism; WAL is checkpointed before and after rekey | Verified on the pinned build. No passphrase-derived SQLCipher key exists. | Locked |
| D-008 | One custom IPC command `gateway` with a versioned envelope and per-command typed payloads | Centralizes window/origin checks, size caps, rate limits, and sanitized errors; no generic bridge. | Command inventory can grow |
| D-009 | The `main` window is created in Rust, not `tauri.conf.json`, so navigation/new-window handlers exist before the first load | Guarantees the handlers cannot be bypassed by a config-created window. | Yes |
| D-010 | One capability (`main-window`) with only `core:event:default`; custom commands need no capability | Least privilege. File dialogs and I/O stay in Rust. | Permission list can be reduced further |
| D-011 | Ad-hoc signing and SHA-256 record; no self-signed certificate unless the owner asks | The owner's recorded decision allows ad-hoc for local use. Keychain prompts may vary after rebuilds; documented. | Yes, owner decision |
| D-012 | Plaid link flow is Hosted Link + `/link/token/get` polling from Rust; live linking is **unverified** until the owner enrols and a Sandbox test runs | Current documented constraints forbid custom schemes and Production loopback. No server/hosting is approved. See [plaid-feasibility.md](../integrations/plaid-feasibility.md). | Revisit after owner decision |
| D-013 | Demo mode is a separate database at `demo/demo.cmdb` with a fixed synthetic key and no header/passphrase | Synthetic data only; cannot open the real ledger. Demo is explicitly labeled. | Yes |
| D-014 | CSV import is a Rust-owned streaming job with an in-memory preview token; the renderer gets counts, never a path | Path and file I/O never cross IPC. Preview/confirm/rollback per batch. | Yes |
| D-015 | Dates are stored as date-only `YYYY-MM-DD`; month selection is `[first day, first day of next month)`; timezone defaults to `America/New_York` | Avoids midnight-UTC and DST date shifts. | Yes |
| D-016 | The mock desktop bridge is selected only when `import.meta.env.DEV` **and** `VITE_MOCK_BRIDGE=1`; production builds must not contain it | Browser tests must not be mistaken for native proof; release must not offer a fake backend. Build check scans `dist` for the mock. | Yes |
| D-017 | Automatic backups embed the current wrapped DEK; restore always prompts. Portable export is a separate-password container with a fresh export DEK | Matches the security review. | Format version bump |
| D-018 | A statement balance is displayed only as "last statement balance"; due amounts are only provider/manual dated facts; missing is never zero | Spec §8 and §11. | Locked |
| D-019 | Locking is enforced in Rust (native session/idle checks), not by renderer timers; hiding is not locking | SEC-05. Scheduled sync skips and records `skipped: vault locked`. | Locked |
| D-020 | The app's financial rules live only in Rust; the renderer formats backend-provided totals | One source of truth for calendar-month accounting. | Locked |
| D-021 | Rust IPC contract fixtures are committed under `fixtures/contract/` and parsed by both Rust and TypeScript tests | Prevents silent drift across the language boundary without a codegen dependency. | Yes |
| D-022 | Logging is off by default; the only release logging is redacted lifecycle/security metadata | No financial payloads, secrets, paths, or SQL parameters. | Yes |
| D-023 | Provide `docs/reference/implementation-status.md` after every phase and never mark a control verified without a test or recorded manual evidence | Plan §19 and the review's honesty requirements. | Locked |

| D-024 | Backup and restore overhaul: one **Back up now** action with an optional per-file passphrase; restore is staged/validated and supports same-vault, cross-vault, fresh-install, selective-scope, and destructive no-current-passphrase paths | Supersedes the portable-export-only destination model in D-017 while keeping legacy portable containers restorable. Cross-vault full restore adopts the backup identity and retires the previous vault's backups with `cm-retired-`. | Locked 2026-10-24 |
