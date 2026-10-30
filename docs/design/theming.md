# Theming

cardigan_money themes are **data, not code**. A theme is one JSON file that selects component
variants and sets design tokens. Adding a theme requires no React or CSS changes; it does need a
placket colorway for the app mark (section 3.8), which is code-side because the mark must exist
before settings load and as a fixed macOS icon. This document is the contract: if a knob is
described here, it is implemented; if it is not here, a theme cannot change it.

- Reference JSON Schema (editor autocomplete/validation): [`src/themes/theme.schema.json`](../../src/themes/theme.schema.json)
- Canonical schema (single source of truth): [`src/theme/schema.ts`](../../src/theme/schema.ts)
- Bundled themes: [`src/themes/`](../../src/themes)
- Runtime: [`src/theme/apply.ts`](../../src/theme/apply.ts), [`src/theme/tokens.ts`](../../src/theme/tokens.ts)

---

## 1. Mental model

```
theme.json ──► zod validation ──► CSS custom properties on <html>  ──► every component
             │                    └─ data-nav / data-card-art / data-density / data-motion
             │                       / data-overview-composition / data-overview-trend
             └─ options ─────────► component variants (AppShell, CardsView, OverviewView)
```

There are two kinds of themable decisions:

| Kind | Example | How a theme expresses it |
|---|---|---|
| **Tokens** | colors, type, radii, spacing, motion, shadows | values in the JSON document |
| **Variants** | card art vs data tiles, density, motion | one of a fixed enum in `options` |

Everything else — screens, data, copy, security behavior, the ledger — is structure and is not
themed. Section 6 explains where to go when you want to change structure.

**Where it is stored.** The theme id is `settings.themeId` in the encrypted SQLCipher database;
light/dark/system is `settings.theme`. Both survive lock, restart, and demo mode, and neither is
written to `localStorage` (there is a test for that). Blueprint is the default theme; unknown ids
fall back to `blueprint` with one console warning.

---

## 2. Add a theme in four steps

```bash
cp src/themes/ledger.theme.json src/themes/sandstorm.theme.json
# edit id ("sandstorm"), name, description, palettes, options
# add a placket palette to scripts/build-brand-marks.mjs, then:
node scripts/build-brand-marks.mjs   # writes src/assets/brand/placket-sandstorm.svg
# import it in src/brand/marks.ts
pnpm theme:check      # validates the document, contrast, schema, and colorway
pnpm dev              # Settings → Appearance → Sandstorm
```

Rules the checks enforce:

1. **`id` must equal the file name** (`sandstorm.theme.json` → `"id": "sandstorm"`).
2. **Both `light` and `dark` palettes are required.** A dark-first theme still declares a light
   companion; make it deliberate, not an inverted copy.
3. **Contrast passes** (matrix in section 5).
4. **No unknown keys.** Typos fail loudly instead of silently doing nothing.
5. **Values are constrained**: hex colors, pixel numbers in range, a font stack charset, a CSS
   easing, a bounded shadow color. No `;`, `{`, `}`, `url()`, or code execution.
6. **A placket colorway exists** for the theme (`src/brand/marks.test.ts`). Without one the mark
   falls back to Blueprint, but the check fails so a theme is never shipped half-branded.

The picker in Settings → Appearance is generated from the files, sorted by `name`. There is no
registry to edit.

---

## 3. Document reference

Minimal shape:

```jsonc
{
  "$schema": "./theme.schema.json",
  "id": "ledger",
  "name": "Ledger",
  "description": "One line for the picker.",
  "options": { "cardArt": "faces",
               "density": "comfortable", "motion": "full",
               "overview": { "composition": "ring", "trend": "bars",
                             "topCategories": 5, "trendMonths": 12 } },
  "type":    { "...": "see 3.2" },
  "shape":   { "...": "see 3.3" },
  "spacing": { "...": "see 3.4" },
  "motion":  { "...": "see 3.5" },
  "modes":   { "light": { "...": "palette" }, "dark": { "...": "palette" } },
  "issuers": { "default": { "from": "#39424c", "to": "#1b2026", "ink": "#f4f7f8" } }
}
```

### 3.1 `options` — the variants

**Navigation is not a theme option.** The left source list and the top tab strip are
`settings.navigation` (`sidebar` or `topbar`), stored in the encrypted settings table and chosen in
Settings → Appearance. Any theme can run with either layout; the theme picker preview follows the
selected layout.

| Key | Values | What changes | Where |
|---|---|---|---|
| `cardArt` | `faces` · `tiles` | Cards page: physical card art with issuer colors, or the data-tile layout | `CardsView.tsx` |
| `density` | `comfortable` · `compact` | Row height, panel padding, screen padding in tables and panels | `:root[data-density]` in `styles.css` |
| `motion` | `full` · `calm` | `calm` removes all transitions; the OS reduced-motion setting always wins | `:root[data-motion]` in `styles.css` |
| `overview.composition` | `ring` · `ranked-bars` · `hidden` | Proportional donut, horizontal ranked bars, or no composition chart | `OverviewView.tsx` |
| `overview.trend` | `bars` · `hidden` | Monthly column trend, or no trend chart | `OverviewView.tsx` |
| `overview.topCategories` | 3–6 | Category segments before the neutral "Other" segment | `OverviewView.tsx` |
| `overview.trendMonths` | 6–12 | Trailing months in the trend window | `OverviewView.tsx` |

### 3.2 `type`

| Key | Range | Effect |
|---|---|---|
| `sans` | font stack | Body and UI |
| `mono` | font stack | Masks (`•••• 4101`), dates, identifiers |
| `display` | font stack | Headings and hero numbers — this is how Editorial gets serif |
| `scale` | 0.85–1.2 | Multiplies structural type sizes (body, titles, table text, balances) |
| `displayWeight` | 300–800 | Headings and money |
| `displayTracking` | −0.06–0.02 | em tracking for headings and hero numbers |
| `lineHeight` | 1.2–1.8 | Body line height |

`mono` also sets the app wordmark. The name is rendered at weight 500, and the `_` takes the
theme's `accent`, so the separator stays distinct in every palette.

Use system fonts. The CSP is `font-src 'self'`; a webfont must be bundled and would need its own
`@font-face` — not supported by the theme format today.

### 3.3 `shape`

| Key | Range | Effect |
|---|---|---|
| `controlRadius` | 0–24 px | Buttons, inputs, chips, segmented controls |
| `panelRadius` | 0–28 px | Panels, tables, filter bars, warnings |
| `cardRadius` | 0–28 px | Card faces, vault card, wizard card |
| `borderWidth` | 0–2 px | Hairlines on panels, cards, inputs |

### 3.4 `spacing`

| Key | Range | Effect |
|---|---|---|
| `screen` | 12–48 px | Content padding |
| `card` | 10–32 px | Panel / metric / card-face padding |
| `gap` | 6–24 px | Grid and stack gaps |

### 3.5 `motion`

| Key | Range | Effect |
|---|---|---|
| `fast` | 0–1000 ms | Hover, press, chip and row transitions |
| `base` | 0–1000 ms | Panel and layout transitions |
| `easing` | CSS easing | Default timing function |

### 3.6 `modes.light` / `modes.dark` — the palette

Every key is required in both modes.

| Key | Role | Contrast requirement |
|---|---|---|
| `bg` | app canvas, month nav ground | — |
| `sidebar` | navigation ground | — |
| `surface` | cards, toolbars, panels | — |
| `surface2` | inputs, tracks, hover fills | — |
| `surface3` | stronger recessed fill | — |
| `text` | primary text | ≥ 7:1 on `surface` |
| `text2` | secondary text | ≥ 4.5:1 on `surface` |
| `muted` | labels, tertiary text | ≥ 4.5:1 on `surface` and `bg` |
| `border` | hairlines | — (not a control boundary) |
| `borderStrong` | visible dividers | — |
| `borderControl` | input / select boundary | ≥ 3:1 on `surface` (WCAG 1.4.11) |
| `accent` | primary action, selection, active nav | ≥ 3:1 on `bg` |
| `accentInk` | accent-colored text and links | ≥ 4.5:1 on `surface` and the accent wash |
| `onAccent` | text on an accent fill | ≥ 4.5:1 against `accent` |
| `success` | fresh, healthy, enforced | ≥ 4.5:1 on `surface` and the success wash |
| `warning` | stale, reconnect, attention | ≥ 4.5:1 on the warning wash |
| `danger` | failure, destructive | ≥ 4.5:1 on `surface` |
| `focus` | focus ring | ≥ 3:1 on `bg` |
| `chart` | 3–6 ordered series colors | used by charts in order |
| `shadow` | soft elevation color (alpha allowed) | — |
| `shadowStrong` | card-face / overlay shadow color | — |

`accent` and `success` are separate on purpose: a theme may own a warm accent (Editorial's
terracotta) without turning "Fresh" into a warning. Ledger and Graphite happen to set them to the
same color.

**Derived tints are not themed.** These are computed in CSS from the palette so themes stay small
and harmonious:

```
--accent-soft  = color-mix(in srgb, accent 13%, surface)
--success-soft = color-mix(in srgb, success 13%, surface)
--accent-line  = color-mix(in srgb, accent 34%, border)
--warning-soft / --warning-line, --danger-soft / --danger-line  (12% / 36–38%)
```

### 3.7 `issuers` — card art

Optional. Keys are issuer slugs (lowercase, dashes): `chase`, `american-express`, `citi`,
`synchrony`, `bilt`, `paypal`, `demo-bank`. Special key `default` is the fallback for unknown
issuers.

| Key | Role | Requirement |
|---|---|---|
| `from` / `to` | gradient endpoints of the card face | — |
| `ink` | all text on the face | ≥ 4.5:1 against both `from` and `to` |

Only used when `options.cardArt` is `"faces"`. Without an `issuers` block, card faces use a
neutral graphite fallback. Slugs are derived from the stored issuer name, so `"American Express"`
finds `american-express`.

### 3.8 Brand mark colorways

The app mark is the **Placket**, a fixed glyph with one colorway per theme. It is deliberately not
a theme JSON key: the mark must exist on the lock screen, before settings load, and as the macOS
app icon, so its palettes are build-time assets rather than runtime theme values.

| Theme | Tile | Placket | Buttons | Accent button | Neckline thread |
|---|---|---|---|---|---|
| `blueprint` (default) | `#081120` | `#f4f7ff` | `#081120` | `#2f5bff` | `#c45a18` |
| `editorial` | `#1b1712` | `#faf6ee` | `#1b1712` | `#a0472b` | `#c97c55` |
| `graphite` | `#101216` | `#e9edf1` | `#15191e` | `#33628f` | `#7fb0e0` |
| `ledger` | `#17453a` | `#f7f1e3` | `#1f4c42` | `#b26a4c` | `#5a9d8c` |

- **Blueprint is the fixed default.** It is the Dock icon and the fallback when a
  theme has no colorway. A missing colorway never leaves a blank slot.
- **The in-app mark follows `theme.id`** in the header, the vault gate, onboarding, and the boot
  screen. The theme picker shows each theme's colorway beside its name.
- **Assets and mapping.** `src/assets/brand/placket-<id>.svg` is generated by
  `scripts/build-brand-marks.mjs`; `src/brand/marks.ts` maps theme ids to those assets and exposes
  `brandMarkFor()`. The script refuses to run for a bundled theme without a palette, and
  `src/brand/marks.test.ts` covers the mapping plus the Blueprint fallback.
- **The thread** is the V-neck seam. It carries the theme's secondary color, is decorative, and
  disappears below about 32 px.
- **The Dock icon is always Blueprint.** It is generated from
  `src/assets/brand/placket-blueprint-icon.svg` with
  `pnpm tauri icon src/assets/brand/placket-blueprint-icon.svg`. Do not generate it from a theme
  colorway: the icon cannot follow the in-app theme.

---

## 4. Theme anatomy, annotated

```jsonc
{
  "id": "editorial",                  // == file name
  "name": "Editorial",                // picker label
  "options": {
    "cardArt": "tiles",               // no card faces
    "density": "comfortable",
    "motion": "calm",                 // no transitions
    "overview": {
      "composition": "ring",          // or ranked-bars / hidden
      "trend": "bars",                // or hidden
      "topCategories": 6,
      "trendMonths": 12
    }
  },
  "type": {
    "display": "\"Iowan Old Style\", Palatino, ui-serif, Georgia, serif",
    "scale": 1.04                     // slightly larger type
  },
  "modes": {
    "light": { "accent": "#a0472b", "success": "#5f6b46", "...": "..." },
    "dark":  { "accent": "#d98b62", "success": "#a3b083", "...": "..." }
  }
}
```

---

## 5. What the checks do

`pnpm theme:check` runs [`src/theme/schema.test.ts`](../../src/theme/schema.test.ts):

1. Every `*.theme.json` parses, and the count matches the files on disk (a skipped file fails).
2. `id` matches the file name; ids are unique.
3. The full contrast matrix from 3.6, for every theme × both modes, plus each issuer's `ink`
   against both gradient ends. Failures name the theme, mode, pair, and measured ratio.
4. CSS variable compilation: required variables exist and equal the source values.
5. `src/themes/theme.schema.json` is in sync with the Zod contract.
6. Every bundled theme has a placket colorway, and a missing or unknown theme id falls back to
   Blueprint (`src/brand/marks.test.ts`).

Regenerate the editor schema after changing `src/theme/schema.ts`:

```bash
pnpm theme:schema
```

### Visual QA (2026-10-24)

After the Overview rebuild, the three bundled themes were rendered at 1440×960 and 1100×760 in
both modes:

- **Ledger** — ring composition, 12-month trend.
- **Graphite** — ranked bars, 6-month trend, compact density.
- **Editorial** — serif display, ring, six categories plus Other, calm motion.

Navigation was theme-controlled when this QA ran; it is now the independent `settings.navigation`.

Legends, the due rail, the attention card, and the sectioned Settings layout stayed legible, and
there was no horizontal overflow. The automated contrast matrix remains the floor; VoiceOver and
native macOS behavior are still owner smoke items. The proposal images in this folder remain the
visual reference; screenshots are not duplicated here because they age faster than the theme
contract.

### Visual QA (2026-10-28)

**Blueprint** — the [wychoi.com](https://wychoi.com) palette: white paper and navy ink in light,
near-black and deep navy in dark, electric blue for action, orange for markers. It was rendered at
1440×960 and 1100×760 in both modes with the demo data: the picker card, Overview (ranked bars,
12-month trend), Cards (all eight issuer faces), Transactions, and Settings stayed legible, the
contrast matrix passes, and there was no horizontal overflow.

---

## 6. What themes cannot change (and where to go instead)

| Want to change | Themed? | Where it lives |
|---|---|---|
| Colors, type, radii, spacing, motion, shadows | **Yes** | theme JSON |
| Card art vs tiles, density, motion level | **Yes** | `options` |
| Navigation layout (left sidebar vs top tab strip) | **No — user setting** | Settings → Appearance; encrypted `settings.navigation` |
| Card-face colors per issuer | **Yes** | `issuers` |
| The app mark's placket colorway | **Code-side, not a theme key** (3.8) | `src/brand/marks.ts`, `scripts/build-brand-marks.mjs` |
| Wording, labels, aria-labels | No | JSX in `src/features/**` (keep accessible names: the e2e suite depends on them) |
| Information architecture, which panels exist | No | `SettingsView.tsx`, `OverviewView.tsx`, … |
| Chart composition/trend variants, top-N, window | **Yes** | `options.overview` |
| A new chart type or visualization | No — style it with `--chart-*` | new component + CSS |
| Data, calculations, security behavior | No | Rust services |

**To add a new themable knob**, touch these in order and nothing else:

1. `src/theme/schema.ts` — add the field with bounds and a `.describe()` (this becomes the docs
   and the JSON Schema).
2. `src/theme/tokens.ts` — emit a CSS variable, **or** `src/theme/apply.ts` — set a `data-*`
   attribute for a component variant.
3. `src/styles.css` — consume the variable/attribute.
4. `src/theme/schema.test.ts` — add a contrast check if the value carries text.
5. `pnpm theme:schema && pnpm theme:check`.
6. Add the key to at least one theme (or provide a CSS fallback in `:root`), and add a row to
   section 3 of this document.

The schema test does not hard-code the theme list, so adding files never breaks the suite.

---

## 7. Recipes

**Dark-first theme.** Copy `graphite`, keep both palettes, set `dark` first-class and make `light`
a deliberate daylight translation. Mode still comes from `settings.theme`.

**Compact, dense theme.** `"density": "compact"`, `type.scale` 0.96, `spacing.screen` 24,
`shape.panelRadius` 8. Graphite is exactly this.

**Paper / editorial theme.** Serif `display`, cream `surface`, `cardArt: "tiles"`,
`motion: "calm"`, larger `spacing`, small radii. Editorial.

**Portfolio / brand theme.** Sample the four colors from the source's own tokens: a light canvas,
a dark ink, one action color for `accent`, and one marker color for `warning` and chart series 2.
Keep the action color for `focus`, use a darker tint for `success`, and let a system serif carry
`display`. Blueprint is this recipe.

**New theme colorway.** Add a palette to `scripts/build-brand-marks.mjs`, run the script, import
the new SVG in `src/brand/marks.ts`, and run `pnpm test`. The header, gate and picker follow the
theme id; Blueprint remains the fixed fallback.

**Only change the accent.** Change `accent`, `accentInk`, and `success` in both modes; the wash
and lines derive automatically.

**One issuer with custom art.** Add `"issuers": { "default": {…}, "chase": {…} }` — only `chase`
changes.

**High-contrast theme.** Raise `text`→`surface` toward black/white, set `borderControl` to a 4.5:1
grey, keep `muted` ≥ 4.5:1, and set `motion: "calm"`. The checks are the floor, not the target.

---

## 8. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Theme not in the picker | File name/id mismatch, or a validation error | Run `pnpm theme:check`; check the console (`[theme] … is invalid`) |
| Settings reverts to Ledger | Unknown `themeId` in the DB, or the Rust validator rejected it | Settings ids must match `^[a-z0-9][a-z0-9-]{0,39}$` |
| Picker shows a theme but nothing changes | Theme loaded but the document has no `modes` values? | Validation would fail; reload and check the console |
| Some text unreadable | A palette pair below the threshold | `pnpm theme:check` prints the ratio needed |
| Mark stays Blueprint after a theme change | The theme id has no placket colorway | Add a palette in `scripts/build-brand-marks.mjs`, run it, import it in `src/brand/marks.ts` |
| Card faces all look the same | No `issuers` block, or issuer names don't slug to your keys | Add `default`, or match the slug (`american-express`) |
| Lint/test fails after a schema edit | The committed JSON Schema is stale | `pnpm theme:schema` |

---

## 9. Why it is built this way

- **CSS variables, not a CSS-in-JS library.** Themes apply by setting ~60 custom properties on
  `:root`; the browser does the cascade, and there is no re-render cost when switching.
- **Zod as the single source.** Types, runtime validation, the JSON Schema, and the docs all come
  from one place, so they cannot drift.
- **Variants are enums, not plugins.** A theme cannot ship JavaScript, which keeps the CSP
  (`default-src 'none'`) and the security model intact.
- **The app mark is a build asset, not a theme token.** It must render before settings load and as
  the macOS icon, so each theme gets a prebuilt placket instead of a runtime rule.
- **Contrast is a test, not a guideline.** The palette is small enough to measure every pair, and
  a theme that fails never reaches the picker.
