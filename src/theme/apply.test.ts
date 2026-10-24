import { afterEach, describe, expect, it } from "vitest";
import { THEMES } from "./index";
import { applyTheme, clearTheme } from "./apply";

afterEach(() => clearTheme());

describe("applyTheme", () => {
  it("maps the overview variants onto data attributes", () => {
    const theme = THEMES[0];
    applyTheme(theme, "dark");

    expect(document.documentElement.dataset.overviewComposition).toBe(
      theme.options.overview.composition,
    );
    expect(document.documentElement.dataset.overviewTrend).toBe(theme.options.overview.trend);
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.style.getPropertyValue("--chart-6")).toBe(
      theme.modes.dark.chart[5],
    );
  });

  it("clears the overview attributes with the rest of the theme", () => {
    applyTheme(THEMES[0], "light");
    clearTheme();

    expect(document.documentElement.dataset.overviewComposition).toBeUndefined();
    expect(document.documentElement.dataset.overviewTrend).toBeUndefined();
    expect(document.documentElement.style.getPropertyValue("--chart-6")).toBe("");
  });
});
