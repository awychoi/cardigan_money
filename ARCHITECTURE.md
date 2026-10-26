# Architecture: cardigan_money

A technical tour of how cardigan_money is put together: the process boundary, the encryption and
storage model, how provider credentials move through the system, and what a backup and restore
actually do. [SECURITY.md](SECURITY.md) states the security claims and residual risks;
[README.md](README.md) is the user-facing overview.

**Status:** local, single-user Apple Silicon build, ad-hoc signed and not distribution-ready. A real
Plaid Trial/Production Item has not yet been verified; Sandbox linking is verified.

## 1. Process and trust boundaries

cardigan_money is a Tauri 2 application.

```
┌──────────────────────────── macOS app ─────────────────────────────┐
│  WebView (untrusted renderer)                                      │
│  React + TypeScript + Vite                                         │
│  No network, no filesystem, no SQL, no Keychain access             │
│                              │                                     │
│                    one IPC command: gateway                        │
│                    versioned envelope (v1), typed payloads        │
│                    window/origin check, size caps, rate limits     │
│                              │                                     │
│  Rust core (trusted)         ▼                                     │
│  ├── vault: SQLCipher DB + authenticated header                    │
│  ├── crypto: Argon2id KEK, XChaCha20-Poly1305, random 256-bit DEK  │
│  ├── macOS Keychain: Plaid client credentials and Item tokens      │
│  ├── providers: Plaid (allowlisted HTTPS), CSV import              │
│  ├── services: restore, backups, sync, scheduler, exports          │
│  └── native AppKit sheets for every secret                         │
└────────────────────────────────────────────────────────────────────┘
```

- The renderer is treated as hostile. It receives formatted data, never the passphrase, DEK, KEK,
  provider tokens, the Plaid client secret, raw provider payloads, or filesystem paths.
- Secrets are collected with native AppKit `NSSecureTextField` sheets. The renderer sees only
  success, failure, or cancellation.
- One Tauri capability (`main-window`) and one gateway command. New windows and navigation are
  blocked; the CSP allows no remote asset origin.
- The frontend IPC contract is validated twice: Rust deserializes with `deny_unknown_fields`, and
  the TypeScript client parses every response with Zod. Committed fixtures under
  `fixtures/contract/` keep both sides aligned.

## 2. Vault cryptography and storage

- A random 256-bit **data encryption key (DEK)** is the SQLCipher raw key.
- An Argon2id **key-encryption key (KEK)** derives from the passphrase and wraps the DEK with
  XChaCha20-Poly1305.
- KDF parameters, salt, wrapped DEK, vault ID, and generation live in a 152-byte authenticated
  `vault.header` plus mirror, outside the encrypted database. The header is bound as AAD.
- The app fails closed if SQLCipher is not actually active or any integrity check fails. There is no
  plaintext fallback.
- The database is SQLite in STRICT-table mode with forward-only, checksummed migrations
  (`src-tauri/migrations/`). A pre-migration encrypted snapshot is written before a non-empty ledger
  migrates.
- The passphrase is never stored, logged, sent over IPC, or placed in Keychain. Losing it makes the
  ledger unreadable; only a backup passphrase can recover a restored copy.

Domain tables cover accounts, transactions, transaction overrides, merchant rules, balance and
payment snapshots, manual payment metadata, CSV import batches, sync runs, connections, security
events, and settings. Live provider snapshots are separated from restored snapshots by a `restored`
flag so a rollback never masquerades as fresh bank data.

## 3. Provider credentials and the Plaid lifecycle

There are two kinds of provider secrets, and they live in different places.

| Secret | Where it lives | Scope |
| --- | --- | --- |
| Plaid `client_id` + secret | macOS Keychain, one slot per environment | App-wide, per Sandbox / Trial-Production |
| Plaid Item `access_token` | macOS Keychain, account `plaid-access-token-<credential_ref>` | One per linked Item |
| `credential_ref` | Encrypted ledger, `connections.credential_ref` | Points at the Keychain token; contains no secret |

Keychain items use `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` and are not synchronizable, so they
never leave the Mac and never enter a backup. Backups deliberately exclude both secret kinds.

A connection is linked in Rust:

1. A Hosted Link session is created and opened in the system browser (no custom scheme, no loopback
   listener). The link token and URL stay in Rust memory.
2. The app polls `/link/token/get` from Rust. Duplicate link metadata is refused **before**
   `/item/public_token/exchange`, so an accidental second link does not consume a Plaid Item.
3. On success, the access token is stored in Keychain under a fresh random `credential_ref`, and the
   ledger row records provider, environment, institution, status, and cursor.

Sync runs in Rust after unlock, keeps a per-connection cursor, and only advances it after the page
commits. A re-authentication error demotes the connection to `reconnect_required`; other failures
record a sanitized error. Scheduled sync targets 08:00 and 20:00 local time and never runs while the
vault is locked.

**Update/Reconnect** uses Plaid update mode with the stored token: it repairs or extends the existing
Item and does not create another one. A connection without a token (`newlink-…` reference) instead
offers **Link again…**, which creates a new Item.

Status values: `healthy`, `reconnect_required`, `error`, `disconnected`. A `healthy` connection with
no cursor is a restored connection pending its first full resync.

## 4. Backups

A backup is a self-contained encrypted container (`.cmbak`): a SQLCipher snapshot plus the
authenticated header that carries the wrapped DEK. Kinds are daily, manual, pre-migration,
pre-restore, and pre-change. Retention keeps 7 daily, 10 manual, and 5 of each pre-change kind,
never the last remaining backup, and never retired containers. A backup saved outside the app's
backup folder is never pruned by the app.

## 5. Restore and Plaid Item reuse

Restore is staged: the candidate is copied, decrypted, migrated, rewritten, and fully validated
before the live vault is touched. Scopes are full database, financial data + settings, and financial
data only; partial scopes reset connections. Cross-vault and fresh restores may adopt the backup's
vault identity and passphrase, and a destructive path exists for a locked vault whose passphrase is
lost.

After any restore, restored transaction history stays visible, but restored provider snapshots are
marked `restored = 1` and the dashboard hides live balances, credit values, limits, and due metadata
until fresh provider data arrives. Every restored connection clears its cursor and last-success
time, so the next sync is a full resync (provider IDs are upserted, so this is safe).

What happens to the Plaid tokens depends on what this Mac still has:

| Restore path | Keychain token present | Keychain token absent |
| --- | --- | --- |
| Same-vault full restore | Keep `credential_ref` and token; connection is `healthy` with no cursor; resync reuses the Item | Keep the reference, start `reconnect_required`; the UI offers a new link |
| Cross-vault full restore, **reuse local Items** | Keep reference and token; reuse the Item as above | Rotate to a `newlink-…` reference; `reconnect_required`; new Item |
| Cross-vault full restore, delete local / delete and revoke / keep | Tokens deleted or revoked per choice, or kept but orphaned; references rotate; new Item required | New Item required |
| Fresh-install restore | Reuse any matching local token automatically | New Item required |
| Destructive restore | Reuse only when chosen; otherwise delete-all/leave-all per choice | New Item required |
| Any scope that resets connections | Connections and their tokens are deleted | — |

Reuse is possible because a Plaid access token is Item-scoped and device-local: it is not bound to
the vault identity or passphrase. When a restored `credential_ref` still has a token in this Mac's
Keychain, keeping the reference is enough to call Plaid again with the same Item.

Consequences:

- Reusing an Item preserves Plaid Trial capacity. The free Trial allows 10 Production Items, and
  `/item/remove` does **not** restore capacity, so a forced re-link is permanent loss.
- Reuse cannot cross machines. Tokens never enter backups, and Plaid has no API to list or claim an
  Item without its access token, so a backup restored on another Mac must create new Items.
- If a token was revoked at the bank, the first sync fails with a re-authentication error and the
  connection demotes to `reconnect_required`, where update mode or a new link can repair it.
- After an unlocked restore, and on unlock after a locked restore, the app starts a background sync
  for reused connections. Errors are recorded by the sync service, never surfaced as restore
  failures.
- Reuse only binds a reference that came from the restored ledger to a token already on this Mac.
  Tokens never travel in the backup, references are random UUIDs, and cross-vault reuse is an explicit
  choice. The remaining trust boundary is documented in
  [docs/security/threat-model.md](docs/security/threat-model.md).

## 6. Network and data handling

- The only outbound host allowlist is Plaid's two documented hosts. Requests and responses are
  size-capped, TLS-only, and parsed as untrusted input.
- CSV import is a Rust-owned streaming job; the renderer receives an opaque job id, counts, and
  preview rows, never a path. Limits are 20 MiB, 100,000 rows, 40 columns, and 4,096 characters per
  field.
- CSV export always writes plaintext to a user-chosen location, with an explicit warning, owner-only
  permissions, and formula-injection escaping.
- Logging is off by default; release logging is redacted lifecycle/security metadata only.

## 7. Build, tests, and release artifacts

The Rust toolchain and package caches can live inside the checkout for sandboxed builds:

```sh
. ./scripts/dev-env.sh          # sets RUSTUP_HOME, CARGO_HOME, stores, Playwright browsers
pnpm verify                     # typecheck, lint, Vitest, production web build
pnpm verify:all                 # adds format:check, Playwright, cargo fmt/clippy/test
pnpm tauri build                # macOS .app, then ad-hoc sign and record SHA-256
```

Test layers:

- Rust integration tests in `src-tauri/tests/` (vault, cipher, restore matrix, backups, sync,
  imports, ledger, Plaid provider, properties, performance).
- Vitest component/unit tests in `src/`.
- Playwright browser tests against the mock bridge only (`VITE_MOCK_BRIDGE=1`); they prove UI
  behavior, not native behavior.
- Scripted checks for plain-SQLite rejection and release-artifact scanning.

Release artifacts are ad-hoc signed (`codesign --force --deep --sign -`), verified with
`codesign --verify --deep --strict`, hashed, and recorded in
[docs/reference/implementation-status.md](docs/reference/implementation-status.md).

## 8. Further reading

- [SECURITY.md](SECURITY.md) — security policy, claims, and residual risks.
- [docs/security/architecture.md](docs/security/architecture.md) — control-by-control security
  architecture.
- [docs/security/threat-model.md](docs/security/threat-model.md) — assets, adversaries, controls.
- [docs/integrations/plaid-guide.md](docs/integrations/plaid-guide.md) — Plaid setup, linking,
  reconnect, duplicate prevention, and Trial limits.
- [docs/plans/backup-restore-overhaul-2026-10.md](docs/plans/backup-restore-overhaul-2026-10.md) —
  restore matrix and Plaid token choices, including the 2026-10-26 reuse revision.
- [docs/reference/decisions.md](docs/reference/decisions.md) — decision log.
