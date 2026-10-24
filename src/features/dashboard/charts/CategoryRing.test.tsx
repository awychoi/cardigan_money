import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Breakdown } from "../../../lib/bridge";
import { CategoryRing } from "./CategoryRing";
import { buildComposition, toChartData } from "./chartMath";

function category(key: string, grossMinor: number, refundsMinor = 0): Breakdown {
  return {
    key,
    grossMinor,
    refundsMinor,
    netMinor: grossMinor + refundsMinor,
    count: 1,
  };
}

function dataFor(categories: Breakdown[], topN = 6) {
  return toChartData(buildComposition(categories, topN));
}

describe("CategoryRing", () => {
  it("renders an accessible named ring with a legend and exact table", () => {
    const data = dataFor([category("Dining", 60_000), category("Travel", 40_000)]);
    const { container } = render(
      <CategoryRing data={data} totalMinor={100_000} refundsMinor={-2_500} netMinor={97_500} />,
    );

    const chart = screen.getByRole("img", { name: /gross spending by category/i });
    expect(chart).toBeInTheDocument();
    expect(chart.getAttribute("aria-label")).toContain("Dining $600.00, 60 percent");
    expect(container.querySelectorAll(".ring-slice")).toHaveLength(2);
    expect(screen.getByText("$1,000.00")).toBeInTheDocument();
    expect(screen.getAllByText("$600.00").length).toBeGreaterThan(0);
    expect(screen.getAllByText("60%").length).toBeGreaterThan(0);
    expect(screen.getByText("$25.00 refunds → $975.00 net")).toBeInTheDocument();

    const table = screen.getByRole("table", {
      name: /category spending, gross, refunds, and net/i,
    });
    expect(table).toBeInTheDocument();
    expect(screen.getByRole("rowheader", { name: "Dining" })).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "60%" })).toBeInTheDocument();
  });

  it("renders one full segment without a divide-by-zero", () => {
    const data = dataFor([category("Dining", 42_000)]);
    const { container } = render(
      <CategoryRing data={data} totalMinor={42_000} refundsMinor={0} netMinor={42_000} />,
    );
    expect(container.querySelectorAll(".ring-slice")).toHaveLength(1);
    expect(screen.getAllByText("100%").length).toBeGreaterThan(0);
    expect(screen.getByRole("img").getAttribute("aria-label")).toContain("100 percent");
  });

  it("renders the empty state when there is no posted spending", () => {
    render(<CategoryRing data={[]} totalMinor={0} refundsMinor={0} netMinor={0} />);
    expect(
      screen.getByRole("img", { name: "No posted spending in this month." }),
    ).toBeInTheDocument();
    expect(screen.getByText("No posted spending this month.")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  it("folds the remainder into a neutral Other segment", () => {
    const data = dataFor(
      [
        category("Dining", 50_000),
        category("Travel", 30_000),
        category("Shopping", 20_000),
        category("Fees", 10_000),
      ],
      3,
    );
    const { container } = render(
      <CategoryRing data={data} totalMinor={110_000} refundsMinor={0} netMinor={110_000} />,
    );
    expect(container.querySelectorAll(".ring-slice")).toHaveLength(4);
    expect(screen.getAllByText("Other").length).toBeGreaterThan(0);
    expect(screen.getAllByText("9%").length).toBeGreaterThan(0);
  });
});
