import { themeSchema, type Theme } from "./schema";
import type { ThemeMode } from "./schema";

export { DEFAULT_THEME_ID } from "./schema";

const modules = import.meta.glob("../themes/*.theme.json", {
  eager: true,
  import: "default",
});

function describeIssues(error: { issues: { path: PropertyKey[]; message: string }[] }): string {
  return error.issues
    .map((issue) => `${issue.path.map(String).join(".") || "(root)"}: ${issue.message}`)
    .join("\n  ");
}

function loadThemes(): Theme[] {
  const themes: Theme[] = [];
  for (const [path, raw] of Object.entries(modules)) {
    const fileName = path.split("/").pop()?.replace(/\.theme\.json$/, "") ?? path;
    const parsed = themeSchema.safeParse(raw);
    if (!parsed.success) {
      // A bundled theme that fails validation is a build-time bug, not a user
      // error. Never let it take the app down: skip it and make the test suite
      // shout about it (src/theme/schema.test.ts asserts this list is complete).
      console.error(`[theme] ${path} is invalid:\n  ${describeIssues(parsed.error)}`);
      continue;
    }
    if (parsed.data.id !== fileName) {
      console.error(
        `[theme] ${path}: id "${parsed.data.id}" must match the file name "${fileName}"`,
      );
      continue;
    }
    themes.push(parsed.data);
  }
  themes.sort((a, b) => a.name.localeCompare(b.name));
  return themes;
}

/** Every bundled theme, sorted by display name. */
export const THEMES: Theme[] = loadThemes();

if (THEMES.length === 0) {
  throw new Error("No valid themes were found in src/themes/*.theme.json");
}

export const DEFAULT_THEME = THEMES.find((theme) => theme.id === "ledger") ?? THEMES[0];

const warned = new Set<string>();

/** Unknown or missing ids fall back to the default theme rather than failing. */
export function getTheme(id: string | null | undefined): Theme {
  const found = THEMES.find((theme) => theme.id === id);
  if (found) return found;
  if (id && !warned.has(id)) {
    warned.add(id);
    console.warn(`[theme] unknown theme id "${id}" — using ${DEFAULT_THEME.id}`);
  }
  return DEFAULT_THEME;
}

/** Resolve the stored preference against the OS appearance. */
export function resolveMode(
  preference: "light" | "dark" | "system",
  prefersDark: boolean,
): ThemeMode {
  if (preference === "system") return prefersDark ? "dark" : "light";
  return preference;
}

export const THEME_IDS = THEMES.map((theme) => theme.id);
