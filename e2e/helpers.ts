import { test as base, expect, request as pwRequest } from "@playwright/test";
import type { APIRequestContext, BrowserContext, Page } from "@playwright/test";
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const DAY0 = "2026-10-18";
export const SHOTS = join(dirname(fileURLToPath(import.meta.url)), "..", "tmp", "e2e");
mkdirSync(SHOTS, { recursive: true });

// Login lockout is per IP, read from X-Forwarded-For. Every context gets its own address.
const run = [1 + Math.floor(Math.random() * 253), 1 + Math.floor(Math.random() * 253)];
let seq = 0;
export function freshIp(): string {
  seq += 1;
  return `10.${run[0]}.${run[1]}.${seq % 250 || 250}`;
}

export function adminCreds(): { login: string; password: string } {
  const login = process.env.ARENA_ADMIN_LOGIN;
  const password = process.env.ARENA_ADMIN_PASSWORD;
  if (!login || !password) throw new Error("Set ARENA_ADMIN_LOGIN and ARENA_ADMIN_PASSWORD");
  return { login, password };
}

function origin(): string {
  const url = process.env.ARENA_URL;
  if (!url) throw new Error("Set ARENA_URL");
  return new URL(url).origin;
}

/** A request client with its own X-Forwarded-For and the site Origin. */
export async function apiClient(ip = freshIp()): Promise<APIRequestContext> {
  return pwRequest.newContext({ baseURL: process.env.ARENA_URL, extraHTTPHeaders: { "X-Forwarded-For": ip, Origin: origin() } });
}

export async function signIn(c: APIRequestContext, login: string, password: string) {
  const r = await c.post("/api/auth/login", { data: { login, password } });
  expect(r.status(), "login status").toBe(200);
}

export async function adminApi(): Promise<APIRequestContext> {
  const c = await apiClient();
  const { login, password } = adminCreds();
  await signIn(c, login, password);
  return c;
}

/** Browser context that is signed in, sending its own X-Forwarded-For. */
export async function loginContext(browser: import("@playwright/test").Browser, login: string, password: string, extra: Record<string, unknown> = {}): Promise<BrowserContext> {
  const ctx = await browser.newContext({ baseURL: process.env.ARENA_URL, extraHTTPHeaders: { "X-Forwarded-For": freshIp() }, ...extra });
  const r = await ctx.request.post("/api/auth/login", { data: { login, password }, headers: { Origin: origin() } });
  expect(r.status(), "login status").toBe(200);
  return ctx;
}

/** Collects console and page errors of every page of one test. */
export class Guard {
  problems: string[] = [];
  private allowed: string[] = [];
  /** Failed loads of these URL paths are expected (for example a 401 before sign-in). */
  allowFailedLoad(...paths: string[]) {
    this.allowed.push(...paths);
  }
  attach(page: Page) {
    page.on("pageerror", (e) => this.problems.push(`pageerror: ${e.message}`));
    page.on("console", (m) => {
      if (m.type() !== "error") return;
      let path = "";
      try {
        path = new URL(m.location().url).pathname;
      } catch {
        // no location
      }
      if (m.text().startsWith("Failed to load resource") && this.allowed.includes(path)) return;
      this.problems.push(`console.error: ${m.text()} (${m.location().url})`);
    });
  }
}

/**
 * `page` fails the test on any console error or page error. Each test has its own X-Forwarded-For.
 * `adminPage` is a page of a context signed in as the admin (own address too).
 */
export const test = base.extend<{ guard: Guard; adminPage: Page }>({
  guard: async ({}, use) => {
    const g = new Guard();
    await use(g);
    expect(g.problems, "console and page errors").toEqual([]);
  },
  context: async ({ browser }, use) => {
    const ctx = await browser.newContext({ extraHTTPHeaders: { "X-Forwarded-For": freshIp() } });
    await use(ctx);
    await ctx.close();
  },
  page: async ({ context, guard }, use) => {
    const page = await context.newPage();
    guard.attach(page);
    await use(page);
  },
  adminPage: async ({ browser, guard }, use) => {
    const { login, password } = adminCreds();
    const ctx = await loginContext(browser, login, password);
    const page = await ctx.newPage();
    guard.attach(page);
    await use(page);
    await ctx.close();
  },
});

export { expect };

/** Stable page: wait for the board numbers, not for network idle. */
export async function openBoard(page: Page, date?: string) {
  await page.goto(date ? `/?date=${date}` : "/");
  await expect(page.getByRole("heading", { name: "Leaderboard", level: 2 })).toBeVisible();
  await expect(page.locator("tr.board-tr, li.board-item").first()).toBeVisible();
}

export async function shot(page: Page, name: string, width: number) {
  await page.setViewportSize({ width, height: width === 360 ? 740 : 900 });
  await page.waitForTimeout(400);
  await page.screenshot({ path: join(SHOTS, `${name}-${width}.png`), fullPage: true });
}

export async function noHorizontalScroll(page: Page) {
  const r = await page.evaluate(() => ({ s: document.scrollingElement!.scrollWidth, c: document.scrollingElement!.clientWidth }));
  expect(r.s, `scrollWidth ${r.s} vs clientWidth ${r.c}`).toBeLessThanOrEqual(r.c);
}
