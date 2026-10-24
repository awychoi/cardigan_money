import { describe, expect, it } from "vitest";
import type { Breakdown } from "../../../lib/bridge";
import { barScale, buildComposition, percentShares, ringSlices, toChartData } from "./chartMath";

function category(key: string, grossMinor: number, refundsMinor = 0, count = 1): Breakdown {
  return {
    key,
    grossMinor,
    refundsMinor,
    netMinor: grossMinor + refundsMinor,
    count,
  };
}

describe("percentShares", () => {
  it("rounds with largest remainder so shares sum to 100", () => {
    const shares = percentShares([1, 1, 1]);
    expect(shares).toEqual([34, 33, 33]);
    expect(shares.reduce((sum, value) => sum + value, 0)).toBe(100);

    const uneven = percentShares([2, 1]);
    expect(uneven).toEqual([67, 33]);
    expect(uneven.reduce((sum, value) => sum + value, 0)).toBe(100);
  });

  it("handles a single segment and an all-zero total", () => {
    expect(percentShares([42])).toEqual([100]);
    expect(percentShares([0, 0])).toEqual([0, 0]);
    expect(percentShares([])).toEqual([]);
    expect(percentShares([0, 5])).toEqual([0, 100]);
  });

  it("keeps large arrays at exactly 100", () => {
    const shares = percentShares([7, 11, 13, 17, 19, 23]);
    expect(shares.reduce((sum, value) => sum + value, 0)).toBe(100);
  });
});

describe("buildComposition", () => {
  it("ranks by gross, keeps N, and folds the rest into Other", () => {
    const segments = buildComposition(
      [
        category("Dining", 1_000, -100, 2),
        category("Travel", 4_000, 0, 3),
        category("Groceries", 3_000, -50, 4),
        category("Shopping", 2_000, 0, 1),
        category("Fees", 500, 0, 1),
      ],
      3,
    );

    expect(segments.map((segment) => segment.key)).toEqual([
      "Travel",
      "Groceries",
      "Shopping",
      "__other__",
    ]);
    expect(segments[3]).toMatchObject({
      label: "Other",
      grossMinor: 1_500,
      refundsMinor: -100,
      netMinor: 1_400,
      count: 3,
      isOther: true,
    });
  });

  it("omits Other when N covers every category", () => {
    const segments = buildComposition([category("Dining", 100), category("Travel", 200)], 4);
    expect(segments.map((segment) => segment.key)).toEqual(["Travel", "Dining"]);
  });

  it("ignores zero-gross categories and clamps N to six", () => {
    const zero = buildComposition([category("Empty", 0), category("Refunds", 0)], 3);
    expect(zero).toEqual([]);

    const many = buildComposition(
      Array.from({ length: 8 }, (_, index) => category(`C${index}`, 8 - index)),
      99,
    );
    expect(many).toHaveLength(7);
    expect(many[6].key).toBe("__other__");
  });
});

describe("ringSlices", () => {
  it("accumulates offsets around the ring", () => {
    expect(ringSlices([50, 30, 20])).toEqual([
      { share: 50, offset: 0 },
      { share: 30, offset: 50 },
      { share: 20, offset: 80 },
    ]);
  });

  it("handles a full single slice and all-zero shares", () => {
    expect(ringSlices([100])).toEqual([{ share: 100, offset: 0 }]);
    expect(ringSlices([0, 0])).toEqual([
      { share: 0, offset: 0 },
      { share: 0, offset: 0 },
    ]);
  });
});

describe("barScale", () => {
  it("uses the bottom as the baseline for positive months", () => {
    const scale = barScale([10, 20, 0]);
    expect(scale.zeroPercent).toBe(100);
    expect(scale.bars.map((bar) => bar.heightPercent)).toEqual([50, 100, 0]);
    expect(scale.bars.every((bar) => !bar.negative)).toBe(true);
  });

  it("supports a zero baseline with negative months", () => {
    const scale = barScale([10, -10, 0]);
    expect(scale.zeroPercent).toBe(50);
    expect(scale.bars.map((bar) => bar.heightPercent)).toEqual([50, 50, 0]);
    expect(scale.bars.map((bar) => bar.negative)).toEqual([false, true, false]);
  });

  it("handles all-negative and all-zero windows", () => {
    const negative = barScale([-5, -10]);
    expect(negative.zeroPercent).toBe(0);
    expect(negative.bars.map((bar) => bar.heightPercent)).toEqual([50, 100]);
    expect(negative.bars.every((bar) => bar.negative)).toBe(true);

    expect(barScale([0, 0])).toMatchObject({ zeroPercent: 100 });
    expect(barScale([]).bars).toEqual([]);
  });
});

describe("toChartData", () => {
  it("assigns shares, palette colors, and a neutral Other", () => {
    const data = toChartData(
      buildComposition(
        [
          category("Dining", 50),
          category("Travel", 30),
          category("Shopping", 20),
          category("Fees", 10),
        ],
        3,
      ),
    );
    expect(data.map((item) => item.label)).toEqual(["Dining", "Travel", "Shopping", "Other"]);
    expect(data.map((item) => item.share)).toEqual([46, 27, 18, 9]);
    expect(data.map((item) => item.color)).toEqual([
      "var(--chart-1)",
      "var(--chart-2)",
      "var(--chart-3)",
      "var(--muted)",
    ]);
    expect(data.reduce((sum, item) => sum + item.share, 0)).toBe(100);
  });
});
