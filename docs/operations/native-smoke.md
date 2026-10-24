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
- [ ] Backup: native save dialogs for CSV/portable, native confirmation for delete/restore/erase, restore from file picker, wrong passphrase failure, successful restore with pre-restore backup.
- [ ] CSV import: native file picker, mapping/preview/commit, rollback; export warning and 0600 file.
- [ ] Demo: open/reset/exit; no real Keychain/provider/import/backup/export access; synthetic banner.
- [ ] Accessibility: VoiceOver labels for native sheets and primary controls; full keyboard traversal; visible focus; contrast.
- [ ] Artifact: `codesign --verify --deep --strict`, hash, Gatekeeper behavior on this Mac.

Record results in `docs/reference/implementation-status.md` / the handoff record. Do not mark an item passed without performing it.
