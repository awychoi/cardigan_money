# Native smoke-test checklist (owner, Apple Silicon)

Run on the owner's Mac with a release build. Record pass/fail and the build SHA-256. Automated suites do not replace these checks.

- [ ] App launches; no devtools/shortcut; no external network requests on startup.
- [ ] Create vault: native secure passphrase sheet (not HTML), confirmation, strength/warning copy.
- [ ] Wrong passphrase: clear error, no unlock; repeated attempts throttled.
- [ ] Touch ID/Keychain prompt (if enabled) is app-bound; inspect Keychain item class/accessibility in Keychain Access (`WhenUnlockedThisDeviceOnly`, non-synchronizable).
- [ ] Lock now, idle auto-lock, screen lock, sleep, another-user session, and window close behavior.
- [ ] First close while unlocked shows native three-choice dialog; remember/reverse.
- [ ] Menu Quit is blocked while syncing; quit while locked does not block.
- [ ] State restoration does not reopen the window/route after relaunch.
- [ ] Screen capture / screen recording attempts show protected/blank content where macOS permits.
- [ ] Backup: native save dialog for **Back up now** (default directory plus a custom directory), native passphrase sheet with blank and custom passphrases, locked backup with the current passphrase (app stays locked), wrong passphrase failure, delete confirmation.
- [ ] Restore, same vault: native file picker, typed `replace`, native backup/current passphrase sheets, pre-restore backup created, post-restore "Change passphrase now / Change later" dialog, restored data visible.
- [ ] Restore, cross-vault: warning, Plaid choice, backup identity/passphrase adopted, previous backups appear under Retired backups with `cm-retired-` names, retired delete works, pre-restore backup still needs the previous passphrase.
- [ ] Restore, fresh install from the locked gate: no typed word, vault created and left locked, unlock with the backup passphrase.
- [ ] Restore, locked existing: current passphrase requested, no ledger render, stays locked; destructive path warns that no pre-restore backup is made and offers delete-all/leave tokens.
- [ ] Restore completion: cancel the new-passphrase sheet and confirm the restored vault still opens with the passphrase in effect; repeat with "Change passphrase now" and unlock with the new passphrase.
- [ ] CSV import: native file picker, mapping/preview/commit, rollback; export warning and 0600 file.
- [ ] Demo: open/reset/exit; no real Keychain/provider/import/backup/export access; synthetic banner.
- [ ] Accessibility: VoiceOver labels for native sheets and primary controls; full keyboard traversal; visible focus; contrast.
- [ ] Artifact: `codesign --verify --deep --strict`, hash, Gatekeeper behavior on this Mac.

Record results in `docs/reference/implementation-status.md` / the handoff record. Do not mark an item passed without performing it.
