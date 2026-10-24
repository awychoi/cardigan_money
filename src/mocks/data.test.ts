import { describe, expect, it } from "vitest";
import { mockDashboard, mockLedger, mockSpendHistory } from "./data";

const OCTOBER = { startDate: "2026-10-01", endDateExclusive: "2026-11-01" };
const EUR_ACCOUNT = "aaaaaaaa-0000-4000-8000-000000000007";

describe("mock ledger subtotals", () => {
  it("keeps currencies separate and puts USD first", () => {
    const page = mockLedger({ ...OCTOBER, pageSize: 200 });
    expect(page.subtotals.map((item) => item.currency)).toEqual(["USD", "EUR"]);

    const eur = page.subtotals.find((item) => item.currency === "EUR");
    expect(eur).toMatchObject({
      eligibleMinor: 13_800,
      pendingMinor: 0,
      eligibleCount: 4,
      pendingCount: 0,
    });
    expect(page.rowCount).toBeGreaterThan(0);
  });

  it("keeps a foreign-currency account's subtotal when filtered", () => {
    const page = mockLedger({
      ...OCTOBER,
      accountIds: [EUR_ACCOUNT],
      pageSize: 200,
    });
    expect(page.subtotals).toHaveLength(1);
    expect(page.subtotals[0]).toMatchObject({
      currency: "EUR",
      eligibleMinor: 13_800,
      eligibleCount: 4,
    });
  });
});

describe("mock spend history", () => {
  it("ends at the current month and reconciles with the dashboard", () => {
    const history = mockSpendHistory();
    expect(history.currency).toBe("USD");
    expect(history.startMonth).toBe("2025-11");
    expect(history.endMonth).toBe("2026-10");
    expect(history.months).toHaveLength(12);

    const last = history.months[11];
    expect(last.month).toBe("2026-10");
    expect(last.complete).toBe(false);
    expect(last.netMinor).toBe(mockDashboard("demo", "2026-10").summary.netSpendingMinor);
    expect(last.grossMinor - last.refundsMinor).toBe(last.netMinor);

    expect(history.months[0]).toMatchObject({
      month: "2025-11",
      grossMinor: 0,
      refundsMinor: 0,
      netMinor: 0,
      count: 0,
      complete: true,
    });
  });

  it("moves the window with the selected end month", () => {
    const history = mockSpendHistory(6, "2026-09");
    expect(history.startMonth).toBe("2026-04");
    expect(history.endMonth).toBe("2026-09");
    expect(history.months).toHaveLength(6);
    expect(history.months[5].netMinor).toBe(
      mockDashboard("demo", "2026-09").summary.netSpendingMinor,
    );
    expect(history.months[5].complete).toBe(true);
  });

  it("rejects an out-of-range window", () => {
    expect(() => mockSpendHistory(0)).toThrowError(/invalid|months/i);
    expect(() => mockSpendHistory(25)).toThrowError(/invalid|months/i);
  });
});
