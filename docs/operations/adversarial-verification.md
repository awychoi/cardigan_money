# Adversarial verification record

Automated checks executed against the Phase 7/8 tree. This is evidence, not a claim that the app is "watertight".

| Attack / failure case | Evidence | Result |
| --- | --- | --- |
| Plain SQLite instead of SQLCipher | `scripts/verify-plain-sqlite-reject.sh`; `tests/vault_cipher.rs` | Rejected; key required; wrong key fails |
| Wrong passphrase | `tests/vault_flow.rs`, `tests/backups.rs` | Fails closed, no replacement, no plaintext |
| Header/DB/backup tamper | `tests/backups.rs` (DB byte flip, truncation); `tests/vault_cipher.rs` (header AAD) | Rejected before install/unlock |
| Backup from another vault | `tests/backups.rs` | `Blocked`, no staged install |
| Path traversal / symlink | CSV import symlink rejection, export symlink rejection (`tests/imports.rs`); backup strict filenames | Rejected |
| CSV malformed/oversized/binary/BOM/quotes | `tests/imports.rs`, `src-tauri/src/providers/csv.rs` unit tests | Bounded/rejected or parsed safely |
| Formula injection in CSV export | `tests/imports.rs` | Text fields prefixed with `'`; amounts numeric |
| Malformed/extra IPC fields, oversized payloads | `src-tauri/src/ipc.rs` tests + gateway `deny_unknown_fields` | Rejected; typed errors |
| Provider failure mid-pagination | `tests/sync.rs` | Cursor stays at last committed page |
| Lock mid-sync | `tests/sync.rs` | Stops; no cursor past committed page; resumable |
| Duplicate/pending replacement | `tests/sync.rs` | No double count; pending retired |
| Network URL abuse | `providers/network.rs` tests | Non-HTTPS, non-allowlisted host, port, userinfo, fragment rejected |
| Oversized network body | `providers/network.rs` test | Cap enforced |
| Restore from locked state | `tests/backups.rs` | Refused |
| Concurrent sync triggers | `tests/sync.rs` | Second trigger skipped |
| Renderer HTML password/remote assets/localStorage | `e2e/security.spec.ts` | None present; requests local/data only |
| Dev mock in production bundle | `scripts/check-release-artifacts.mjs` | Sentinel absent; no source maps |

Not covered here: live Plaid, macOS Keychain ACL, screen capture/content protection, native sheet/file dialogs, sleep/wake, and a real power-loss between restore renames. Those remain native smoke or documented residual risks.
