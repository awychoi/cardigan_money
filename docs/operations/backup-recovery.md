# Backup, restore, and recovery guide

cardigan_money stores the ledger in a SQLCipher database inside the app-data
directory. Encrypted backups are **self-contained**: each container carries its
own vault header (KDF parameters + wrapped data key), so a backup can be opened
without the original `vault.header`, including on a fresh install.

**There is no recovery for a lost passphrase.** Backups help only if you know
the passphrase that protected them. There is no reset key, escrow, support
override, or hidden account.

## 1. Where backups live

- App backups: `~/Library/Application Support/com.awychoi.cardiganmoney/backups/`
- Each file is `cm-backup-<kind>-<vault-prefix>-gen<generation>-<UTC>-<nonce>.cmbak`, mode `0600`.
- Kinds: `daily`, `manual`, `pre-migration`, `pre-restore`, `pre-change`, and the
  legacy `portable` kind (existing portable containers remain restorable).
- Retention: 7 daily, 10 manual, and 5 each of pre-migration/pre-restore/pre-change.
  Pruning only matches normal container names, only counts backups for the
  current vault, and never removes the last backup.
- Backups contain the encrypted ledger. They do **not** contain macOS Keychain
  items, Plaid access tokens, or the Plaid client secret.

## 2. Back up now

There is one manual action: **Settings → Backups & recovery → Back up now**,
or **Back up now** on the locked gate. Restore starts from the same sections or
**Restore from backup…** on the locked/fresh gate.

1. A native save dialog opens in the app backup directory. You may choose a
   different destination; that directory is not remembered.
2. A native passphrase sheet always appears.
   - Leave it blank to use the current vault passphrase (and the current header
     and data key).
   - Enter a different passphrase (at least 16 characters) to create a separate
     header and data key for this backup only. It uses the current KDF
     parameters and the same vault identity. The snapshot is re-encrypted with
     the backup key.
3. While the vault is locked, the sheet asks for the current vault passphrase
   first. The app opens the encrypted ledger only long enough to create and
   verify the snapshot; it stays locked and never renders ledger data. A blank
   current passphrase is rejected.
4. A backup written to the app backup directory is listed and follows the
   manual retention limit. A backup written elsewhere is neither listed nor
   pruned; you are responsible for it.

Automatic backups are unchanged and require an unlocked session:

- A daily encrypted backup is attempted after unlock when the last backup is at
  least 24 hours old. A backup failure never prevents unlock; it is shown in
  Settings and recorded in the security event log.
- A pre-migration backup is created before applying pending schema migrations.
- A pre-restore backup is created immediately before replacing an existing vault.
- A pre-change backup is created before a passphrase/DEK rotation.

## 3. Restore

Restore is explicit and validated on a staged copy before the live vault is
touched. The native file picker and secure sheets collect the file and
passphrases; the renderer only sees non-secret choices.

### 3.1 What the flow asks

1. Choose the `.cmbak` file in the native picker.
2. Review the backup kind, vault prefix, schema, and generation.
3. Choose a restore scope:
   - **Full database** — everything.
   - **Financial data + settings** — financial tables, settings, merchant
     rules, and import history; security events, connections, account sources,
     and sync history are reset.
   - **Financial data only** — financial tables; settings return to defaults
     and security events, connections, account sources, sync history, and
     import history are reset.
4. If the app is replacing an existing vault, type `replace`.
5. If the backup belongs to a different vault, choose what happens to the old
   vault's Plaid Items: delete local tokens, delete local tokens and revoke the
   old Items remotely, or keep the old tokens for rollback.
6. If the file is not owner-only (`0600`), acknowledge that explicitly. The
   acknowledgement bypasses only the file-mode check; owner, regular-file,
   no-symlink, size, header, hash, and seal checks still run.
7. Enter the backup passphrase. When restoring into a locked existing vault,
   enter the current vault passphrase too so a verified pre-restore backup can
   be created.

### 3.2 Restore matrix

| Target | Vault identity and passphrase | Pre-restore backup | Confirmation | Old backups |
| --- | --- | --- | --- | --- |
| Existing, same vault, full | Keep current vault ID and passphrase | Yes; abort on failure | Type `replace` | Visible |
| Existing, different vault, full | Adopt backup `vault_id`; backup passphrase becomes the vault passphrase with a fresh header/DEK | Yes; abort on failure | Type `replace` and cross-vault warning | Renamed `cm-retired-`; never auto-pruned |
| Fresh install, full | Adopt backup `vault_id` and passphrase | None | Warning; no typed word | None |
| Existing, partial scope | Keep current vault ID and passphrase | Yes; abort on failure | Type `replace` | Visible |
| Fresh install, partial scope | Adopt backup `vault_id` and passphrase | None | Warning | None |
| Existing, destructive no-current-passphrase | Adopt backup `vault_id` and passphrase with a fresh header/DEK | No | Type `replace`; explicit no-backup warning | Renamed `cm-retired-` when the identity changes |

A restore into an existing vault is available while locked only if the current
vault passphrase is supplied for the pre-restore backup. The destructive path is
the only exception and is full-database only, because a partial restore needs
the current database to reset unselected data. It asks whether to delete all
app-managed provider tokens or leave them.

### 3.3 Validation and install

Before any live file moves, the app validates the container header, embedded
vault header, AEAD seal, file length, database SHA-256, `vault_id` consistency,
SQLCipher integrity, SQLite integrity, foreign keys, schema compatibility, and a
schema-object allowlist. The staged database is migrated to the current schema,
the selected scope is applied, and it is re-keyed to the target key.

- Same-vault full and partial restores keep the current header and data key.
- Cross-vault and fresh full restores create a fresh header and data key under
  the backup passphrase with the backup `vault_id`; the backup's original DEK is
  never adopted.
- A fresh install is assembled in a temporary directory and renamed into place.
- Existing-vault installs rename the old database aside, install the verified
  candidate, verify it again, and only then commit the new header. Failures
  before the swap leave the live files untouched; failures after it restore the
  old files where possible.

After a restore, every restored provider connection clears its cursor and
last-success time, so the next sync is a full resync. Restored provider balances,
available credit, limits, and due metadata stay hidden (the `restored` flag on
balance/payment snapshots) until a fresh live fetch produces new values.
Restored transaction history remains visible.

Provider tokens never travel in a backup. A restored connection therefore has
one of two states:

- **Token still in this Mac's Keychain** — the credential reference and token are
  kept, the connection stays `healthy` with no cursor, and the next sync reuses
  the same Plaid Item. Reuse is automatic for same-vault and fresh-install
  restores and an explicit **reuse local Items** choice for cross-vault and
  destructive restores. Reused connections resync in the background after an
  unlocked restore (or on unlock after a locked restore).
- **No local token** — the reference rotates to `newlink-…` and the connection
  starts `reconnect_required`. Scheduled and manual sync are blocked, and the
  table offers **Link again…** to create a new Plaid Item.

If a reused token has been revoked at the bank, the first sync fails and the
connection demotes to `reconnect_required`, where update mode or a new link can
repair it. Reuse matters for Plaid Trial accounts: 10 Production Items are
allowed and deleting an Item does not restore capacity.

### 3.4 Restore completion

Every successful restore shows a native acknowledgement:

- **Change passphrase now** — opens a native new-passphrase sheet. For
  cross-vault/fresh restores the backup passphrase is the current passphrase;
  for a same-vault restore the current (or previously supplied) vault
  passphrase is used. If the sheet is cancelled or fails, the restore remains
  valid with the passphrase already in effect.
- **Change later** — keeps that passphrase.

The app states when a pre-restore backup still requires the previous vault
passphrase.

### 3.5 Retired backups

On a full cross-vault restore, every normal backup from the previous vault —
including the pre-restore backup — is renamed with a `cm-retired-` prefix. They
are excluded from the normal list, retention, and automatic pruning, and remain
on disk until you delete them from the **Retired backups** section. Restoring one
requires the native file picker, the previous passphrase, and the cross-vault
warning.

## 4. Recovery scenarios

### Lost or corrupted `vault.header` (both copies)
The ledger cannot be opened in place even if `ledger.cmdb` exists. Restore a
self-contained backup from the locked gate. If the vault cannot be opened, use
the destructive no-current-passphrase path (full database only) or a fresh
install. The destructive path also recovers a header-only vault whose database
was lost: it installs the verified backup and writes a new header. Keep backups
outside the Mac.

### Lost passphrase
There is no recovery. A backup only helps if its passphrase is known. Do not
send the passphrase, backup, or CSV to anyone.

### Corrupt database or failed migration
Do not run repair tools on the only copy. Preserve the vault directory, unlock
if possible, then restore the newest verified backup. A failed migration rolls
back its own transaction; a pre-migration backup exists.

### Failed restore
The live ledger should be unchanged (validation and the pre-restore backup
happen before replacement). If the app reports a post-swap failure, quit the
app, preserve the vault directory, and use the newest `pre-restore` backup. A
crash between the existing-vault file renames remains a narrow documented
window, mitigated by the pre-restore backup and the `.cmdb.restore-old` file.

### Failed passphrase rotation
The rotation journal and pre-change backup exist. Relaunch and unlock with the
new passphrase if the rekey completed; otherwise unlock with the old passphrase
and retry. If neither works, restore the pre-change backup (old passphrase).

### Stolen backup or disk
Rotate the vault passphrase (new DEK) and delete old backups encrypted under the
old passphrase. Old backups remain decryptable with the old passphrase until
deleted. Revoke Plaid Items/client secret separately when live linking exists.

### Crypto-erase
**Settings → Backups & recovery → Erase all local data…** requires a typed
confirmation and a native confirmation. It removes the vault directory,
header/mirror, journal, backups (including retired containers), demo database,
staged/install temporary directories, and app-managed provider tokens, with a
best-effort overwrite before deletion. It cannot erase Time Machine, cloud
copies, exports, or other devices.

## 5. Verification evidence

The container format, tamper detection, wrong passphrase, retention, portable
container restore, pre-change backup, and crypto-erase behavior are covered by
`src-tauri/tests/backups.rs`. The unified backup action is covered by
`src-tauri/tests/backup_now.rs`; the restore matrix, scopes, ignored vaults,
destructive path, provider gate, retired backups, non-private acknowledgement,
and post-restore rotation are covered by
`src-tauri/tests/restore_overhaul.rs`. Native file pickers, secure sheets, and
the post-restore dialog are owner smoke items; see the native smoke checklist in
`docs/reference/test-plan.md`.
