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

Cardigan Money consolidates a person's credit card balances and transactions into a single dashboard using Plaid - with transactions streamed live into the vault. The vault is a SQLCipher database, and an Argon2id-derived key wraps its data key behind your passphrase that's never stored - forget it and the vault is lost - a security trade off.

Plaid credentials and Item tokens live in the macOS Keychain, and the bank connection is covered by Plaid's own security. The app is completely local: skip Plaid and it runs fully offline on statement CSV imports or manual entry.

## Decisions split

### What I decided

- **No passphrase recovery.** A decision that can be seen as a double edged sword but a decision was made nonetheless for security reasons.
- **The secrets stay mine.** The agent built the Plaid integration without ever seeing the client secret. All secrets are owned by the user, stored in Apple Keychain.
- **Staged restores, and a fail-closed open.** A backup is validated before the live vault is touched.
- **A theme engine** Because why not
- **Refuse duplicate links** to avoid unnecesary spend of Plaid Trial account items which cannot be recovered.
- **Branding** Again, why not.
- **UI & UX refinements** based on the agent's first draft

### What the agent decided

- **Envelope encryption.** An Argon2id-derived key wraps a random 256-bit data key; SQLCipher only ever sees the raw key. The cost is more machinery at unlock and in every backup.
- **One IPC command, `gateway`,** with typed payloads and the window built in Rust, so every call is checked in one place.
- **Hosted Link plus polling** — the only Plaid flow that survives a ban on servers and custom URL schemes. The cost is a link URL that has to be treated as a bearer token.
- **Lock on event** Instead of the unreliable and finicky timer method.
- **The frontend design draft** Served as the basis for refinement,
- **The tech stack** becasue the human is not a programmer by trade.

## What I learned

- **I could not offload the plan and the build.** My most useful prompt was to make the agent ask me everything it needed. The questions — backup matrix, restore paths, what happens when a passphrase is lost — became the design, because an agent otherwise proceeds on a plausible assumption.
- **A simple ask became a complex app.** I thought this was a database, a frontend, and a fetch job. It is tens of thousands of lines, and most of that is not features. It is backups, restore, rekeying, locking, import rollback, and the refusal paths. Complexity lives in what happens when something fails.
- **It taught me the stack.** The agent explained why Tauri, SQLCipher, and native secure input were needed, which made the complexity legible instead of mysterious.
- **I had to check the translation.** Asking the agent to explain my prompt back in its own words caught the places where we were not on the same page.
