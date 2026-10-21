import type { Page } from "@playwright/test";
import { expect, openBoard, test } from "./helpers";

/** Animations that are still running (finite ones that just ended do not count). */
async function running(page: Page) {
  await page.waitForTimeout(600);
  return page.evaluate(() =>
    document
      .getAnimations()
      .filter((a) => a.playState === "running")
      .map((a) => `${(a as CSSAnimation).animationName ?? (a as CSSTransition).transitionProperty ?? "?"} on ${(a.effect as KeyframeEffect | null)?.target?.className ?? "?"}`),
  );
}

const trackAnimation = (page: Page) => page.locator(".ds-ticker__track").first().evaluate((e) => getComputedStyle(e).animationName);

test.describe("reduced motion", () => {
  test.use({ reducedMotion: "reduce" });

  test("board: nothing animates, ticker is static", async ({ page }) => {
    await openBoard(page);
    await expect(page.locator(".ds-ticker__item").first()).toBeVisible();
    expect(await trackAnimation(page)).toBe("none");
    expect(await running(page)).toEqual([]);
  });

  test("fight: nothing animates, rounds do not advance by themselves", async ({ page }) => {
    await page.goto("/fight?ids=decu,cooker,tekkerrss");
    const dialog = page.getByRole("dialog");
    await dialog.getByRole("button", { name: /FIGHT!/ }).click();
    await expect(dialog.getByText("Round 1 of 8")).toBeVisible();
    await expect(dialog.getByRole("button", { name: /Auto|Pause/ })).toHaveCount(0);
    expect(await running(page)).toEqual([]);
    await page.waitForTimeout(3500); // longer than one round (2.8 s)
    await expect(dialog.getByText("Round 1 of 8")).toBeVisible();
    await dialog.getByRole("button", { name: /Next round/ }).click();
    await expect(dialog.getByText("Round 2 of 8")).toBeVisible();
    expect(await running(page)).toEqual([]);
  });
});

test.describe("normal motion (control)", () => {
  test("the ticker moves", async ({ page }) => {
    await openBoard(page);
    await expect(page.locator(".ds-ticker__item").first()).toBeVisible();
    expect(await trackAnimation(page)).toBe("ds-ticker");
  });
});
