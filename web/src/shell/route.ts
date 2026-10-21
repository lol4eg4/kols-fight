// Route matching and the /team rules. Pure functions: no React, no window. The router in router.tsx uses them.

import type { Tag } from "./contract";
import type { ScreenEntry } from "./plugin";

export type Params = Record<string, string>;

export const HOME_PATH = "/";
export const LOGIN_PATH = "/team/login";
export const TEAM_HOME = "/team/wallets";

/** Paths that open over the home page instead of replacing it. */
export const OVERLAY_PATHS = ["/kol/:id", "/fight"] as const;

/** Anchors of the home page sections. The board puts these ids on its sections; the header links to them. */
export const ANCHORS = {
  leaderboard: "leaderboard",
  network: "network",
  fight: "fight",
  score: "how-it-works",
} as const;

export function normalizePath(path: string): string {
  const p = path.replace(/\/{2,}/g, "/");
  return p.length > 1 ? p.replace(/\/+$/, "") : p;
}

/** Matches "/kol/:id" against "/kol/decu". Returns the params, or null. */
export function matchPath(pattern: string, pathname: string): Params | null {
  const want = normalizePath(pattern).split("/");
  const got = normalizePath(pathname).split("/");
  if (want.length !== got.length) return null;
  const params: Params = {};
  for (let i = 0; i < want.length; i++) {
    const w = want[i]!;
    const g = got[i]!;
    if (w.startsWith(":")) {
      if (g === "") return null;
      try {
        params[w.slice(1)] = decodeURIComponent(g);
      } catch {
        return null; // malformed %-escape
      }
    } else if (w !== g) {
      return null;
    }
  }
  return params;
}

export function isTeamPath(pathname: string): boolean {
  const p = normalizePath(pathname);
  return p === "/team" || p.startsWith("/team/");
}

export type Role = "staff" | "admin";

/** public: anyone. staff: staff or admin. admin: admin only. */
export function tagAllows(tag: Tag, role: Role | null): boolean {
  if (tag === "public") return true;
  if (tag === "staff") return role === "staff" || role === "admin";
  return role === "admin";
}

/** A screen the login rule applies to: under /team and not public (the login screen itself is public). */
export function needsSession(screen: Pick<ScreenEntry, "path" | "tag">): boolean {
  return isTeamPath(screen.path) && screen.tag !== "public";
}

/** The ?next= value, only when it is a path inside /team. Anything else (another site, a script) gives the team home. */
export function safeNext(raw: string | null | undefined): string {
  if (!raw || /[\u0000-\u001f\\]/.test(raw)) return TEAM_HOME;
  if (raw !== "/team" && !raw.startsWith("/team/") && !raw.startsWith("/team?") && !raw.startsWith("/team#")) return TEAM_HOME;
  // Resolve it the way the browser will: "/team/../x" must not leave /team.
  const url = new URL(raw, "http://arena.invalid");
  if (url.origin !== "http://arena.invalid" || !isTeamPath(url.pathname) || normalizePath(url.pathname) === LOGIN_PATH) return TEAM_HOME;
  return raw;
}

/** Where a closed /team page sends a visitor: the login with the page to come back to. */
export function loginUrl(current: string): string {
  return `${LOGIN_PATH}?next=${encodeURIComponent(current)}`;
}

export type Resolved =
  | { kind: "redirect"; to: string }
  | { kind: "screen"; screen: ScreenEntry; params: Params }
  /** Opens over `base`, the home screen (null when the board is not built). */
  | { kind: "overlay"; base: ScreenEntry | null; overlay: ScreenEntry; params: Params }
  | { kind: "notfound"; team: boolean };

export function resolveRoute(screens: readonly ScreenEntry[], pathname: string): Resolved {
  const path = normalizePath(pathname);
  if (path === "/team") return { kind: "redirect", to: TEAM_HOME };
  for (const screen of screens) {
    const params = matchPath(screen.path, path);
    if (!params) continue;
    if ((OVERLAY_PATHS as readonly string[]).includes(screen.path)) {
      return { kind: "overlay", base: screens.find((s) => s.path === HOME_PATH) ?? null, overlay: screen, params };
    }
    return { kind: "screen", screen, params };
  }
  return { kind: "notfound", team: isTeamPath(path) };
}
