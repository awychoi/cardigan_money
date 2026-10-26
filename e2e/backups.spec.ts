import { test, expect } from "@playwright/test";
import { openAccountsTab, openSettingsSection, openUnlocked } from "./helpers";

test.describe("backup and recovery UI (browser mock)", () => {
  test("creates, restores with a typed confirmation, and deletes an encrypted backup", async ({
    page,
  }) => {
    await openUnlocked(page);
    await openSettingsSection(page, "Backups & recovery");
    await page.getByRole("button", { name: "Back up now" }).click();
    await expect(page.getByText(/Encrypted backup created/).first()).toBeVisible();
    await expect(page.getByText("manual").first()).toBeVisible();

    await page.getByRole("button", { name: "Restore backup…" }).click();
    await expect(page.getByText(/Replace the current ledger/)).toBeVisible();
    const apply = page.getByRole("button", { name: "Restore this backup…" });
    await expect(apply).toBeDisabled();
    await page.getByLabel("Type replace to confirm").fill("replace");
    await expect(apply).toBeEnabled();
    await apply.click();
    await expect(page.getByText(/Restored a manual backup/)).toBeVisible();
    await expect(page.getByText(/pre-restore backup was created and verified/)).toBeVisible();

    await page.getByRole("button", { name: "Delete" }).first().click();
    await expect(page.getByText("Backup deleted.")).toBeVisible();
  });

  test("back up now is the only manual backup action", async ({ page }) => {
    await openUnlocked(page);
    await openSettingsSection(page, "Backups & recovery");
    await expect(page.getByRole("button", { name: "Back up now" })).toBeVisible();
    await expect(page.getByRole("button", { name: /Portable encrypted export/ })).toHaveCount(0);
  });

  test("backs up from the locked gate without unlocking", async ({ page }) => {
    await page.goto("/");
    await expect(
      page.getByRole("heading", { name: /Your encrypted ledger is locked/ }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Back up now" }).click();
    await expect(page.getByText(/Encrypted backup created/).first()).toBeVisible();
    await expect(
      page.getByRole("heading", { name: /Your encrypted ledger is locked/ }),
    ).toBeVisible();
  });

  test("a fresh install can restore a backup from the locked gate", async ({ page }) => {
    await page.goto("/?mockVault=empty");
    await expect(page.getByRole("heading", { name: /Create your encrypted ledger/ })).toBeVisible();
    await page.getByRole("button", { name: "Restore from backup…" }).click();
    await expect(page.getByText(/fresh install/i).first()).toBeVisible();
    await expect(page.getByLabel("Type replace to confirm")).toHaveCount(0);
    await page.getByRole("button", { name: "Restore this backup…" }).click();
    await expect(
      page.getByRole("heading", { name: /Your encrypted ledger is locked/ }),
    ).toBeVisible();
    await expect(page.getByText(/Backup restored/).first()).toBeVisible();
  });

  test("crypto-erase requires the typed confirmation and returns to the gate", async ({ page }) => {
    await openUnlocked(page);
    await openSettingsSection(page, "Backups & recovery");
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
    await page.getByRole("button", { name: "Settings", exact: true }).click();
    await expect(page.getByRole("button", { name: "Backups & recovery", exact: true })).toHaveCount(
      0,
    );
    await expect(page.getByText("Backups & recovery")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Erase all local data…" })).toHaveCount(0);
  });
});

test.describe("sync status UI (browser mock)", () => {
  test("shows the provider-free state and disables manual sync", async ({ page }) => {
    await openUnlocked(page);
    await expect(page.getByText(/No provider connected/).first()).toBeVisible();
    await openAccountsTab(page, "Bank connections");
    await expect(page.getByRole("heading", { name: "Bank connections" })).toBeVisible();
    await expect(page.getByText(/No bank connected yet/)).toBeVisible();
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
  await openAccountsTab(page, "Bank connections");
  await expect(page.getByRole("heading", { name: "Plaid connection" })).toBeVisible();
  await page.getByRole("button", { name: /Add Sandbox credentials/ }).click();
  await expect(page.getByText(/Sandbox credentials saved/)).toBeVisible();
  await page.getByRole("button", { name: "Link a Sandbox institution…" }).click();
  await expect(page.getByText(/Waiting for the Plaid Sandbox flow/)).toBeVisible();
  await expect(page.getByText(/Sandbox Item linked/)).toBeVisible({ timeout: 8000 });
  await expect(page.getByRole("cell", { name: "Sandbox Bank" })).toBeVisible();
  await page.getByRole("button", { name: "Show verification report" }).click();
  await expect(page.getByLabel("Plaid verification report")).toContainText("accountCount");
  await expect(page.getByLabel("Plaid verification report")).toContainText("Sandbox Bank");
});

test("configures Trial credentials and links a mock production Item", async ({ page }) => {
  await openUnlocked(page);
  await openAccountsTab(page, "Bank connections");
  await expect(page.getByRole("heading", { name: "Plaid connection" })).toBeVisible();
  await page.getByRole("radio", { name: /Trial \/ Production/ }).check();
  await expect(page.getByText(/Real bank data will be imported/)).toBeVisible();
  await page.getByRole("button", { name: /Add Trial \/ Production credentials/ }).click();
  await expect(page.getByText(/Trial \/ Production credentials saved/)).toBeVisible();
  await page.getByRole("button", { name: "Link a real institution…" }).click();
  await expect(page.getByText(/Waiting for the Plaid Trial \/ Production flow/)).toBeVisible();
  await expect(page.getByText(/Trial \/ Production Item linked/)).toBeVisible({ timeout: 8000 });
  await expect(page.getByRole("cell", { name: "Production Bank (mock)" })).toBeVisible();
  await expect(page.getByRole("cell", { name: "Trial / Production" })).toBeVisible();
});

test("updates a healthy Plaid connection through update mode", async ({ page }) => {
  await openUnlocked(page);
  await openAccountsTab(page, "Bank connections");
  await page.getByRole("button", { name: /Add Sandbox credentials/ }).click();
  await page.getByRole("button", { name: "Link a Sandbox institution…" }).click();
  await expect(page.getByText(/Sandbox Item linked/)).toBeVisible({ timeout: 8000 });
  await page.getByRole("button", { name: "Update…" }).click();
  await expect(page.getByText(/Update the Sandbox Item in your browser/)).toBeVisible();
  await expect(page.getByText(/Waiting for the Plaid Sandbox flow/)).toBeVisible();
  await expect(page.getByText(/Sandbox Item updated/)).toBeVisible({ timeout: 8000 });
  await expect(page.getByRole("cell", { name: "Sandbox Bank" })).toHaveCount(1);
});

test("refuses a duplicate Plaid Item before an access token is requested", async ({ page }) => {
  await openUnlocked(page);
  await openAccountsTab(page, "Bank connections");
  await page.getByRole("button", { name: /Add Sandbox credentials/ }).click();
  await page.getByRole("button", { name: "Link a Sandbox institution…" }).click();
  await expect(page.getByText(/Sandbox Item linked/)).toBeVisible({ timeout: 8000 });
  await page.getByRole("button", { name: "Link a Sandbox institution…" }).click();
  await expect(page.getByText(/Sandbox Bank is already connected/)).toBeVisible({
    timeout: 8000,
  });
  await expect(page.getByText(/No Plaid access token was requested/)).toBeVisible();
  await expect(page.getByRole("cell", { name: "Sandbox Bank" })).toHaveCount(1);
});
