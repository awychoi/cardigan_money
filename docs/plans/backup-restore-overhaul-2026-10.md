# Backup and restore overhaul

Status: approved design, implementation not started.
Date: 2026-10-24.

## Goal

Make encrypted backups usable for real recovery, including a fresh install,
while preserving the existing vault's identity where the user is not replacing
it. One manual backup action covers local and portable use. Restore is explicit,
validated before it touches the live vault, and never silently destroys an
existing vault.

## Approved decisions

1. Same-vault full restore into an existing vault keeps the current vault
   passphrase. The backup passphrase only decrypts the backup.
2. Keep Keychain provider tokens for same-vault full restores so Plaid update
   mode can reuse the existing Item. Delete them for cross-vault and fresh
   restores, subject to the per-restore Plaid Item choice.
3. After any restore, live provider balances, available credit, limits, and due
   metadata are hidden until the connection is reconnected. Restored
   transaction history remains visible.
4. Every restore that replaces an existing vault requires typed confirmation:
   the user types `replace`.
5. `Back up now` always shows the native passphrase sheet. A blank passphrase
   means "use the vault passphrase" when the vault is unlocked. While locked,
   the vault passphrase is required; blank is not allowed.
6. Accept a non-private backup file only after an explicit acknowledgement.
   The acknowledgement bypasses only the POSIX mode check; owner, regular-file,
   no-symlink, size, header, hash, and seal checks remain.
7. After a cross-vault restore, other-vault backups stay on disk under normal
   retention but are hidden from the app's backup list. Restoring one requires
   the native file picker and the cross-vault warning.
8. If an existing vault is locked and the current passphrase is unavailable, a
   separate destructive restore path may replace it without a pre-restore
   backup. It requires typed confirmation and is full-database only.
9. Cross-vault and fresh restores let the user choose the restore scope:
   full database, financial data plus settings, or financial data only.
10. On cross-vault restore, ask how to handle the old vault's Plaid Items:
    delete local tokens, also revoke the old Items remotely, or keep the old
    tokens for rollback.
11. In the destructive no-current-passphrase path, ask whether to delete all
    app-managed provider tokens or leave them.
12. Abort a normal restore if the pre-restore backup of the current vault fails.
13. Any valid backup kind can be restored.
14. After a successful restore, always prompt to change the passphrase, with
    buttons `Change passphrase now` and `Change later`. Never force the choice.
15. Automatic backups (daily, pre-migration, pre-restore, pre-change) continue
    to inherit the vault passphrase in effect when they are created.

## Backup creation

There is one user action, **Back up now**. The former separate portable export
action is removed from the UI. Existing `portable` containers remain restorable.

- The native save dialog defaults to the app backup directory and allows a
  different destination. The chosen directory is not remembered.
- The native passphrase sheet is always shown.
  - When unlocked, blank uses the current vault header and DEK.
  - When locked, blank is rejected; the user must enter the current vault
    passphrase so the app can unwrap the DEK, read the ledger, and seal the
    container. The app stays locked and never renders ledger data.
  - A non-blank passphrase creates a new header and DEK for that backup, using
    the current KDF parameters and the same `vault_id`. The snapshot is
    re-encrypted with the backup DEK.
- A backup written to the default directory is listed and subject to the normal
  manual retention limit. A backup written elsewhere is not listed and is not
  pruned by the app.
- New user-created backups use the `manual` kind. This keeps the container
  compatible with existing readers; "portable" is now a destination/passphrase
  choice, not a separate action.
- Backups never include Keychain tokens or the Plaid client secret.
- Automatic backups are unchanged and require an unlocked session.

## Restore matrix

| Target | Backup | Vault ID and passphrase | Pre-restore backup | Confirmation | Old backups |
|---|---|---|---|---|---|
| Existing, same vault, full | Same vault | Keep current vault ID and passphrase | Yes; abort on failure | Type `replace` | Visible |
| Existing, different vault, full | Any vault | Adopt backup `vault_id`; backup passphrase becomes vault passphrase | Yes; abort on failure | Type `replace` and cross-vault warning | Hidden, normal retention |
| Fresh install, full | Any vault | Adopt backup `vault_id` and passphrase | None | Warning; no typed word required | None |
| Existing, partial scope | Same or different | Keep current vault ID and passphrase | Yes; abort on failure | Type `replace` | Visible |
| Fresh install, partial scope | Any vault | Adopt backup `vault_id` and passphrase | None | Warning | None |
| Existing, destructive no-current-passphrase | Any vault | Adopt backup `vault_id` and passphrase | No | Type `replace`; explicit no-backup warning | Hidden when the ID changes |

A restore into an existing vault is available while locked only if the current
vault passphrase is supplied for the pre-restore backup. The destructive path
above is the only exception, and it is full-database only because partial
restore needs the current database to preserve or reset unselected data.

## Restore scopes

The restore flow asks the user to choose one scope. The scope applies to
cross-vault and fresh restores. For an existing vault, a partial scope keeps the
current `vault_id` and passphrase; a full cross-vault restore adopts the
backup's identity.

| Scope | Data taken from the backup | Data reset in the target |
|---|---|---|
| Full database | Everything | Nothing |
| Financial + settings | Financial tables, settings, merchant rules, import history | Security events, connections, account sources, sync history |
| Financial only | Financial tables | Settings (defaults), security events, connections, account sources, sync history, import history |

Financial tables: accounts, balance snapshots, payment snapshots, manual
payment metadata, transactions, transaction overrides, and merchant rules.
Import history means import batches and import rows. Reset means delete the
target rows (or write default settings), not preserve the current values.

If resetting connections or import history, clear the affected foreign keys:
balance and payment snapshot `source_connection_id` becomes null, and
transaction `import_batch_id` becomes null when import history is reset.

Selective restore is built on a staged copy; the live vault is not mutated until
the complete candidate has been validated.

## Provider connections and tokens

- After every restore, all restored Plaid connections start as
  `reconnect_required`. Scheduled sync, manual sync, and live provider fields
  are blocked until the user reconnects.
- Same-vault full restore keeps the existing Keychain tokens and connection
  credential references. The user can reconnect with Plaid update mode, which
  reuses the existing Item and does not consume a new trial token.
- Cross-vault full restore and fresh restore reset restored connection
  credential references and require a new link. The per-restore Plaid choice
  controls the old vault's tokens and Items.
- Any scope that resets connections deletes the old tokens; there is no
  connection left to reuse them.
- Hiding account detail means live balances, available credit, limits, and due
  metadata. It does not hide restored transaction history.
- The per-restore Plaid choice for cross-vault full restore is:
  delete local tokens only; delete local tokens and revoke the old Items
  remotely; or keep the old tokens for rollback.
- The destructive no-current-passphrase path cannot read the old connection
  references. It asks whether to delete all app-managed provider tokens or
  leave them.

## Backup list and file selection

- The list shows only backups in the default directory whose `vault_id` matches
  the current vault. Other-vault backups are read for retention but not exposed.
- Each list item includes kind, creation time, size, and schema/generation.
  There is no passphrase hint.
- A non-private backup is rejected by default. The restore flow offers an
  acknowledgement that bypasses only the mode check.
- Deletion is unchanged and requires a native destructive confirmation.

## Restore completion

After a restore succeeds, the app always shows a success acknowledgement with:

- **Change passphrase now** - opens the native new-passphrase sheet. The
  passphrase just entered for the backup is used as the current passphrase.
- **Change later** - keeps the backup passphrase.

If the native sheet is cancelled or fails, the restore remains valid with the
backup passphrase. No rollback of restored data occurs. The app should state
when a pre-restore backup uses a different (previous) passphrase.

## Security invariants

- No backup path or passphrase is sent to the renderer.
- Native file pickers and native secure sheets perform path/passphrase input.
- The backup is staged with `0600`, validated before any live file is replaced,
  and installed atomically (fresh directory) or through the existing journal
  path (existing vault).
- Same-vault restore keeps the current header and DEK.
- Cross-vault and fresh full restore create a fresh header/DEK with the backup
  `vault_id` and backup passphrase; they do not adopt the backup's old DEK.
- Partial restore keeps the current header/DEK and builds a candidate database
  under the current key.
- The pre-restore backup is created and verified before the swap. It carries the
  pre-restore vault passphrase.
- Destructive no-passphrase restore still stages and validates the candidate,
  and keeps the old vault files until the new vault is successfully installed.
- Schema checks, SQLCipher integrity, SQLite integrity, `vault_id` consistency,
  and a schema-object allowlist run before install.
- Attempts are rate-limited.
- Restore is unavailable in demo mode.

## Implementation phases

1. Add the spec and test scaffolding; update the bridge/contracts where the
   restore result and backup item metadata will change.
2. Unified `Back up now` for an unlocked vault: default/custom destination,
   optional custom passphrase, list/retention behavior.
3. Locked backup and the sessionless restore foundation: open/validate/backup
   with an entered passphrase without creating a UI session.
4. Fresh-install and existing full restore, including same/cross-vault identity
   handling, typed confirmation, pre-restore backup, and the post-restore
   change prompt.
5. Selective restore scopes on a staged candidate database.
6. Provider reconnect gate, Plaid Item/token choices, non-private
   acknowledgement, hidden other-vault backups, and final docs/tests.

## Tests to add

- Backup now: unlocked blank, unlocked custom, locked required, custom
  destination not listed, default destination retained, wrong passphrase,
  cancel/cleanup.
- Fresh restore: full, scoped, wrong passphrase, tamper, wrong schema, atomic
  install, no partial vault, provider gate.
- Existing same-vault restore: current passphrase remains, typed confirmation,
  pre-restore failure aborts, tokens retained, reconnect gate.
- Existing cross-vault restore: warning, identity/passphrase adoption, old
  backups hidden, Plaid Item choice, pre-restore failure aborts.
- Locked existing restore: current passphrase required for the backup, no UI
  session, destructive path, token choice.
- Selective scopes: table sets, reset semantics, FK cleanup, vault identity,
  fresh defaults.
- Post-restore prompt: change now, change later, cancel, old backups unaffected.
- Non-private acknowledgement: mode bypass only, symlink/owner/size still
  rejected.
- Renderer contract: no path or passphrase fields.
