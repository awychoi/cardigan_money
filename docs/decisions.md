# Decision log: cardigan_money

Decisions that shape the build. The specification and plan are the source of truth; this file records concrete choices the implementing agent made within them, and the reason.

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
| D-012 | Plaid link flow is Hosted Link + `/link/token/get` polling from Rust; live linking is **unverified** until the owner enrols and a Sandbox test runs | Current documented constraints forbid custom schemes and Production loopback. No server/hosting is approved. See [plaid-feasibility.md](plaid-feasibility.md). | Revisit after owner decision |
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
| D-023 | Provide `docs/implementation-status.md` after every phase and never mark a control verified without a test or recorded manual evidence | Plan §19 and the review's honesty requirements. | Locked |
