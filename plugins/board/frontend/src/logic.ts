// Pure board logic: filters, sort, small formats, source links. No React, no shell: unit-tested.

import type { BadgeKind, BoardRow, Links, Metrics, PartKind, Tier, Wallet } from "./types";

export const TIERS: Tier[] = ["goat", "diamond", "sendoor", "degen"];
export const BADGES: BadgeKind[] = ["sniper", "on_fire", "whale", "scalper", "hodl", "live", "in_top_today"];
export const BADGE_ICON: Record<BadgeKind, string> = {
  sniper: "🎯",
  on_fire: "🔥",
  whale: "🐋",
  scalper: "⚡",
  hodl: "🙌",
  live: "🟢",
  in_top_today: "📡",
};

/** Weights of the four Score parts, in formula order. */
export const PART_WEIGHT: Record<PartKind, number> = { profit: 0.4, accuracy: 0.25, form: 0.2, activity: 0.15 };

export type TabId = "all" | "top100" | "network";
export type SortId = "rank" | "d30" | "d7" | "d1" | "winrate" | "last";
export const SORTS: SortId[] = ["rank", "d30", "d7", "d1", "winrate", "last"];

export interface Filters {
  tab: TabId;
  query: string;
  tier: Tier | "all";
  badge: BadgeKind | "all";
  sort: SortId;
}

export const NO_FILTERS: Filters = { tab: "all", query: "", tier: "all", badge: "all", sort: "rank" };

/** True when anything narrows the list (the "Clear filters" button resets it; the sort is kept). */
export function isFiltered(f: Filters): boolean {
  return f.query.trim() !== "" || f.tier !== "all" || f.badge !== "all";
}

/** Win rate shown for a KOL: wr30, or the all-time rate (marked with *) when wr30 is missing. */
export function winRate(m: Metrics | null): { value: number | null; allTime: boolean } {
  if (!m) return { value: null, allTime: false };
  if (m.wr30 !== null) return { value: m.wr30, allTime: false };
  if (m.wr_all !== null) return { value: m.wr_all, allTime: true };
  return { value: null, allTime: false };
}

export function matchesQuery(row: Pick<BoardRow, "name" | "x_handle">, query: string): boolean {
  const q = query.trim().toLowerCase().replace(/^@/, "");
  if (!q) return true;
  return row.name.toLowerCase().includes(q) || (row.x_handle ?? "").toLowerCase().includes(q);
}

/** Rows of a tab. The network tab uses the ids the network_filter slot reported (none until it does). */
export function tabRows(rows: BoardRow[], tab: TabId, networkIds: ReadonlySet<string> | null): BoardRow[] {
  if (tab === "top100") return rows.filter((r) => r.top100);
  if (tab === "network") return networkIds ? rows.filter((r) => networkIds.has(r.kol)) : [];
  return rows;
}

function sortValue(row: BoardRow, sort: SortId): number | null {
  const m = row.metrics;
  switch (sort) {
    case "rank":
      return -row.rank;
    case "d30":
      return m?.d30 ?? null;
    case "d7":
      return m?.d7 ?? null;
    case "d1":
      return m?.d1 ?? null;
    case "winrate":
      return winRate(m).value;
    case "last":
      return m?.last_min_at_fetch === null || m?.last_min_at_fetch === undefined ? null : -m.last_min_at_fetch;
  }
}

/** Best first; rows without a value go last; equal values keep board order. */
export function sortRows(rows: BoardRow[], sort: SortId): BoardRow[] {
  return [...rows].sort((a, b) => {
    const va = sortValue(a, sort);
    const vb = sortValue(b, sort);
    if (va === null && vb === null) return a.rank - b.rank;
    if (va === null) return 1;
    if (vb === null) return -1;
    return vb - va || a.rank - b.rank;
  });
}

export function applyFilters(rows: BoardRow[], f: Filters, networkIds: ReadonlySet<string> | null): BoardRow[] {
  const kept = tabRows(rows, f.tab, networkIds).filter(
    (r) => matchesQuery(r, f.query) && (f.tier === "all" || r.tier === f.tier) && (f.badge === "all" || r.badges.includes(f.badge)),
  );
  return sortRows(kept, f.sort);
}

/** 0.442 -> "44%". */
export function formatPercent(v: number): string {
  return `${Math.round(v * 100)}%`;
}

/** Median hold: "15s", "4m", "2h", "3d". */
export function formatHold(seconds: number): string {
  if (seconds < 60) return `${Math.round(seconds)}s`;
  if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.round(seconds / 3600)}h`;
  return `${Math.round(seconds / 86400)}d`;
}

/** The snapshot shown is the latest and closed more than a day ago: "Today's update is late". */
export function isLate(closedAt: string | null, isLatest: boolean, now: number = Date.now()): boolean {
  if (!isLatest || !closedAt) return false;
  const t = Date.parse(closedAt);
  return Number.isFinite(t) && now - t > 24 * 3600 * 1000;
}

/** Card address. The date stays in the address only when it is not the latest snapshot. */
export function cardPath(kol: string, date: string | null): string {
  const base = `/kol/${encodeURIComponent(kol)}`;
  return date ? `${base}?date=${encodeURIComponent(date)}` : base;
}

/** A valid "YYYY-MM-DD" from the address, or null. */
export function dateParam(raw: string | null): string | null {
  return raw && /^\d{4}-\d{2}-\d{2}$/.test(raw) ? raw : null;
}

export type LinkId = "x" | "kolexplorer" | "solanatracker" | "kolscan" | "fomo" | "pumpfun" | "telegram";

/** Outbound source links of a KOL. Handles come validated from the server; they are still encoded here. */
export function sourceLinks(xHandle: string | null, links: Links, wallets: Wallet[]): { id: LinkId; url: string }[] {
  const e = encodeURIComponent;
  const main = wallets.find((w) => w.main)?.address ?? wallets[0]?.address ?? null;
  const out: { id: LinkId; url: string }[] = [];
  if (xHandle) out.push({ id: "x", url: `https://x.com/${e(xHandle)}` });
  if (links.kolexplorer) out.push({ id: "kolexplorer", url: `https://kolexplorer.com/kol/${e(links.kolexplorer)}` });
  if (links.solanatracker && main) out.push({ id: "solanatracker", url: `https://www.solanatracker.io/wallet/${e(main)}` });
  if (links.kolscan && main) out.push({ id: "kolscan", url: `https://kolscan.io/account/${e(main)}` });
  if (links.fomo) out.push({ id: "fomo", url: `https://fomo.family/profile/${e(links.fomo)}` });
  if (links.pumpfun) out.push({ id: "pumpfun", url: `https://pump.fun/profile/${e(links.pumpfun)}` });
  if (links.telegram) out.push({ id: "telegram", url: `https://t.me/${e(links.telegram)}` });
  return out;
}

/** Solscan link from the server, kept only when it is the Solscan account page. */
export function safeSolscan(url: string): string | null {
  return /^https:\/\/solscan\.io\/account\/[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(url) ? url : null;
}
