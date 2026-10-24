# UI overhaul plan (2026-10)

Status: **Phases 0–5 implemented (2026-10-24); Phase 6 next.** Written 2026-10-24.
Scope: charts, the Overview rebuild, the Transactions re-grade, and Settings sectioning.
This is the only live plan; the initial build plan is archived at
[archive/2026-10-initial-build-plan.md](archive/2026-10-initial-build-plan.md).

Effort is expressed as phases and exit gates, not calendar dates — the same convention as the
initial plan.

---

## 1. Baseline

- The app is a Phase 10 local release candidate. Current evidence lives in
  [implementation-status.md](../reference/implementation-status.md).
- The theming system is shipped: design tokens, a validated theme contract, three themes, the
  sidebar/topbar shell, card faces, and the picker in Settings. See
  [theming.md](../design/theming.md) and [proposal.md](../design/proposal.md).
- The Overview still renders four equal metric cards. The due-date rail and spending composition
  from the mockup do **not** exist in the app yet — they are structure, not styling.
- `transactions_reporting_date_idx (reporting_date, pending)` exists, so a 12-month aggregate is an
  indexed range scan. No migration is needed for the trail.
- The e2e suite pins Overview copy (`Current card balances`, `Upcoming reported payments`,
  `Pending in this month`, `current as of`, `Other currencies are shown separately`) and reads
  `.metric` / `.metric-value` in two places. Phase 3 preserves those deliberate hooks.

## 2. Decisions

Accepted 2026-10-24. They can still change until Phase 3 starts.

| # | Decision | Why |
|---|---|---|
| 1 | **The ring shows gross spend, not net.** Center shows gross; caption shows `− refunds → net`. | A proportional ring cannot represent negatives, and refunds are negative by definition. The hero keeps net spending. Omitting refund-affected categories would hide real spend. |
| 2 | **Top N + "Other"**, N = 3–6, ranked by gross; "Other" is a neutral tint, not a hue. | Keeps the chart readable and the palette honest. |
| 3 | **The trend window ends at the selected month**, so browsing history moves the window. The current month is marked "so far". | A trend pinned to today contradicts the month-scoped model; an unmarked partial month reads as a collapse. |
| 4 | **Both charts are theme variants**, not hard-coded: `composition: ring \| ranked-bars \| hidden`, `trend: bars \| hidden`. | The theming contract is the extension path; no dead options. |
| 5 | **Negative months render below a zero baseline.** | Rare but real after a large refund; clamping would lie. |

## 3. Non-goals

- No chart library. Inline SVG and CSS only, so the CSP (`default-src 'none'`) and the "no remote
  assets" rule hold.
- No new network calls, accounts, or provider features.
- No change to the security model, the vault, or the IPC envelope.
- No rewrite of the renderer architecture; components stay in `src/features`.

---

## 4. Phase 0 — `spend_history` command

**Contract** ([contract.ts](../../src/lib/bridge/contract.ts)):

```ts
spend_history: {
  request: { months?: number /* 1..24 */; endMonth?: string /* YYYY-MM */ };
  response: {
    currency: "USD";
    startMonth: string;
    endMonth: string;
    months: Array<{
      month: string;        // YYYY-MM, always N rows, zero-filled
      grossMinor: number;   // purchases + fees + interest
      refundsMinor: number; // positive magnitude
      netMinor: number;     // gross − refunds
      count: number;
      complete: boolean;    // false for the current and future months
    }>;
  };
}
```

**Rust** — [ledger.rs](../../src-tauri/src/storage/ledger.rs):

- `spend_history(conn, end: Month, months: u32, current_month: Month, currency: &str) -> SpendHistory`.
- One query grouping on `substr(reporting_date, 1, 7)`, reusing the existing
  `EFFECTIVE_TYPE`, `EFFECTIVE_EXCLUDED`, `SPEND_TYPES` constants and the `account_spend`
  predicate set (`removed = 0`, `pending = 0`, `currency = USD`).
- Zero-fill missing months from a `BTreeMap`; **reject** `months` outside 1–24 rather than clamping;
  mark the current and any future month incomplete.
- Resolve the current month in the service from the same demo/real source as the dashboard
  (`demo_today()` or the settings timezone), default `endMonth` to it, and reject malformed values.
- Register the command in the gateway, require an unlocked session, keep the sanitized-error path.

**Mock** — [bridge.ts](../../src/mocks/bridge.ts) via `summaryFor(month)` in
[data.ts](../../src/mocks/data.ts), so the browser behaves like Rust.

**Tests**

- current-month row equals `dashboard.summary.netSpendingMinor` exactly (reconciliation guard);
- zero months appear as zeros, ordering is stable, out-of-range windows are rejected;
- EUR rows never leak into a USD total;
- locked session is rejected with `vault_locked`;
- request validation rejects `months: 0`, `months: 25`, malformed `endMonth`.

**Exit gate:** `cargo test` for the new module, contract tests green, mock parity verified by a
Vitest call to the mock bridge.

**Implemented 2026-10-24.** Deliberate differences from the draft above:

- `months` is validated and rejected, never clamped. The current/future distinction is applied to
  every month in the window, not just the last one.
- The storage function takes `current_month` and `currency` explicitly so it stays independent of
  demo mode and timezone; the service resolves both.
- Refunds are returned as a positive magnitude; `net = gross − refunds` is asserted, and the
  current-month `net` is reconciled against `month_summary`.
- Evidence: 6 new Rust integration tests, 3 mock-data tests, 2 contract/bridge tests; `pnpm verify:all`
  green. The chart UI is still Phase 2–3 work.

---

## 5. Phase 1 — theme schema

In [schema.ts](../../src/theme/schema.ts), add to `options`:

```jsonc
"overview": {
  "composition": "ring" | "ranked-bars" | "hidden",
  "trend": "bars" | "hidden",
  "topCategories": 5,   // 3..6
  "trendMonths": 12     // 6..12
}
```

- Widen `chart` from 3–5 to 3–6 colors.
- Update all three themes: Ledger `ring/bars/5/12`; Graphite `ranked-bars/bars/4/6`;
  Editorial `ring/bars/6/12`, plus a sixth chart color each.
- `pnpm theme:schema && pnpm theme:check`.
- Add the new keys to [theming.md](../design/theming.md) §3.1 and the extension checklist.

**Exit gate:** schema test green, JSON Schema regenerated, all three themes validate, docs updated.

**Implemented 2026-10-24.** The schema, all three themes (with sixth colors and overview options),
`applyTheme` data attributes, the JSON Schema, and the theming guide are in place. A new
`applyTheme` test checks the attributes and `--chart-6` compilation.

---

## 6. Phase 2 — chart components

New folder `src/features/dashboard/charts/`:

| File | Responsibility |
|---|---|
| `chartMath.ts` | pure helpers: shares, donut dash offsets, bar scale and zero baseline, top-N selection. No DOM. |
| `chartMath.test.ts` | rounding sums to 100%, single segment, zero total, negative months, N > categories. |
| `CategoryRing.tsx` | stroke-dasharray donut; 0/1/N segments; center total; `role="img"` + `aria-label`. |
| `RankedBars.tsx` | horizontal ranked composition for themes that choose it; labels always visible. |
| `TrendBars.tsx` | CSS-grid bars; zero baseline; negative support; current-month hatch + "so far"; selected-month emphasis. |
| `ChartLegend.tsx` | `<ul>` of swatch, label, amount, share; shared by ring and bars. |
| `ChartTable.tsx` | visually hidden table with exact figures for screen readers. |

Rules: colors only from `--chart-1..6` and `--muted`; type from `--type-scale`; no color-only
meaning (labels always present); motion respects the theme and the OS setting.

**Exit gate:** component tests for accessible names/labels and the 0/1/N cases; no layout
overflow at 1100 px.

**Implemented 2026-10-24.** All six files exist, plus `RankedBars.tsx` for the Graphite variant.
Tests cover rounding (sums to 100), single/zero/N segments, negative and zero months, empty states,
legend/table figures, and the 0/1/N accessible names. `charts.css` includes current-month hatching,
a shared zero baseline, selected-month emphasis, and reduced-motion handling.

---

## 7. Phase 3 — Overview rebuild

[OverviewView.tsx](../../src/features/dashboard/OverviewView.tsx), in the mockup's order:

1. Hero: net spending, the purchases/refunds/fees arithmetic, pending as a label (not a card).
2. Due rail: `$known total`, 14-day strip from `overview.upcoming.entries`, four nearest payments,
   "amount not reported" as a ring pip and text.
3. Composition: ring or ranked bars per theme, legend, "Other", refund footnote.
4. Trend: 12 bars ending at the selected month, current month marked.
5. Balances: total, "7 of 8 reported", four rows with state chips (replaces the freshness panel).
6. Needs attention: amber card with three items and actions (replaces the two notice bars).
7. One footnote: `current as of …`, plus the currency notice when present.

Preserved deliberately: `h1 Overview`, the three metric labels, `.metric`/`.metric-value` on the
balances metric, `.month-label` in the shell, the currency notice, the demo label. The two green
notice bars go; their content moves into the attention card.

**Exit gate:** e2e green (no spec churn beyond the deliberate ones), ring and trend reconcile with
the hero, 1100 × 760 clean, theme switch changes composition without a reload.

**Implemented 2026-10-24.** The Overview now renders the hero (with eligible/pending/total rows),
the 14-day due rail and four nearest payments, the theme-selected composition (ring, ranked bars,
or hidden), the `spend_history` trend, balances with state chips, and a three-item attention card
with actions. The old freshness panel and the two green notice bars are gone; the currency notice
and `current as of` footnote remain. Trend loading/error states are explicit.

- Deliberate e2e additions: charts are present, a theme switch swaps the ring for ranked bars
  without a reload, month navigation changes the ring and trend while balances stay fixed, and the
  Overview is checked at 1100 × 760.
- The attention actions are labelled "Open connections", "Review balances", and "Open ledger"
  rather than "Settings"/"Cards" so they do not collide with navigation accessible names.
- 5 Overview component tests cover the layout, theme variants, hidden composition, action routing,
  and the empty state.

---

## 8. Phase 4 — Transactions re-grade

[LedgerView.tsx](../../src/features/dashboard/LedgerView.tsx):

- Group rows by reporting date with a sticky day header and a day subtotal.
- Merchant monograms; category as a quiet chip; status as dot + word.
- Credits (payments/refunds) stop rendering as errors; rust is reserved for excluded/failed.
- Meta line: `26 rows · 18 eligible · $420.16 USD`.
- **Cross-currency subtotals were fixed ahead of this phase.** `eligible_subtotal_minor` and
  `pending_subtotal_minor` are gone; `LedgerPage.subtotals` returns one entry per currency with
  amounts and eligible/pending counts. Use it; never add a hardcoded USD predicate because that
  would zero a foreign-currency-only filter. See the fix in `ledger.rs` and the regression test
  `ledger_subtotals_never_sum_different_currencies`.
- **Decide the pagination contract before building day subtotals.** The table loads 50 rows per
  request, so a day split across pages cannot have a correct full-month subtotal on the client.
  Either return day groups from Rust or label each day subtotal as page-local; the exit gate must
  not claim full-month day totals otherwise.
- Sticky filter bar; keep every existing label (`Search transactions`, `Clear filters`,
  `By supplier`) and the detail-panel flow.

**Exit gate:** ledger e2e green, subtotal reconciles per currency, day totals are internally
consistent with the rows shown (and only full-month if backend grouping is added).

**Implemented 2026-10-24.**

- Date-sorted views group rows by reporting date; amount/merchant sorts stay flat and keep the
  Date column. Day subtotals are explicitly labelled **page subtotal** because the table is
  paginated. A scroll observer hides group headers once their entire group is above the sticky
  table header, so labels do not pile up.
- Merchant monograms, category chips, dot + word status, and credit styling are in place:
  refunds/payments use `amount-credit`, transfers are neutral, and rust is reserved for excluded
  rows (including an excluded refund). The supplier table's refund column is also credit-styled.
- The meta line keeps `N rows` as its own text node (existing e2e hook), adds `N eligible`, and
  renders eligible/pending subtotals per currency with explicit currency codes.
- The filter bar is sticky; the transaction table scrolls under a sticky column header.
- Tests: 7 `ledgerPresentation` unit tests (monogram, amount tone, status, day totals, currency
  separation) and an e2e test for grouping, page subtotals, sticky classes, credits, and the
  hidden superseded headers.

---

## 9. Phase 5 — Settings sectioning

[SettingsView.tsx](../../src/features/settings/SettingsView.tsx):

- Section list: Appearance, Lock & idle, Accounts, Connections & sync, Backups & recovery,
  Merchant rules, Import & export, Security events, About.
- Render one section at a time. The panels already exist as components, so this is mostly layout
  and an active-section state. Keep crypto-erase in Backups & recovery with its typed confirmation.
- **Test impact:** `backups.spec.ts`, `edits.spec.ts`, `security.spec.ts`, and `shell.spec.ts`
  click controls without navigating first. Add `openSettingsSection(page, name)` to
  [helpers.ts](../../e2e/helpers.ts) and update those specs in the same commit.

**Exit gate:** all settings-driven specs green; no control is unreachable; 1100 px clean.

**Implemented 2026-10-24.**

- Left section list, one section rendered at a time: Appearance, Lock & idle, Accounts,
  Connections & sync, Backups & recovery, Merchant rules, Import & export, Security events, About.
  Demo mode hides the four real-vault sections; Accounts stays read-only.
- Control placement: window close behavior, reporting timezone, and passphrase change live in
  **Lock & idle**; manual payment metadata sits with **Accounts**; security facts, demo mode, and
  quit live in **About**; crypto-erase remains inside **Backups & recovery** with its typed
  confirmation.
- `openSettingsSection(page, name)` added to [helpers.ts](../../e2e/helpers.ts) and used by the
  backup, edit, shell, and new settings specs. The security spec only checks the Settings heading,
  so it still lands on Appearance.
- Two new e2e tests cover section isolation, control reachability, and demo section visibility.
  Playwright is now 40 tests; Vitest stays 71.

---

## 10. Phase 6 — polish

Empty/loading states for charts, skeleton for the Overview while `dashboard_get` is in flight,
dark-mode QA for every theme, screenshots for [theming.md](../design/theming.md), and a final
`pnpm verify` plus `cargo test`.

---

## 11. Test strategy

| Layer | Additions |
|---|---|
| Rust | history aggregation, window clamp, currency filter, reconciliation, unlock requirement |
| Contract | request/response schemas, unknown-command rejection, payload caps |
| Vitest | chart math, component accessibility, theme schema, mock parity |
| Playwright | charts render with numbers; month switch changes the ring and trend; theme switches composition; 1100 × 760; existing suites unchanged except Phase 5 helpers |
| Manual | dark mode per theme, reduced motion, VoiceOver read of the chart summary |

Accessible names are a compatibility surface: 36 Playwright tests assert them. Renaming copy is a
deliberate, same-commit change — never a side effect.

---

## 12. Acceptance criteria

- Ring shares sum to 100% within rounding; ring total + refunds = hero net, in minor units.
- The trend's last bar equals that month's dashboard net exactly; every month present, zeros included.
- Theme switch swaps ring ↔ ranked bars with no reload and no code path difference.
- A new theme file can enable/disable either chart through `options` alone.
- No new runtime dependency; bundle delta limited to the chart code.
- `tsc`, `eslint --max-warnings=0`, Vitest, Playwright, `cargo test`, and `pnpm build` all green.

## 13. Risks

| Risk | Mitigation |
|---|---|
| History totals drift from the Overview headline | shared SQL constants and an exact reconciliation test |
| Ring's gross-vs-net confusion | center/caption arithmetic; net stays in the hero |
| E2E coupling (Overview labels, Settings sectioning) | preserve genuine hooks; update specs deliberately in the same commit |
| Charts become the loudest thing on screen | capped segments, theme chart ramp, restrained ring weight |
| Current-month bar misread | hatch + "so far" label |
| Scope creep into Transactions/Settings while building charts | phases are independent; charts ship first |

## 14. Sequencing

Phase 0 → 1 → 2 → 3 ships the chart feature end-to-end. Phases 4 and 5 are independent after that
and can be reordered. One commit per phase so each gate is a recoverable snapshot.
