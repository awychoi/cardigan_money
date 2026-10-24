/**
 * Small color helpers used by the theme tests and any UI that needs to reason
 * about contrast. Mirrors CSS `color-mix(in srgb, …)` so tests assert the same
 * value the browser renders.
 */

export type Rgb = [number, number, number];

export function parseHex(hex: string): Rgb {
  const h = hex.replace("#", "");
  return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
}

function channel(value: number): number {
  const c = value / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

export function luminance(hex: string): number {
  const [r, g, b] = parseHex(hex);
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

export function contrastRatio(a: string, b: string): number {
  const la = luminance(a);
  const lb = luminance(b);
  const hi = Math.max(la, lb);
  const lo = Math.min(la, lb);
  return (hi + 0.05) / (lo + 0.05);
}

/** Same math as `color-mix(in srgb, a <weightA>%, b)`. */
export function mixSrgb(a: string, b: string, weightA: number): string {
  const ca = parseHex(a);
  const cb = parseHex(b);
  const w = Math.min(1, Math.max(0, weightA));
  const out = ca.map((value, index) => Math.round(value * w + cb[index] * (1 - w)));
  return `#${out.map((value) => value.toString(16).padStart(2, "0")).join("")}`;
}

/** The app derives these from the base palette; keep the percentages in one place. */
export const MIX = {
  accentSoft: 0.13,
  accentLine: 0.34,
  warningSoft: 0.12,
  warningLine: 0.36,
  dangerSoft: 0.12,
  dangerLine: 0.38,
} as const;
