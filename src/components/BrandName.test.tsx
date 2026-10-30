import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { BrandName } from "./BrandName";

describe("BrandName", () => {
  it("renders the name with a separable accent underscore", () => {
    const { container } = render(<BrandName />);
    expect(screen.getByLabelText("cardigan_money")).toBeInTheDocument();
    expect(container.querySelector(".brand-name")).toHaveTextContent("cardigan_money");
    expect(container.querySelector(".brand-name-separator")).toHaveTextContent("_");
  });

  it("accepts a class for title-sized contexts", () => {
    const { container } = render(<BrandName className="brand-name brand-title" />);
    expect(container.firstElementChild).toHaveClass("brand-name", "brand-title");
  });
});
