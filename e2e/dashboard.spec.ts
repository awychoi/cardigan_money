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
    await expect(page.getByText(/current as of/i)).toBeVisible();
    await expect(page.getByText(/Other currencies are shown separately/i)).toBeVisible();
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
    await page.getByRole("button", { name: "Previous month" }).click();
    await expect(page.locator(".month-label")).toHaveText("September 2026");
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
