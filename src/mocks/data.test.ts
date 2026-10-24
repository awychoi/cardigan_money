import { describe, expect, it } from "vitest";
import { mockLedger } from "./data";

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
