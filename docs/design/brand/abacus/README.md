# Abacus alternatives — 28 Oct 2026

Six directions that carry the [wychoi.com](https://wychoi.com) abacus into cardigan_money. The
installed Forest button is unchanged; these are suggestions, not a decision.

![Six abacus directions](preview/board.png)

The source is a 256 px raster: a `#07090d` rounded tile, a white rounded frame, slate rods, five
white beads (three over two), and one `#5b8dff` bead. It is flat, geometric, and already says
“calculation” — which is what a ledger is. The strongest directions keep the bead rhythm and the
single accent; the weaker ones add detail that disappears below 32 px.

## The six

| # | Direction | What it does | At 16 px | Choose it if |
| --- | --- | --- | --- | --- |
| 01 | [Card abacus](icons/01-card-abacus.svg) | The frame becomes a credit card: 1.58 ratio, chip, two rods, five beads. | Card outline and beads, chip lost | The icon should explain the product before the brand. |
| 02 | [C abacus](icons/02-c-abacus.svg) | A round C holds the rods and beads. | **Strongest** — the C and blue bead survive | You want the name in the mark and the Dock to work. |
| 03 | [Five beads](icons/03-five-beads.svg) | The abacus distilled: two rods, five beads, a ghost card. | Beads and blue dot, rods fade | You want the quietest, most modern mark. |
| 04 | [Placket](icons/04-placket.svg) | The abacus turns on its side and becomes a cardigan placket: V-neck, three buttons. | White bar with three dots | You want the cardigan and the abacus to be one object. |
| 05 | [Knit abacus](icons/05-knit-abacus.svg) | The site geometry in today’s forest, cream and rust, with four-hole buttons. | Cream frame and one rust dot | The new mark must sit beside the installed brand. |
| 06 | [Ledger abacus](icons/06-ledger-abacus.svg) | A folded page turns the frame into the ledger itself. | White page with dots | The mark should feel like a document, not an app. |

**Recommendation.** [02 C abacus](icons/02-c-abacus.svg) is the strongest stand-alone mark: the
initial, the abacus, and 16 px legibility in one shape. [01 Card abacus](icons/01-card-abacus.svg)
is the best product mark when the icon has room to explain itself.
[04 Placket](icons/04-placket.svg) is the one people will remember. If continuity with the current
Forest button matters more than a clean break, [05 Knit abacus](icons/05-knit-abacus.svg) is the
bridge.

## Colorways

![The final three, in three palettes](preview/colorways.png)

The site palette is the literal inheritance. **Cardigan** keeps the installed forest, cream and
rust. **Blueprint** matches the theme added on 28 Oct 2026. If the app mark should follow themes,
pin one palette or keep the frame constant and let the accent change with `accent`.

## Legibility

The board shows 64/32/24/16 px on light and dark strips. The measured result:

- **02 C** is the clearest at 16 px without the surrounding context.
- **01 Card** and **06 Ledger** need 32 px; at 16 px the chip and fold disappear but the silhouette
  and the blue bead hold.
- **03 Five beads** loses the rods at 16 px; the five beads become the mark.
- **04 Placket** survives as a white bar with three dots — distinctive, not self-explanatory.
- **05 Knit** survives because the frame is the thickest of the set.

## Files

| Path | What |
| --- | --- |
| [`source/wychoi-abacus-256.png`](source/wychoi-abacus-256.png) | The original site mark, 256 px |
| [`source/wychoi-abacus.svg`](source/wychoi-abacus.svg) | The same mark redrawn, tile-only viewBox |
| [`icons/`](icons) | Six 1024 px candidates, path-only |
| [`icons/colorways/`](icons/colorways) | C, Card and Placket in site, cardigan and blueprint |
| [`build.mjs`](build.mjs) | Generates every SVG from the shared geometry |
| [`render.mjs`](render.mjs) | Renders `preview/board.png` and `preview/colorways.png` |

All icons are 1024 × 1024 with the artwork on an 824 px tile (`rx 184`), matching the rest of the
brand. They are flat and path-only: no filters, gradients, or live text.

```bash
node docs/design/brand/abacus/build.mjs
node docs/design/brand/abacus/render.mjs
```

## If one is chosen

1. Replace [`src/assets/brand-mark.svg`](../../../../src/assets/brand-mark.svg) with the chosen tile
   (the 824 px viewBox is already the crop the app uses).
2. Export a 1024 px PNG and run `pnpm tauri icon path/to/1024.png` to regenerate the Tauri icon set.
3. Check the Dock, the 26 px header slot, the vault gate, and the boot screen at 16 and 32 px.
4. Update [`docs/design/brand/README.md`](../README.md) and the artifact record, then rebuild.
