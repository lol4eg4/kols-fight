import { DAY0, expect, openBoard, shot, test } from "./helpers";

test.describe("board", () => {
  test("day 0: 20 rows and Show more, podium, search, tier filter, network", async ({ page }) => {
    await openBoard(page, DAY0);
    const rows = page.locator("tr.board-tr");
    const more = page.getByRole("button", { name: /^Show more/ });
    await expect(rows).toHaveCount(20);
    await expect(more).toHaveText("Show more · 20 of 122");
    await more.click();
    await expect(rows).toHaveCount(40);
    await expect(more).toHaveText("Show more · 40 of 122");
    for (let i = 0; i < 5; i++) await more.click();
    await expect(rows).toHaveCount(122);
    await expect(more).toHaveCount(0);

    const podium = page.locator(".board-podium__place");
    await expect(podium).toHaveCount(3);
    await expect(podium.locator(".board-podium__name")).toHaveText(["Decu", "Cooker", "Trunoest"]);

    await page.getByRole("searchbox", { name: "Search" }).fill("tekk");
    await expect(rows).toHaveCount(1);
    await expect(rows.first()).toContainText("Tekkerrss");
    await page.getByRole("searchbox", { name: "Search" }).fill("");
    await expect(rows).toHaveCount(20);

    const tiers = page.getByRole("group", { name: "Tier" });
    await tiers.getByRole("button", { name: "GOAT" }).click();
    await expect(rows).toHaveCount(10);
    await tiers.getByRole("button", { name: "All tiers" }).click();
    await expect(rows).toHaveCount(20);

    await page.getByRole("tab", { name: /Our network/ }).click();
    const board = (await (await page.request.get(`/api/board?date=${DAY0}`)).json()) as { rows: { kol: string; in_network: boolean }[] };
    const expected = board.rows.filter((r) => r.in_network).map((r) => r.kol);
    expect(expected.length).toBeGreaterThan(0);
    while (await more.count()) await more.click();
    await expect(rows).toHaveCount(expected.length);
    const shown = await rows.evaluateAll((els) => els.map((e) => e.getAttribute("data-kol")));
    expect(shown.sort()).toEqual([...expected].sort());
    await shot(page, "home", 1280);
    await shot(page, "home", 360);
  });

  test("today: live data renders rows", async ({ page }) => {
    await openBoard(page);
    await expect(page.locator("tr.board-tr")).toHaveCount(20);
    await expect(page.getByRole("button", { name: /^Show more · 20 of \d{3}$/ })).toBeVisible();
    await expect(page.getByRole("heading", { name: "KOL ARENA" })).toBeVisible();
    await expect(page.locator(".board-podium__place")).toHaveCount(3);
  });
});
