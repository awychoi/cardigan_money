---
treatment: hero
order: 10
kicker: Private · Local-first
banner: docs/design/app-blueprint-overview-dark.png
banner_alt: Cardigan Money overview screen showing net spending, reported payments, and card balances.
meta:
  - label: Hardest part
    value: Restore. No recovery key, no stored passphrase, and the vault has to fail closed.
  - label: What I learned
    value: A local-first security model is mostly a set of refusals; every convenience has to earn its place.
link:
  href: https://github.com/awychoi/cardigan_money
---

## What it is

Cardigan Money consolidates a person's credit-card balances and transactions into one Plaid-synced dashboard. Or keep it offline via CSV import or manual entry.

The vault is SQLCipher-encrypted; an Argon2id-derived key wraps the data key behind a passphrase that is never stored — irrecoverable, by design. Plaid secures the bank connections. Secrets live in the macOS Keychain.