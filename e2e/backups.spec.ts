import { test, expect } from "@playwright/test";
import { openUnlocked } from "./helpers";

test.describe("backup and recovery UI (browser mock)", () => {
  test("creates, lists, restores, and deletes an encrypted backup", async ({ page }) => {
    await openUnlocked(page);
    await page.getByRole("button", { name: "Settings" }).click();
    await page.getByRole("button", { name: "Back up now" }).click();
    await expect(page.getByText(/Encrypted backup created/)).toBeVisible();
    await expect(page.getByText("manual").first()).toBeVisible();

    await page.getByRole("button", { name: "Restore backup…" }).click();
    await expect(page.getByText(/Replace the current ledger/)).toBeVisible();
    await page.getByRole("button", { name: "Choose backup and restore…" }).click();
    await expect(page.getByText(/pre-restore backup was created/)).toBeVisible();

    await page.getByRole("button", { name: "Delete" }).first().click();
    await expect(page.getByText("Backup deleted.")).toBeVisible();
  });

  test("portable export reports a separate-password container", async ({ page }) => {
    await openUnlocked(page);
    await page.getByRole("button", { name: "Settings" }).click();
    await page.getByRole("button", { name: "Portable encrypted export…" }).click();
    await expect(page.getByText(/Portable encrypted export written/)).toBeVisible();
  });

  test("crypto-erase requires the typed confirmation and returns to the gate", async ({ page }) => {
    await openUnlocked(page);
    await page.getByRole("button", { name: "Settings" }).click();
    await page.getByRole("button", { name: "Erase all local data…" }).click();
    const confirmButton = page.getByRole("button", { name: "Erase all local data…" });
    await expect(confirmButton).toBeDisabled();
    await page.getByLabel("Type erase to confirm").fill("erase");
    await expect(confirmButton).toBeEnabled();
    await confirmButton.click();
    await expect(page.getByText(/Create your encrypted ledger/)).toBeVisible();
  });

  test("demo mode hides backup, restore, and erase controls", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("button", { name: /Open demo/ }).click();
    await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
    await page.getByRole("button", { name: "Settings" }).click();
    await expect(page.getByText("Backups & recovery")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Erase all local data…" })).toHaveCount(0);
  });
});
