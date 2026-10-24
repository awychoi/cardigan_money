# Plaid Sandbox verification record — 2026-10-24

Verified against build SHA-256 `2ef8b68bdb1dd2708dadcb4bbb3e75bd6e71a4404393429c02a1218ee3bb3138`. Evidence supplied by the owner from the app's read-only **Show verification report** output. The report contains aggregates only; no tokens, credential references, account numbers, descriptions, or amounts.

## Evidence

Two Sandbox Items were linked. The owner quit the app, reopened it, unlocked the same vault, and ran sync again. The before/after reports show:

| Connection | Status | Accounts | Provider transactions | Pending | Cursor present | Last success |
| --- | --- | ---: | ---: | ---: | --- | --- |
| Charles Schwab (Sandbox) | healthy | 2 | 18 | 0 | yes | 2026-10-24T04:45:00Z → 04:45:52Z |
| Chase (Sandbox) | healthy | 0 | 0 | 0 | yes | 2026-10-24T04:45:01Z → 04:45:53Z |

- First Charles Schwab run added **2 balance snapshots**; the post-restart run added 2 more (append-only), while accounts/transactions stayed 18/2. No duplicate accounts or transactions appeared.
- The post-restart transactions step reported `+0` because it resumed from the persisted cursor.
- Liabilities for both Items reported `partial` / `available_but_null`; no prior liability values were deleted and no values were fabricated.
- Chase returned no credit-card accounts. This app intentionally imports Plaid `credit` accounts only; a depository-only Item remains a healthy connection with 0 imported accounts. It was not treated as a failure.

## What this verifies

- Native Keychain setup and token retrieval work functionally: Hosted Link completed, public-token exchange succeeded, and subsequent syncs used the stored access token.
- Hosted Link + polling + exchange worked end to end against Plaid Sandbox.
- Plaid Transactions Sync imported 18 transactions across 2 credit accounts; balances synced; the persisted cursor survived a full app restart without duplication.
- Liabilities capability reporting is honest (`available_but_null`), not fabricated.

## Not verified / limitations

- **No Production Item or real institution** was linked. Production remains disabled in code.
- **Keychain attribute inspection was not performed** by the owner (Keychain Access UI/ACL). The functional round trip is verified; attribute inspection remains a native smoke item.
- **Reconnect/update mode was implemented and unit/mock-covered but not exercised live.** The connection stayed healthy, so no reconnect prompt occurred.
- The test used Sandbox institutions/accounts; no real personal financial data was involved.
- The Chase Item's 0-account result is expected from the credit-only account filter, not evidence of missing depository support.
- Hosted Link URL/token handling is enforced in code and not exposed, but browser/network-level inspection was not performed.

## Reproduce

Use `docs/integrations/plaid-guide.md` with the `2ef8b68...` build, sync, **Show verification report**, then quit/reopen and repeat. Compare account/transaction counts and `cursorPresent`.
