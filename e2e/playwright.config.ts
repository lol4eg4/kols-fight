import { defineConfig, devices } from "@playwright/test";

const baseURL = process.env.ARENA_URL;
if (!baseURL) throw new Error("Set ARENA_URL to the running site, for example http://127.0.0.1:21252");

export default defineConfig({
  testDir: ".",
  testMatch: "**/*.e2e.ts",
  workers: 1,
  fullyParallel: false,
  retries: 0,
  timeout: 60_000,
  expect: { timeout: 10_000 },
  reporter: [["list"]],
  outputDir: "../tmp/e2e/results",
  use: { baseURL, headless: true, ...devices["Desktop Chrome"], viewport: { width: 1280, height: 900 } },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});
