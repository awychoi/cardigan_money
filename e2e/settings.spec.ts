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

  test("navigation layout is independent of the theme", async ({ page }) => {
    await openUnlocked(page);
    await expect(page.locator(".app")).toHaveAttribute("data-nav", "sidebar");

    await page.getByRole("button", { name: "Settings", exact: true }).click();
    const navigation = page.getByRole("group", { name: "Navigation layout" });
    await navigation.getByRole("button", { name: "Top bar" }).click();
    await expect(page.locator(".app")).toHaveAttribute("data-nav", "topbar");
    await expect(page.locator(".app")).toHaveClass(/app-topbar/);
    await expect(page.locator(".sidebar")).toHaveCount(0);

    // Ledger used to force a sidebar; it must keep the chosen top bar.
    await page.locator(".theme-card", { hasText: "Ledger" }).click();
    await expect(page.locator(".app")).toHaveAttribute("data-nav", "topbar");
    await expect(page.locator(".sidebar")).toHaveCount(0);

    // Graphite used to force a top bar; it must keep the chosen sidebar.
    await navigation.getByRole("button", { name: "Sidebar" }).click();
    await expect(page.locator(".app")).toHaveAttribute("data-nav", "sidebar");
    await page.locator(".theme-card", { hasText: "Graphite" }).click();
    await expect(page.locator(".app")).toHaveAttribute("data-nav", "sidebar");
    await expect(page.locator(".sidebar")).toBeVisible();
    await page.getByRole("button", { name: "Overview", exact: true }).click();
    await expect(page.locator(".app")).toHaveClass(/app-sidebar/);
  });

  test("shows the maker credit on the locked gate, About and footer", async ({ page }) => {
    await openLocked(page);
    await expect(page.locator(".vault-maker")).toContainText("Made by Wai Yin Choi");
    await expect(page.locator(".vault-maker")).toContainText("wychoi.com");

    await page.getByRole("button", { name: "Unlock vault" }).click();
    await page.getByRole("heading", { name: "Overview" }).waitFor();
    await expect(page.locator(".footer .maker-line")).toContainText("Made by Wai Yin Choi");

    await page.getByRole("button", { name: "Settings", exact: true }).click();
    await page.getByRole("button", { name: "About", exact: true }).click();
    const card = page.locator(".maker-card");
    await expect(card).toContainText("Made by Wai Yin Choi");
    await expect(card).toContainText("New York");
    await expect(card.locator("img")).toHaveCount(1);
    await expect(card.getByRole("button", { name: "wychoi.com" })).toBeVisible();
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
