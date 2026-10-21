import { randomBytes } from "node:crypto";
import type { APIRequestContext, Page } from "@playwright/test";
import { adminApi, apiClient, expect, freshIp, loginContext, test } from "./helpers";

// One staff account of our own for the whole file; revoked at the end.
const staff = { login: `e2e_${Date.now().toString(36)}_${randomBytes(2).toString("hex")}`, password: randomBytes(12).toString("hex"), id: 0 };
let admin: APIRequestContext;

test.beforeAll(async () => {
  admin = await adminApi();
  const r = await admin.post("/api/admin/staff", { data: { login: staff.login, role: "staff", password: staff.password } });
  expect(r.status(), "create staff").toBe(201);
  staff.id = ((await r.json()) as { id: number }).id;
});

test.afterAll(async () => {
  if (staff.id) await admin.post(`/api/admin/staff/${staff.id}/revoke`);
  await admin?.dispose();
});

async function fillLogin(page: Page, login: string, password: string) {
  await page.getByLabel("Login").fill(login);
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Sign in" }).click();
}

test.describe("team sign-in", () => {
  test("login works and logout works", async ({ page, guard }) => {
    guard.allowFailedLoad("/api/auth/me");
    await page.goto("/team/wallets");
    await expect(page).toHaveURL(/\/team\/login\?next=/);
    await fillLogin(page, staff.login, staff.password);
    await expect(page).toHaveURL(/\/team\/wallets$/);
    await expect(page.getByText(`Signed in as ${staff.login}`)).toBeVisible();
    await expect(page.getByRole("heading", { name: "Wallets", exact: true }).first()).toBeVisible();

    await page.getByRole("button", { name: "Sign out" }).click();
    await expect(page).toHaveURL(/\/team\/login/);
    await expect(page.getByRole("button", { name: "Sign in" })).toBeVisible();
    const me = await page.request.get("/api/auth/me");
    expect(me.status()).toBe(401);
    await page.goto("/team/wallets");
    await expect(page).toHaveURL(/\/team\/login\?next=/);
  });

  test("a wrong password shows the error", async ({ page, guard }) => {
    guard.allowFailedLoad("/api/auth/me", "/api/auth/login");
    await page.goto("/team/login");
    await fillLogin(page, staff.login, "not-the-password-1");
    await expect(page.getByText("Wrong login or password.")).toBeVisible();
    await expect(page).toHaveURL(/\/team\/login/);
  });

  test("the sixth try after five failures is refused with the lock message", async ({ page, guard }) => {
    guard.allowFailedLoad("/api/auth/me", "/api/auth/login");
    await page.goto("/team/login");
    for (let i = 1; i <= 5; i++) {
      await fillLogin(page, staff.login, `wrong-password-${i}`);
      await expect(page.getByText(/Wrong login or password\.|Too many tries/)).toBeVisible();
      await page.getByLabel("Password").fill("");
    }
    // Even the right password is refused from the locked address.
    await fillLogin(page, staff.login, staff.password);
    await expect(page.getByText(/Too many tries\. Try again in \d+ minutes?\./)).toBeVisible();
    await expect(page).toHaveURL(/\/team\/login/);
    const api = await page.request.post("/api/auth/login", { data: { login: staff.login, password: staff.password } });
    expect(api.status()).toBe(429);

    // Another address is not locked.
    const other = await apiClient(freshIp());
    const ok = await other.post("/api/auth/login", { data: { login: staff.login, password: staff.password } });
    expect(ok.status()).toBe(200);
    await other.dispose();
  });

  test("a staff account cannot open /team/staff", async ({ browser }) => {
    const ctx = await loginContext(browser, staff.login, staff.password);
    const page = await ctx.newPage();
    const errors: string[] = [];
    page.on("pageerror", (e) => errors.push(e.message));
    page.on("console", (m) => m.type() === "error" && !m.text().startsWith("Failed to load resource") && errors.push(m.text()));
    await page.goto("/team/staff");
    await expect(page.getByText("Your role cannot open this page.")).toBeVisible();
    await expect(page.getByRole("heading", { name: "Staff", exact: true })).toHaveCount(0);
    await page.goto("/team/wallets");
    const menu = page.getByRole("navigation", { name: "Team sections" });
    await expect(menu.getByRole("link", { name: "Wallets" })).toBeVisible();
    await expect(menu.getByRole("link", { name: "Staff" })).toHaveCount(0);

    const r = await ctx.request.get("/api/admin/staff");
    expect(r.status()).toBe(403);
    const w = await ctx.request.post("/api/admin/staff", { data: { login: "e2e_nope_x", role: "staff", password: "0123456789abcdef" }, headers: { Origin: new URL(page.url()).origin } });
    expect(w.status()).toBe(403);
    expect(errors).toEqual([]);
    await ctx.close();
  });

  test("the admin can open /team/staff and sees the staff account", async ({ adminPage: page }) => {
    await page.goto("/team/staff");
    await expect(page.getByRole("heading", { name: "Staff", exact: true }).first()).toBeVisible();
    await expect(page.getByRole("list", { name: "Staff members" }).getByText(staff.login)).toBeVisible();
  });
});
