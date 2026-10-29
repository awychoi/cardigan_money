import { describe, expect, it } from "vitest";
import { THEME_IDS } from "../theme";
import {
  BRAND_MARKS,
  DEFAULT_BRAND_THEME,
  brandMarkFor,
  brandThemeFor,
  isBrandThemeId,
  type BrandThemeId,
} from "./marks";

describe("brand marks", () => {
  it("has one colorway per bundled theme", () => {
    // Adding a theme without a colorway fails here, not in a dark header.
    expect(Object.keys(BRAND_MARKS).sort()).toEqual([...THEME_IDS].sort());
    expect(new Set(Object.values(BRAND_MARKS)).size).toBe(THEME_IDS.length);
  });

  it("resolves every theme to its own colorway", () => {
    for (const id of THEME_IDS) {
      expect(isBrandThemeId(id)).toBe(true);
      expect(brandMarkFor(id)).toBe(BRAND_MARKS[id as BrandThemeId]);
    }
  });

  it("falls back to Blueprint for a missing or unknown theme", () => {
    expect(DEFAULT_BRAND_THEME).toBe("blueprint");
    expect(brandMarkFor(undefined)).toBe(BRAND_MARKS.blueprint);
    expect(brandMarkFor(null)).toBe(BRAND_MARKS.blueprint);
    expect(brandMarkFor("")).toBe(BRAND_MARKS.blueprint);
    expect(brandMarkFor("sandstorm")).toBe(BRAND_MARKS.blueprint);
    // Not inherited properties from Object.prototype.
    expect(brandMarkFor("constructor")).toBe(BRAND_MARKS.blueprint);
    expect(brandThemeFor("sandstorm")).toBe("blueprint");
    expect(brandThemeFor("ledger")).toBe("ledger");
  });
});
