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

Cardigan Money consolidates a person's credit-card balances and transactions into a single dashboard, with Plaid syncing them into the vault. The vault is a SQLCipher-encrypted database, and an Argon2id-derived key wraps its data key behind a passphrase that is never stored. Forget the passphrase and the vault is lost — a deliberate security trade-off.

Plaid credentials and Item tokens live in the macOS Keychain, and the bank connection is covered by Plaid's own security. Everything else stays local: skip Plaid and the app runs fully offline on statement CSV imports or manual entry.

## The decision split

I set the direction and the security constraints; the agent made the implementation calls inside them.

### What I decided

- **No passphrase recovery.** A double-edged sword, but a deliberate one: losing the passphrase loses the vault, and I chose that over a recovery path I could not audit.
- **The secrets stay mine.** The agent built the Plaid integration without ever seeing the client secret. Plaid credentials and Item tokens stay in the macOS Keychain.
- **Staged restores, and a fail-closed open.** A backup is validated before the live vault is touched, and the app will not open a ledger it cannot prove is encrypted.
- **A theme engine.** Four themes, each in light and dark, because why not.
- **Refuse duplicate links.** A second link at an already-connected bank is stopped before it spends another Plaid Trial Item; removing an Item does not restore that capacity.
- **Branding.** A mark, an icon, and a maker credit, because why not.
- **UI and UX refinements.** The agent's first draft was a starting point, not a finish.

### What the agent decided

- **Envelope encryption.** An Argon2id-derived key wraps a random 256-bit data key; SQLCipher only ever sees the raw key. The cost is more machinery at unlock and in every backup.
- **One IPC command, `gateway`,** with typed payloads and the window built in Rust, so every call is checked in one place.
- **Hosted Link plus polling** — the only Plaid flow left once servers and custom URL schemes were ruled out. The cost is a link URL that has to be treated as a bearer token.
- **Lock on system events.** Screen lock, sleep, fast user switching, logout, and quit lock the vault, instead of leaving it to a timer.
- **A frontend design draft.** The first UI pass became the base for refinement, not the final design.
- **The tech stack.** Tauri, React, Rust, and SQLCipher, chosen and explained for someone who is not a programmer by trade.

## What I learned

- **I could not completely offload the plan and the build.** My most useful prompt was to make the agent ask me everything it needed. The questions — backup matrix, restore paths, what happens when a passphrase is lost — became the design, because an agent otherwise proceeds on a plausible assumption.
- **A simple ask became a complex app.** I thought this was a database, a frontend, and a fetch job. It is tens of thousands of lines, and most of that is not features. It is backups, restore, rekeying, locking, import rollback, and the refusal paths. Complexity lives in what happens when something fails.
- **It taught me the stack.** The agent explained why Tauri, SQLCipher, and native secure input were needed, which made the complexity legible instead of mysterious.
- **I had to check the translation.** Asking the agent to explain my prompt back in its own words caught the places where we were not on the same page.
