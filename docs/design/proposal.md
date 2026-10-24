# UI direction — *Quiet ledger, tactile cards*

A proposal for how cardigan_money should look. Written as a design + front-end handoff, not a
mood board: every recommendation below is something that can be built from the current data
contract and shipped inside the current security model.

**Status:** direction doc. The design tokens, the theme engine
([theming.md](theming.md)), the sidebar/topbar shell, and the card-face wallet have since been
implemented. The Overview rewrite, the Transactions re-grade, and the Settings section split are
still proposals. The original artifacts are
[`mockup.html`](mockup.html) (open it in a browser), this document, and five exported images in
this folder.

Current state, for comparison: the app is a calm, warm-neutral dashboard with correct
information but a web-app shape — top tabs, full-width banners, a footer, four equal metric
cards. Everything is styled with 1,146 lines of hand-written CSS; Tailwind is imported for
preflight only.

---

## 1. How I looked

- Ran the real front end against the demo mock bridge (`VITE_MOCK_BRIDGE=1`) and captured
  Overview, Cards, Transactions, Settings, the locked vault gate, dark mode, and a 900 px
  window.
- Read [`styles.css`](../../src/styles.css), [`AppShell.tsx`](../../src/app/AppShell.tsx), the
  four views, and the bridge [contract](../../src/lib/bridge/contract.ts) to see what data the
  UI has but does not use.
- Checked the constraints that should shape design decisions:
  - **CSP** — `default-src 'none'`, `img-src 'self' data:`, `font-src 'self'`. No remote fonts,
    no remote issuer logos, no CDN chart library. Everything must be local, inline SVG, or data
    URIs.
  - **Window** — 1440×960 default, 1100×760 minimum, native title bar
    ([lib.rs](../../src-tauri/src/lib.rs)).
  - **Tests** — 133 `getByRole`, 65 `getByText`, 34 `getByLabel` assertions in `e2e/`. A re-skin
    that renames navigation, headings, or buttons breaks the suite. Presentation can change;
    accessible names should not.

---

## 2. What the current UI says

Not a list of things I dislike — a list of places where the visuals fight the product.

| # | Observation | Evidence | Consequence |
|---|---|---|---|
| 1 | It reads as a website, not a macOS app | top tab bar ([AppShell](../../src/app/AppShell.tsx#L275)), full-bleed banners, a page footer with version text | the strongest trust signal — "this is a native local app" — is given away |
| 2 | Every number weighs the same | four identical metric cards ([OverviewView](../../src/features/dashboard/OverviewView.tsx#L51)); $42.50 "Pending" is as loud as $420.16 net spending | the eye has nowhere to land; the month's headline is one of four equals |
| 3 | The product's hard-won distinctions are paragraphs | "A statement balance is not the remaining amount due." repeated on every card ([CardsView](../../src/features/dashboard/CardsView.tsx#L105)); "Click to filter the ledger" as a sentence | roughly half of each tile's text is disclaimer copy; state is explained instead of encoded |
| 4 | Notices are green | two stacked `inline-notice` bars for stale balances and broken connections ([OverviewView](../../src/features/dashboard/OverviewView.tsx#L107)); `.inline-notice` uses the accent green | the accent means "good", "info", and "something is wrong" at once |
| 5 | Red means "negative", not "bad" | `.amount-negative` is `--danger` ([styles.css](../../src/styles.css#L955)); it is used for card payments and refunds | a payment that reduces your balance looks like an error |
| 6 | Composition is computed but hidden | `summary.suppliers` / `summary.categories` are returned and rendered only behind the "By supplier" toggle on Transactions | the Overview has no "where did it go", the one question a spending app exists to answer |
| 7 | Credit data is unused | `availableCreditMinor`, `creditLimitMinor` in the [contract](../../src/lib/bridge/contract.ts#L199) appear nowhere in the UI | no utilization anywhere; a card app without utilization |
| 8 | Settings is a scroll | 11 equal panels in one `settings-grid` ([SettingsView](../../src/features/settings/SettingsView.tsx#L135)); the page is ~3,900 px tall at 1440 | appearance, accounts, connections, backups, crypto-erase, rules, import, quit all compete |
| 9 | Type is flat | body 11–14 px, no display scale, weights 550/650, `tabular-nums` only in tables | numbers don't feel precise; hierarchy has to be carried by borders |
| 10 | Two styling systems | `@import "tailwindcss"` ([styles.css](../../src/styles.css#L1)) with no utility classes; tokens are CSS variables but not exposed to Tailwind | new work has to choose a side every time |

Two more that are less visible but matter:

- **No motion at all.** Month changes, panel opens, and row selection snap. 120–160 ms of easing
  is the cheapest quality upgrade available.
- **Hairlines carry all structure.** Borders are 1.35:1 against the canvas; they do the work that
  space and elevation should do, and control borders are below the 3:1 WCAG minimum for UI
  components.

---

## 3. The direction

**Quiet ledger, tactile cards.** Two registers that do different jobs:

- The *ledger* is typographic and restrained: warm paper, one green, tabular numbers, hairlines
  kept for tables only.
- The *cards* are tactile: real card proportions, issuer-tinted but tonally harmonised, with a
  balance that reads at a glance and a state chip that never relies on color alone.

Four principles:

1. **Chrome recedes, numbers lead.** A native shell (sidebar, toolbar, status bar) replaces the
   tab strip and banners. The largest type on any screen is money.
2. **State is a design element, not a sentence.** Seven states get a label, a color, and a shape.
   The paragraph is available once, behind an info affordance.
3. **Physical things get physical form.** Cards look like cards. Everything else is a table or a
   number.
4. **One accent, used only for "confirmed".** Green = fresh, healthy, enforced. Amber =
   attention. Rust = failure or destruction. Nothing informational is green.

Deliberately *not* the reference set: mint-neon fintech, gradient dashboards, donut charts,
illustrated empty states. Closer references: Things 3's restraint, Apple Wallet's physicality,
Copilot Money's card art, Linear's density discipline.

---

## 4. The system

### 4.1 Color

Semantic tokens, two themes, one meaning each. Measured WCAG ratios (AA needs 4.5:1 for text,
3:1 for control boundaries):

| Token | Light | Dark | Role | Ratio |
|---|---|---|---|---|
| `--bg` | `#f7f5ef` | `#14171c` | canvas | — |
| `--sidebar` | `#efeae0` | `#181c22` | navigation ground | — |
| `--surface` | `#fffdf8` | `#1c2128` | cards, toolbars | — |
| `--ink` | `#161d26` | `#edf0f4` | primary text | 16.7 : 1 |
| `--ink-2` | `#3f4a58` | `#c6cdd7` | secondary text | 8.9 : 1 |
| `--muted` | `#5f6a78` | `#97a2b1` | tertiary text, labels | 5.0 : 1 on canvas |
| `--line` | `#e3ddce` | `rgba(255,255,255,.08)` | hairlines inside tables only | — |
| `--line-control` | `#9c917d` | `rgba(255,255,255,.30)` | input / select boundaries | 3.0 : 1 |
| `--accent` | `#2c6659` | `#7fc2b0` | confirmed, primary action | 6.1 : 1 on canvas |
| `--accent-soft` | `#deece6` | `#1e3833` | confirmed wash | 8.0 : 1 with `--accent-ink` |
| `--amber` | `#8a5c12` | `#e4bf7e` | attention, stale, reconnect | 5.0 : 1 on wash |
| `--rust` | `#9c3a2e` | `#e9a79c` | failure, destructive | 5.6 : 1 on wash |

Two findings from measuring: the app's current `--muted` is fine (5.3:1) — the legibility problem
is size and repetition, not contrast. But the current input border (`#d9d4c8`) is **1.35:1**,
below the 3:1 required to identify a control; `--line-control` exists for exactly that reason and
should not be used for decoration.

**Dark mode** keeps the same semantic mapping: warm-tinted neutrals rather than blue-black,
cards keep their hue but lift ~8 L*, hairlines become 8% white instead of solid grey, and the
accent brightens so it stays legible on `#14171c`.

### 4.2 Type

One family — the system stack. That is a feature on macOS: it matches native chrome, costs no
bytes, and cannot be blocked by the CSP. No webfont.

| Role | Size / weight / tracking | Notes |
|---|---|---|
| Hero money | 50 / 640 / −0.032em | `tabular-nums`, always |
| Title | 25 / 650 / −0.022em | one per screen |
| Section number | 23–34 / 620 | balances, due total |
| Body | 13 / 400 | 1.45 line height |
| Secondary | 11.5 / 400 | muted |
| Label | 9.5 / 650 / +0.10em | uppercase, muted `#5f6a78`, never the decorative grey |
| Mono | 10.5 | last-4, dates, IDs |

Minimum text size in the product becomes 10.5 px (footnotes) and 11 px (fine print). The current
11 px fine print at 12 px muted stays for legal copy only, and never more than one line on a
primary screen.

### 4.3 Space, radius, elevation

- 4-based spacing; screen padding 28 px, card padding 18–20 px, grid gap 14 px.
- Radii: 7 (controls), 11 (panels), 15 (card faces, hero cards). One radius per element class.
- Three elevations only: flat (in-table), raised (1 px hairline + 1 px shadow), floating
  (overlay/sheet). Card faces get a colored shadow, `0 10px 22px -8px rgb(15 20 26 / 45%)`.
- Motion: 120 ms `cubic-bezier(.2,0,0,1)` for hovers, 160 ms for panels; month changes
  cross-fade the numbers only. All of it behind `prefers-reduced-motion`.

### 4.4 State model

The distinction between *missing*, *stale*, *unsupported*, *zero*, *pending*, and *excluded* is
the product. It gets one component and one table:

| State | Label | Color | Shape | Used by |
|---|---|---|---|---|
| Fresh | `Fresh` | accent | filled dot | balances, connections |
| Stale | `Stale · 10d` | amber | filled dot + age | balances |
| Needs action | `Reconnect`, `Sync error` | amber | filled dot | connections, accounts |
| Unknown | `Balance unknown`, `not reported` | neutral / amber text | ring dot, em dash | balances, due amounts |
| Pending | `Pending` | neutral | hollow ring | ledger rows |
| Excluded | `Excluded` | neutral, row at 55% | tag, no dot | ledger rows |
| Other currency | `EUR`, `€45.00` | neutral | currency tag | balances, rows |
| Failure | `Sync failed` | rust | filled dot | connections, events |

Rule: never color alone. Every state has a word, and the shape (filled vs ring) survives
greyscale. This is what replaces the repeated disclaimer paragraphs.

### 4.5 Components

| Component | Purpose | Notes |
|---|---|---|
| `StatusChip` | the table above | one component, size + tone props |
| `Monogram` | issuer identity without trademarks | initials on a tonal tile; inline SVG marks optional later |
| `CardFace` | the wallet object | 1.586 ratio, issuer hue, utilization edge, next-payment line |
| `Meter` | utilization | thin track, visible label ("29% of $15,000") |
| `DayStrip` | 14-day due window | 14 cells, filled pip = known, ring = not reported |
| `RankedBar` | composition | single hue, amount right-aligned, top 6 + "all categories" |
| `Panel` | settings / content block | header, body, footer actions |
| `Table` | ledger, accounts, details | sticky header, 36–40 px rows, day grouping |
| `Toolbar` | month + contextual actions | draggable region (`-webkit-app-region: drag`) |
| `StatusBar` | trust facts | cipher version, last integrity check, lock state |
| `InfoHint` | the paragraph, once | popover/sheet, keyboard accessible |
| `Switch`, `Segmented` | settings, view modes | the theme control becomes a 3-way segmented, not a cycling button |

`Button` keeps its four variants but gains a consistent 27/31 px height, a real disabled state,
and a press state.

### 4.6 Tailwind

Don't migrate; *expose*. Move tokens to `src/styles/tokens.css`, then map them in Tailwind 4:

```css
@theme {
  --color-surface: var(--surface);
  --color-ink: var(--ink);
  --color-accent: var(--accent);
  --font-sans: -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif;
}
```

Keep the existing component classes (`.card`, `.btn`, `.chip`) for repeated shells; use utilities
for new layout. Rewriting 400 class strings would add risk and no visual value.

---

## 5. Screens

The mockup shows Overview, Cards, Settings, and the language sheet. Images:
[overview](screen-overview.jpg) · [cards](screen-cards.jpg) · [settings](screen-settings.jpg) ·
[language](screen-language.jpg) · [dark](screen-overview-dark.jpg).

### Shell → sidebar, toolbar, status bar

- 230 px sidebar: brand, four destinations with counts, provider status, appearance, lock,
  version. The accent wash marks the active destination.
- 52 px toolbar: month stepper on the left (the app's global control), contextual actions on the
  right. It is the drag region; controls opt out.
- 26 px status bar: "Encrypted local ledger · no telemetry", last integrity check, version +
  cipher. This is where trust facts belong — quietly persistent.
- **Demo mode** stops being a 40 px banner. It becomes: an amber `Demo` chip in the toolbar, the
  synthetic-data sentence in the status bar, and the demo actions in the toolbar overflow. One
  line instead of three.
- Native title bar: to let the sidebar reach the top edge, the window needs
  `titleBarStyle: "Overlay"` + `hiddenTitle` and a 28 px top inset for the traffic lights. That's
  a Tauri config change with a visual-regression risk; the fallback (sidebar starting under the
  standard title bar) looks fine and is zero-risk. Recommend the fallback first.

### Overview → three questions, three blocks

The page answers: *what did I spend, what is due, what do I owe — and what needs fixing.*

- **Hero**: net spending at 50 px with the arithmetic as typographic evidence:
  `$606.94 purchases − $300.00 refunds + $113.22 fees`. This is the product's honesty on the
  surface, and it fits in one line.
- **Where it went**: top 6 categories as ranked bars, right-aligned amounts, one footnote for the
  refund. Built from `summary.categories` — no backend change.
- **Due in 14 days**: known total, a 14-cell day strip with filled pips for known amounts and
  ring pips for "not reported", then the four nearest payments. Built from
  `overview.upcoming.entries`.
- **Card balances**: known total, "7 of 8 reported", four rows with freshness chips. The missing
  account is named in the subline, not silently dropped.
- **Needs attention**: one amber card with three concrete items and their action ("Retry sync",
  "Reconnect", "Add balance"). This is what the two green notice bars become.
- **One footnote**, once: "Balances and due dates are current as of … — they do not change with
  the selected spending month." with a "How totals work" link.

The four equal metrics and the freshness list disappear as such; their content is redistributed
into these five blocks.

### Cards → wallet + table + inspector

- **Wallet view** (default): 3-up grid at 1440, 2-up at 1100. Each face carries issuer, last-4,
  balance, next payment, state chip, and a utilization edge. Stale faces desaturate; the missing
  balance gets a dashed edge and the words "Unknown — never shown as $0.00".
- **Details view**: the metadata that used to be five lines per tile becomes a 6-column table
  (balance, limit used, next payment, statement, source, updated) with the disclaimer **once** in
  the footer. This is the biggest readability win in the app.
- A card opens an inspector (existing `detail-panel` pattern) with statement metadata, due source,
  and per-card notes.
- Issuer hues are tuned into one tonal range — blues, graphite, one silver, one deep teal — so the
  wallet reads as a family rather than a brand collage. Marks are monograms, not trademarks.
  Inline SVG marks are possible later; nothing may be fetched.

### Transactions → scan faster, keep the semantics

Not mocked, but specified:

- Sticky filter bar; same five controls and labels, but arranged as a toolbar with "Clear filters"
  on the right.
- Group rows by reporting date with a sticky day header and a day subtotal. Merchants get a 24 px
  monogram. Category becomes a quiet chip. Status becomes dot + word.
- Amounts: signed, right-aligned, `tabular-nums`. Stop using red for credits. Reserve `--rust` for
  excluded/failed and use a neutral "credit" treatment for payments and refunds, with the type tag
  carrying the meaning.
- The meta line becomes `26 rows · 18 eligible · $420.16 USD` and the pending count becomes a
  chip. (See the bug in §9.)
- The "By supplier" toggle stays as a view mode, not a separate mental model.

### Settings → sub-navigation

- 186 px section list inside the content area: Appearance, Lock & idle, Accounts, Connections &
  sync, Backups & recovery, Merchant rules, Import & export, Security events, About.
- One pane at a time, two or three panels maximum per pane. Appearance gets real theme previews
  and a reduce-motion switch. Accounts keeps the existing table but drops the redundant type
  column and moves destructive actions into the account editor.
- Crypto-erase stays a rust-bordered card under Backups & recovery with its typed confirmation
  unchanged. Security copy stays on the page; it just stops sharing a scroll with CSV import.

### Vault gate → the calmest screen in the app

Keep the single centered card, but: 48 px mark, 22 px headline, one primary action, "Open demo"
as a quiet row under a dashed divider, and the three legal lines collapsed to one with an info
hint. Add an explicit waiting state for the native passphrase sheet ("Waiting for macOS…"), since
right now the click appears to do nothing until the sheet appears.

---

## 6. What I would deliberately not do

- **No webfonts.** CSP is `font-src 'self'`, and a system font is the native choice. If the brand
  later wants a display face for the wordmark only, bundle one weight as a local asset.
- **No remote or recycled issuer logos.** Trademarks aside, `img-src` forbids network assets.
  Monograms now; inline SVG marks are a possible follow-up.
- **No chart library.** The bars and the day strip are markup and CSS; the data is already there.
- **No new backend commands** for anything on these screens. One optional ask in §10.
- **No full Tailwind rewrite.** Tokens + primitives first.

---

## 7. Plan

Ordered so each phase is shippable and no phase depends on a later one.

| Phase | Scope | Files | Effort |
|---|---|---|---|
| 0 | Tokens, type scale, Tailwind `@theme`, base elements, `prefers-reduced-motion` | new `src/styles/tokens.css`, `styles.css` | S |
| 1 | Primitives: `StatusChip`, `Monogram`, `Meter`, `Panel`, `Segmented`, `Switch`, `InfoHint`, `StatusBar` | new `src/components/*` | S |
| 2 | Shell: sidebar, toolbar, status bar, demo treatment, theme control | `AppShell.tsx`, `App.tsx`, `ThemeToggle.tsx` | M |
| 3 | Overview: hero, composition, day strip, balances, attention | `OverviewView.tsx` | M |
| 4 | Cards: `CardFace`, wallet, details table, inspector | `CardsView.tsx` | M |
| 5 | Transactions: day grouping, monograms, status, sticky bar | `LedgerView.tsx`, `TransactionEditor.tsx` | M |
| 6 | Settings: section navigation, pane split, form controls | `SettingsView.tsx`, panels | L |
| 7 | Polish: empty/loading skeletons, motion, 1100 px QA, contrast audit | — | S |

S ≈ a focused pass; M ≈ a pass plus tests; L ≈ the panel split is the one place that touches a
lot of JSX.

**Test strategy.** Keep every accessible name the e2e suite asserts: `Overview`, `Cards`,
`Transactions`, `Settings`, `Sync now`, `Lock now`, `Previous month`, `Add account`,
`Export CSV…`, `No provider connected`, `Demo mode — synthetic data`, `Your encrypted ledger is
locked`, plus the settings headings. Three specs assert banner copy directly
(`e2e/shell.spec.ts:13`, `e2e/backups.spec.ts:53`, `e2e/dashboard.spec.ts:97`); if the banners
move into the toolbar/status bar, update those specs in the same commit rather than keeping dead
UI to satisfy them. Add `data-testid` only where a name genuinely must change.

---

## 8. Risks

| Risk | Mitigation |
|---|---|
| e2e suite is coupled to copy and roles | keep names; update the three banner assertions deliberately |
| Native title bar makes the sidebar mock-up impossible as drawn | ship the standard-title-bar fallback; overlay title bar is a separate pass |
| 1100 px minimum window | wallet 3→2 columns, overview right rail stacks below 1180 px |
| Card hues clash with the warm neutral identity | one tonal range; one silver and one teal only; verify greyscale |
| Removing banners weakens the "synthetic data" warning | demo chip + status-bar sentence + settings row; keep the accessible text |
| CSS churn while security work lands | phases are presentation-only; no bridge or Rust changes |
| Contrast regressions | the token table above is measured; add a check to the design review for any new color |

Accessibility notes: 3:1 for all control boundaries, 4.5:1 for text, focus ring 2 px with 2 px
offset (current pattern is good — keep it), visible labels for every icon button, and no
information carried by color alone.

---

## 9. One real bug found while doing this

The Transactions meta line reports an **"Eligible subtotal" that mixes currencies**. In the demo
it reads `$558.16` while net spending is `$420.16`; the difference is exactly the €138.00 of EUR
rows. The SQL sums `amount_minor` for posted spend rows without a currency filter
([ledger.rs](../../src-tauri/src/storage/ledger.rs#L370)), and the UI formats the result as USD
([LedgerView.tsx](../../src/features/dashboard/LedgerView.tsx#L385)).

That contradicts the app's own promise ("Other currencies are shown separately and never summed
with USD") and is exactly the kind of trust defect this design direction is trying to prevent.
Suggested fix: filter the subtotal to the dashboard currency, or show per-currency subtotals. The
design already assumes this: the redesigned meta line is `18 eligible · $420.16 USD`.

---

## 10. Decisions I need from you

1. **Brand.** The mockup presents "Cardigan · money" rather than the repo-style `cardigan_money`.
   Keep the underscore, or adopt the two-line wordmark?
2. **Card identity.** Monograms only (safe, coherent, no trademarks), or invest in a set of
   hand-drawn inline SVG issuer marks?
3. **Accent.** Stay with ledger green, or shift to something warmer (ink/terracotta) and reserve
   green strictly for confirmations?
4. **Density.** Is the comfortable density in the mockup right for the ledger, or should the
   Transactions table stay compact for 50-row scanning?
5. **Scope.** Start with Phases 0–2 (tokens + shell, no screen rewrites), or do Overview + Cards
   first because they carry the visual identity?
6. **Trend line.** A 6-month net-spending sparkline would need one new command. Worth it, or skip
   until later?
