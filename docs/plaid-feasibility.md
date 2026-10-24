# Plaid feasibility for Cardigan Money

**Scope:** local macOS desktop app (Tauri, no server, no inbound listener) linking Plaid Items for Transactions and balance data, with optional Liabilities.

**Status: research only — nothing was tested.** No API call was made, no Link session opened, no credentials exist. Everything below comes from reading Plaid's public docs on **2026-10-23** (retrieval date). Limits, flows, and defaults are as documented on that date and may change. Inference is labelled **Inferred**; unresolved gaps are labelled **Unverified**.

## Summary

| Question | Answer as documented on 2026-10-23 | Confidence |
| --- | --- | --- |
| Free Trial links real Items? | Yes — 10 Production Items, 8 products incl. Transactions, Balance, Liabilities | Verified |
| Link SDK for macOS/Tauri? | No. Platforms: web, iOS, Android, React Native, Flutter | Verified |
| Can the app receive a redirect? | Not in Production: HTTPS only, no custom schemes, localhost HTTP only in Sandbox | Verified |
| Link inside a Tauri webview? | Webview Link is deprecated; Plaid routes those integrators to Hosted Link | Verified |
| Hosted Link with no server? | Yes to start; the `public_token` must be polled from `/link/token/get` | Verified |
| Webhook required? | No — but without one, polling is the only delivery path | Verified |
| Liabilities everywhere? | No — US plus limited Canada, specific account types, per-Item errors | Verified |
| Shippable today? | No — no credentials, no enrolment, no tested link | Blocker |

## 1. Trial plan, Item limits, product access, pricing caveats

**Eligibility.** New Plaid teams in the **US/Canada** created **on or after 2026-04-15**. Unavailable if the team has a pending/approved Production application or existing Limited Production access ([billing — Trial plans](https://plaid.com/docs/account/billing/#trial-plans), [Support: plans](https://support.plaid.com/hc/en-us/articles/16110502116887-What-are-Plaid-s-prices-and-pricing-plans-and-how-do-they-differ)). Approval is "auto-approved for most developers", not guaranteed instant ([Support: environments](https://support.plaid.com/hc/en-us/articles/16110110883479-How-are-Sandbox-Production-Trial-plan-and-Limited-Production-different)).

**Item limit.** **10 Production Items**, then a paid plan is required. Removing Items does **not** restore capacity: "Removing Items created on a Trial plan (using `/item/remove`) will *not* allow you to create more Items." Every Production access token counts, including lost ones ([billing](https://plaid.com/docs/account/billing/#trial-plan-limitations)). No Sandbox cap is documented; Sandbox is described as unlimited test Items ([Support](https://support.plaid.com/hc/en-us/articles/16110110883479-How-are-Sandbox-Production-Trial-plan-and-Limited-Production-different)).

**Products (all 8).** Assets, Auth, Balance, Identity, Investments (incl. Refresh), **Liabilities**, **Transactions** (incl. Refresh), Statements ([billing](https://plaid.com/docs/account/billing/#trial-plan-limitations)). Almost all OAuth institutions work immediately "without completing the full Production registration process" ([Support: plans](https://support.plaid.com/hc/en-us/articles/16110502116887-What-are-Plaid-s-prices-and-pricing-plans-and-how-do-they-differ)). Compliance-centre items (MSA, security questionnaire, app display info) are not required until upgrade; Chase and PNC also waive their gates for Trial accounts ([OAuth guide](https://plaid.com/docs/link/oauth/#prerequisites-for-adding-oauth-support)).

**Pricing caveats — no dollar figures, deliberately.** Plaid publishes no price list: "A price list is not available in the documentation… Pricing information for Pay-as-you-go and Growth plans will be displayed on the last page before you submit your request" ([billing](https://plaid.com/docs/account/billing/#pricing-information)). Any number here would be invented.

- **Transactions** and **Liabilities** are *subscription* products: billed monthly while a valid `access_token` exists — **even if the Item is broken and no calls are made**. Only `/item/remove` stops it; no proration; removal is irreversible for history.
- **Balance** (`/accounts/balance/get`) is *per-request* in Production; Sandbox is always free.
- Trial Items on subscription products **start billing on upgrade**, and afterwards you "will only retain Production access to any products you explicitly requested in your Production request form" ([billing](https://plaid.com/docs/account/billing/#trial-plan-limitations)). Treat Trial → paid as re-enrolment.
- Hosted Link is free; only Plaid-delivered sessions (SMS/email, Link Delivery beta) carry a per-link fee ([Hosted Link](https://plaid.com/docs/link/hosted-link/#pricing)). Opening the URL yourself avoids it.

## 2. Transactions, Balance, Liabilities availability

| Capability | Documented availability | Endpoint |
| --- | --- | --- |
| Transactions | Depository, credit, loan accounts; up to **730 days** initial history (default **90**); Plaid retains new data while the Item lives | [`/transactions/sync`](https://plaid.com/docs/transactions/) |
| Balance | Any Item initialised with **any other product**; `balance` is **not** a valid `products` value | [`/accounts/balance/get`](https://plaid.com/docs/api/products/signal/#accountsbalanceget) |
| Liabilities | **US plus limited Canada**; `credit` → `credit card`/`paypal`; `loan` → auto, commercial, commercial line of credit, consumer, home equity, home equity loan, line of credit, loan, mortgage, student | [`/liabilities/get`](https://plaid.com/docs/api/products/liabilities/#liabilitiesget) |

Balance is "available for use exclusively in combination with other Plaid products" ([Balance](https://plaid.com/docs/balance/)) — Transactions satisfies that. `/accounts/balance/get` forces a live refresh (typically <10s, occasionally 30s+), unlike cached `/accounts/get`.

**Liabilities is not universal** — "available primarily for US financial institutions, with some limited coverage of Canadian institutions" ([API reference](https://plaid.com/docs/api/products/liabilities/#liabilitiesget)). Two distinct failures:

1. **Item/institution does not support it** → `PRODUCTS_NOT_SUPPORTED`, or `liabilities` absent from `products`/`available_products` in `/item/get`.
2. **Item supports it but has no eligible accounts** (e.g. checking-only) → `NO_LIABILITY_ACCOUNTS`.

**Configuration matters.** `liabilities` in `products` filters the institution list to those supporting it ("Connectivity not supported" otherwise) and can exclude accounts. `optional_products` keeps institutions visible, adds it only where supported — but **bills** it there. `additional_consented_products` defers billing until the endpoint is called, but for Data-Transparency-Messaging Items (default for new US/CA customers) later consent needs update mode ([initializing products](https://plaid.com/docs/link/initializing-products/), [update mode](https://plaid.com/docs/link/update-mode/#requesting-additional-consented-products)).

**The UI must keep four states distinct** — collapsing them into "no data" is the likeliest bug:

| Observation | Meaning | UI treatment |
| --- | --- | --- |
| `200` + populated `liabilities` | Available | Show data |
| `200` + empty arrays or `null` fields | **Available but null** — bank returned nothing | "Not reported by your bank"; never render `null` as `0` |
| `PRODUCTS_NOT_SUPPORTED`, `NO_LIABILITY_ACCOUNTS`, `PRODUCT_NOT_ENABLED`, or absent from `/item/get` | **Unsupported** for this Item | Hide the surface; do not offer retry as if transient |
| `PRODUCT_NOT_READY`, `ITEM_LOGIN_REQUIRED`, `INSTITUTION_ERROR`/5xx, `RATE_LIMIT_EXCEEDED`, `ACCESS_NOT_GRANTED` | **Temporarily unavailable** (or consent missing) | Backoff; for `ITEM_LOGIN_REQUIRED`/`ACCESS_NOT_GRANTED` prompt reconnect |

Source: [Item errors](https://plaid.com/docs/errors/item/#products_not_supported). `balances.available` can legitimately be `null`, and `current` "may be `null`" from `/accounts/balance/get` — same rule.

## 3. Desktop linking constraints

Hard constraints, not preferences:

- **HTTPS-only redirects.** "Redirect URIs must use HTTPS. The only exception is on Sandbox, where, for testing purposes, redirect URIs pointing to localhost are allowed over HTTP. **Custom URI schemes are not supported in any environment.**" ([OAuth guide](https://plaid.com/docs/link/oauth/#constructing-valid-redirect-uris)). `cardigan_money://` is invalid as a `redirect_uri` everywhere, Sandbox included.
- **No production loopback.** A `127.0.0.1` listener is Sandbox-only. In Production the URI must be `https` and allowlisted in the Dashboard ([/link/token/create](https://plaid.com/docs/api/link/#linktokencreate)). Self-signed local TLS is not a workaround.
- **Webview Link is deprecated.** "Using webviews to present Link is deprecated… migrate to Hosted Link or one of Plaid's official SDKs" ([Webview SDK](https://plaid.com/docs/link/webview/)). There is no macOS SDK: supported platforms are web, iOS, Android, React Native, Flutter, and "Webviews, iFrames, or flows where you don't control the frontend" route to Hosted Link ([Link overview](https://plaid.com/docs/link/#link-supported-platforms-and-presentation-modes)).
- **A browser is therefore the only supported Link surface** — not a webview, not an in-app window.

**Nuance — do not overstate "no custom scheme".** `redirect_uri` (the OAuth return leg) cannot be a custom scheme. A *different* field, Hosted Link's `completion_redirect_uri`, is documented to accept e.g. `wonderwallet://hosted-link-complete` and "does not need to be registered with the Plaid Dashboard" — but that guidance targets **native mobile** with `is_mobile_app: true` ([Hosted Link](https://plaid.com/docs/link/hosted-link/#integrating-hosted-link-in-native-mobile-applications)). Whether it works for a macOS app opening the URL in the default browser is **Unverified**, and it would only signal "finished" — the token still comes from polling.

## 4. Hosted Link + `/link/token/get` polling

**Verdict: the only flow that fits.** No hosting, no redirect URI, no custom scheme, no inbound listener.

1. `POST https://sandbox.plaid.com/link/token/create` (or `production.plaid.com`) with `client_id`, `secret`, `client_name` (≤30 chars), `language`, `country_codes`, `user.client_user_id`, `products`, and **`hosted_link: {}`**. "As long as you include the `hosted_link` object… the `hosted_link_url` will be present in the response" ([Hosted Link](https://plaid.com/docs/link/hosted-link/#creating-a-link-token)).
2. Response: `link_token`, `hosted_link_url`, `expiration`.
3. Open `hosted_link_url` in the **default browser**. No SDK, no webview, no JS bridge.
4. User completes Link, including any OAuth hand-off, in that browser session.
5. Poll `POST /link/token/get` with `client_id`, `secret`, `link_token`. Session data — including `public_token` — is "available from `/link/token/get` for **six hours** after the session has completed" ([Hosted Link](https://plaid.com/docs/link/hosted-link/#obtaining-a-public-token)).
6. Detect completion via `link_sessions[].finished_at` / `results`; take `results.item_add_results[].public_token`. Use `results`, not `on_success` (which predates Multi-Item Link).
7. `POST /item/public_token/exchange` → `access_token` + `item_id`; store in the macOS Keychain; discard `link_token`; never persist `hosted_link_url`.

**Lifetime.** Hosted Link URL defaults to **30 minutes** when Plaid is not delivering it (24h SMS / 7d email), customisable via `hosted_link.url_lifetime_seconds` ([Hosted Link](https://plaid.com/docs/link/hosted-link/#creating-a-link-token)). Under Hosted Link the `link_token` "will expire at the same time as the Hosted Link URL" — so **the 4-hour general default does not apply** ([/link/token/create response](https://plaid.com/docs/api/link/#linktokencreate)). Abandon past expiry → `INVALID_LINK_TOKEN`, mint a fresh token ([invalid tokens](https://plaid.com/docs/link/handle-invalid-link-token/)). Whether `/link/token/get` still serves data after token expiry but inside the six-hour window is **Unverified** — poll promptly.

**Attacker with the URL.** Plaid does not call this a bearer token, and we found no published logging guidance for it; we classify it as one because possession is sufficient to open that session. An attacker can: **complete the session with their own bank account**, so the app exchanges a `public_token` for an Item they control (an *integrity* failure — wrong account, misleading balances — not exfiltration); **consume or abort** the attempt (DoS); and observe the Link UI. They **cannot** get the `public_token` (that needs the client secret), read stored `access_token`s, or reach previously linked Items. Exposure is bounded by the URL's ~30-minute default.

**Handling rules.** Never log, print, persist, or place `hosted_link_url` in crash reports, analytics, or clipboard history; hold it in memory for the seconds needed to hand it to the browser; open it immediately. **Residual risk that cannot be engineered away:** opening an `https` URL writes it to browser history, and possibly synced history on other devices. This is inherent to URL hand-off; we found no documented control for it. Accept it, keep lifetimes short.

## 5. Hosted HTTPS redirect page — evaluation

**What must be hosted.** A page at an `https` URL on a domain the owner controls, added to the Dashboard allowlist ([/link/token/create](https://plaid.com/docs/api/link/#linktokencreate), [Dashboard](https://dashboard.plaid.com/developers/api)). On OAuth return the page receives an `oauth_state_id` and must reinitialise Link with the *same* `link_token` plus `receivedRedirectUri` ([OAuth guide](https://plaid.com/docs/link/oauth/#reinitializing-link)).

**What crosses it.** The opaque `oauth_state_id`; the `link_token` (normally in `localStorage` from the same browser, or passed in); and on success the `public_token` is delivered **to the page**, not to the Mac.

**Retention.** The page's JS context holds both tokens transiently; whatever happens next is the owner's liability, and a static page has nowhere safe to put them.

**Why it does not give the app the token.** A static HTTPS page has no channel to a sandboxed macOS process. Every option fails: a **webhook receiver** needs a server and public endpoint; **manual paste** moves a secret through the clipboard and probably an editor; a **custom-scheme redirect** carrying the token is unsupported as an OAuth redirect and any local app can claim the scheme. The page solves the *OAuth return leg* only, never *token delivery*.

**Is it even needed?** On desktop, Plaid documents OAuth as a **pop-up flow** in all environments "unless the page is accessed through a mobile webview" ([OAuth guide](https://plaid.com/docs/link/oauth/#redirect-uris-and-desktop--mobile-web)). Since Hosted Link is an ordinary Plaid page in the default browser, the OAuth round-trip is handled inside it and no `redirect_uri` should be required. **Inferred**, not stated for Hosted Link on desktop — confirm against an OAuth Sandbox institution.

**Conclusion: not practical.** It adds hosting, a domain, TLS and a JS page, and still strands the `public_token`. Do not build it unless a server appears for other reasons.

## 6. Reconnect, OAuth behaviour, token lifetime

- **Update mode** is the only reconnect path: create a `link_token` with `access_token` and **no `products`** array; the `access_token` does not change, so no re-exchange ([update mode](https://plaid.com/docs/link/update-mode/#using-update-mode)).
- **Hosted Link supports update mode** — "fully supported with all Plaid products and Link flows (including update mode)" ([Hosted Link](https://plaid.com/docs/link/hosted-link/)). Reconnect reuses `hosted_link: {}` + polling with the `access_token` attached.
- **Update-mode tokens expire in 30 minutes**, versus 4 hours for a normal token ([Link overview](https://plaid.com/docs/link/#error-handling-flows)).
- **OAuth re-entry:** update mode at an OAuth institution sends the user back through the bank's OAuth flow to restore revoked or missing permissions ([update mode](https://plaid.com/docs/link/update-mode/#requesting-additional-products-accounts-permissions-or-use-cases)).
- **Consent expires.** Many large US institutions need refresh every 12 months (Chase: 6 months / 1 year / "always"; Brex and Revolut US: 3 months; USAA: 18 months). `consent_expiration_time` comes from `/item/get`; `PENDING_DISCONNECT` fires **7 days** before. Sandbox `ins_129644`, `ins_132241`, `ins_117181` populate a 30-day expiry ([OAuth guide](https://plaid.com/docs/link/oauth/#refreshing-item-consent)).
- **`ITEM_LOGIN_REQUIRED`** triggers reconnect; in Sandbox it also hits any Item older than 30 days ([Item errors](https://plaid.com/docs/errors/item/#item_login_required)).
- **One-time use — the docs conflict.** Some pages call the `link_token` "a short lived, one-time use token" ([Webview SDK](https://plaid.com/docs/link/webview/#create)), yet update mode says you "can reuse the same Link token" to repair multiple Items ([update mode](https://plaid.com/docs/link/update-mode/#using-update-mode)). Treat it as **single-session by convention**: mint fresh per attempt, never design around reuse.

## 7. Recommendation and owner checklist

**Implement Hosted Link + `/link/token/get` polling in Rust, with the system browser as the only UI surface.** Nothing else in Plaid's documented surface fits a no-server, no-custom-scheme, no-loopback desktop app: create token with `hosted_link: {}` → open `hosted_link_url` via the Tauri opener → poll `/link/token/get` every few seconds with backoff, hard-stopping just past the 30-minute default → exchange `public_token` → Keychain. Then `/transactions/sync`, `/accounts/balance/get`, `/liabilities/get` on the user's schedule. Reconnect by repeating with `access_token` and no products.

**Do not implement:** the Webview SDK, an in-app browser window, a loopback HTTP/HTTPS listener, a custom-scheme return, or a redirect page.

**Product configuration.** Start with `products: ["transactions"]` and `optional_products: ["liabilities"]`, so Liabilities does not filter out institutions and the unsupported UI state is exercised naturally. If recurring Liabilities cost is a concern, `additional_consented_products: ["liabilities"]` defers billing until first call, at the cost of a possible update-mode consent step.

**De-risk before spending quota.** Sandbox is free, needs no approval, and has unlimited Items. Link can be bypassed there entirely: `/sandbox/public_token/create` mints a public token with no Link session ([Link overview](https://plaid.com/docs/link/#introduction-to-link)), letting the whole data layer and error-state UI be verified before the linking UX is exercised.

### Owner checklist (the agent must not perform these)

1. Create a Dashboard account at [dashboard.plaid.com/signup](https://dashboard.plaid.com/signup) and verify email — owner-only; the account and any terms accepted during signup are the owner's, not the agent's.
2. Submit the Trial application at [dashboard.plaid.com/trial-plan](https://dashboard.plaid.com/trial-plan) — owner-only; it is a contract, not a form an agent may accept.
3. Copy the `client_id`, **Sandbox** secret, and (after approval) **Production** secret. Deliver by a secure channel — never chat, never a committed file. The agent needs the env-var *names*, not values.
4. Confirm the Products page shows Transactions, Balance, Liabilities enabled by the Trial bundle ([products](https://dashboard.plaid.com/settings/team/products)); request nothing extra.
5. Choose environment order: Sandbox first (`sandbox.plaid.com`), then Trial/Production (`production.plaid.com`) — different secrets.
6. No redirect URI is needed for `hosted_link: {}`. Only if the OAuth redirect fallback is ever adopted: add an `https` URI at [dashboard.plaid.com/developers/api](https://dashboard.plaid.com/developers/api).
7. Acknowledge billing: Transactions and Liabilities bill monthly while the Item exists, including while broken. `/item/remove` is the only off-switch, and it is irreversible.
8. Approve which institution/account may be used for the one live linking test, and be present for it (bank MFA required).
9. After the first link, confirm the app shows the correct institution and masks, and that `/item/get` lists the expected products.
10. Re-run this checklist after any plan change: Trial → paid alters product access and starts billing.

## 8. Explicit blockers

| Blocker | Consequence | Unblocked by |
| --- | --- | --- |
| **No credentials** | No API call possible, not even Sandbox | Owner steps 1–3 |
| **No Trial enrolment** | Production Items impossible; Terms unaccepted | Owner step 2 |
| **No hosting** | Redirect-page alternative impossible (Hosted Link unaffected) | Domain + HTTPS host, if ever needed |
| **No server** | No webhook, so no `SESSION_FINISHED` push; polling mandatory; no server-side token custody | Design accepts this |
| **No custom scheme** | Cannot catch an OAuth return in-process (`cardigan_money://` is not a valid `redirect_uri`) | Nothing in scope |
| **No production loopback** | Cannot catch a redirect on `127.0.0.1` in Production | Nothing in scope |
| **No macOS SDK** | Link cannot be embedded; a browser is required | Nothing in scope |

**Live linking remains UNVERIFIED.** Until the owner completes enrolment and a Sandbox link runs end-to-end, "this app can link a Plaid Item" is a documentation-based expectation, not a fact. Gate one is a Sandbox link; gate two is one real Trial Item.

## Data flow

**Leaves the Mac (TLS to `sandbox.plaid.com` / `production.plaid.com` only):**

- `POST /link/token/create` — `client_id`, `secret`, `client_user_id` (random local UUID), product names.
- `POST /link/token/get` — `client_id`, `secret`, `link_token`.
- `POST /item/public_token/exchange` — `client_id`, `secret`, `public_token`.
- `POST /transactions/sync`, `/accounts/balance/get`, `/liabilities/get`, `/item/get`, `/item/remove` — `client_id`, `secret`, `access_token`.

**Leaves the Mac via the browser (outside the app's process and TLS trust boundary):** `hosted_link_url` to Plaid's hosted page, then whatever the user does there — including bank credentials typed into the institution's own OAuth page. The app cannot see, log, or intercept any of it.

**Does not leave the Mac:** the `client_secret` and `access_token` go nowhere except Plaid's API over TLS; never in a URL, webview, log line, crash report, or file outside the Keychain. No inbound listener, no telemetry endpoint, no owner-operated server — nothing is sent to the owner or a third party. `hosted_link_url` is memory-only.

**Cautions:** do not disable TLS verification or route through a logging proxy; scrub `access_token`, `secret`, and `hosted_link_url` from any error type deriving `Debug`; prefer the Tauri opener API over shelling out, so the URL never appears in a child process's argv.

## Sources and caveats

All retrieved **2026-10-23**: [Hosted Link](https://plaid.com/docs/link/hosted-link/) · [/link/token/create & /get](https://plaid.com/docs/api/link/) · [Link overview](https://plaid.com/docs/link/) · [OAuth guide](https://plaid.com/docs/link/oauth/) · [Update mode](https://plaid.com/docs/link/update-mode/) · [Invalid tokens](https://plaid.com/docs/link/handle-invalid-link-token/) · [Webview SDK](https://plaid.com/docs/link/webview/) · [Billing](https://plaid.com/docs/account/billing/) · [Transactions](https://plaid.com/docs/transactions/) · [Balance](https://plaid.com/docs/balance/) · [Liabilities](https://plaid.com/docs/liabilities/) · [Item errors](https://plaid.com/docs/errors/item/) · [Support: plans](https://support.plaid.com/hc/en-us/articles/16110502116887-What-are-Plaid-s-prices-and-pricing-plans-and-how-do-they-differ) · [Support: environments](https://support.plaid.com/hc/en-us/articles/16110110883479-How-are-Sandbox-Production-Trial-plan-and-Limited-Production-different).

**Known uncertainties.** (1) Whether `/link/token/get` serves data after the Hosted Link token expires but inside the six-hour window. (2) Whether a desktop Hosted Link session with no `redirect_uri` completes OAuth institutions via the pop-up flow — strongly implied, not stated. (3) Whether `completion_redirect_uri` accepts a custom scheme on macOS with `is_mobile_app: false`. (4) Actual pricing, which is not public. (5) Whether a brand-new account has usable Sandbox credentials before Trial approval. (6) Plaid's docs are undated, so "current" means "as served on 2026-10-23".

## Current status (2026-10-24)

The Sandbox adapter and Hosted Link flow are **implemented**. Sandbox verification with owner credentials is **pending**; no live Item or institution coverage is claimed yet. See `docs/plaid-guide.md` for the owner test. No link URL, link token, public token, access token, or client secret is exposed to the renderer. Production remains disabled.
