# Independent security review — 2026-10-23

Reviewer: a separate review subagent (not the implementer), read-only. Scope: backup/restore, sync, gateway auth/validation, network policy, native confirmations, session state. This is an **automated independent review, not a human penetration test or a substitute for the owner smoke tests**.

## Findings and disposition

| # | Severity | Finding | Disposition |
| --- | --- | --- | --- |
| 1 | Medium | `crypto_erase` lacked `require_real`; opening demo then invoking the command could delete the real vault/backups/demo. | **Fixed.** Command now requires a real session; the service also rejects demo mode. Regression test: `commands::tests::demo_mode_blocks_destructive_real_vault_commands`. |
| 2 | Medium | Restore validated the vault ID before a native confirmation that can block; a lock/relock or demo-open during the prompt could leave `install_restore_confirmed` acting on a different session. | **Fixed.** Install re-checks locked/real mode and the current vault ID after the prompt before taking the connection/DEK; staged data is removed on mismatch. Regression test: `restore_rechecks_mode_after_a_native_prompt_gap`. |
| 3 | Low–Medium | `settings_update` wrote `guard.settings` after releasing the session lock, so a concurrent lock/demo switch could repopulate a closed session or leak stale settings. | **Fixed.** Re-acquires the lock and only updates when still unlocked, real mode, and the same vault ID/generation. |
| 4 | Low | Backup reader chmodded any owner-selected file to 0600 before validating the backup header. | **Fixed.** Reader no longer chmods; it rejects non-owner-only modes with a clear error. Owners must `chmod 600` an externally copied backup. |
| 5 | Low | Symlink TOCTOU between `symlink_metadata` and `File::open`/chmod in backup reads. | **Fixed.** Backup reads/hash/staging use `O_NOFOLLOW` and validate the opened file descriptor; no chmod. Symlinked backup paths are rejected. Regression in `tests/backups.rs`. |

## Verified sound by the reviewer
- Network SSRF posture: HTTPS-only, exact host allowlist, no credentials/fragments, non-443 rejected, redirects disabled.
- Backup deletion filename validation prevents traversal.
- Sync/backup views omit `credential_ref`, cursors, and absolute paths.
- No Critical or High findings.

## Residual / accepted risks
- The 4 MiB response cap is enforced by `read_bounded`; `NetworkPolicy::client()` alone does not cap body size. There are **no production provider call sites yet** (Plaid Phase 9 is blocked), so this is accepted until the adapter is implemented and must be wired through `read_bounded` then.
- Native file pickers, confirmation/sheet behavior, Keychain ACLs, screen capture, sleep/wake, and a power-loss window between restore renames are not verifiable by an automated reviewer. Owner smoke required.
- Restore requires an unlocked existing vault; fresh-install restore remains unsupported.
- Live Plaid and live institution coverage are not implemented or verified.

## Follow-up evidence
- `cargo test` after fixes: 144 passed (1 ignored 50k perf), including 9 backup tests and the new demo-mode guard test.
- `cargo audit`: 0 vulnerabilities (5 allowed Linux/unmaintained warnings). `cargo deny check`: all ok. `pnpm audit`: clean.
- Hardened artifact rebuilt and re-signed after these fixes. Final binary SHA-256: `9448f3939a6ae0574010758dbe7e1c4103eca8331f2076850da2da0140467c08`; `codesign --verify --deep --strict` passes (ad-hoc).
