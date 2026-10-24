# Plaid Sandbox setup and verification guide

Live linking is **Sandbox-only in this build**. No Production Item or real bank credential is used. Production remains disabled until Sandbox is verified and the owner explicitly enables it.

## Owner prerequisites

1. Plaid Dashboard account with verified email.
2. Sandbox `client_id` and Sandbox secret available (Trial approval is not required for Sandbox on many accounts; create/finish the Trial application if the dashboard requires it).
3. Transactions, Balance, and optional Liabilities enabled for the account.
4. Billing/limits acknowledged. Sandbox is for testing; Production/Liabilities can bill while an Item exists. `/item/remove` is the only off-switch.

Do not send the secret, access token, public token, link token, or Hosted Link URL in chat or commit them.

## In-app flow

1. Unlock the real vault (Plaid is unavailable in demo mode).
2. **Settings → Connections & sync → Plaid Sandbox → Add Sandbox credentials…**
3. Enter the Client ID and Sandbox secret in the native AppKit sheet.
   - Stored as generic-password Keychain items under service `com.awychoi.cardiganmoney.plaid`.
   - `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`; `kSecAttrSynchronizable=false`.
   - Never stored in SQLCipher, settings, exports, logs, URLs, or renderer state.
4. Optionally click **Test credentials** to verify the Keychain credentials against Plaid without opening Hosted Link.
5. **Link a Sandbox institution…**
   - Rust creates a Hosted Link token (`hosted_link: {}`, products `transactions`, optional `liabilities`) and opens only `https://secure.plaid.com/...` via the Tauri opener. The URL is a short-lived bearer and is never logged/persisted.
   - Rust polls `/link/token/get` with backoff, then exchanges the public token in Rust.
   - The access token is stored in Keychain under a random credential reference; the connection row contains only that opaque reference.
6. Choose a Sandbox institution with a credit card; for example First Platypus Bank (`ins_109508`) using Sandbox username `user_transactions_dynamic` and any non-blank password. Only Plaid `credit` accounts are imported into this card ledger. If the app imports 0 accounts, the chosen Item had no credit-card account; remove it or link a different Sandbox Item.
7. Initial Transactions Sync + balances run; Liabilities is attempted but shown unavailable/partial if the Item or plan lacks it.

## Reconnect

A connection in `reconnect_required`/`error` shows **Reconnect…**. The app creates an update-mode Hosted Link token with the existing access token and no products, opens Hosted Link, and keeps the same access token, connection, account mapping, and transactions. No duplicate Item is created.

## Disconnect

Local disconnect deletes the access token from Keychain and marks the connection disconnected; it attempts `/item/remove` first. Remote revocation failure does not leave a local token behind.

## Verification checklist

- [ ] Native credential sheet saves; Keychain shows the item attributes above.
- [ ] Hosted Link opens; no URL/token appears in app UI/logs.
- [ ] Sandbox institution with a credit card links.
- [ ] Transactions Sync imports transactions; repeat sync is idempotent; cursor persists.
- [ ] Balances appear; missing values are not fabricated as $0.00.
- [ ] Liabilities reports supported or unavailable without deleting prior metadata.
- [ ] Account mapping creates each credit account once; reconnect does not duplicate.
- [ ] Sync after restart uses the persisted cursor.
- [ ] Wrong/expired link token, cancelled link, and disconnected Item show sanitized errors.

Use **Show verification report** in the Plaid section to copy sanitized counts/statuses for the verification record. It contains no tokens, credential references, account numbers, descriptions, or amounts. Record the institution, account/transaction counts, and step statuses in `docs/reference/implementation-status.md`. Do not claim real-bank coverage from a Sandbox test.
