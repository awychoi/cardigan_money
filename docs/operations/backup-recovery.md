# Backup, restore, and recovery guide

cardigan_money stores the ledger in a SQLCipher database inside the app-data directory. Encrypted backups are **self-contained**: each container carries its own vault header (KDF parameters + wrapped data key), so a backup can be opened without the original `vault.header`.

**There is no recovery for a lost passphrase.** Backups help only if you know the passphrase that protected them. There is no reset key, escrow, support override, or hidden account.

## 1. Where backups live

- App backups: `~/Library/Application Support/com.awychoi.cardiganmoney/backups/`
- Each file is `cm-backup-<kind>-<vault-prefix>-gen<generation>-<UTC>-<nonce>.cmbak`, mode `0600`.
- Kinds: daily, manual, pre-migration, pre-restore, pre-change. Portable exports are written wherever you choose.
- Retention: 7 daily, 10 manual, and 5 each of pre-migration/pre-restore/pre-change. Pruning only matches the app's strict filename pattern and never removes the last backup.
- Backups contain the encrypted ledger. They do **not** contain macOS Keychain items, Plaid access tokens, or the Plaid client secret.

## 2. Automatic and manual backups

- A daily encrypted backup is attempted after unlock when the last backup is at least 24 hours old. A backup failure never prevents unlock; it is shown in Settings and recorded in the security event log.
- A pre-migration backup is created before applying pending schema migrations.
- A pre-restore backup is created immediately before replacing the ledger during restore.
- A pre-change backup is created before a passphrase/DEK rotation.
- Use **Settings → Backups & recovery → Back up now** before unusual work. Scheduled sync and auto-lock may mean the latest automatic backup is older than your latest edits.

## 3. Restoring

1. Unlock the real vault (restore is available only while unlocked so the restored data can be re-keyed to your current passphrase).
2. Open **Settings → Backups & recovery → Restore backup…** and confirm.
3. Choose the `.cmbak` file in the native file picker.
4. Enter the backup's passphrase in the native secure sheet.
5. The app fully validates before touching the live ledger: container header, embedded vault header, AEAD seal, file length, database SHA-256, vault ID match, SQLCipher integrity check, SQLite integrity check, schema compatibility, and migration compatibility.
6. On success, the app creates a pre-restore backup, closes the live connection, atomically swaps in the validated database, re-keys it to the **current** passphrase, and reloads settings from the restored data. The current `vault.header` and passphrase stay in effect.
7. On any failure before the swap, the live ledger is untouched. If the new database fails its post-swap integrity check, the old file is restored.

A backup from a different vault ID is rejected. A wrong passphrase, truncated file, modified bytes, or failed seal is rejected. The backup file itself is never modified.

### Limitations

- Restore into a fresh install with no existing vault is not supported in this version. Keep the original app-data vault directory (or a backup) before reinstalling. Empty-install restore is a recorded Phase 10/backlog item.
- Restoring an older backup replaces accounts, transactions, overrides, rules, imports, settings, and the security-event history with that backup's state. Changes made after the backup are lost; the pre-restore backup preserves the replaced state.
- A restore is not a recovery from a forgotten passphrase. You must know both the current passphrase and the backup passphrase.
- The swap uses atomic per-file renames in the vault directory. A power loss between the two renames is a documented crash window; the pre-restore backup and the `.cmdb.restore-old` file are the recovery path.

## 4. Portable encrypted export

**Settings → Backups & recovery → Portable encrypted export…** writes a full-history container with a **separate** passphrase and a fresh random data key. It is intended for off-app archival or moving to another install. It contains no provider tokens or client secret.

Restoring a portable export requires it to belong to the same vault ID and requires the current vault to be unlocked; its passphrase re-keys the staged data to the current passphrase. Store the portable passphrase in a password manager, separately from the file.

A plaintext CSV export (ledger toolbar) is **not** a backup: it is unencrypted, may leave the app's control immediately, and has formula-injection escaping only for spreadsheet safety.

## 5. Recovery scenarios

### Lost or corrupted `vault.header` (both copies)
The ledger cannot be opened even if `ledger.cmdb` exists. Restore a self-contained backup into an existing vault installation; if the app cannot start/unlock to reach Settings, a fresh-install restore is not yet supported — use the pre-upgrade vault directory plus a backup on a machine with an existing vault. This is a known limitation and a reason to keep backups outside the Mac.

### Lost passphrase
There is no recovery. A backup only helps if its passphrase is known. Do not send the passphrase, backup, or CSV to anyone.

### Corrupt database or failed migration
Do not run repair tools on the only copy. Preserve the vault directory, unlock if possible, then restore the newest verified backup. A failed migration rolls back its own transaction; a pre-migration backup exists.

### Failed restore
The live ledger should be unchanged (validation happens before replacement). If the app reports a post-swap failure, quit the app, preserve the vault directory, and use the newest `pre-restore` backup.

### Failed passphrase rotation
The rotation journal and pre-change backup exist. Relaunch and unlock with the new passphrase if the rekey completed; otherwise unlock with the old passphrase and retry. If neither works, restore the pre-change backup (old passphrase).

### Stolen backup or disk
Rotate the vault passphrase (new DEK) and delete old backups encrypted under the old passphrase. Old backups remain decryptable with the old passphrase until deleted. Revoke Plaid Items/client secret separately when live linking exists.

### Crypto-erase
**Settings → Backups & recovery → Erase all local data…** requires a typed confirmation and a native confirmation. It removes the vault directory, header/mirror, journal, backups, demo database, and staged restore files, with a best-effort overwrite before deletion. It cannot erase Time Machine, cloud copies, exports, or other devices.

## 6. Verification evidence

The container format, tamper detection, wrong-vault rejection, retention, portable export, pre-change backup, and crypto-erase behavior are covered by `src-tauri/tests/backups.rs`. Native file pickers and the native confirmation/sheet presentation are not automated; see the native smoke checklist in `docs/reference/test-plan.md`. `cargo test --test backups` is the reproducible check.
