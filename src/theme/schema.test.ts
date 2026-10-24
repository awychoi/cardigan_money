import { readFileSync, writeFileSync } from "node:fs";
import { z } from "zod";
import { MIX, contrastRatio, mixSrgb } from "./color";
import { THEMES } from "./index";
import { themeSchema, type Theme, type ThemeMode } from "./schema";
import { themeVariables } from "./tokens";

const SCHEMA_FILE = "src/themes/theme.schema.json";

function ratio(theme: Theme, mode: ThemeMode, foreground: string, background: string): number {
  const palette = theme.modes[mode];
  const resolve = (value: string) => {
    if (value === "surface") return palette.surface;
    if (value === "bg") return palette.bg;
    if (value === "accent") return palette.accent;
    if (value === "accentSoft") return mixSrgb(palette.accent, palette.surface, MIX.accentSoft);
    if (value === "successSoft") return mixSrgb(palette.success, palette.surface, MIX.accentSoft);
    if (value === "warningSoft") return mixSrgb(palette.warning, palette.surface, MIX.warningSoft);
    return value;
  };
  return contrastRatio(resolve(foreground), resolve(background));
}

describe("theme files", () => {
  it("loads every bundled theme file", () => {
    // Count files rather than hard-coding ids, so adding a theme needs no test
    // edit. A file that fails validation is skipped by the registry and shows
    // up here as a count mismatch.
    const files = Object.keys(import.meta.glob("../themes/*.theme.json", { eager: true })).length;
    expect(files).toBeGreaterThan(0);
    expect(THEMES).toHaveLength(files);
    expect(new Set(THEMES.map((theme) => theme.id)).size).toBe(files);
    expect(THEMES.some((theme) => theme.id === "ledger")).toBe(true);
  });

  it("has a stable id for every file", () => {
    for (const theme of THEMES) {
      expect(theme.id).toMatch(/^[a-z0-9][a-z0-9-]*$/);
      expect(theme.name.length).toBeGreaterThan(0);
    }
  });

  it("keeps every text pair readable in both modes", () => {
    const failures: string[] = [];
    for (const theme of THEMES) {
      for (const mode of ["light", "dark"] as const) {
        const p = theme.modes[mode];
        const checks: [string, number, string, string][] = [
          ["text on surface", 7, p.text, "surface"],
          ["text2 on surface", 4.5, p.text2, "surface"],
          ["muted on surface", 4.5, p.muted, "surface"],
          ["muted on bg", 4.5, p.muted, "bg"],
          ["accentInk on surface", 4.5, p.accentInk, "surface"],
          ["accentInk on accent wash", 4.5, p.accentInk, "accentSoft"],
          ["onAccent on accent", 4.5, p.onAccent, "accent"],
          ["success on surface", 4.5, p.success, "surface"],
          ["success on success wash", 4.5, p.success, "successSoft"],
          ["warning on warning wash", 4.5, p.warning, "warningSoft"],
          ["danger on surface", 4.5, p.danger, "surface"],
          ["accent on bg", 3, p.accent, "bg"],
          ["focus on bg", 3, p.focus, "bg"],
          ["borderControl on surface", 3, p.borderControl, "surface"],
        ];
        for (const [label, minimum, fg, bg] of checks) {
          const value = ratio(theme, mode, fg, bg);
          if (value < minimum) {
            failures.push(`${theme.id}/${mode}: ${label} = ${value.toFixed(2)} (needs ${minimum})`);
          }
        }

        for (const [slug, art] of Object.entries(theme.issuers ?? {})) {
          const from = contrastRatio(art.ink, art.from);
          const to = contrastRatio(art.ink, art.to);
          if (from < 4.5)
            failures.push(`${theme.id}/${mode}: issuer ${slug} ink/from = ${from.toFixed(2)}`);
          if (to < 4.5)
            failures.push(`${theme.id}/${mode}: issuer ${slug} ink/to = ${to.toFixed(2)}`);
        }
      }
    }
    expect(failures).toEqual([]);
  });

  it("compiles to CSS custom properties", () => {
    for (const theme of THEMES) {
      for (const mode of ["light", "dark"] as const) {
        const vars = themeVariables(theme, mode);
        for (const [name, value] of Object.entries(vars)) {
          expect(name.startsWith("--"), `${name} must be a custom property`).toBe(true);
          expect(value.length).toBeGreaterThan(0);
        }
        expect(vars["--bg"]).toBe(theme.modes[mode].bg);
        expect(vars["--accent"]).toBe(theme.modes[mode].accent);
        expect(vars["--font-sans"]).toBe(theme.type.sans);
        expect(vars["--radius-panel"]).toBe(`${theme.shape.panelRadius}px`);
        expect(vars["--motion-fast"]).toBe(`${theme.motion.fast}ms`);
        expect(vars["--chart-1"]).toBe(theme.modes[mode].chart[0]);
        expect(vars["--chart-6"]).toBe(theme.modes[mode].chart[5]);
      }
    }
  });

  it("keeps the committed JSON Schema in sync with the Zod contract", () => {
    const generated = {
      $schema: "https://json-schema.org/draft/2020-12/schema",
      title: "cardigan_money theme",
      description:
        "Theme document for cardigan_money. See docs/design/theming.md. Regenerate with pnpm theme:schema.",
      ...z.toJSONSchema(themeSchema, { target: "draft-2020-12", io: "input" }),
    };
    const text = `${JSON.stringify(generated, null, 2)}\n`;

    if (process.env.UPDATE_THEME_SCHEMA === "1") {
      writeFileSync(SCHEMA_FILE, text);
      return;
    }

    const committed = readFileSync(SCHEMA_FILE, "utf8");
    expect(committed, "run pnpm theme:schema").toBe(text);
  });
});
