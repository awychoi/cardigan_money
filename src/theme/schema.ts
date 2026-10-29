import { z } from "zod";

/**
 * The theme contract.
 *
 * Every key here is implemented: it maps onto a CSS custom property or a
 * component variant that the app actually reads. There are no reserved keys.
 * Run `pnpm test` (or `pnpm theme:check`) to validate theme files and their
 * contrast; run `pnpm theme:schema` to regenerate the JSON Schema.
 *
 * Authoring guide: docs/design/theming.md
 */

const hex = z
  .string()
  .regex(/^#[0-9a-f]{6}$/i, "use #rrggbb")
  .describe("Hex color, #rrggbb");

/** Colors that may carry alpha, used for shadows. */
const alphaColor = z
  .string()
  .max(60)
  // Charset is bounded: no letters beyond "rgb/rgba", no semicolons, no url().
  .regex(/^(#[0-9a-f]{6}|rgba?\([\d\s.,%/]+\))$/i, "use #rrggbb, rgb(r g b / a), or rgba(...)")
  .describe("CSS color, alpha allowed (shadows)");

/** A font stack. Restricted charset: no semicolons, braces, or url(). */
const fontStack = z
  .string()
  .min(1)
  .max(180)
  .regex(/^[A-Za-z0-9 ,"'-]+$/, "plain CSS font family stack only")
  .describe("CSS font-family stack, comma separated");

const slug = z
  .string()
  .regex(/^[a-z0-9][a-z0-9-]{0,31}$/, "lowercase slug: a-z, 0-9, dashes")
  .describe("Lowercase slug, also used as a CSS custom-property segment");

const palette = z
  .strictObject({
    bg: hex.describe("App canvas"),
    sidebar: hex.describe("Navigation ground (sidebar)"),
    surface: hex.describe("Cards, toolbars, panels"),
    surface2: hex.describe("Recessed fills: inputs, tracks, hover"),
    surface3: hex.describe("Stronger recessed fill"),
    text: hex.describe("Primary text"),
    text2: hex.describe("Secondary text"),
    muted: hex.describe("Tertiary text and labels · must be ≥ 4.5:1 on surface"),
    border: hex.describe("Hairlines"),
    borderStrong: hex.describe("Visible dividers"),
    borderControl: hex.describe("Input boundaries · must be ≥ 3:1 on surface"),
    accent: hex.describe("Primary action, selection, active navigation"),
    accentInk: hex.describe("Accent-colored text and links on surfaces · ≥ 4.5:1"),
    success: hex.describe("Confirmed state: fresh, healthy, enforced · ≥ 4.5:1 on surface"),
    onAccent: hex.describe("Text on an accent fill · ≥ 4.5:1 against accent"),
    warning: hex.describe("Attention, stale, reconnect"),
    danger: hex.describe("Failure and destructive"),
    focus: hex.describe("Focus ring · ≥ 3:1 on bg"),
    chart: z.array(hex).min(3).max(6).describe("Ordered series colors for bars and charts (3–6)"),
    shadow: alphaColor.describe("Soft elevation color"),
    shadowStrong: alphaColor.describe("Card-face / overlay shadow color"),
  })
  .describe("One mode of a theme: every semantic color, no derived values");

const type = z
  .strictObject({
    sans: fontStack.describe("UI and body font"),
    mono: fontStack.describe("Masks, dates, identifiers"),
    display: fontStack.describe("Headings and hero numbers"),
    scale: z.number().min(0.85).max(1.2).describe("Multiplier applied to structural type sizes"),
    displayWeight: z.number().int().min(300).max(800).describe("Heading / hero weight"),
    displayTracking: z
      .number()
      .min(-0.06)
      .max(0.02)
      .describe("em tracking for headings and hero numbers"),
    lineHeight: z.number().min(1.2).max(1.8).describe("Body line height"),
  })
  .describe("Typography");

const shape = z
  .strictObject({
    controlRadius: z.number().min(0).max(24).describe("Buttons, inputs, chips"),
    panelRadius: z.number().min(0).max(28).describe("Panels and tables"),
    cardRadius: z.number().min(0).max(28).describe("Card faces and hero cards"),
    borderWidth: z.number().min(0).max(2).describe("Hairline thickness"),
  })
  .describe("Radii and hairline weight");

const spacing = z
  .strictObject({
    screen: z.number().min(12).max(48).describe("Content padding"),
    card: z.number().min(10).max(32).describe("Panel padding"),
    gap: z.number().min(6).max(24).describe("Grid gap"),
  })
  .describe("Spacing");

const motion = z
  .strictObject({
    fast: z.number().int().min(0).max(1000).describe("Hover / press duration (ms)"),
    base: z.number().int().min(0).max(1000).describe("Panel / layout duration (ms)"),
    easing: z
      .string()
      .max(60)
      .regex(/^[a-z0-9(),.\s-]+$/, "a CSS easing or cubic-bezier()")
      .describe("CSS timing function"),
  })
  .describe('Motion; `options.motion: "calm"` disables it regardless');

const chartOptions = z
  .strictObject({
    composition: z
      .enum(["ring", "ranked-bars", "hidden"])
      .describe(
        "ring = proportional donut; ranked-bars = horizontal bars; hidden = no composition",
      ),
    trend: z.enum(["bars", "hidden"]).describe("bars = monthly column trend; hidden = no trend"),
    topCategories: z
      .number()
      .int()
      .min(3)
      .max(6)
      .describe("Category segments before the neutral Other segment (3–6)"),
    trendMonths: z
      .number()
      .int()
      .min(6)
      .max(12)
      .describe("Trailing months in the trend window (6–12)"),
  })
  .describe("Overview chart variants. Every option is implemented");

const options = z
  .strictObject({
    cardArt: z
      .enum(["faces", "tiles"])
      .describe("faces = physical card art in the Cards view; tiles = data tiles"),
    density: z
      .enum(["comfortable", "compact"])
      .describe("comfortable = airy rows; compact = denser tables and panels"),
    motion: z.enum(["full", "calm"]).describe("calm turns transitions off"),
    overview: chartOptions,
  })
  .describe("Component variants. Each one is implemented in the app");

const issuerArt = z
  .strictObject({
    from: hex.describe("Gradient start"),
    to: hex.describe("Gradient end"),
    ink: hex.describe("Card face text · ≥ 4.5:1 against from and to"),
  })
  .describe("Card-face colors for one issuer");

const theme = z
  .strictObject({
    $schema: z.string().optional().describe("Optional editor hint"),
    id: slug.describe("Must match the file name: <id>.theme.json"),
    name: z.string().min(1).max(40).describe("Shown in Settings → Appearance"),
    description: z.string().max(240).optional().describe("One line for the picker"),
    options,
    type,
    shape,
    spacing,
    motion,
    modes: z
      .strictObject({
        light: palette,
        dark: palette,
      })
      .describe("Both modes are required so a theme never renders blank"),
    issuers: z
      .record(slug, issuerArt)
      .optional()
      .describe(
        'Optional card art by issuer slug, e.g. "american-express"; "default" is the fallback',
      ),
  })
  .strict();

export const themeSchema = theme;
export type Theme = z.infer<typeof theme>;
export type ThemePalette = z.infer<typeof palette>;
export type ThemeOptions = z.infer<typeof options>;
export type ThemeMode = keyof Theme["modes"];
export type IssuerArt = z.infer<typeof issuerArt>;

export const THEME_MODE_NAMES: ThemeMode[] = ["light", "dark"];

export const DEFAULT_THEME_ID = "ledger";
