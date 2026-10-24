import type { LedgerRow } from "../../lib/bridge";

const SPEND_TYPES = new Set(["purchase", "refund", "fee", "interest"]);

export interface DayCurrencyTotal {
  currency: string;
  eligibleMinor: number;
  pendingMinor: number;
  eligibleCount: number;
  pendingCount: number;
}

export type AmountTone = "normal" | "credit" | "transfer" | "excluded";
export type StatusTone = "ok" | "warning" | "danger";

/** First letters of the first two words, for the merchant monogram. */
export function merchantMonogram(name: string): string {
  const words = name
    .trim()
    .split(/\s+/)
    .filter((word) => word.length > 0);
  const initials = words
    .slice(0, 2)
    .map((word) => word[0])
    .join("")
    .toUpperCase();
  return initials || "?";
}

/**
 * Credits (refunds/payments) are not errors. Rust is reserved for excluded
 * rows; transfers are neutral because they can move in either direction.
 */
export function amountTone(row: LedgerRow): AmountTone {
  if (row.excluded) return "excluded";
  if (row.txType === "refund" || row.txType === "payment") return "credit";
  if (row.txType === "transfer") return "transfer";
  return "normal";
}

export function statusFor(row: LedgerRow): { label: string; tone: StatusTone } {
  if (row.excluded) return { label: "Excluded", tone: "danger" };
  if (row.pending) return { label: "Pending", tone: "warning" };
  return { label: "Posted", tone: "ok" };
}

/**
 * Day subtotals for the rows shown on this page. The table is paginated, so a
 * day that straddles a page boundary has a partial total here; the UI labels
 * it as page-local. Currencies are never summed together.
 */
export function dayTotals(rows: LedgerRow[]): DayCurrencyTotal[] {
  const totals = new Map<string, DayCurrencyTotal>();
  for (const row of rows) {
    if (row.excluded || !SPEND_TYPES.has(row.txType)) continue;
    const current = totals.get(row.currency) ?? {
      currency: row.currency,
      eligibleMinor: 0,
      pendingMinor: 0,
      eligibleCount: 0,
      pendingCount: 0,
    };
    if (row.pending) {
      current.pendingMinor += row.amountMinor;
      current.pendingCount += 1;
    } else {
      current.eligibleMinor += row.amountMinor;
      current.eligibleCount += 1;
    }
    totals.set(row.currency, current);
  }
  return [...totals.values()];
}
