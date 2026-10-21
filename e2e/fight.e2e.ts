import type { Page } from "@playwright/test";
import { DAY0, expect, openBoard, shot, test } from "./helpers";

const IDS = ["decu", "cooker", "tekkerrss"];

/** Plays a loaded fight with the keyboard: Start on the VS screen, Next through the rounds to the K.O. */
async function playToKo(page: Page, dialog = page.getByRole("dialog")) {
  const start = dialog.getByRole("button", { name: /FIGHT!/ });
  await expect(start).toBeVisible();
  await start.focus();
  await page.keyboard.press("Enter");
  const voids: number[] = [];
  const total = 8;
  for (let n = 1; n <= total; n++) {
    await expect(dialog.getByText(`Round ${n} of ${total}`)).toBeVisible();
    await expect(dialog.getByRole("button", { name: /Next round|To the K\.O\./ })).toBeFocused();
    if ((await dialog.locator(".ds-round--void").count()) > 0) voids.push(n);
    const next = dialog.getByRole("button", { name: /Next round|To the K\.O\./ });
    await next.press("Enter");
  }
  await expect(dialog.getByRole("heading", { name: /K\.O\.!|DRAW/ })).toBeFocused();
  return voids;
}

test.describe("fight", () => {
  test("pick 3 by keyboard on day 0, VS, rounds, K.O., share link reopens the same fight", async ({ page }) => {
    await openBoard(page, DAY0);
    const search = page.getByRole("searchbox", { name: "Find a KOL" });
    await search.scrollIntoViewIfNeeded();
    for (const q of IDS) {
      await search.focus();
      await page.keyboard.type(q);
      await expect(page.getByRole("list", { name: "Matches" }).getByRole("button").first()).toContainText(new RegExp(q, "i"));
      await page.keyboard.press("Enter");
    }
    await expect(page.getByText("3 of 4 picked", { exact: true })).toBeVisible();

    const go = page.getByRole("button", { name: /FIGHT!/ });
    await go.focus();
    await page.keyboard.press("Enter");

    const dialog = page.getByRole("dialog");
    await expect(page).toHaveURL(/\/fight\?ids=decu,cooker,tekkerrss/);
    await expect(dialog.getByRole("heading", { name: "Decu vs Cooker vs Tekkerrss", level: 2 })).toBeVisible();
    await expect(dialog.getByRole("region", { name: "Fighters" })).toBeVisible();
    await shot(page, "fight-vs", 1280);
    await shot(page, "fight-vs", 360);
    await page.setViewportSize({ width: 1280, height: 900 });

    await playToKo(page, dialog);
    const ko = dialog.locator("#fight-ko-title");
    await expect(ko).toHaveText(/K\.O\.!|DRAW/);
    const winnerLine = await dialog.locator(".fight-ko__winner").innerText();
    const scoreLine = await dialog.locator(".fight-ko__score").innerText();
    await shot(page, "fight-ko", 1280);
    await shot(page, "fight-ko", 360);
    await page.setViewportSize({ width: 1280, height: 900 });

    const link = await dialog.getByLabel("Fight link").inputValue();
    expect(link).toMatch(/\/fight\?ids=decu,cooker,tekkerrss&date=\d{4}-\d{2}-\d{2}/);

    await page.goto(link);
    const again = page.getByRole("dialog");
    await expect(again.getByRole("heading", { name: "Decu vs Cooker vs Tekkerrss", level: 2 })).toBeVisible();
    await playToKo(page, again);
    await expect(again.locator(".fight-ko__winner")).toHaveText(winnerLine);
    await expect(again.locator(".fight-ko__score")).toHaveText(scoreLine);
  });

  test("day 0 fight by link: void rounds are grey, Decu wins", async ({ page }) => {
    await page.goto(`/fight?ids=${IDS.join(",")}&date=${DAY0}`);
    const dialog = page.getByRole("dialog");
    await expect(dialog.getByRole("heading", { name: "Decu vs Cooker vs Tekkerrss", level: 2 })).toBeVisible();
    // On day 0 the 24-hour and 7-day win rate rounds have no data for Tekkerrss (Solana Tracker only).
    const voids = await playToKo(page, dialog);
    await expect(dialog.locator(".fight-ko__winner")).toHaveText("Decu wins");
    if (voids.length > 0) {
      const opacity = await dialog.locator(".ds-round--void .ds-round__fighters").first().evaluate((e) => getComputedStyle(e).opacity);
      expect(Number(opacity)).toBeLessThan(1);
      await expect(dialog.locator(".ds-round--void .ds-round__status").first()).toContainText("Not counted");
    }
    // All eight rounds are listed under Rounds at the K.O.
    await expect(dialog.locator(".fight-all-rounds .ds-round")).toHaveCount(8);
  });
});
