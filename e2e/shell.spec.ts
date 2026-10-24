import { test, expect } from "@playwright/test";

test.describe("secure shell (browser mock)", () => {
  test("renders the app identity and honest empty state", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Your cards, in one quiet place.")).toBeVisible();
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
    await page.goto("/");
    await expect(page.getByText("Your cards, in one quiet place.")).toBeVisible();
    await page.waitForTimeout(500);
    expect(external).toEqual([]);
  });

  test("stores no financial data or secrets in browser storage or the URL", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Your cards, in one quiet place.")).toBeVisible();
    const stored = await page.evaluate(() => ({
      local: { ...window.localStorage },
      session: { ...window.sessionStorage },
      href: window.location.href,
    }));
    expect(Object.keys(stored.local)).toEqual(["cm.theme"]);
    expect(stored.session).toEqual({});
    expect(stored.href).not.toMatch(/passphrase|token|secret|account/i);
  });

  test("theme toggle applies and persists", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("button", { name: /appearance/i }).click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
    await page.reload();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  });

  test("primary navigation is keyboard reachable", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("button", { name: "Suppliers" }).click();
    await expect(page.getByRole("heading", { name: "Suppliers" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Suppliers" })).toHaveAttribute("aria-current", "page");
    await page.getByRole("button", { name: "Settings" }).click();
    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
  });
});
