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

A macOS app for one person’s credit-card balances and calendar-month spending. The ledger is encrypted with SQLCipher; the passphrase and provider tokens stay in Rust and the macOS Keychain; there is no account and no cloud sync.

## Decisions

I set the constraints; the agent made the calls inside them. The repository keeps a decision log that separates the two.

### What I decided

- **Local-first, no account, no cloud.** Not a feature added at the end. It removed whole categories of design before any code existed.
- **No passphrase recovery, by design.** Losing the passphrase means losing the ledger. I would rather hold that risk than hand the app a recovery path I cannot audit.
- **The secrets stay mine.** The agent built the Plaid integration around a client secret it never saw. I enrolled the trial account, typed the credentials into the native sheet, and ran the verification myself.
- **Reviews before code.** A security critique and a CTO-style review happened before implementation, and no phase closed while a high-severity finding was open.
- **Themes as data, not code.** New looks arrive as a file, so I can change the app’s appearance without asking the agent to re-read the codebase.

### What the agent decided

- **Envelope encryption.** An Argon2id-derived key wraps a random 256-bit data key; SQLCipher only ever receives the raw key. That let automatic backups exist without retaining the passphrase.
- **One IPC command, `gateway`,** with typed payloads per command, instead of a broad Tauri bridge. The window is created in Rust so the security handlers exist before the first page load.
- **Hosted Link plus polling** for Plaid, once research showed custom URL schemes are unsupported and a local callback only works in Sandbox. My “no server” rule removed the alternatives.
- **Staged restores.** A candidate backup is validated before anything touches the live vault, and the app refuses to open a ledger it cannot prove is encrypted.
- **One Plaid Item per institution.** A duplicate burns one of ten trial slots, and removing an Item does not refund it.
- **Locking in Rust,** driven by real screen-lock and idle signals rather than a timer in the interface.

## What I learned

The scarce input was never the code. It was knowing what *correct* looks like: a statement balance is not an amount due, a refund lands in a different month from its purchase, missing is not zero. The agent writes plausible code quickly; it cannot supply the standard the code is judged against. That standard came from twenty years of audit and financial systems.

So I stopped reading code line by line and started interviewing it: what did you understand, what did you assume, what did you leave out, and what have you not verified. Review by interview catches drift that a diff does not.

The drift is real. Left alone, the agent optimises for completeness — more copy, more tabs, more explanation of things I never asked about. A surprising amount of my time went into deleting rather than building. The constraint is focus, not capability.

And “unverified” has to stay a word. The discipline that mattered most was writing down what had *not* been proven: live Plaid linking, native passphrase entry, Keychain behaviour under a rebuild. A control counts when a test or a recorded manual check says so, not when the code looks like it does.

It is early, not distribution-ready, and has no passphrase recovery. That is the honest version, and it is the one worth putting on the record.
