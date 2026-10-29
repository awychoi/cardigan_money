import blueprint from "../assets/brand/placket-blueprint.svg";
import editorial from "../assets/brand/placket-editorial.svg";
import graphite from "../assets/brand/placket-graphite.svg";
import ledger from "../assets/brand/placket-ledger.svg";

/**
 * One placket colorway per bundled theme.
 *
 * The keys mirror `src/themes/<id>.theme.json`; `src/brand/marks.test.ts` fails
 * if a theme ships without a colorway. The color values live in
 * `scripts/build-brand-marks.mjs`, which regenerates the SVGs.
 */
export const BRAND_MARKS = {
  blueprint,
  editorial,
  graphite,
  ledger,
} as const;

export type BrandThemeId = keyof typeof BRAND_MARKS;

/**
 * The fixed fallback. Blueprint is the Dock icon and the mark any theme
 * without a colorway gets instead of no mark at all.
 */
export const DEFAULT_BRAND_THEME: BrandThemeId = "blueprint";

export function isBrandThemeId(id: string | null | undefined): id is BrandThemeId {
  return typeof id === "string" && Object.hasOwn(BRAND_MARKS, id);
}

/** Resolve a theme id to the colorway actually shown, falling back to Blueprint. */
export function brandThemeFor(themeId: string | null | undefined): BrandThemeId {
  return isBrandThemeId(themeId) ? themeId : DEFAULT_BRAND_THEME;
}

/** Resolve a theme id to its placket, falling back to Blueprint. */
export function brandMarkFor(themeId: string | null | undefined): string {
  return BRAND_MARKS[brandThemeFor(themeId)];
}
