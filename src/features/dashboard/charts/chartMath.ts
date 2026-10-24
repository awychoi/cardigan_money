import type { Breakdown } from "../../../lib/bridge";

/**
 * Pure chart math. No DOM, no React, no formatting.
 *
 * Money stays in integer minor units; percentages are the only floats, and the
 * share helper uses largest-remainder rounding so the displayed shares always
 * sum to exactly 100.
 */

export interface CompositionSegment {
  key: string;
  label: string;
  grossMinor: number;
  /** Signed, matching Breakdown.refundsMinor (refunds are negative). */
  refundsMinor: number;
  netMinor: number;
  count: number;
  isOther: boolean;
}

export interface ChartDatum {
  key: string;
  label: string;
  valueMinor: number;
  /** Signed, matching Breakdown.refundsMinor. */
  refundsMinor: number;
  netMinor: number;
  count: number;
  share: number;
  color: string;
  isOther: boolean;
}

export interface RingSlice {
  share: number;
  offset: number;
}

export interface BarItem {
  heightPercent: number;
  negative: boolean;
}

export interface BarScale {
  /** Distance from the top of the plot to the zero line, 0–100. */
  zeroPercent: number;
  bars: BarItem[];
}

export const MAX_CHART_SEGMENTS = 6;

/**
 * Rank categories by gross spend, keep the top N, and fold the rest into one
 * neutral segment. Refunds never drive ranking because a proportional ring
 * cannot represent a negative share.
 */
export function buildComposition(categories: Breakdown[], topN: number): CompositionSegment[] {
  const limit = Math.max(1, Math.min(MAX_CHART_SEGMENTS, Math.trunc(topN)));
  const ranked = categories
    .filter((category) => category.grossMinor > 0)
    .sort((a, b) => b.grossMinor - a.grossMinor || a.key.localeCompare(b.key));

  const selected = ranked.slice(0, limit);
  const rest = ranked.slice(limit);
  const segments: CompositionSegment[] = selected.map((category) => ({
    key: category.key,
    label: category.key,
    grossMinor: category.grossMinor,
    refundsMinor: category.refundsMinor,
    netMinor: category.netMinor,
    count: category.count,
    isOther: false,
  }));

  if (rest.length > 0) {
    segments.push({
      key: "__other__",
      label: "Other",
      grossMinor: rest.reduce((sum, category) => sum + category.grossMinor, 0),
      refundsMinor: rest.reduce((sum, category) => sum + category.refundsMinor, 0),
      netMinor: rest.reduce((sum, category) => sum + category.netMinor, 0),
      count: rest.reduce((sum, category) => sum + category.count, 0),
      isOther: true,
    });
  }

  return segments;
}

/**
 * Largest-remainder percentages. The result sums to 100 for any positive
 * input; an all-zero input returns all zeros rather than NaN.
 */
export function percentShares(values: number[]): number[] {
  const positive = values.map((value) => Math.max(0, value));
  const total = positive.reduce((sum, value) => sum + value, 0);
  if (total <= 0) return values.map(() => 0);

  const exact = positive.map((value) => (value / total) * 100);
  const shares = exact.map((value) => Math.floor(value));
  const remaining = 100 - shares.reduce((sum, value) => sum + value, 0);
  const order = exact
    .map((value, index) => ({ index, fraction: value - Math.floor(value) }))
    .sort((a, b) => b.fraction - a.fraction || a.index - b.index);

  for (let index = 0; index < remaining; index += 1) {
    shares[order[index % order.length].index] += 1;
  }
  return shares;
}

/** Cumulative offsets for a pathLength=100 donut. */
export function ringSlices(shares: number[]): RingSlice[] {
  let offset = 0;
  return shares.map((share) => {
    const slice = { share, offset };
    offset += share;
    return slice;
  });
}

/**
 * Scale signed values onto a 0–100 plot with a shared zero baseline. A bar's
 * height is its absolute share of the largest span; the caller positions
 * positive bars below the baseline and negative bars above it.
 */
export function barScale(values: number[]): BarScale {
  const maxPositive = Math.max(0, ...values);
  const minNegative = Math.min(0, ...values);
  const span = maxPositive - minNegative;
  if (span <= 0) {
    return { zeroPercent: 100, bars: values.map(() => ({ heightPercent: 0, negative: false })) };
  }
  return {
    zeroPercent: (maxPositive / span) * 100,
    bars: values.map((value) => ({
      heightPercent: (Math.abs(value) / span) * 100,
      negative: value < 0,
    })),
  };
}

/** Attach shares and theme colors to composition segments. */
export function toChartData(segments: CompositionSegment[]): ChartDatum[] {
  const shares = percentShares(segments.map((segment) => segment.grossMinor));
  return segments.map((segment, index) => ({
    key: segment.key,
    label: segment.label,
    valueMinor: segment.grossMinor,
    refundsMinor: segment.refundsMinor,
    netMinor: segment.netMinor,
    count: segment.count,
    share: shares[index] ?? 0,
    color: segment.isOther ? "var(--muted)" : `var(--chart-${index + 1})`,
    isOther: segment.isOther,
  }));
}
