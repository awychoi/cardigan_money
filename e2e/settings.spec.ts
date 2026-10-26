import { test, expect } from "@playwright/test";
import { openAccountsTab, openLocked, openSettingsSection, openUnlocked } from "./helpers";

test.describe("settings sections (browser mock)", () => {
  test("renders one section at a time and keeps controls reachable", async ({ page }) => {
    await openUnlocked(page);
    await openAccountsTab(page, "All accounts");
    await expect(page.getByRole("heading", { name: "All accounts" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Backups & recovery" })).toHaveCount(0);

    await page.getByRole("button", { name: "Backups & recovery", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Backups & recovery" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "All accounts" })).toHaveCount(0);

    await page.getByRole("button", { name: "Lock & idle", exact: true }).click();
    await expect(page.getByLabel("When the window closes")).toBeVisible();

    await page.getByRole("button", { name: "About", exact: true }).click();
    await expect(page.getByRole("heading", { name: "About" })).toBeVisible();
    await expect(page.getByText(/4\.6\.1 community/).first()).toBeVisible();
  });

  test("detects and persists a reporting timezone from the friendly dropdown", async ({ page }) => {
    await openUnlocked(page);
    await openSettingsSection(page, "Lock & idle");
    const select = page.getByRole("combobox", { name: "Reporting timezone", exact: true });
    await expect(select).toBeVisible();
    await expect(
      page.getByText(/This Mac reports|could not detect this Mac's timezone/),
    ).toBeVisible();
    await expect(select.locator("option[value='America/New_York']")).toContainText(
      "America/New_York",
    );
    await select.selectOption("America/Los_Angeles");
    await page.getByRole("button", { name: "Save timezone" }).click();
    await expect(page.getByText("Saved.")).toBeVisible();
    await page.getByRole("button", { name: "Overview" }).click();
    await openSettingsSection(page, "Lock & idle");
    await expect(
      page.getByRole("combobox", { name: "Reporting timezone", exact: true }),
    ).toHaveValue("America/Los_Angeles");
  });

  test("demo keeps the safe sections and hides real-vault sections", async ({ page }) => {
    await openLocked(page);
    await page.getByRole("button", { name: /Open demo/ }).click();
    await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
    await page.getByRole("button", { name: "Settings", exact: true }).click();

    for (const label of ["Appearance", "Lock & idle", "Accounts", "Security events", "About"]) {
      await expect(page.getByRole("button", { name: label, exact: true })).toBeVisible();
    }
    for (const label of [
      "Connections & sync",
      "Backups & recovery",
      "Merchant rules",
      "Import & export",
    ]) {
      await expect(page.getByRole("button", { name: label, exact: true })).toHaveCount(0);
    }
    await openAccountsTab(page, "All accounts");
    await expect(page.getByRole("tab", { name: "All accounts", exact: true })).toBeVisible();
    await expect(page.getByRole("tab", { name: "Bank connections", exact: true })).toHaveCount(0);
    await expect(page.getByRole("tab", { name: "CSV import", exact: true })).toHaveCount(0);
  });
});
