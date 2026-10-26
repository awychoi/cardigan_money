# Plaid setup and verification guide

The app supports two Plaid environments, with separate client credentials:

| Environment | Host | What a link creates | Use |
| --- | --- | --- | --- |
| **Sandbox** | `sandbox.plaid.com` | Synthetic test Items | Development and verification; no real bank data |
| **Trial / Production** | `production.plaid.com` | Real Items at real institutions | Plaid's free Trial plan, or a paid Production plan |

Sandbox Items contain test data. Trial/Production Items contain real financial data and count
against the plan's Production Item limit. Plaid's Trial plan includes up to **10 Production
Items**; removing an Item does **not** restore Trial capacity. Transactions and Liabilities are
subscription products that can begin billing after upgrading from Trial.

The production *wiring* is implemented: credentials, Hosted Link, token exchange, sync, reconnect,
disconnect/revoke, and restore cleanup are all environment-aware. A real Trial Item has **not** been
linked or verified by the maintainer as of 2026-10-25; verify with your own institution before
relying on it.

## Owner prerequisites

1. Plaid Dashboard account with verified email.
2. Environment credentials:
   - Sandbox: Sandbox `client_id` and Sandbox secret.
   - Trial/Production: Production `client_id` and Production secret (available after Trial
     approval enough for Production access).
3. Transactions, Balance, and optional Liabilities enabled for the account.
4. Billing/limits acknowledged. Sandbox is free; Production/Liabilities can bill while an Item
   exists, including while it is broken. `/item/remove` is the only off-switch and does not restore
   Trial capacity.

Do not send the secret, access token, public token, link token, or Hosted Link URL in chat or commit
them.

## In-app flow

1. Unlock the real vault (Plaid is unavailable in demo mode). A new vault also offers an optional
   Plaid credentials step in the setup wizard; the credentials are stored the same way. Linking an
   institution still happens in Settings.
2. **Settings → Accounts → Bank connections → Plaid connection**.
3. Select **Sandbox** or **Trial / Production**.
4. Click **Add … credentials…** and enter the Client ID and matching secret in the native AppKit
   sheet.
   - Each environment has separate generic-password Keychain items under service
     `com.awychoi.cardiganmoney.plaid`.
   - `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`; `kSecAttrSynchronizable=false`.
   - Never stored in SQLCipher, settings, exports, logs, URLs, or renderer state.
   - Sandbox and Production secrets are never mixed. Each connection records its environment.
5. Optionally click **Test credentials** to verify that environment's Keychain credentials against
   the matching host without opening Hosted Link.
6. Click **Link a Sandbox institution…** or **Link a real institution…**.
   - Rust creates a Hosted Link token (`hosted_link: {}`, products `transactions`, optional
     `liabilities`) and opens only `https://secure.plaid.com/...` via the Tauri opener. The URL is a
     short-lived bearer and is never logged/persisted.
   - Rust polls `/link/token/get` with backoff. Before exchanging the public token, it compares the
     returned institution with active connections. If the institution already has an active Item in
     that environment, Rust stops here and does **not** request an access token, so no duplicate Item
     is created and no Trial Item slot is spent. The UI points to **Update…**/**Reconnect…**.
   - Otherwise the public token is exchanged in Rust.
   - The access token is stored in Keychain under a random credential reference; the connection row
     contains only that opaque reference and the environment.
7. Complete Link in the browser. Only Plaid `credit` accounts are imported into this card ledger. If
   the app imports 0 accounts, the chosen Item had no credit-card account; remove it or link a
   different Item.
8. Initial Transactions Sync + balances run; Liabilities is attempted but shown
   unavailable/partial if the Item or plan lacks it.

### Sandbox test institution

Choose a Sandbox institution with a credit card; for example First Platypus Bank (`ins_109508`)
using Sandbox username `user_transactions_dynamic` and any non-blank password. The app imports only
Plaid `credit` accounts.

### Real institution (Trial/Production)

Use the owner's real institution and credentials. The owner must complete any bank MFA inside
Plaid's hosted page in the default browser. The app never sees bank usernames or passwords. For
OAuth institutions, Plaid opens the bank's OAuth page in a browser pop-up and returns to the Link
flow; the app sets no `redirect_uri` and detects completion by polling `/link/token/get`. Linking
creates a Production Item on the owner's Plaid plan; check the Dashboard's Item count and product
billing before and after.

## Duplicate prevention

cardigan_money keeps **one active Plaid Item per institution and environment**. This is a deliberate
single-user policy: one bank login normally covers every card at that institution, and it avoids
spending the finite Trial Item allowance on an accidental second link.

- Link metadata from `/link/token/get` provides the institution id (and name when the id is absent).
  Rust checks active connections before `/item/public_token/exchange`.
- If the institution is already connected, the public token is not exchanged. Per Plaid's
  [duplicate-Item guidance](https://plaid.com/docs/link/duplicate-items/), an access-token request is
  what creates the duplicate Item, so this consumes no Trial slot.
- The refusal is recorded as a `plaid_duplicate_blocked` security event and shown as a neutral
  notice. Nothing is written to the ledger or Keychain.
- A `disconnected` connection is exempt, so a bank can be linked fresh after an intentional
  disconnect. A restored connection with no local token (`newlink-…` reference) is also exempt so
  **Link again…** continues to work.
- The UI exposes **Update…** on healthy connections and **Reconnect…** on unhealthy ones. Both use
  Plaid update mode with the existing access token, so adding accounts or repairing the Item does
  not create another Item.

The limitation is intentional: two separate logins at the same institution cannot both be active
at once. Use **Update…**/**Reconnect…** for the existing login; this build does not provide an
override for a second login at the same institution.

This guard applies to new link attempts. It does not retroactively merge or remove duplicate
connections created by an older build; existing duplicates remain until handled outside the link
flow.

## Reconnect

A healthy connection shows **Update…**; a connection in `reconnect_required`/`error` shows
**Reconnect…**. The app uses that connection's recorded environment and access token to create an
update-mode Hosted Link token (no products, `hosted_link: {}`), opens Hosted Link, and keeps the same
access token, connection, account mapping, and transactions. No duplicate Item is created. A
Trial/Production connection uses the Production credentials and host; a Sandbox connection uses
Sandbox.

## Disconnect

Local disconnect deletes the access token from Keychain and marks the connection disconnected; it
attempts `/item/remove` against the connection's environment first. Remote revocation failure does
not leave a local token behind. For a real Item, verify in the Plaid Dashboard that the Item was
removed; only `/item/remove` stops subscription billing.

## Verification checklist

### Both environments

- [ ] Native credential sheet saves for the selected environment; Keychain item uses the attributes
      above and is distinct from the other environment.
- [ ] Hosted Link opens; no URL/token appears in app UI/logs.
- [ ] Transactions Sync imports transactions; repeat sync is idempotent; cursor persists.
- [ ] Balances appear; missing values are not fabricated as $0.00.
- [ ] Liabilities reports supported or unavailable without deleting prior metadata.
- [ ] Account mapping creates each credit account once; reconnect does not duplicate.
- [ ] Linking a bank whose institution already has an active Item is refused before public-token
      exchange; the connection count stays at one and no access token is requested.
- [ ] **Update…** on a healthy connection and **Reconnect…** on an unhealthy one reuse the existing
      connection and do not add another Item.
- [ ] Sync after restart uses the persisted cursor.
- [ ] Wrong/expired link token, cancelled link, and disconnected Item show sanitized errors.
- [ ] Disconnect attempts `/item/remove` and removes the local token.

### Sandbox

- [ ] Sandbox institution with a credit card links.
- [ ] No real bank data is imported.

### Trial / Production

- [ ] Production credentials are used; the connection row shows `production`.
- [ ] The institution is the intended real institution, and account masks match what the owner
      expects.
- [ ] The Dashboard shows the expected Item and product usage.
- [ ] Disconnect removes the real Item; confirm the Dashboard Item count decreases (Trial capacity
      is still consumed).
- [ ] Billing implications are understood before and after linking.

Use **Show verification report** in the Plaid section to copy sanitized counts/statuses for the
verification record. It contains no tokens, credential references, account numbers, descriptions,
or amounts. Record the environment, institution, account/transaction counts, and step statuses in
`docs/reference/implementation-status.md`. Do not claim real-bank coverage from a Sandbox test, and
do not claim Production verification until a real Item has been linked and recorded.
