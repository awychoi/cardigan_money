import { themeVariables } from "./tokens";
import type { Theme, ThemeMode } from "./schema";

/**
 * Apply a theme by setting CSS custom properties on the document root.
 *
 * Data attributes switch component variants and density/motion in CSS:
 *   data-theme   light | dark        (existing attribute; drives color-scheme)
 *   data-skin    theme id
 *   data-nav     sidebar | topbar
 *   data-card-art faces | tiles
 *   data-density comfortable | compact
 *   data-motion  full | calm
 *   data-overview-composition ring | ranked-bars | hidden
 *   data-overview-trend bars | hidden
 */
export function applyTheme(theme: Theme, mode: ThemeMode): void {
  const root = document.documentElement;
  const vars = themeVariables(theme, mode);

  for (const [name, value] of Object.entries(vars)) {
    root.style.setProperty(name, value);
  }

  root.dataset.theme = mode;
  root.dataset.skin = theme.id;
  root.dataset.nav = theme.options.navigation;
  root.dataset.cardArt = theme.options.cardArt;
  root.dataset.density = theme.options.density;
  root.dataset.motion = theme.options.motion;
  root.dataset.overviewComposition = theme.options.overview.composition;
  root.dataset.overviewTrend = theme.options.overview.trend;
  root.style.colorScheme = mode;
}

/** Remove every theme property. Used by tests and by the locked screen. */
export function clearTheme(): void {
  const root = document.documentElement;
  for (const name of Array.from(root.style)) {
    if (name.startsWith("--")) root.style.removeProperty(name);
  }
  delete root.dataset.skin;
  delete root.dataset.nav;
  delete root.dataset.cardArt;
  delete root.dataset.density;
  delete root.dataset.motion;
  delete root.dataset.overviewComposition;
  delete root.dataset.overviewTrend;
  root.style.removeProperty("color-scheme");
}
