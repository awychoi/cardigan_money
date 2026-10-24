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

test.describe("sync status UI (browser mock)", () => {
  test("shows the provider-free state and disables manual sync", async ({ page }) => {
    await openUnlocked(page);
    await expect(page.getByText(/No provider connected/).first()).toBeVisible();
    await page.getByRole("button", { name: "Settings" }).click();
    await expect(page.getByRole("heading", { name: "Connections & sync" })).toBeVisible();
    await expect(page.getByText(/No provider connected. Import a CSV/)).toBeVisible();
    await expect(
      page.getByRole("main").getByRole("button", { name: "Sync now", exact: true }),
    ).toBeDisabled();
  });
});

test("demo theme toggle applies immediately without a vault-locked warning", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: /Open demo/ }).click();
  await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
  const toggle = page.getByRole("button", { name: /Appearance: System/ });
  await toggle.click();
  await expect(page.getByRole("button", { name: /Appearance: Light/ })).toBeVisible();
  await expect(page.getByText(/vault is locked/i)).toHaveCount(0);
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
});

test("configures Sandbox credentials and links a mock Plaid Item", async ({ page }) => {
  await openUnlocked(page);
  await page.getByRole("button", { name: "Settings" }).click();
  await expect(page.getByRole("heading", { name: "Plaid Sandbox" })).toBeVisible();
  await page.getByRole("button", { name: /Add Sandbox credentials/ }).click();
  await expect(page.getByText(/Sandbox credentials saved/)).toBeVisible();
  await page.getByRole("button", { name: "Link a Sandbox institution…" }).click();
  await expect(page.getByText(/Waiting for the Plaid Sandbox flow/)).toBeVisible();
  await expect(page.getByText(/Sandbox Item linked/)).toBeVisible({ timeout: 8000 });
  await expect(page.getByRole("cell", { name: "Sandbox Bank" })).toBeVisible();
});
