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

Cardigan Money is a macOS app for the credit cards one person actually carries. It gathers every balance into one ledger, puts every transaction on one timeline, and brings the next payments into one view. The total you owe becomes a figure you read rather than four balances you add up, and a month of spending becomes one set of categories instead of four separate reconciliations. Pending, posted, refunds, fees, payments, and transfers stay distinct, and a balance the app cannot see is reported as unknown rather than counted as zero.

Plaid is the live feed. You bring your own Plaid credentials, link an institution once, and balances and transactions sync on a schedule and at launch. One Item per institution keeps the feed from doubling up: a second link is stopped before Plaid issues a token, and Update/Reconnect repairs the existing connection instead. The cost is that two separate logins at the same bank cannot both be connected; I took that over duplicate Items, double-counted accounts, and wasted Trial capacity.

### What the security costs

Most of the engineering went into making the ledger safe enough to hold real balances, and every choice there has a cost.

**What I decided**

- **No passphrase recovery.** Lose it and the ledger is gone. That is the price of not shipping a reset path I cannot audit.
- **The secrets stayed mine.** The agent built the Plaid integration without ever seeing the client secret. The cost was that enrollment, credential entry, and verification were mine to do.
- **Reviews before code.** A security critique and a CTO-style review preceded the build, and no phase closed with a high-severity finding open. The cost was time before a line of product code.

**What the agent decided**

- **Envelope encryption.** An Argon2id-derived key wraps a random 256-bit data key; SQLCipher only ever sees the raw key. The cost is more machinery at unlock and in every backup.
- **One IPC command, `gateway`,** with typed payloads, plus a window built in Rust so its guards exist before the first page loads. The cost is a single chokepoint every feature has to fit through.
- **Hosted Link plus polling** — the only Plaid flow left once servers and custom URL schemes were ruled out. The cost is a link URL that has to be treated as a bearer token.
- **Staged restores, and a fail-closed open.** A backup is validated before the live vault is touched, and the app refuses a ledger it cannot prove is encrypted. The cost is more failure paths to test.

The build is ad-hoc signed and not notarized. That is a deliberate limit for a personal app, not a claim of distribution readiness.

### Privacy

No account, no cloud, no telemetry, and nothing scraped. The ledger is a SQLCipher database on the Mac and is unlocked by a passphrase typed into a native macOS sheet; the passphrase is never stored or sent to the web view, and keys and provider tokens stay in Rust and the macOS Keychain. Backups are encrypted containers and exclude provider secrets. The app's own network client reaches only Plaid's documented hosts, and logging is off by default. No server of mine ever sees it. The exception is the plaintext CSV export, which exists so the data can leave, and it warns before writing.

### The rest

Plaid is optional. Cards can be entered by hand, with statement balance, minimum due, next due date, and a valid-through date. A statement CSV from an issuer can be imported with column mapping, a preview, duplicate review, and batch rollback. Spending is counted by calendar month rather than statement cycle, so months compare cleanly; the 14-day upcoming-payments view and current balances are independent of the month you are reading, and paging through history does not rewrite them. Missing balances are shown as missing, never zero. A synthetic demo opens without a passphrase.

## What I learned

The spec I started from told the agent not to ask follow-up questions. My first instruction was the opposite: read it, then ask me everything you need. The unanswered questions turned out to be the design — the backup matrix, the restore paths, what happens when a passphrase is lost. An agent will proceed on a plausible assumption; the assumptions are where the risk lives.

I thought this was a database, a frontend, and a fetch job. It is tens of thousands of lines, and most of that is not features. It is backups, restore, rekeying, locking, import rollback, and the refusal paths. Complexity lives in what happens when something fails.

The security reviews were worth their cost. The one I asked for before the build produced twenty findings; later read-only passes found that demo mode could erase the real vault and that a restore could race a long native prompt. So the habit became writing down what had _not_ been verified — live Trial/Production linking, native passphrase entry, Keychain behavior after a rebuild — because an unchecked claim is the one thing I cannot review.
