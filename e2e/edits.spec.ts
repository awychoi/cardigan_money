import { test, expect } from "@playwright/test";
import { openSettingsSection, openUnlocked } from "./helpers";

test.describe("edits, rules, metadata, and CSV workflow (browser mock)", () => {
  test("edits a transaction override and clears it without changing the source", async ({
    page,
  }) => {
    await openUnlocked(page);
    await page.getByRole("button", { name: "Transactions", exact: true }).click();
    await page.getByLabel("Search transactions").fill("Blue Bottle");
    await page.getByRole("row").filter({ hasText: "Blue Bottle Coffee" }).first().click();
    await page.getByRole("button", { name: /Edit transaction/ }).click();
    await page.getByLabel("Merchant alias").fill("Blue Bottle Edited");
    await page.getByLabel("Category", { exact: true }).fill("Coffee");
    await page.getByLabel(/Notes/).fill("owner note");
    await page.getByRole("button", { name: "Save edits" }).click();
    await expect(page.getByRole("heading", { name: "Blue Bottle Edited" })).toBeVisible();
    await expect(page.getByText("owner note")).toBeVisible();
    await expect(page.getByText(/Blue Bottle Coffee raw description/)).toBeVisible();
    await page.getByRole("button", { name: "Clear user edits" }).click();
    await expect(page.getByRole("heading", { name: "Blue Bottle Coffee" })).toBeVisible();
  });

  test("saves dated manual payment metadata and shows its source", async ({ page }) => {
    await openUnlocked(page);
    await openSettingsSection(page, "Accounts");
    const accountSelect = page.getByLabel("Manual metadata account");
    await expect(accountSelect).toBeVisible();
    await page.getByLabel("Statement balance").fill("120.00");
    await page.getByLabel("Minimum due").fill("25.00");
    await page.getByLabel("Next due date").fill("2026-10-29");
    await page.getByLabel("Valid through").fill("2026-11-15");
    await page.getByRole("button", { name: "Save manual metadata" }).click();
    await expect(page.getByText(/Manual payment metadata saved/)).toBeVisible();
    await page.getByRole("button", { name: "Cards", exact: true }).click();
    await expect(page.getByText(/Due source: manual/).first()).toBeVisible();
  });

  test("creates and deletes a merchant rule", async ({ page }) => {
    await openUnlocked(page);
    await openSettingsSection(page, "Merchant rules");
    await page.getByLabel("Match text").fill("amzn");
    await page.getByLabel("Canonical merchant").fill("Amazon Test");
    await page.getByLabel("Category (optional)").fill("Shopping");
    await page.getByRole("button", { name: "Add rule" }).click();
    await expect(page.getByText(/Rule added and reapplied/)).toBeVisible();
    await expect(page.getByRole("cell", { name: "Amazon Test" })).toBeVisible();
    await page.getByRole("button", { name: "Delete" }).click();
    await expect(page.getByText(/Rule deleted/)).toBeVisible();
  });

  test("validates, imports, and rolls back a CSV batch", async ({ page }) => {
    await openUnlocked(page);
    await openSettingsSection(page, "Import & export");
    await page.getByRole("button", { name: "Choose CSV file…" }).click();
    await expect(page.getByText(/5 columns/)).toBeVisible();
    await page.getByLabel("Date column").selectOption("0");
    await page.getByLabel("Description column").selectOption("1");
    await page.getByLabel("Date format").selectOption("iso");
    await page.getByLabel("Amount column").selectOption("2");
    await page.getByLabel("Category column").selectOption("3");
    await page.getByLabel("Source ID column").selectOption("4");
    await page.getByRole("button", { name: "Validate and preview" }).click();
    await expect(page.getByText(/New/)).toBeVisible();
    await expect(page.getByText(/Needs review/)).toBeVisible();
    await page.getByRole("button", { name: "Confirm import" }).click();
    await expect(page.getByText(/Imported 2 rows/)).toBeVisible();
    await expect(page.getByRole("cell", { name: "imported" })).toBeVisible();
    await page.getByRole("button", { name: "Roll back" }).click();
    await page.getByRole("button", { name: "Roll back", exact: true }).last().click();
    await expect(page.getByText(/Batch rolled back/)).toBeVisible();
    await expect(page.getByRole("cell", { name: "rolled_back" })).toBeVisible();
  });

  test("warns before a plaintext export and reports the row count", async ({ page }) => {
    await openUnlocked(page);
    await page.getByRole("button", { name: "Transactions", exact: true }).click();
    await page.getByRole("button", { name: "Export CSV…" }).click();
    await expect(page.getByText(/Plaintext export/)).toBeVisible();
    await expect(page.getByText(/owner-only \(0600\)/)).toBeVisible();
    await page.getByRole("button", { name: "Choose destination…" }).click();
    await expect(page.getByText(/Exported \d+ rows as plaintext CSV/)).toBeVisible();
  });

  test("demo mode disables edits and import/export", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("button", { name: /Open demo/ }).click();
    await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
    await page.getByRole("button", { name: "Transactions", exact: true }).click();
    await expect(page.getByRole("button", { name: "Export CSV…" })).toHaveCount(0);
    await openSettingsSection(page, "Accounts");
    await expect(page.getByText(/Demo accounts are read-only/)).toBeVisible();
    await page.getByRole("button", { name: "Lock & idle", exact: true }).click();
    await expect(page.getByText(/Passphrase change is unavailable in demo mode/)).toBeVisible();
  });
});
