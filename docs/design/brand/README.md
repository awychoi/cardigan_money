# Brand

**Installed (29 Oct 2026): Placket, one colorway per theme. Blueprint is the default theme.**

The app mark is the 04 Placket from the [abacus exploration](abacus/README.md). It follows the active
theme in the header, vault gate, onboarding and boot screen. **Blueprint is the fixed default**: the
Dock icon, and any theme without a colorway. The theme picker shows each colorway beside its name.

![One placket, four themes](abacus/preview/theme-colorways.png)

| Theme | Tile | Placket | Buttons | Accent button | Thread |
| --- | --- | --- | --- | --- | --- |
| Blueprint (default) | `#081120` | `#f4f7ff` | `#081120` | `#2f5bff` | `#c45a18` |
| Editorial | `#1b1712` | `#faf6ee` | `#1b1712` | `#a0472b` | `#c97c55` |
| Graphite | `#101216` | `#e9edf1` | `#15191e` | `#33628f` | `#7fb0e0` |
| Ledger | `#17453a` | `#f7f1e3` | `#1f4c42` | `#b26a4c` | `#5a9d8c` |

The geometry is fixed; the five colors are the colorway. Assets are in `src/assets/brand/`, generated
by [`scripts/build-brand-marks.mjs`](../../../scripts/build-brand-marks.mjs) and mapped in
[`src/brand/marks.ts`](../../../src/brand/marks.ts). The theming contract is
[3.8 Brand mark colorways](../theming.md#38-brand-mark-colorways).

## The maker

The personal brand is **Wai Yin Choi** at [wychoi.com](https://wychoi.com), marked by the abacus in
[`abacus/source/wychoi-abacus-256.png`](abacus/source/wychoi-abacus-256.png). It is a separate mark
from the app's placket and appears in the app's About panel, lock gate, and footer. The website is
opened by a Rust command that accepts no URL from the renderer.

## GitHub social preview

Upload [`hero/github-social-preview.png`](hero/github-social-preview.png) (1280 × 640, 436 KB) in
GitHub → Settings → Social preview. It is also the repository README hero. The
[SVG source](hero/github-social-preview.svg) is editable; run
`node docs/design/brand/hero/build-github-hero.mjs` to regenerate the 1× and 2× PNGs. Upload the 1×
PNG (GitHub's recommended size and under its 1 MB limit); the 2× file is for local/retina use.

## What changed

The previous Forest C button and the Knit Graph exploration are no longer installed. They were
chosen on 25 Oct 2026 and are preserved in git history. The repository README now uses the
[GitHub social hero](hero/github-social-preview.png) with the Blueprint placket; the old banner
was removed.

## Files

| Path | What |
| --- | --- |
| [`abacus/`](abacus) | The abacus exploration, the chosen Placket, the source mark, and the colorway boards |
| [`abacus/preview/theme-colorways.png`](abacus/preview/theme-colorways.png) | The four installed colorways |
| [`hero/github-social-preview.png`](hero/github-social-preview.png) | GitHub social preview / README hero (1280×640) |
| [`hero/github-social-preview.svg`](hero/github-social-preview.svg) | Editable vector source for the hero |
| [`hero/github-social-preview@2x.png`](hero/github-social-preview@2x.png) | 2560 × 1280 local/retina render |
| [`hero/build-github-hero.mjs`](hero/build-github-hero.mjs) | Rebuilds the hero SVG and PNGs |
| `../../../src/assets/brand/` | The shipped per-theme placket SVGs and the 1024 px icon source |
| `../../../src/assets/wychoi-abacus.svg` | The personal abacus mark used in the maker credit |

The old Forest C icons, heroes, previews, banner and build scripts were removed on 29 Oct 2026
because they described a superseded mark; recover them from git history if the exploration is
needed again.
