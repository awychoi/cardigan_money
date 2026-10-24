import { test, expect } from "@playwright/test";

// Browser tests prove renderer behavior only. Native Tauri navigation/new-window
// blocking, devtools absence, content protection, and CSP injection are covered
// by Rust tests, config assertions, and the native smoke checklist.
test.describe("renderer security surface", () => {
  test("exposes no dangerous HTML sinks or remote asset references", async ({ page }) => {
    const requests: string[] = [];
    page.on("request", (request) => {
      requests.push(request.resourceType() + " " + request.url());
    });
    await page.goto("/");
    await expect(page.getByText("Your cards, in one quiet place.")).toBeVisible();
    const html = await page.content();
    expect(html).not.toMatch(/<iframe/i);
    // Absolute URLs inside CSS comments (e.g. the Tailwind banner) do not cause
    // requests. Assert no markup element can load a remote resource, and that
    // the browser actually issued only loopback/data requests.
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

  test("does not expose a password input for the ledger", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Your cards, in one quiet place.")).toBeVisible();
    expect(await page.locator("input[type='password']").count()).toBe(0);
  });

  test("never writes ledger-shaped keys to local storage", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Your cards, in one quiet place.")).toBeVisible();
    await page.getByRole("button", { name: /appearance/i }).click();
    const keys = await page.evaluate(() => Object.keys(window.localStorage));
    expect(keys.every((key) => key === "cm.theme")).toBe(true);
  });
});
