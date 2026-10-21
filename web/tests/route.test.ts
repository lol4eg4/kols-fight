import { describe, expect, test } from "bun:test";
import { ANCHORS, LOGIN_PATH, TEAM_HOME, isTeamPath, loginUrl, matchPath, needsSession, normalizePath, resolveRoute, safeNext, tagAllows } from "../src/shell/route";
import type { ScreenEntry } from "../src/shell/plugin";

const screen = (key: string, path: string, tag: ScreenEntry["tag"]): ScreenEntry => ({ key, plugin: key.split("/")[0]!, name: key.split("/")[1]!, path, tag, load: null });
const screens: ScreenEntry[] = [
  screen("board/home", "/", "public"),
  screen("board/kol_card", "/kol/:id", "public"),
  screen("fight/fight_window", "/fight", "public"),
  screen("auth/login", "/team/login", "public"),
  screen("auth/staff", "/team/staff", "admin"),
  screen("wallets/wallets", "/team/wallets", "staff"),
  screen("shell/design", "/team/design", "staff"),
];

describe("matchPath", () => {
  test("static and parameter segments", () => {
    expect(matchPath("/", "/")).toEqual({});
    expect(matchPath("/fight", "/fight")).toEqual({});
    expect(matchPath("/kol/:id", "/kol/decu")).toEqual({ id: "decu" });
    expect(matchPath("/kol/:id", "/kol/mrpunk.eth")).toEqual({ id: "mrpunk.eth" });
    expect(matchPath("/kol/:id", "/kol/a%20b")).toEqual({ id: "a b" });
  });
  test("no match", () => {
    expect(matchPath("/kol/:id", "/kol")).toBeNull();
    expect(matchPath("/kol/:id", "/kol/")).toBeNull();
    expect(matchPath("/kol/:id", "/kol/a/b")).toBeNull();
    expect(matchPath("/fight", "/fights")).toBeNull();
    expect(matchPath("/kol/:id", "/kol/%E0%A4%A")).toBeNull(); // malformed escape
  });
  test("a trailing slash does not matter", () => {
    expect(matchPath("/fight", "/fight/")).toEqual({});
    expect(normalizePath("/team/wallets/")).toBe("/team/wallets");
    expect(normalizePath("//team///x")).toBe("/team/x");
    expect(normalizePath("/")).toBe("/");
  });
});

describe("resolveRoute", () => {
  test("home is a screen", () => {
    expect(resolveRoute(screens, "/")).toMatchObject({ kind: "screen", screen: { key: "board/home" } });
  });
  test("/kol/:id and /fight are overlays over the home screen", () => {
    const card = resolveRoute(screens, "/kol/decu");
    expect(card).toMatchObject({ kind: "overlay", base: { key: "board/home" }, overlay: { key: "board/kol_card" }, params: { id: "decu" } });
    expect(resolveRoute(screens, "/fight")).toMatchObject({ kind: "overlay", base: { key: "board/home" }, overlay: { key: "fight/fight_window" } });
  });
  test("an overlay with no home screen has no base", () => {
    expect(resolveRoute(screens.filter((s) => s.path !== "/"), "/fight")).toMatchObject({ kind: "overlay", base: null });
  });
  test("/team goes to the team home", () => {
    expect(resolveRoute(screens, "/team")).toEqual({ kind: "redirect", to: TEAM_HOME });
    expect(resolveRoute(screens, "/team/")).toEqual({ kind: "redirect", to: TEAM_HOME });
  });
  test("team screens, the gallery, and unknown paths", () => {
    expect(resolveRoute(screens, "/team/wallets")).toMatchObject({ kind: "screen", screen: { key: "wallets/wallets" } });
    expect(resolveRoute(screens, "/team/design")).toMatchObject({ kind: "screen", screen: { key: "shell/design" } });
    expect(resolveRoute(screens, "/nowhere")).toEqual({ kind: "notfound", team: false });
    expect(resolveRoute(screens, "/team/nowhere")).toEqual({ kind: "notfound", team: true });
  });
});

describe("the /team rules", () => {
  test("which screens need a session: closed team screens, not the login", () => {
    const by = (key: string) => screens.find((s) => s.key === key)!;
    expect(needsSession(by("auth/login"))).toBe(false);
    expect(needsSession(by("board/home"))).toBe(false);
    expect(needsSession(by("wallets/wallets"))).toBe(true);
    expect(needsSession(by("auth/staff"))).toBe(true);
    expect(needsSession(by("shell/design"))).toBe(true);
  });

  test("tag against role", () => {
    expect(tagAllows("public", null)).toBe(true);
    expect(tagAllows("staff", null)).toBe(false);
    expect(tagAllows("staff", "staff")).toBe(true);
    expect(tagAllows("staff", "admin")).toBe(true);
    expect(tagAllows("admin", "staff")).toBe(false);
    expect(tagAllows("admin", "admin")).toBe(true);
    expect(tagAllows("admin", null)).toBe(false);
  });

  test("login redirect carries the page to come back to", () => {
    expect(loginUrl("/team/wallets")).toBe("/team/login?next=%2Fteam%2Fwallets");
    expect(loginUrl("/team/network?tab=x")).toBe(`${LOGIN_PATH}?next=${encodeURIComponent("/team/network?tab=x")}`);
  });

  test("safeNext keeps only paths inside /team", () => {
    expect(safeNext("/team/wallets")).toBe("/team/wallets");
    expect(safeNext("/team/network?tab=x#y")).toBe("/team/network?tab=x#y");
    expect(safeNext("/team")).toBe("/team");
    for (const bad of [null, undefined, "", "/", "/kol/decu", "//evil.example/team", "https://evil.example/team/x", "/teamx", "/team/../../etc", "/team/%2e%2e/kol/x", "javascript:alert(1)", "/team\\evil", "/team/x\n", LOGIN_PATH, `${LOGIN_PATH}?next=/team/x`]) {
      expect(safeNext(bad)).toBe(TEAM_HOME);
    }
  });

  test("isTeamPath", () => {
    expect(isTeamPath("/team")).toBe(true);
    expect(isTeamPath("/team/x")).toBe(true);
    expect(isTeamPath("/teams")).toBe(false);
    expect(isTeamPath("/")).toBe(false);
  });
});

test("anchors of the home sections", () => {
  expect(ANCHORS).toEqual({ leaderboard: "leaderboard", network: "network", fight: "fight", score: "how-it-works" });
});
