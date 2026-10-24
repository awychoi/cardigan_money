import { expect, type Page } from "@playwright/test";

export async function openLocked(page: Page): Promise<void> {
  await page.goto("/");
  await expect(page.getByText(/Your encrypted ledger is locked/)).toBeVisible();
}

export async function openUnlocked(page: Page): Promise<void> {
  await openLocked(page);
  await page.getByRole("button", { name: "Unlock vault" }).click();
  await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();
}
