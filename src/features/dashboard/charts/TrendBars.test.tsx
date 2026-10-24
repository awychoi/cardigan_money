import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { MonthTotal } from "../../../lib/bridge";
import { TrendBars } from "./TrendBars";

function month(
  key: string,
  netMinor: number,
  complete = true,
  grossMinor = Math.max(netMinor, 0),
): MonthTotal {
  return {
    month: key,
    grossMinor,
    refundsMinor: 0,
    netMinor,
    count: 1,
    complete,
  };
}

describe("TrendBars", () => {
  it("renders a named trend, the current-month marker, and exact table", () => {
    const { container } = render(
      <TrendBars
        months={[
          month("2026-08", 10_000),
          month("2026-09", 20_000),
          month("2026-10", 5_000, false),
        ]}
        selectedMonth="2026-10"
        currentMonth="2026-10"
      />,
    );

    const chart = screen.getByRole("img", { name: /monthly net spending/i });
    expect(chart.getAttribute("aria-label")).toContain("Oct $50.00");
    expect(screen.getByText("so far")).toBeInTheDocument();
    expect(container.querySelector(".trend-col-selected")).toBeInTheDocument();
    expect(container.querySelector(".trend-bar-current")).toBeInTheDocument();

    const table = screen.getByRole("table", {
      name: /monthly gross, refunds, net, and rows/i,
    });
    expect(table).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "So far" })).toBeInTheDocument();
    expect(screen.getAllByRole("cell", { name: "Complete" })).toHaveLength(2);
  });

  it("positions negative months below the shared zero baseline", () => {
    const { container } = render(
      <TrendBars
        months={[month("2026-09", 10_000), month("2026-10", -10_000, false)]}
        selectedMonth="2026-10"
        currentMonth="2026-10"
      />,
    );
    const bars = container.querySelectorAll<HTMLElement>(".trend-bar");
    expect(bars).toHaveLength(2);
    expect(bars[0].style.bottom).toBe("50%");
    expect(bars[1].style.top).toBe("50%");
    expect(bars[1].style.bottom).toBe("");
  });

  it("renders a zero bar at the baseline instead of faking a height", () => {
    const { container } = render(
      <TrendBars
        months={[month("2026-09", 0), month("2026-10", 10_000, false)]}
        selectedMonth="2026-10"
        currentMonth="2026-10"
      />,
    );
    expect(container.querySelectorAll(".trend-bar-zero")).toHaveLength(1);
  });

  it("marks future months and handles an empty window", () => {
    const { container, rerender } = render(
      <TrendBars
        months={[month("2026-10", 1_000, false), month("2026-11", 0, false)]}
        selectedMonth="2026-11"
        currentMonth="2026-10"
      />,
    );
    expect(container.querySelectorAll(".trend-col-future")).toHaveLength(1);

    rerender(<TrendBars months={[]} selectedMonth="2026-10" currentMonth="2026-10" />);
    expect(screen.getByText("No trend data yet.")).toBeInTheDocument();
    expect(screen.getByRole("table")).toBeInTheDocument();
  });
});
