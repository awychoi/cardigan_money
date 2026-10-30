---
treatment: hero
order: 10
kicker: Private · Local-first
meta:
  - label: Status
    value: Personal project; early
  - label: Stack
    value: Tauri 2, React, TypeScript, Rust, SQLCipher
  - label: Data
    value: Encrypted at rest; no account, cloud, or telemetry
  - label: Recovery
    value: No passphrase recovery, by design
note: Early and personal. Interfaces, storage formats, and behavior can change.
link:
  label: View the repository
  href: https://github.com/awychoi/cardigan_money
---
## What it is

A macOS app for one person’s credit-card ledger: card balances, upcoming payments, and spending by calendar month. Month totals are kept separate from statement cycles, and pending, posted, refunds, and payments stay distinct. The ledger is encrypted with SQLCipher and unlocked with a passphrase typed into a native macOS sheet; keys and provider tokens stay in Rust and the macOS Keychain. No account, no cloud, no telemetry.

## Decisions

I set the constraints; the agent made the calls inside them. The repository keeps a decision log that separates the two.

### What I decided

- **Local-first, no account, no cloud.** A constraint from the start, not a feature added later.
- **No passphrase recovery, by design.** Losing it loses the ledger. I would rather own that risk than ship a recovery path I cannot audit.
- **The secrets stay mine.** The agent built the Plaid integration without ever seeing the client secret; I enrolled the trial, entered the credentials, and ran the verification.
- **The ten-Item trial is a budget.** I asked whether a duplicate connection would waste a slot. It would, so duplicate links are refused and a restore reuses the token already in the Keychain.

### What the agent decided

- **Envelope encryption.** An Argon2id-derived key wraps a random 256-bit data key; SQLCipher only ever sees the raw key.
- **One IPC command, `gateway`,** with typed payloads, and the window built in Rust so the guards exist before the first page loads.
- **Hosted Link plus polling** for Plaid — the only flow left once custom URL schemes and a local callback were ruled out.
- **Staged restores.** A backup is validated before the live vault is touched, and the app refuses to open a ledger it cannot prove is encrypted.

## What I learned

The spec I started from told the agent not to ask follow-up questions. My first instruction was the opposite: read it, then ask me everything you need. The unanswered questions turned out to be the design — the backup matrix, the restore paths, what happens when a passphrase is lost. An agent will proceed on a plausible assumption; the assumptions are where the risk lives.

I thought this was a database, a frontend, and a fetch job. It is closer to twenty thousand lines, and most of that is not features. It is backups, restore, rekeying, locking, import rollback, and the refusal paths. Complexity lives in what happens when something fails.

The security review I asked for before the build turned the plan into twenty findings; later read-only reviews found that demo mode could erase the real vault and that a restore could race a long native prompt. None of that is visible from a feature list. So the habit became writing down what had *not* been verified — live Plaid linking, native passphrase entry, Keychain behaviour after a rebuild — because an unchecked claim is the one thing I cannot review.
