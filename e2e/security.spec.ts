import { test, expect } from "@playwright/test";
import { openLocked, openUnlocked } from "./helpers";

// Browser tests prove renderer behavior only. Native Tauri navigation/new-window
// blocking, devtools absence, content protection, and CSP injection are covered
// by Rust tests, config assertions, and the native smoke checklist.
test.describe("renderer security surface", () => {
  test("exposes no dangerous HTML sinks or remote asset references", async ({ page }) => {
    const requests: string[] = [];
    page.on("request", (request) => {
      requests.push(request.resourceType() + " " + request.url());
    });
    await openUnlocked(page);
    const html = await page.content();
    expect(html).not.toMatch(/<iframe/i);
    for (const selector of [
      "script[src^='http']",
      "link[href^='http']",
      "img[src^='http']",
      "source[src^='http']",
      "video[src^='http']",
      "audio[src^='http']",
    ]) {
      expect(await page.locator(selector).count(), selector).toBe(0);
    }
    expect(requests.every((r) => /127\.0\.0\.1|localhost|data:/.test(r))).toBe(true);
  });

  test("never provides an HTML password input, locked or unlocked", async ({ page }) => {
    await openLocked(page);
    expect(await page.locator("input[type='password']").count()).toBe(0);
    await page.getByRole("button", { name: "Unlock vault" }).click();
    await expect(page.getByText("Your cards, in one quiet place.")).toBeVisible();
    expect(await page.locator("input[type='password']").count()).toBe(0);
    expect(await page.evaluate(() => document.documentElement.outerHTML)).not.toMatch(
      /type=["']password/i,
    );
  });

  test("never writes ledger-shaped keys to local storage", async ({ page }) => {
    await openUnlocked(page);
    await page.getByRole("button", { name: /appearance/i }).click();
    await page.getByRole("button", { name: "Settings" }).click();
    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
    const keys = await page.evaluate(() => Object.keys(window.localStorage));
    expect(keys).toEqual([]);
  });
});
