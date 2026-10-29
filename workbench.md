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

## Why local-first

Most personal finance apps begin with an account and a sync service. A credit-card ledger is exactly the kind of data that should not need either. Local-first was the design constraint, not a feature added at the end.

## What I learned

Privacy is architecture. Key management, recovery, backups, and the boundary between the UI and the encrypted store are the product.

The app is early, not distribution-ready, and has no passphrase recovery. It is a personal project about learning local-first engineering and agentic development.

## What it shows

The best data protection is not collecting the data. The second best is keeping it where the user can hold it.
