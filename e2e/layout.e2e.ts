import { DAY0, expect, noHorizontalScroll, openBoard, test } from "./helpers";

test.describe("360 px wide", () => {
  test.use({ viewport: { width: 360, height: 740 } });

  test("home: no horizontal scroll", async ({ page }) => {
    await openBoard(page);
    // On a phone the board is a list of cards, not the table.
    await expect(page.locator("li.board-item").first()).toBeVisible();
    await expect(page.locator("table.board-table")).toHaveCount(0);
    await noHorizontalScroll(page);
  });

  test("home on day 0: no horizontal scroll", async ({ page }) => {
    await openBoard(page, DAY0);
    await noHorizontalScroll(page);
  });

  test("KOL card: no horizontal scroll", async ({ page }) => {
    await page.goto("/kol/tekkerrss");
    await expect(page.getByRole("dialog", { name: "Tekkerrss" }).getByRole("progressbar", { name: "Arena Score" }).first()).toBeVisible();
    await noHorizontalScroll(page);
    const d = await page.getByRole("dialog").evaluate((e) => ({ s: e.scrollWidth, c: e.clientWidth }));
    expect(d.s, "dialog scrollWidth").toBeLessThanOrEqual(d.c);
  });

  test("fight: no horizontal scroll", async ({ page }) => {
    await page.goto("/fight?ids=decu,cooker");
    const dialog = page.getByRole("dialog");
    await expect(dialog.getByRole("button", { name: /FIGHT!/ })).toBeVisible();
    await noHorizontalScroll(page);
    const d = await dialog.evaluate((e) => ({ s: e.scrollWidth, c: e.clientWidth }));
    expect(d.s, "dialog scrollWidth").toBeLessThanOrEqual(d.c);
  });

  test("team wallets: no horizontal scroll", async ({ adminPage: page }) => {
    await page.goto("/team/wallets");
    await expect(page.getByRole("heading", { name: "Wallets", exact: true }).first()).toBeVisible();
    await expect(page.getByText("KOLs with a confirmed wallet")).toBeVisible();
    await noHorizontalScroll(page);
  });
});
