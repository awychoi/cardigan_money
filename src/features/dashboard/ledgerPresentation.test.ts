import { describe, expect, it } from "vitest";
import type { LedgerRow } from "../../lib/bridge";
import { amountTone, dayTotals, merchantMonogram, statusFor } from "./ledgerPresentation";

function row(overrides: Partial<LedgerRow> = {}): LedgerRow {
  return {
    id: "row-1",
    reportingDate: "2026-10-02",
    merchant: "Blue Bottle Coffee",
    merchantEdited: false,
    rawDescription: "Blue Bottle Coffee raw description",
    accountId: "acct-1",
    accountName: "Everyday Card",
    accountLastFour: "4101",
    category: "Dining",
    categoryEdited: false,
    txType: "purchase",
    pending: false,
    amountMinor: 1_234,
    currency: "USD",
    sourceKind: "demo",
    authorizedDate: null,
    postedDate: "2026-10-02",
    reportingDateSource: "posted",
    excluded: false,
    notes: null,
    ...overrides,
  };
}

describe("merchantMonogram", () => {
  it("uses the first letters of the first two words", () => {
    expect(merchantMonogram("Blue Bottle Coffee")).toBe("BB");
    expect(merchantMonogram("  Café de Flore ")).toBe("CD");
    expect(merchantMonogram("Amazon Store Card (demo)")).toBe("AS");
  });

  it("handles single-word and empty names", () => {
    expect(merchantMonogram("Bilt")).toBe("B");
    expect(merchantMonogram("   ")).toBe("?");
  });
});

describe("amountTone", () => {
  it("does not treat credits as errors", () => {
    expect(amountTone(row({ txType: "refund", amountMinor: -2_500 }))).toBe("credit");
    expect(amountTone(row({ txType: "payment", amountMinor: -100_000 }))).toBe("credit");
    expect(amountTone(row({ txType: "transfer", amountMinor: -50_000 }))).toBe("transfer");
    expect(amountTone(row({ txType: "purchase" }))).toBe("normal");
  });

  it("reserves rust for excluded rows, even when they are credits", () => {
    expect(amountTone(row({ txType: "refund", excluded: true }))).toBe("excluded");
  });
});

describe("statusFor", () => {
  it("maps excluded, pending, and posted states", () => {
    expect(statusFor(row({ excluded: true }))).toEqual({ label: "Excluded", tone: "danger" });
    expect(statusFor(row({ pending: true }))).toEqual({ label: "Pending", tone: "warning" });
    expect(statusFor(row())).toEqual({ label: "Posted", tone: "ok" });
  });
});

describe("dayTotals", () => {
  it("sums eligible and pending spend per currency", () => {
    const totals = dayTotals([
      row({ amountMinor: 1_000 }),
      row({ id: "row-2", amountMinor: -2_500, txType: "refund" }),
      row({ id: "row-3", amountMinor: 300, pending: true }),
      row({ id: "row-4", amountMinor: 2_000, currency: "EUR", txType: "purchase" }),
    ]);

    expect(totals).toEqual([
      {
        currency: "USD",
        eligibleMinor: -1_500,
        pendingMinor: 300,
        eligibleCount: 2,
        pendingCount: 1,
      },
      {
        currency: "EUR",
        eligibleMinor: 2_000,
        pendingMinor: 0,
        eligibleCount: 1,
        pendingCount: 0,
      },
    ]);
  });

  it("ignores payments, transfers, and excluded rows", () => {
    const totals = dayTotals([
      row({ txType: "payment", amountMinor: -100_000 }),
      row({ txType: "transfer", amountMinor: 50_000 }),
      row({ txType: "purchase", excluded: true }),
    ]);
    expect(totals).toEqual([]);
  });
});
