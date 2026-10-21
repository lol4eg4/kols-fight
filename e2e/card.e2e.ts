import { DAY0, expect, openBoard, shot, test } from "./helpers";

test.describe("KOL card", () => {
  test("opens from the list by keyboard, traps focus, Esc returns focus", async ({ page }) => {
    await openBoard(page, DAY0);
    const opener = page.getByRole("link", { name: "Open the card of Tekkerrss" });
    await opener.focus();
    await expect(opener).toBeFocused();
    await page.keyboard.press("Enter");

    const card = page.getByRole("dialog", { name: "Tekkerrss" });
    await expect(card).toBeVisible();
    await expect(page).toHaveURL(/\/kol\/tekkerrss\?date=2026-10-18$/);
    await expect(card.getByRole("progressbar", { name: "Arena Score" }).first()).toHaveAttribute("aria-valuenow", "55");

    // Tab and Shift+Tab many times: focus never leaves the dialog.
    const inside = () => page.evaluate(() => !!document.activeElement?.closest("dialog[open]"));
    for (let i = 0; i < 25; i++) {
      await page.keyboard.press("Tab");
      expect(await inside(), `Tab ${i}`).toBe(true);
    }
    for (let i = 0; i < 25; i++) {
      await page.keyboard.press("Shift+Tab");
      expect(await inside(), `Shift+Tab ${i}`).toBe(true);
    }

    await page.keyboard.press("Escape");
    await expect(card).toBeHidden();
    await expect(opener).toBeFocused();
    expect(new URL(page.url()).pathname).toBe("/");
  });

  test("direct link opens the card; closing lands on /", async ({ page }) => {
    await page.goto(`/kol/tekkerrss?date=${DAY0}`);
    const card = page.getByRole("dialog", { name: "Tekkerrss" });
    await expect(card).toBeVisible();
    await expect(card.getByRole("progressbar", { name: "Arena Score" }).first()).toHaveAttribute("aria-valuenow", "55");
    await expect(card).toContainText(/diamond/i);
    await expect(card).toContainText("#18 of 122");
    await shot(page, "kol-card", 1280);
    await shot(page, "kol-card", 360);
    await page.setViewportSize({ width: 1280, height: 900 });

    await page.getByRole("button", { name: "Close the card" }).click();
    await expect(card).toBeHidden();
    const u = new URL(page.url());
    expect(u.pathname).toBe("/");
    expect(u.search).toBe("");
    await expect(page.locator("tr.board-tr").first()).toBeVisible();
  });
});
