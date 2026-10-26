import { test, expect } from "@playwright/test";
import { openAccountsTab, openLocked, openSettingsSection, openUnlocked } from "./helpers";

test.describe("secure shell (browser mock)", () => {
  test("starts locked with no password input", async ({ page }) => {
    await openLocked(page);
    await expect(page.getByText(/native macOS sheet/i)).toBeVisible();
    expect(await page.locator("input[type='password']").count()).toBe(0);
  });

  test("renders the app identity and honest empty state after unlock", async ({ page }) => {
    await openUnlocked(page);
    await expect(page.getByText("Mock desktop bridge")).toBeVisible();
    await expect(page.getByRole("button", { name: "Overview" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    await expect(page.getByRole("button", { name: "Sync now" })).toBeDisabled();
    await expect(page.getByText(/No provider connected/)).toBeVisible();
    await expect(page.getByText(/no telemetry/i)).toBeVisible();
  });

  test("makes no request outside the local app origin", async ({ page }) => {
    const external: string[] = [];
    page.on("request", (request) => {
      const url = request.url();
      if (url.startsWith("data:") || url.startsWith("blob:")) return;
      try {
        const parsed = new URL(url);
        if (parsed.hostname !== "127.0.0.1" && parsed.hostname !== "localhost") {
          external.push(url);
        }
      } catch {
        external.push(url);
      }
    });
    await openUnlocked(page);
    await page.waitForTimeout(500);
    expect(external).toEqual([]);
  });

  test("stores no financial data or secrets in browser storage or the URL", async ({ page }) => {
    await openUnlocked(page);
    const stored = await page.evaluate(() => ({
      local: { ...window.localStorage },
      session: { ...window.sessionStorage },
      href: window.location.href,
    }));
    expect(stored.local).toEqual({});
    expect(stored.session).toEqual({});
    expect(stored.href).not.toMatch(/passphrase|token|secret|account/i);
  });

  test("theme toggle applies through encrypted settings", async ({ page }) => {
    await openUnlocked(page);
    await page.getByRole("button", { name: /appearance/i }).click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
    const keys = await page.evaluate(() => Object.keys(window.localStorage));
    expect(keys).toEqual([]);
  });

  test("locks and returns to the gate", async ({ page }) => {
    await openUnlocked(page);
    await page.getByRole("button", { name: "Lock now" }).click();
    await expect(page.getByText(/Your encrypted ledger is locked/)).toBeVisible();
    await expect(page.getByText("Your cards, in one quiet place.")).toHaveCount(0);
  });

  test("primary navigation and the supplier toggle preserve the month", async ({ page }) => {
    await openUnlocked(page);
    await page.getByRole("button", { name: "Cards", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Cards" })).toBeVisible();
    await page.getByRole("button", { name: "Transactions", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Transactions" })).toBeVisible();
    await page.getByRole("button", { name: "By supplier" }).click();
    await expect(page.getByRole("heading", { name: "By supplier" })).toBeVisible();
    await expect(page.getByRole("button", { name: "By supplier" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await openSettingsSection(page, "About");
    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
    await expect(page.getByText(/4\.6\.1 community/).first()).toBeVisible();
    await expect(page.getByText(/Argon2id/)).toBeVisible();
  });

  test("walks the onboarding wizard with no provider and adds an account", async ({ page }) => {
    await page.goto("/?mockOnboarding=1");
    await page.getByRole("button", { name: "Unlock vault" }).click();
    await expect(page.getByText("Connect your bank.")).toBeVisible();
    await expect(page.getByText(/macOS Keychain/)).toBeVisible();
    await page.getByRole("button", { name: "Add a card manually" }).click();
    await expect(page.getByText("Add a card manually.")).toBeVisible();
    await page.getByLabel("Issuer").fill("Chase");
    await page.getByLabel("Display name").fill("Wizard Card");
    await page.getByLabel(/Last four/).fill("1234");
    await page.getByRole("button", { name: "Add account" }).click();
    await expect(page.getByText("Set your reporting timezone.")).toBeVisible();
    await expect(page.getByText(/Added Wizard Card/)).toBeVisible();
    await page.getByRole("button", { name: "Continue", exact: true }).click();
    await expect(page.getByText("You are ready.")).toBeVisible();
    await page.getByRole("button", { name: "Finish setup" }).click();
    await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
    await openAccountsTab(page, "All accounts");
    await expect(page.getByText("Wizard Card", { exact: true })).toBeVisible();
    await expect(page.getByText("Chase").first()).toBeVisible();
  });

  test("keeps the detected timezone visible and selectable during onboarding", async ({ page }) => {
    await page.goto("/?mockOnboarding=1");
    await page.getByRole("button", { name: "Unlock vault" }).click();
    await page.getByRole("button", { name: "Continue without a bank" }).click();
    await expect(page.getByText("Set your reporting timezone.")).toBeVisible();
    await expect(page.getByText(/Detected from this Mac/)).toBeVisible();
    const select = page.getByRole("combobox", { name: "Reporting timezone", exact: true });
    await expect(select).toBeVisible();
    await select.selectOption("America/Los_Angeles");
    await expect(select).toHaveValue("America/Los_Angeles");
    await page.getByRole("button", { name: "Continue", exact: true }).click();
    await expect(page.getByText("You are ready.")).toBeVisible();
    await expect(
      page.locator(".facts dd").filter({ hasText: "America/Los_Angeles" }),
    ).toBeVisible();
  });

  test("configures Plaid credentials in onboarding and reports their storage", async ({ page }) => {
    await page.goto("/?mockOnboarding=1");
    await page.getByRole("button", { name: "Unlock vault" }).click();
    await page.getByRole("button", { name: /Add Sandbox credentials/ }).click();
    await expect(page.getByText(/Sandbox credentials saved to the macOS Keychain/)).toBeVisible();
    await expect(page.getByText("Stored in Keychain")).toBeVisible();
    await page.getByRole("button", { name: "Continue", exact: true }).click();
    await page.getByRole("button", { name: "Continue", exact: true }).click();
    await expect(page.getByText(/Sandbox credentials \(Keychain\)/)).toBeVisible();
    await expect(page.getByText(/no bank is linked yet/)).toBeVisible();
  });

  test("sends Add account from an empty overview to Settings → Accounts", async ({ page }) => {
    await page.goto("/?mockOnboarding=1&mockAccounts=empty");
    await page.getByRole("button", { name: "Unlock vault" }).click();
    await expect(page.getByText("Connect your bank.")).toBeVisible();
    await page.getByRole("button", { name: "Skip setup" }).click();
    await expect(page.getByText("Your cards, in one quiet place.")).toBeVisible();
    await page.getByRole("button", { name: "Add account" }).click();
    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Accounts", exact: true })).toHaveAttribute(
      "aria-current",
      "page",
    );
    await expect(page.getByRole("tab", { name: "All accounts", exact: true })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(page.getByRole("button", { name: "Add card manually" })).toBeVisible();
  });

  test("manages accounts in settings and archives reversibly", async ({ page }) => {
    await openUnlocked(page);
    await openAccountsTab(page, "All accounts");
    await page.getByRole("button", { name: "Add card manually" }).click();
    await page.getByLabel("Issuer").fill("Bilt");
    await page.getByLabel("Display name").fill("Local Test Card");
    await page.getByLabel(/Last four/).fill("4321");
    await page.getByRole("button", { name: "Add account" }).click();
    await expect(page.getByText("Local Test Card", { exact: true })).toBeVisible();
    const row = page
      .getByRole("row")
      .filter({ has: page.getByText("Local Test Card", { exact: true }) });
    await row.getByRole("button", { name: "Archive" }).click();
    await row.getByRole("button", { name: "Archive", exact: true }).click();
    await expect(page.getByText("Local Test Card", { exact: true })).toHaveCount(0);
    await page.getByLabel("Show archived accounts").check();
    await expect(page.getByText("Local Test Card", { exact: true })).toBeVisible();
    const archivedRow = page
      .getByRole("row")
      .filter({ has: page.getByText("Local Test Card", { exact: true }) });
    await archivedRow.getByRole("button", { name: "Restore" }).click();
    await expect(page.getByText("Local Test Card", { exact: true })).toBeVisible();
  });

  test("persists the window close behavior setting", async ({ page }) => {
    await openUnlocked(page);
    await openSettingsSection(page, "Lock & idle");
    await page.getByLabel("When the window closes").selectOption("hide");
    await expect(page.getByText("Saved.")).toBeVisible();
    await page.getByRole("button", { name: "Overview" }).click();
    await openSettingsSection(page, "Lock & idle");
    await expect(page.getByLabel("When the window closes")).toHaveValue("hide");
  });

  test("offers encrypted vault creation when none exists", async ({ page }) => {
    await page.goto("/?mockVault=empty");
    await expect(page.getByText(/Create your encrypted ledger/)).toBeVisible();
    await expect(page.getByText(/no recovery if you lose it/i)).toBeVisible();
    await page.getByRole("button", { name: /Create vault/ }).click();
    await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
  });
});
