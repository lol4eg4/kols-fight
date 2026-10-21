import { expect, shot, test } from "./helpers";
import type { Page } from "@playwright/test";

async function shots(page: Page, name: string) {
  await shot(page, name, 1280);
  await shot(page, name, 360);
  await page.setViewportSize({ width: 1280, height: 900 });
}

const h1 = (page: Page, name: string) => page.getByRole("heading", { name, exact: true }).first();

test.describe("staff screens (admin)", () => {
  test("network", async ({ adminPage: page }) => {
    await page.goto("/team/network");
    await expect(h1(page, "Network")).toBeVisible();
    await expect(page.getByText(/^\d+ of \d+ KOLs$/)).toBeVisible();
    await expect(page.getByRole("heading", { name: "Tekkerrss", exact: true })).toBeVisible();
    await expect(page.getByText("Network sheet", { exact: true }).first()).toBeVisible();
    await shots(page, "team-network");
  });

  test("wallets", async ({ adminPage: page }) => {
    await page.goto("/team/wallets");
    await expect(h1(page, "Wallets")).toBeVisible();
    await expect(page.getByText("KOLs with a confirmed wallet")).toBeVisible();
    await expect(page.getByRole("heading", { name: "Tekkerrss", exact: true })).toBeVisible();
    await shots(page, "team-wallets");
  });

  test("fomo", async ({ adminPage: page }) => {
    await page.goto("/team/fomo");
    await expect(h1(page, "FOMO match")).toBeVisible();
    const kol = page.getByLabel("KOL", { exact: true });
    await expect(kol).toBeVisible();
    expect(await kol.locator("option").count()).toBeGreaterThan(10);
    await expect(page.getByRole("button", { name: "Find" })).toBeDisabled();
    await shots(page, "team-fomo");
  });

  test("picker", async ({ adminPage: page }) => {
    await page.goto("/team/picker");
    await expect(h1(page, "Picker")).toBeVisible();
    await expect(page.getByText(/^\d+ of \d+ KOLs\./)).toBeVisible();
    await expect(page.getByText("Tekkerrss", { exact: true }).first()).toBeVisible();
    await shots(page, "team-picker");
  });

  test("collection", async ({ adminPage: page }) => {
    await page.goto("/team/collection");
    await expect(h1(page, "Collection")).toBeVisible();
    await expect(page.getByRole("heading", { name: "Sources" })).toBeVisible();
    await expect(page.getByText("Solana Tracker", { exact: true }).first()).toBeVisible();
    await expect(page.getByRole("heading", { name: "Runs" })).toBeVisible();
    await expect(page.getByRole("list", { name: "Collection runs" }).getByText("Rank the board").first()).toBeVisible();
    await shots(page, "team-collection");
  });
});
