import { test, expect } from "@playwright/test";
import { openLocked, openUnlocked } from "./helpers";

test.describe("secure shell (browser mock)", () => {
  test("starts locked with no password input", async ({ page }) => {
    await openLocked(page);
    await expect(page.getByText(/native macOS sheet/i)).toBeVisible();
    expect(await page.locator("input[type='password']").count()).toBe(0);
  });

  test("renders the app identity and honest empty state after unlock", async ({ page }) => {
    await openUnlocked(page);
    await expect(page.getByText("Mock desktop bridge")).toBeVisible();
    await expect(page.getByRole("button", { name: "Overview" })).toHaveAttribute("aria-current", "page");
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
    await expect(page.getByRole("button", { name: "By supplier" })).toHaveAttribute("aria-pressed", "true");
    await page.getByRole("button", { name: "Settings" }).click();
    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
    await expect(page.getByText(/4\.6\.1 community/).first()).toBeVisible();
    await expect(page.getByText(/Argon2id/)).toBeVisible();
  });

  test("walks the onboarding wizard with no provider and adds an account", async ({ page }) => {
    await page.goto("/?mockOnboarding=1");
    await page.getByRole("button", { name: "Unlock vault" }).click();
    await expect(page.getByText("Add your first account.")).toBeVisible();
    await page.getByLabel("Issuer").fill("Chase");
    await page.getByLabel("Display name").fill("Wizard Card");
    await page.getByLabel(/Last four/).fill("1234");
    await page.getByRole("button", { name: "Add account" }).click();
    await expect(page.getByText(/Added Wizard Card/)).toBeVisible();
    await page.getByRole("button", { name: "Continue without a provider" }).click();
    await page.getByRole("button", { name: "Finish setup" }).click();
    await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
    await page.getByRole("button", { name: "Settings" }).click();
    await expect(page.getByText("Wizard Card", { exact: true })).toBeVisible();
    await expect(page.getByText("Chase").first()).toBeVisible();
  });

  test("manages accounts in settings and archives reversibly", async ({ page }) => {
    await openUnlocked(page);
    await page.getByRole("button", { name: "Settings" }).click();
    await page.getByRole("button", { name: "Add account" }).click();
    await page.getByLabel("Issuer").fill("Bilt");
    await page.getByLabel("Display name").fill("Local Test Card");
    await page.getByLabel(/Last four/).fill("4321");
    await page.getByRole("button", { name: "Add account" }).click();
    await expect(page.getByText("Local Test Card", { exact: true })).toBeVisible();
    const row = page.getByRole("row").filter({ has: page.getByRole("cell", { name: "Local Test Card", exact: true }) });
    await row.getByRole("button", { name: "Archive" }).click();
    await row.getByRole("button", { name: "Archive", exact: true }).click();
    await expect(page.getByText("Local Test Card", { exact: true })).toHaveCount(0);
    await page.getByLabel("Show archived accounts").check();
    await expect(page.getByText("Local Test Card", { exact: true })).toBeVisible();
    const archivedRow = page
      .getByRole("row")
      .filter({ has: page.getByRole("cell", { name: "Local Test Card", exact: true }) });
    await archivedRow.getByRole("button", { name: "Restore" }).click();
    await expect(page.getByText("Local Test Card", { exact: true })).toBeVisible();
  });

  test("persists the window close behavior setting", async ({ page }) => {
    await openUnlocked(page);
    await page.getByRole("button", { name: "Settings" }).click();
    await page.getByLabel("When the window closes").selectOption("hide");
    await expect(page.getByText("Saved.")).toBeVisible();
    await page.getByRole("button", { name: "Overview" }).click();
    await page.getByRole("button", { name: "Settings" }).click();
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
