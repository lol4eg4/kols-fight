import { expect, test } from "./helpers";

// Regression: ISSUE-001 — a typo in a number filter replaced the whole list with a server error
// Found by /qa on 2026-10-21
// Report: .gstack/qa-reports/qa-report-127-0-0-1-2026-10-21.md
test("a bad number in a picker filter marks the field and keeps the list", async ({ adminPage: page }) => {
  const refused: string[] = [];
  page.on("response", (r) => r.url().includes("/api/team/picker") && r.status() >= 400 && refused.push(r.url()));
  await page.goto("/team/picker");
  const shown = page.getByRole("status").filter({ hasText: /\d+ of \d+ KOLs/ });
  await expect(shown).toBeVisible();
  const all = (await shown.innerText()).match(/of (\d+) KOLs/)![1];

  const from = page.getByLabel("Score from");
  await from.fill("5a");
  await expect(page.getByText("Enter a number, like 50.")).toBeVisible();
  await expect(from).toHaveAttribute("aria-invalid", "true");
  await expect(shown).toContainText(`${all} of ${all} KOLs`);
  await expect(page.getByText(/Could not load the network/)).toHaveCount(0);

  // A rate written with $ and a comma is a number.
  await page.getByLabel("Rate from, $").fill("$1,500");
  await expect(page.getByText("Enter a number, like 50.")).toHaveCount(1);

  await from.fill("50");
  await expect(page.getByText("Enter a number, like 50.")).toHaveCount(0);
  await expect(from).not.toHaveAttribute("aria-invalid", "true");
  expect(refused).toEqual([]);
});
