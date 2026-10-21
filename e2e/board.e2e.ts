import { DAY0, expect, openBoard, shot, test } from "./helpers";

test.describe("board", () => {
  test("day 0: 122 rows, podium, search, tier filter, network", async ({ page }) => {
    await openBoard(page, DAY0);
    const rows = page.locator("tr.board-tr");
    await expect(rows).toHaveCount(122);

    const podium = page.locator(".board-podium__place");
    await expect(podium).toHaveCount(3);
    await expect(podium.locator(".board-podium__name")).toHaveText(["Decu", "Cooker", "Trunoest"]);

    await page.getByRole("searchbox", { name: "Search" }).fill("tekk");
    await expect(rows).toHaveCount(1);
    await expect(rows.first()).toContainText("Tekkerrss");
    await page.getByRole("searchbox", { name: "Search" }).fill("");
    await expect(rows).toHaveCount(122);

    const tiers = page.getByRole("group", { name: "Tier" });
    await tiers.getByRole("button", { name: "GOAT" }).click();
    await expect(rows).toHaveCount(10);
    await tiers.getByRole("button", { name: "All tiers" }).click();
    await expect(rows).toHaveCount(122);

    await page.getByRole("tab", { name: /Our network/ }).click();
    const board = (await (await page.request.get(`/api/board?date=${DAY0}`)).json()) as { rows: { kol: string; in_network: boolean }[] };
    const expected = board.rows.filter((r) => r.in_network).map((r) => r.kol);
    expect(expected.length).toBeGreaterThan(0);
    await expect(rows).toHaveCount(expected.length);
    const shown = await rows.evaluateAll((els) => els.map((e) => e.getAttribute("data-kol")));
    expect(shown.sort()).toEqual([...expected].sort());
    await shot(page, "home", 1280);
    await shot(page, "home", 360);
  });

  test("today: live data renders rows", async ({ page }) => {
    await openBoard(page);
    expect(await page.locator("tr.board-tr").count()).toBeGreaterThan(100);
    await expect(page.getByRole("heading", { name: "KOL ARENA" })).toBeVisible();
    await expect(page.locator(".board-podium__place")).toHaveCount(3);
  });
});
