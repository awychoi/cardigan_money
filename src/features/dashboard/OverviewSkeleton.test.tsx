import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { OverviewSkeleton } from "./OverviewSkeleton";

describe("OverviewSkeleton", () => {
  it("exposes a status label and stable skeleton blocks", () => {
    const { container } = render(<OverviewSkeleton />);
    expect(
      screen.getByRole("status", { name: "Loading the encrypted ledger" }),
    ).toBeInTheDocument();
    expect(container.querySelectorAll(".skeleton-panel")).toHaveLength(6);
    expect(container.querySelectorAll(".skeleton-block").length).toBeGreaterThan(10);
    expect(screen.queryByRole("heading")).not.toBeInTheDocument();
  });
});
