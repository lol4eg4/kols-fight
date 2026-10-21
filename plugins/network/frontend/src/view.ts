// Types of GET /api/network and the pure logic of the two fillers.

export type Tier = "goat" | "diamond" | "sendoor" | "degen";

export interface OnBoard {
  kol: string;
  name: string;
  rank: number | null;
  score: number | null;
  tier: Tier | null;
  pnl_30d: number | null;
  volume_30d: number | null;
  days_in_market: number | null;
}

export interface Waiting {
  kol: string;
  name: string;
  reason: "wallet_not_confirmed" | "quiet";
  quiet_days: number | null;
}

export interface NetworkView {
  date: string;
  members: OnBoard[];
  waiting: Waiting[];
}

/** Which sentence the waiting list shows for a KOL. */
export type WaitText = "not_confirmed" | "quiet_days" | "quiet_one_day" | "quiet";

export function waitText(w: Waiting): WaitText {
  if (w.reason !== "quiet") return "not_confirmed";
  if (w.quiet_days === null || w.quiet_days < 1) return "quiet";
  return w.quiet_days === 1 ? "quiet_one_day" : "quiet_days";
}

/** Ids of the network KOLs on the board, for the leaderboard's «Our network» tab. */
export function networkIds(v: NetworkView): string[] {
  return v.members.map((m) => m.kol);
}

/** The section's URL for a board date. */
export function networkPath(date: string): string {
  return date ? `/api/network?date=${encodeURIComponent(date)}` : "/api/network";
}

/**
 * One load per key while it is fresh, shared by every caller: both fillers of a board
 * page read the same section once. A failed load is not kept, so a retry loads again.
 */
export function createCache<T>(load: (key: string) => Promise<{ data?: T; error?: unknown }>, ttlMs: number, now: () => number = Date.now) {
  const entries = new Map<string, { at: number; promise: Promise<{ data?: T; error?: unknown }> }>();
  return {
    get(key: string): Promise<{ data?: T; error?: unknown }> {
      const hit = entries.get(key);
      if (hit && now() - hit.at < ttlMs) return hit.promise;
      const promise = load(key).then((r) => {
        if (r.error !== undefined && entries.get(key)?.promise === promise) entries.delete(key);
        return r;
      });
      entries.set(key, { at: now(), promise });
      return promise;
    },
    drop(key?: string) {
      if (key === undefined) entries.clear();
      else entries.delete(key);
    },
  };
}
