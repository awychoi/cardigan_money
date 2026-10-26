import { test, expect } from "@playwright/test";
import { openLocked, openUnlocked } from "./helpers";

test.describe("read-only dashboard (browser mock)", () => {
  test("overview shows calendar-month and current-data metrics with demo labels", async ({
    page,
  }) => {
    await openUnlocked(page);
    await expect(page.getByText("Current card balances")).toBeVisible();
    await expect(page.getByText("Upcoming reported payments")).toBeVisible();
    await expect(page.getByText("Pending in this month")).toBeVisible();
    await expect(page.getByText(/Balances and upcoming payments as of/i)).toBeVisible();
    await expect(page.getByText(/Other currencies are shown separately/i)).toBeVisible();
    await expect(page.getByRole("img", { name: /Gross spending by category/i })).toBeVisible();
    await expect(page.getByRole("img", { name: /Monthly net spending/i })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Needs attention" })).toBeVisible();
  });

  test("theme switch swaps ring for ranked bars without a reload", async ({ page }) => {
    await openUnlocked(page);
    await expect(page.getByRole("img", { name: /Gross spending by category/i })).toBeVisible();

    await page.getByRole("button", { name: "Settings", exact: true }).click();
    await page.locator(".theme-card", { hasText: "Graphite" }).click();
    await page.getByRole("button", { name: "Overview", exact: true }).click();

    await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
    await expect(page.getByRole("list", { name: "Spending by category" })).toBeVisible();
    await expect(page.getByRole("img", { name: /Gross spending by category/i })).toHaveCount(0);
  });

  test("cards show due amount kinds without inventing an amount due", async ({ page }) => {
    await openUnlocked(page);
    await page.getByRole("button", { name: "Cards", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Cards" })).toBeVisible();
    await expect(page.getByText(/amount not reported/).first()).toBeVisible();
    await expect(page.getByText(/reported/).first()).toBeVisible();
    await expect(
      page.getByText(/A statement balance is not the remaining amount due/).first(),
    ).toBeVisible();
    await expect(page.getByText("Reconnect needed").first()).toBeVisible();
    await expect(page.getByText("Sync error").first()).toBeVisible();
  });

  test("ledger groups by day, labels page subtotals, and renders credits as credits", async ({
    page,
  }) => {
    await openUnlocked(page);
    await page.getByRole("button", { name: "Transactions", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Transactions" })).toBeVisible();

    await expect(page.locator(".ledger-day-row").first()).toBeVisible();
    await expect(page.locator(".ledger-day-page").first()).toHaveText("page subtotal");
    await expect(page.locator(".merchant-monogram").first()).not.toBeEmpty();
    await expect(page.locator(".category-chip").first()).toBeVisible();
    await expect(page.locator(".ledger-status-ok").first()).toBeVisible();
    await expect(page.locator(".ledger-meta")).toContainText("eligible");
    await expect(page.locator(".ledger-meta")).toContainText("USD");

    const filterPosition = await page
      .locator(".filter-bar")
      .evaluate((element) => getComputedStyle(element).position);
    expect(filterPosition).toBe("sticky");
    const dayPosition = await page
      .locator(".ledger-day-row th")
      .first()
      .evaluate((element) => getComputedStyle(element).position);
    expect(dayPosition).toBe("sticky");

    await page.locator(".ledger-table-wrap").evaluate((element) => {
      element.scrollTop = 320;
      element.dispatchEvent(new Event("scroll"));
    });
    await expect(page.locator(".ledger-day-row th").first()).toHaveClass(
      /ledger-day-header-hidden/,
    );
    expect(await page.locator(".ledger-day-header-hidden").count()).toBeGreaterThan(0);

    await page.getByLabel("Search transactions").fill("Payment Thank You");
    const paymentRow = page.getByRole("row").filter({ hasText: "Payment Thank You" }).first();
    await expect(paymentRow.locator(".amount-credit")).toBeVisible();
  });

  test("a card click filters the ledger while keeping the month", async ({ page }) => {
    await openUnlocked(page);
    await page.getByRole("button", { name: "Cards", exact: true }).click();
    await page.getByRole("button", { name: /Filter transactions to Freedom Unlimited/ }).click();
    await expect(page.getByRole("heading", { name: "Transactions" })).toBeVisible();
    await expect(page.getByLabel("Filter by card")).not.toHaveValue("");
    await expect(page.getByText(/^\d+ rows$/)).toBeVisible();
    await page.getByRole("button", { name: "Clear filters" }).click();
    await expect(page.getByLabel("Filter by card")).toHaveValue("");
  });

  test("ledger search, pending subtotal, and read-only detail panel work", async ({ page }) => {
    await openUnlocked(page);
    await page.getByRole("button", { name: "Transactions", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Transactions" })).toBeVisible();
    await expect(page.locator(".ledger-meta")).toContainText("EUR");
    await page.getByLabel("Search transactions").fill("Blue Bottle");
    await expect(page.getByText(/eligible subtotal/i)).toBeVisible();
    await expect(page.getByText(/pending subtotal/i)).toBeVisible();
    await expect(page.getByText("Blue Bottle Coffee").first()).toBeVisible();
    await page.getByRole("row").filter({ hasText: "Blue Bottle Coffee" }).first().click();
    await expect(page.getByRole("heading", { name: "Blue Bottle Coffee" })).toBeVisible();
    await expect(page.getByText(/Raw description/)).toBeVisible();
    await expect(page.getByRole("button", { name: /Edit transaction/ })).toBeVisible();
    await page.getByRole("button", { name: "Close transaction details" }).click();
    await expect(page.getByRole("heading", { name: "Blue Bottle Coffee" })).toHaveCount(0);
  });

  test("supplier view drills down without resetting the month", async ({ page }) => {
    await openUnlocked(page);
    await page.getByRole("button", { name: "Transactions", exact: true }).click();
    await page.getByRole("button", { name: "By supplier" }).click();
    await expect(page.getByRole("heading", { name: "By supplier" })).toBeVisible();
    await expect(page.getByText(/Supplier totals reconcile exactly/)).toBeVisible();
    const firstSupplier = page.getByRole("row").nth(1);
    await firstSupplier.click();
    await expect(page.getByRole("heading", { name: "Transactions" })).toBeVisible();
    await expect(page.getByLabel("Search transactions")).not.toHaveValue("");
  });

  test("month navigation changes spending but not current balances", async ({ page }) => {
    await openUnlocked(page);
    await expect(page.locator(".month-label")).toHaveText("October 2026");
    const balancesBefore = await page
      .locator(".metric", { hasText: "Current card balances" })
      .locator(".metric-value")
      .innerText();
    const ringBefore = await page
      .getByRole("img", { name: /Gross spending by category/i })
      .getAttribute("aria-label");
    const trendBefore = await page
      .getByRole("img", { name: /Monthly net spending/i })
      .getAttribute("aria-label");
    await page.getByRole("button", { name: "Previous month" }).click();
    await expect(page.locator(".month-label")).toHaveText("September 2026");
    await expect(page.getByRole("img", { name: /Monthly net spending/i })).toHaveAttribute(
      "aria-label",
      /September 2026/,
    );
    const ringAfter = await page
      .getByRole("img", { name: /Gross spending by category/i })
      .getAttribute("aria-label");
    const trendAfter = await page
      .getByRole("img", { name: /Monthly net spending/i })
      .getAttribute("aria-label");
    expect(ringAfter).not.toBe(ringBefore);
    expect(trendAfter).not.toBe(trendBefore);
    const balancesAfter = await page
      .locator(".metric", { hasText: "Current card balances" })
      .locator(".metric-value")
      .innerText();
    expect(balancesAfter).toBe(balancesBefore);
    await page.getByRole("button", { name: "This month" }).click();
    await expect(page.locator(".month-label")).toHaveText("October 2026");
  });

  test("stays usable at 1100 x 760 without page overflow", async ({ page }) => {
    await page.setViewportSize({ width: 1100, height: 760 });
    await openUnlocked(page);
    await expect(page.getByRole("img", { name: /Gross spending by category/i })).toBeVisible();
    const overviewOverflow = await page.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
    );
    expect(overviewOverflow).toBe(false);
    await page.getByRole("button", { name: "Transactions", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Transactions" })).toBeVisible();
    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
    );
    expect(overflow).toBe(false);
    await expect(page.locator(".table-wrap")).toBeVisible();
  });

  test("demo mode is labeled, isolated, resettable, and exitable", async ({ page }) => {
    await openLocked(page);
    await page.getByRole("button", { name: /Open demo/ }).click();
    await expect(page.getByText(/Demo mode — synthetic data/)).toBeVisible();
    await expect(page.getByText(/Separate database/)).toBeVisible();
    await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
    await page.getByRole("button", { name: "Reset demo" }).click();
    await page.getByRole("button", { name: "Reset", exact: true }).click();
    await expect(
      page.getByText(/Demo data was recreated|Demo mode — synthetic data/).first(),
    ).toBeVisible();
    await page.getByRole("button", { name: "Exit demo" }).first().click();
    await expect(page.getByText(/Your encrypted ledger is locked/)).toBeVisible();
  });
});
