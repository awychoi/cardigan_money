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

Cardigan Money pulls one person's credit-card balances and transactions into a single place, with Plaid doing the syncing. The ledger is a SQLCipher database, and an Argon2id-derived key wraps its data key behind your passphrase.

The passphrase is entered in a native macOS sheet, used only in memory, and never stored; losing it means the ledger cannot be recovered. Plaid credentials and Item tokens live in the macOS Keychain, and the bank connection is covered by Plaid's own security. Apart from that optional link, the app is completely local: skip Plaid and it runs fully offline on statement CSV imports or manual entry.

## What the security costs

A ledger is only worth keeping if it is safe to put real numbers in. Most of the design work went here, and every choice has a price.

### What I decided

- **No passphrase recovery.** Lose it and the ledger is gone. I would rather own that trade than ship a recovery path I cannot audit.
- **The secrets stay mine.** The agent built the Plaid integration without ever seeing the client secret; I enrolled the trial, entered the credentials, and ran the verification.
- **Reviews before code.** A security critique and a CTO-style review came before the build, and no phase closed while a high-severity finding was open.

### What the agent decided

- **Envelope encryption.** An Argon2id-derived key wraps a random 256-bit data key; SQLCipher only ever sees the raw key. The cost is more machinery at unlock and in every backup.
- **One IPC command, `gateway`,** with typed payloads and the window built in Rust, so every call is checked in one place.
- **Hosted Link plus polling** — the only Plaid flow that survives a ban on servers and custom URL schemes. The cost is a link URL that has to be treated as a bearer token.
- **Staged restores, and a fail-closed open.** A backup is validated before the live vault is touched, and the app refuses a ledger it cannot prove is encrypted. The cost is more failure paths to test.

## Privacy

No account, no cloud, no telemetry, and nothing scraped. The ledger sits in SQLCipher on the Mac and is unlocked by a passphrase typed into a native macOS sheet; keys and provider tokens stay in Rust and the macOS Keychain. No server of mine ever sees it.

## The rest

The fallbacks: accounts entered by hand, and a statement CSV from your issuer imported with a preview and rollback. Spending is counted by calendar month rather than statement cycle, an upcoming-payments view stays current whichever month you are reading, a duplicate link is refused rather than spending another Item, and a synthetic demo opens without a passphrase.

## What I learned

- **I could not offload the plan and the build.** My first useful instruction was to make the agent ask me everything it needed. The questions — backup matrix, restore paths, what happens when a passphrase is lost — became the design, because an agent otherwise proceeds on a plausible assumption.
- **I had to check the translation.** Asking the agent to explain my prompt back in its own words caught the places where we were not on the same page.
- **A simple ask became a complex app.** I thought this was a database, a frontend, and a fetch job. It is tens of thousands of lines, and most of that is not features. It is backups, restore, rekeying, locking, import rollback, and the refusal paths. Complexity lives in what happens when something fails.
- **It taught me the stack.** The agent explained why Tauri, SQLCipher, and native secure input were needed, which made the complexity legible instead of mysterious.
