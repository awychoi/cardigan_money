import { test, expect } from "@playwright/test";
import { openLocked, openSettingsSection, openUnlocked } from "./helpers";

test.describe("settings sections (browser mock)", () => {
  test("renders one section at a time and keeps controls reachable", async ({ page }) => {
    await openUnlocked(page);
    await openSettingsSection(page, "Accounts");
    await expect(page.getByRole("heading", { name: "Accounts" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Backups & recovery" })).toHaveCount(0);

    await page.getByRole("button", { name: "Backups & recovery", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Backups & recovery" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Accounts" })).toHaveCount(0);

    await page.getByRole("button", { name: "Lock & idle", exact: true }).click();
    await expect(page.getByLabel("When the window closes")).toBeVisible();

    await page.getByRole("button", { name: "About", exact: true }).click();
    await expect(page.getByRole("heading", { name: "About" })).toBeVisible();
    await expect(page.getByText(/4\.6\.1 community/).first()).toBeVisible();
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
  });
});
