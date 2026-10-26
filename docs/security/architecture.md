# Security architecture: cardigan_money

Status: Phase 0 design, implemented incrementally. This document records the actual formats used by the code, not aspirational ones. Where a control is not yet verified, [implementation-status.md](../reference/implementation-status.md) says so.

## 1. Key hierarchy

```text
owner passphrase (native NSSecureTextField; never in the renderer)
    │ Argon2id(salt, m_cost, t_cost, p_cost)      [security/crypto.rs]
    ▼
KEK (32 bytes, zeroized after the DEK is unwrapped)
    │ XChaCha20-Poly1305 unwrap, AAD = public header fields
    ▼
DEK (32 random bytes, mlock best-effort while unlocked)
    ├── SQLCipher raw key  PRAGMA key = "x'<64 hex>'"   [security/cipher.rs]
    │      └── ledger.cmdb + encrypted WAL
    ├── SQLCipher raw key for consistent encrypted snapshots
    └── wrapped under an export KEK for legacy portable containers (retained for compatibility)
```

- The passphrase is never stored, logged, sent over IPC, placed in Keychain, or written to disk.
- SQLCipher is never given a passphrase. It receives a 256-bit raw key.
- The DEK and KEK are custom `LockedKey` values: `mlock` best-effort, zeroize on drop, no `Debug`, `Clone`, `Serialize`, or `Display`.
- `vault.header` contains the wrapped DEK and KDF metadata but nothing usable without the passphrase.

### KDF parameters

| Parameter | Default at creation | Hard bounds (validated before derivation) |
| --- | --- | --- |
| algorithm | Argon2id v1.3 | only Argon2id id 1 is accepted; unknown/legacy ids fail closed |
| memory | 65,536 KiB (64 MiB) | 8,192–262,144 KiB |
| iterations | 3 (raised up to 10 by calibration if unlock < ~350 ms) | 1–10 |
| lanes | 1 | 1–4; memory ≥ 8 KiB × lanes |
| salt | 16 random bytes | exactly 16 bytes |
| output | 32 bytes | exactly 32 bytes |

Header parameters outside the bounds are rejected **before** Argon2 runs, so a tampered header cannot cause a multi-second or memory-exhausting DoS. A successful unlock never silently downgrades parameters. Calibration is bounded and only performed at vault creation/rotation.

### Passphrase policy

- Exact input is preserved: no trim, no case folding, no Unicode normalization. Leading/trailing spaces count.
- 1–1,024 characters. At least 16 characters, or a conservative estimate of ≥60 bits for shorter passphrases. No composition rules.
- Spaces and long passphrases are allowed. No hints, security questions, "remember me", or Keychain unlock.
- A short exponential delay follows repeated wrong passphrases. There is no permanent lockout (avoids DoS/data loss).

## 2. `vault.header` format (format version 1)

Fixed 152-byte binary file, little-endian. It lives outside the database in the 0700 vault directory, is written atomically and mirrored as `vault.header.bak`.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | magic `CMVAULT1` |
| 8 | 2 | format version = 1 |
| 10 | 1 | cipher id = 1 (SQLCipher 4) |
| 11 | 1 | KDF id = 1 (Argon2id) |
| 12 | 4 | Argon2id memory KiB |
| 16 | 4 | Argon2id iterations |
| 20 | 4 | Argon2id lanes |
| 24 | 16 | KDF salt |
| 40 | 16 | vault UUID |
| 56 | 4 | generation (monotonic, starts at 1) |
| 60 | 4 | schema version |
| 64 | 8 | creation timestamp (Unix seconds) |
| 72 | 8 | last rotation timestamp (0 = never) |
| 80 | 24 | XChaCha20-Poly1305 wrap nonce |
| 104 | 48 | wrapped DEK (32-byte ciphertext + 16-byte tag) |

- **AAD** is bytes `0..104` (everything through the nonce). Any modification to magic, version, cipher/KDF ids, parameters, salt, vault ID, generation, schema version, timestamps, or nonce makes the AEAD unwrap fail.
- A wrong passphrase and a tampered header are indistinguishable at the crypto layer; both fail closed with `wrong_passphrase`. There is no oracle that distinguishes them.
- Truncation or a trailing byte is rejected by an exact-length parse.
- Loading tries the primary, then the mirror, and picks the highest valid generation. If the primary is invalid and the mirror is valid, the app repairs the primary on the next successful unlock.
- Losing **both** copies makes the ledger unreadable unless a self-contained backup exists. The UI and README state this.

### Rotation journal

Passphrase change uses a journal at `vault/rotation.journal`:

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | magic `CMROTAT1` |
| 8 | 4 | previous generation |
| 12 | 152 | new header (wrapped new DEK under the new passphrase) |

Sequence: pre-change encrypted backup → write journal → `PRAGMA rekey` the live DB with the new raw DEK → update `vault_metadata.generation` → write new header/mirror → delete journal. On unlock, if the primary header fails but the journal header opens the DB and matches its metadata, the journal is finalized. If the primary header opens the DB, any journal is stale and removed. If neither opens it, fail closed and point at the pre-change backup.

## 3. SQLCipher activation and fail-closed rules

Pinned provider: `rusqlite 0.37` with `bundled-sqlcipher-vendored-openssl` (libsqlite3-sys 0.35). The resolved SQLCipher version is recorded in [implementation-status.md](../reference/implementation-status.md) from the test output.

Order on every connection (`security/cipher.rs::open_ledger`):

1. Reject symlinks and non-owner files. Reject a file whose first 16 bytes are `SQLite format 3\0`.
2. Open **without** `CREATE` for an existing ledger. Only an explicit creation flow uses `CREATE`, and only after confirming the path does not exist. A failed unlock can never create an empty database.
3. Execute `PRAGMA key = "x'<hex>'"` as the first statement.
4. Require `PRAGMA cipher_version` starting with `4.`, a non-empty `cipher_provider`, and a successful keyed read of `sqlite_master`.
5. If this SQLCipher build supports `PRAGMA cipher_status` (added in 4.12), it must return `1`. Older maintained builds report it unsupported; the keyed read and full integrity check remain mandatory.
6. Apply `cipher_memory_security=ON`, `foreign_keys=ON`, `journal_mode=WAL` (verified), `synchronous=FULL`, `secure_delete=ON`, `temp_store=MEMORY`, `trusted_schema=OFF`, `recursive_triggers=OFF`, a 5-second busy timeout, and SQLite defensive mode.
7. At unlock, after restore, and after backup creation, run full `PRAGMA cipher_integrity_check`; an empty result is success. `PRAGMA integrity_check` is also run.

Any mismatch closes the connection, drops and zeroizes the key, and returns a sanitized error. There is no plaintext fallback, no `cipher_migrate`, no `cipher_compatibility` downgrade, no `cipher_plaintext_header_size`, and no extension loading. A test-only plain-SQLite probe proves the verification rejects a build where `PRAGMA key` is a no-op.

WAL and SHM are verified by test to contain no plaintext sentinel after writes. `temp_store=MEMORY` and `secure_delete=ON` reduce plaintext exposure inside SQLite; SSD remanence remains a documented limit.

## 4. Native secure input

- AppKit `NSAlert` + `NSSecureTextField` accessory view, built in Rust (`security/secure_input.rs`), presented modally on the main thread.
- Used for: create passphrase (two fields + match), unlock, passphrase change, unified backup passphrase, locked-backup current passphrase, restore backup/current passphrases, post-restore new passphrase, and the Plaid client secret.
- The renderer supplies only a purpose enum. Titles, messages, and field count are fixed by Rust. The renderer cannot overlay or read the sheet.
- Returned values become `SecretString`; they are never logged or serialized. For creation, the two entries are compared in Rust (constant-time for the equality check where practical), then the zeroizing buffers are dropped.
- The sheet window sets `NSWindowSharingNone` for the standard screen-capture path. This is the same best-effort protection as the main window; cameras/remote support are out of scope.
- If the native sheet fails to present, the operation fails closed. There is no HTML password field fallback.

## 5. Lock lifecycle

States: `Locked` (no DB connection, no DEK) and `Unlocked` (DEK in `LockedKey`, one SQLCipher connection owned by the session).

Lock triggers:

- Manual **Lock Now** (app menu, `Cmd+L`, and a gateway command).
- macOS screen lock (poll of `CGSessionCopyCurrentDictionary`'s `CGSSessionScreenIsLocked`).
- Sleep (wall-clock vs monotonic gap and workspace notifications where available).
- Fast user switching (console session resolution) and logout (`NSWorkspaceWillPowerOff` / session notifications).
- App quit (Tauri exit hook and window close when quitting).
- Idle auto-lock in Rust: default 15 minutes, configurable 1–240 minutes; disabled only with a visible warning. Idle is computed from native HID activity (`CGEventSourceSecondsSinceLastEventType`), not renderer-reported activity.
- Hiding the window is **not** a lock; the idle timer continues while hidden.

On lock: stop/cancel in-flight sync at a safe point without advancing the cursor, close the connection, drop and zeroize the DEK, emit a single `vault:locked` event to `main`, and the renderer clears all query caches, search/filter state, and sensitive views. The lock screen is shown before any data can be re-fetched. The renderer is never trusted to enforce the lock.

## 6. Database schema notes

- `vault_metadata` (inside the encrypted DB) stores only audit data: vault ID, cipher/provider version strings, schema version, generation, timestamps, last integrity check.
- KDF parameters and the wrapped DEK exist only in `vault.header`. The encrypted table is sufficient for an audit trail, not for unlocking.
- Schema uses `STRICT` tables, `CHECK` constraints, foreign keys, and prepared statements only. Sort/filter identifiers come from closed enums.
- No secrets, tokens, passphrases, or wrapped keys are stored in `settings`.

## 7. Backup container (format version 1)

Self-contained authenticated snapshot: 256-byte header + SQLCipher database bytes. The header embeds a complete vault header, so a backup can be opened without the live `vault.header`.

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 8 | magic `CMBAK01\0` |
| 8 | 2 | format version = 1 |
| 10 | 1 | kind: 1 daily, 2 manual, 3 pre-migration, 4 pre-restore, 5 pre-change, 6 portable |
| 11 | 1 | cipher id = 1 (SQLCipher 4) |
| 12 | 2 | reserved |
| 14 | 152 | embedded vault header (KDF params, salt, wrapped DEK, vault ID, generation, schema) |
| 166 | 8 | created-at Unix seconds |
| 174 | 8 | DB byte length |
| 182 | 32 | SHA-256 of DB bytes |
| 214 | 24 | XChaCha20-Poly1305 seal nonce |
| 238 | 16 | seal tag over the header AAD (empty plaintext) |
| 254 | 2 | reserved |
| 256 | db_len | SQLCipher database bytes |

- **AAD** is bytes `0..238`. The seal key is `SHA-256("cardigan_money backup seal v1" || DEK)`. The embedded vault header is authenticated by the passphrase-derived KEK; the seal binds the public backup manifest and DB hash to the DEK. An attacker without the passphrase can modify neither validly. The DB hash is recomputed and compared before staging.
- A backup from a different vault ID is accepted only through the explicit cross-vault restore flow. Wrong passphrase, header tamper, seal tamper, DB tamper, truncation, or a changed file length fail before replacement.
- **Automatic backups need no retained passphrase**: the embedded vault header is copied from the live vault, and the manifest seal uses the in-memory DEK. A backup made before a rotation opens with the old passphrase only.
- **Portable containers** (legacy kind 6) remain restorable. New user backups use the `manual` kind; a non-blank backup passphrase generates a fresh header and DEK under the current KDF parameters with the same `vault_id`, and the snapshot is rekeyed to it.
- **Restore** is staged and validated before install. Full validation includes the header, embedded vault header, seal, length, DB SHA-256, `vault_id`, SQLCipher/SQLite integrity, foreign keys, schema compatibility, and a schema-object allowlist. The staged database is migrated, the selected scope is applied, and it is rekeyed to the target key.
  - Same-vault full and partial restore keep the current header and DEK. A verified pre-restore backup is mandatory and any failure aborts before the swap.
  - Cross-vault full and fresh full restore create a fresh header/DEK under the backup passphrase with the backup `vault_id`; they never adopt the backup's DEK. Fresh installs are assembled in a temporary directory and renamed into place.
  - A destructive full restore for a locked vault without the current passphrase skips the pre-restore backup, keeps the old files until the new vault verifies, and requires typed confirmation plus a provider-token choice.
  - Every restore marks restored provider snapshots with `restored = 1`, clears the restored connections' cursors and last-success times, and resets credential references when the identity is replaced. Restored live balances, credit values, limits, and due metadata stay hidden until fresh provider data arrives.
  - A restored Plaid connection whose `credential_ref` still has a Keychain token can reuse its Item. The reference and token are kept, and the connection stays `healthy` with no cursor so the next sync is a full resync without creating a Plaid Item. This is automatic for same-vault and fresh-install restores and an explicit `reuse-local` choice for cross-vault and destructive restores.
  - A restored connection with no local token rotates to a `newlink-…` reference and starts `reconnect_required`; sync is blocked and the UI offers a new link. Cross-vault choices are reuse local Items, delete local tokens, delete and revoke remotely, or keep for rollback; a scope that resets connections deletes the old tokens. Tokens never travel in the backup, so a restore on another Mac always requires a new link.
  - Reused connections are resynced after an unlocked restore and on unlock after a locked restore. A revoked token demotes the connection to `reconnect_required` on the next sync. A full cross-vault restore renames the previous vault's normal backups with `cm-retired-`; retired containers are excluded from listing and pruning and remain until manually deleted.
- **Retention** matches only normal container filenames and only the current vault: 7 daily, 10 manual, 5 each pre-migration/pre-restore/pre-change, never the last remaining backup. Retired containers are exempt. Only regular files are considered; symlinks are skipped/rejected. The normal list also filters by `vault_id`.
- **Crypto-erase** locks the session, best-effort overwrites and deletes the vault, header mirror/journal, backups, demo database, and staged files, then fsyncs. Files outside the app-data directory and external copies are explicitly out of scope.

## 8. Keychain

- Service `com.awychoi.cardiganmoney.plaid`; one generic-password item per connection credential reference; separate client ID/secret items for **Sandbox** and **Trial/Production** so environments cannot mix credentials.
- Attributes: `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`, `kSecAttrSynchronizable=false`, data-protection Keychain (`kSecUseDataProtectionKeychain=true`).
- Labels contain only the local connection UUID — no account numbers.
- Tokens are loaded only for the duration of a Rust request and zeroized after use. They are never included in backups, exports, logs, errors, or renderer state.
- A restored connection can keep its Keychain token only when the restored ledger carries the same random `credential_ref` and a token exists for it on this Mac. Cross-vault reuse is explicit; a token never travels with a backup, so this cannot extend provider access to another machine.
- Disconnect deletes the Keychain item. A local disconnect does not silently revoke a remote Plaid Item; remote removal requires explicit confirmation. A Keychain failure fails closed for that provider operation while the offline ledger remains usable.

## 9. Network boundary

- Rust-only `reqwest` client: TLS verification on, HTTPS-only, redirects disabled, connect/read timeouts, response size cap, no compression features, and an exact host allowlist (`sandbox.plaid.com`, `production.plaid.com`).
- No URLs from provider payloads, institutions, imported files, or the renderer are fetched. No remote images/fonts/scripts. No inbound listener, webhook, or local server.
- Provider JSON is typed, bounded, and rejected on malformed/oversized content. Raw payloads are not retained.
- Plaid client secret, link tokens, public tokens, and access tokens stay in Rust/Keychain, are zeroized after use, and are never logged or placed in a URL the app controls. The Hosted Link URL is opened directly in the system browser through a Rust command that accepts only the exact verified Plaid host; it is never logged or persisted. Browser-history exposure is a documented residual risk.
- No automatic billable refresh endpoints. Token exchange is not blindly retried.

## 10. IPC gateway

- One custom Tauri command, `gateway`, carrying a versioned, internally tagged request enum. Each variant has `deny_unknown_fields`, explicit bounds, and typed response data. There is no generic SQL, file, URL, shell, or HTTP command.
- Every call checks `webview.label() == "main"` and an origin allowlist. Events are emitted only to `main`.
- Sensitive operations are rate-limited (native unlock attempts, backup/restore/export, delete-all, Plaid calls). A failed unlock applies an increasing delay. All limits reset only on success or lock, not by renderer request.
- Errors are `{code, message}` with static, sanitized messages. No SQL, paths, tokens, account numbers, or provider payloads.
- The mock bridge is TypeScript-only, behind `import.meta.env.DEV` plus a test flag, and cannot be selected in a release build. Rust is authoritative; contract fixtures are tested on both sides.
- The renderer stores no financial data or secrets in localStorage, sessionStorage, IndexedDB, Cache API, service workers, or URLs. On lock it unmounts sensitive views and clears caches.

## 11. Renderer and Tauri baseline

- Release CSP: `default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; frame-src 'none'; worker-src 'none'; manifest-src 'none'; media-src 'none'`. No HTTPS source, no `unsafe-eval`.
- A separate dev CSP allows only the loopback Vite server and HMR socket.
- One capability, `main-window`, scoped to the `main` window, with only core event permissions. No fs/shell/http/opener/sql/store/process/updater/deep-link/dialog/clipboard/websocket/upload permissions.
- `freezePrototype: true`, `withGlobalTauri: false`, asset protocol disabled, devtools disabled, drag-and-drop disabled, navigation restricted to the app origin, new windows denied.
- `set_content_protected(true)` on the main window; state restoration disabled; no financial data in titles, menus, notifications, or saved state.
- No `dangerouslySetInnerHTML`, `innerHTML`, `eval`, `new Function`, remote assets, or Markdown-to-HTML. Provider/CSV text is rendered as text with control/bidi characters stripped for display.

## 12. Logging and crash hygiene

- Logging is off by default. The redacting logger never records financial payloads, secrets, tokens, headers, SQL parameters, file paths, or account numbers. Diagnostics are local, explicit, rotated, and capped if ever enabled.
- Core dumps disabled (`RLIMIT_CORE=0`); panic hook emits no sensitive data; no third-party crash reporting; no debug symbols in release; `cipher_log` never enabled at DEBUG/TRACE in release.
- macOS Crash Reporter may still record a backtrace; heap contents are not intentionally included. Documented as residual risk.

## 13. Test vectors

Deterministic vectors are recorded from the Rust test suite. They use test-only KDF parameters (8 MiB, 1 iteration) for speed and are **not** production parameters.

- Argon2id (v1.3, m=8192 KiB, t=1, p=1, salt `000102…0f`, passphrase `correct horse battery staple`): recorded in the Phase 0 test output and repeated here after `cargo test`.
- XChaCha20-Poly1305 wrap/unwrap: verified round-trip, AAD mismatch, ciphertext tamper, and nonce tamper all fail.
- Header tamper: every byte in the 152-byte header is flipped in a test; decode or unwrap fails for all 152 positions.
- Truncation/extension: any length other than 152 is rejected.
- Raw-key rekey: proven on the pinned SQLCipher build; old key fails, new key opens, integrity check passes.

Exact hex outputs are appended by `docs/security-test-report.md` when Phase 10 assembles the evidence bundle.
