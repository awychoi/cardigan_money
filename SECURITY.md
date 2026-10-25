# Security policy and incident runbook: cardigan_money

cardigan_money is a local, single-user, encrypted macOS app. It is not a hosted service and has no server component. This document states what the app protects, what it does not, and what to do when something goes wrong.

**No system is watertight.** This is an ad-hoc-signed local build on a general-purpose operating system. Read the residual risks in §7 before relying on it.

## 1. What protects the data

| Control | Implementation |
| --- | --- |
| Encryption at rest | SQLCipher (4.x) opened with a random 256-bit raw key; database and WAL are ciphertext |
| Key hierarchy | Argon2id-derived KEK wraps the DEK with XChaCha20-Poly1305; wrapped DEK lives in an authenticated external header |
| Passphrase entry | Native AppKit `NSSecureTextField` sheet; the renderer never receives the passphrase |
| Renderer boundary | One Tauri window, one capability, typed command gateway, strict CSP with no HTTPS source, blocked navigation/new windows, devtools off |
| Lock lifecycle | Manual Lock Now, screen lock, sleep, fast user switching, logout, quit, and idle timeout (default 15 minutes) close the DB and zeroize keys |
| Provider secrets | macOS Keychain only (`WhenUnlockedThisDeviceOnly`, non-synchronizable); never in the ledger, backups, exports, logs, or renderer |
| Backups | Self-contained authenticated containers (encrypted DB + wrapped DEK header); restore always asks for the passphrase |
| Files | 0700 directories, 0600 files, symlink rejection, atomic writes |
| Network | Rust-only client, TLS verification, HTTPS-only, no redirects, exact `*.plaid.com` allowlist, response caps; the renderer has no network source at all |
| Audit trail | Encrypted `security_events` (unlock failures, lock, backup/restore/export, rekey, credential changes, integrity failure, single-instance rejection) |

## 2. What the app does not protect against

- A compromised macOS account or root/kernel attacker can read the unlocked process. `mlock` and zeroization are best-effort.
- Screen capture is not fully preventable. Content protection blocks the standard path; cameras, some remote-support tools, or a compromised window server may not be blocked.
- A weak or reused passphrase defeats Argon2id. A lost passphrase and lost header are unrecoverable without a backup whose passphrase is known.
- Old backups remain decryptable with the old passphrase after a rotation until deleted.
- Exported CSV is plaintext once written. Cloud sync/copies are outside the app.
- A compromised renderer can still invoke allowed commands and ask the user to unlock; native input prevents silent passphrase capture, not post-unlock abuse.
- TLS depends on the macOS trust store/proxy; there is no certificate pinning.
- A full rollback of both `vault.header` and the database to an older consistent pair is not detectable without external monotonic state.
- Hosted Link places a short-lived bearer URL in the default browser; browser history/extensions are outside the app's control.
- Crypto-erasure is logical, not physical. SSD wear-leveling and copies outside the app remain.
- There is no auto-update, notarization, hardened runtime, App Sandbox, hardware-backed key, Touch ID unlock, or MFA.

The full list is in the [pre-implementation design review](docs/security/design-review-2026-10-23.md) §10; the security decisions it produced are recorded in the [decision log](docs/reference/decisions.md). The architecture is in [security architecture](docs/security/architecture.md).

## 3. Reporting a security issue

This is a personal application owned by one user. Report issues privately to the owner through the same trusted channel used for the app; do not open a public issue containing ledger data or credentials. **Never send the vault passphrase, a backup, an exported CSV, an access token, or the Plaid client secret.** A useful report includes the app version, the security event type, and a sanitized description. There is no bug-bounty program and no guaranteed response time.

## 4. Incident runbook

### Suspected passphrase compromise (someone may know it)
1. Unlock the app and use **Settings → Security → Change passphrase** (native sheet). This generates a new DEK, rekeys the live database, and writes a new header.
2. Verify the old passphrase now fails. Make a fresh encrypted backup.
3. Delete old backups that used the old passphrase (they remain decryptable with it). Use the explicit delete action; never assume rotation removed them.
4. If Plaid is connected, revoke the affected Items in the Plaid dashboard and relink; rotate the Plaid client secret.
5. Review recent security events in Settings.

### Suspected access-token or client-secret leak
1. Disconnect the provider in the app (deletes the Keychain item), or use **Remove all provider credentials**.
2. Revoke the Item or rotate the client secret in the Plaid dashboard (owner action).
3. Relink. Do not reuse the old token or secret.
4. The offline ledger remains usable.

### Lost or stolen Mac
1. Rely on FileVault and the vault passphrase for data at rest. The app cannot wipe the device remotely.
2. Revoke Plaid Items from another device; change the ledger passphrase when you regain access.
3. Assume any unlocked session was exposed. There is no remote-wipe claim.

### Lost or stolen backup/export
1. Treat the passphrase as compromised if it was weak or reused. Rotate the vault passphrase and delete old backups.
2. A plaintext CSV export requires account monitoring; the app cannot revoke it.
3. Rotate provider credentials if the backup could contain them (it should not; backups exclude Keychain material).

### Corrupt database or failed migration
1. Do not run repair tools on the only copy. Preserve the vault directory for forensics.
2. Restore from the newest verified encrypted backup using the native file picker and passphrase sheet. Restore supports same-vault, cross-vault, and fresh-install recovery, plus a destructive full-database path when the current passphrase is unavailable.
3. A failed restore leaves the current database intact by design. If the app cannot unlock, the header may be damaged; try the mirrored header only through the app, never by hand-editing.
4. If both header copies are lost, the ledger cannot be opened in place even if the DB file survives. Restore a self-contained backup from the locked gate: the backup carries its own header and vault identity. A fresh install can create a new vault directly from the backup. Preserve the app-data vault directory and follow the [backup guide](docs/operations/backup-recovery.md).

### Failed rekey
The rotation journal and a pre-change encrypted backup exist. Relaunch and unlock with the new passphrase if the rekey completed; if not, unlock with the old passphrase and retry. If neither works, restore the pre-change backup. Do not delete the journal manually before trying unlock.

### Compromised dependency or build
1. Stop using the artifact. Rebuild from a clean checkout with the lockfiles, run `cargo audit`, `cargo deny check`, `pnpm audit`, and compare the new SHA-256.
2. Rotate the vault passphrase, provider credentials, and any exported files' handling assumptions.
3. Review security events and network/export history.

### Lost passphrase
There is no recovery. A self-contained backup restores access only if its passphrase is known. It can be restored into an existing vault or a fresh install. Do not ask anyone to "reset" the vault; no such mechanism exists by design.

## 5. Operational expectations

- Keep macOS and Xcode command-line tools patched. SQLCipher, SQLite, WebKit, and Rust dependency vulnerabilities are inherited.
- Enable FileVault and a short screen-lock timeout.
- Keep the passphrase in a password manager, not in chat, the repository, an environment file, or notes stored with the backups.
- Verify the artifact SHA-256 before running a rebuilt app.
- Use **Lock Now** before leaving the Mac. Hiding the window is not a lock.
- Expect scheduled sync to skip while locked and to record `skipped: vault locked`.

## 6. Verification status

Security claims are verified only where tests or manual evidence exist. See [implementation status](docs/reference/implementation-status.md) for the current evidence matrix and the [test plan](docs/reference/test-plan.md) for the native smoke checklist. Browser tests do not prove native Tauri or macOS behavior.

## 7. Residual risks (summary)

See §2. The app is best described as a local, encrypted, single-user application with documented limits — not "watertight", "unhackable", or "military-grade", and not distribution-ready.
