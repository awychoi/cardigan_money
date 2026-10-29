import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { BRAND_MARKS } from "../brand/marks";
import { BrandMark } from "./BrandMark";

function markSource(themeId?: string): string | null {
  const { container } = render(<BrandMark themeId={themeId} />);
  return container.querySelector("img")?.getAttribute("src") ?? null;
}

describe("BrandMark", () => {
  it("uses the active theme's colorway", () => {
    expect(markSource("blueprint")).toBe(BRAND_MARKS.blueprint);
    expect(markSource("editorial")).toBe(BRAND_MARKS.editorial);
    expect(markSource("graphite")).toBe(BRAND_MARKS.graphite);
    expect(markSource("ledger")).toBe(BRAND_MARKS.ledger);
  });

  it("falls back to Blueprint when the theme is missing or unknown", () => {
    expect(markSource()).toBe(BRAND_MARKS.blueprint);
    expect(markSource("sandstorm")).toBe(BRAND_MARKS.blueprint);
    const { container } = render(<BrandMark themeId="sandstorm" />);
    expect(container.firstElementChild).toHaveAttribute("data-brand-theme", "blueprint");
  });

  it("stays decorative for assistive technology", () => {
    const { container } = render(<BrandMark themeId="ledger" className="boot-mark" />);
    const wrapper = container.firstElementChild;
    expect(wrapper).toHaveClass("boot-mark");
    expect(wrapper).toHaveAttribute("aria-hidden", "true");
    expect(wrapper).toHaveAttribute("data-brand-theme", "ledger");
    expect(container.querySelector("img")).toHaveAttribute("alt", "");
  });
});
