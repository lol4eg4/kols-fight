import { describe, expect, test } from "bun:test";
import {
  NO_FILTERS,
  applyFilters,
  cardPath,
  dateParam,
  formatHold,
  formatPercent,
  isFiltered,
  isLate,
  matchesQuery,
  safeSolscan,
  sortRows,
  sourceLinks,
  tabRows,
  winRate,
} from "../src/logic";
import type { BoardRow, Metrics } from "../src/types";

const NO_METRICS: Metrics = {
  d1: null,
  d7: null,
  d30: null,
  wr7: null,
  wr30: null,
  wr_all: null,
  avg_pos: null,
  hold_s: null,
  last_min_at_fetch: null,
  unreal: null,
  total: null,
  vol30: null,
  days30: null,
};

function row(rank: number, patch: Partial<BoardRow> = {}, m: Partial<Metrics> | null = {}): BoardRow {
  return {
    kol: `k${rank}`,
    name: `Kol ${rank}`,
    x_handle: `handle${rank}`,
    rank,
    score: 100 - rank,
    tier: "degen",
    badges: [],
    top100: rank <= 100,
    in_network: false,
    source: "kolexplorer",
    fetched_at: "2026-10-18T06:00:00Z",
    stale: false,
    metrics: m === null ? null : { ...NO_METRICS, ...m },
    ...patch,
  };
}

const ADDR = "4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T";

describe("filters", () => {
  const rows = [
    row(1, { name: "Decu", x_handle: "decu0x", tier: "goat", badges: ["sniper"] }),
    row(2, { name: "Cooker", x_handle: "CookerFlips", tier: "diamond" }),
    row(3, { name: "Trunoest", x_handle: null, tier: "diamond", badges: ["sniper", "live"] }),
    row(101, { name: "Late", top100: false }),
  ];

  test("isFiltered ignores the sort and the tab", () => {
    expect(isFiltered(NO_FILTERS)).toBe(false);
    expect(isFiltered({ ...NO_FILTERS, sort: "d30", tab: "top100" })).toBe(false);
    expect(isFiltered({ ...NO_FILTERS, query: "  " })).toBe(false);
    expect(isFiltered({ ...NO_FILTERS, query: "x" })).toBe(true);
    expect(isFiltered({ ...NO_FILTERS, tier: "goat" })).toBe(true);
    expect(isFiltered({ ...NO_FILTERS, badge: "live" })).toBe(true);
  });

  test("query matches name or handle, case-insensitive, with or without @", () => {
    expect(matchesQuery(rows[1], "cook")).toBe(true);
    expect(matchesQuery(rows[1], "@cookerf")).toBe(true);
    expect(matchesQuery(rows[0], "DECU0X")).toBe(true);
    expect(matchesQuery(rows[2], "@nobody")).toBe(false);
    expect(matchesQuery(rows[2], "")).toBe(true);
  });

  test("tabs", () => {
    expect(tabRows(rows, "all", null)).toHaveLength(4);
    expect(tabRows(rows, "top100", null).map((r) => r.rank)).toEqual([1, 2, 3]);
    expect(tabRows(rows, "network", null)).toEqual([]);
    expect(tabRows(rows, "network", new Set(["k2", "k101"])).map((r) => r.kol)).toEqual(["k2", "k101"]);
  });

  test("tier, badge and query combine", () => {
    expect(applyFilters(rows, { ...NO_FILTERS, tier: "diamond" }, null).map((r) => r.name)).toEqual(["Cooker", "Trunoest"]);
    expect(applyFilters(rows, { ...NO_FILTERS, badge: "sniper" }, null).map((r) => r.name)).toEqual(["Decu", "Trunoest"]);
    expect(applyFilters(rows, { ...NO_FILTERS, badge: "sniper", tier: "diamond" }, null).map((r) => r.name)).toEqual(["Trunoest"]);
    expect(applyFilters(rows, { ...NO_FILTERS, tab: "top100", query: "late" }, null)).toEqual([]);
  });
});

describe("sort", () => {
  const rows = [
    row(1, {}, { d30: 100, wr30: null, wr_all: 0.5, last_min_at_fetch: 30 }),
    row(2, {}, { d30: null, wr30: 0.7, last_min_at_fetch: 5 }),
    row(3, {}, { d30: 500, wr30: 0.4, last_min_at_fetch: null }),
    row(4, {}, null),
    row(5, {}, { d30: 500 }),
  ];
  const order = (s: Parameters<typeof sortRows>[1]) => sortRows(rows, s).map((r) => r.rank);

  test("rank keeps board order", () => expect(order("rank")).toEqual([1, 2, 3, 4, 5]));
  test("money: best first, ties by rank, no data last", () => expect(order("d30")).toEqual([3, 5, 1, 2, 4]));
  test("win rate falls back to all-time", () => expect(order("winrate")).toEqual([2, 1, 3, 4, 5]));
  test("last trade: most recent first", () => expect(order("last")).toEqual([2, 1, 3, 4, 5]));
  test("does not mutate the input", () => {
    sortRows(rows, "d30");
    expect(rows.map((r) => r.rank)).toEqual([1, 2, 3, 4, 5]);
  });
});

describe("formats", () => {
  test("winRate", () => {
    expect(winRate(null)).toEqual({ value: null, allTime: false });
    expect(winRate({ ...NO_METRICS, wr30: 0.6, wr_all: 0.4 })).toEqual({ value: 0.6, allTime: false });
    expect(winRate({ ...NO_METRICS, wr_all: 0.4 })).toEqual({ value: 0.4, allTime: true });
    expect(winRate({ ...NO_METRICS, wr30: 0 })).toEqual({ value: 0, allTime: false });
  });

  test("formatPercent", () => {
    expect(formatPercent(0.442)).toBe("44%");
    expect(formatPercent(1)).toBe("100%");
  });

  test("formatHold", () => {
    expect(formatHold(15)).toBe("15s");
    expect(formatHold(240)).toBe("4m");
    expect(formatHold(7200)).toBe("2h");
    expect(formatHold(3 * 86400)).toBe("3d");
  });
});

describe("dates and paths", () => {
  const now = Date.parse("2026-10-21T12:00:00Z");
  test("isLate only for the latest snapshot older than a day", () => {
    expect(isLate("2026-10-18T06:00:00Z", true, now)).toBe(true);
    expect(isLate("2026-10-21T06:00:00Z", true, now)).toBe(false);
    expect(isLate("2026-10-18T06:00:00Z", false, now)).toBe(false);
    expect(isLate(null, true, now)).toBe(false);
    expect(isLate("garbage", true, now)).toBe(false);
  });

  test("cardPath keeps the date only when given", () => {
    expect(cardPath("tekkerrss", null)).toBe("/kol/tekkerrss");
    expect(cardPath("tekkerrss", "2026-10-18")).toBe("/kol/tekkerrss?date=2026-10-18");
    expect(cardPath("a b", null)).toBe("/kol/a%20b");
  });

  test("dateParam accepts YYYY-MM-DD only", () => {
    expect(dateParam("2026-10-18")).toBe("2026-10-18");
    expect(dateParam(null)).toBe(null);
    expect(dateParam("2026-10-18T00:00")).toBe(null);
    expect(dateParam("latest")).toBe(null);
  });
});

describe("links", () => {
  const links = { kolexplorer: "tekkerrss", solanatracker: true, kolscan: true, fomo: "tek", pumpfun: null, telegram: "tekchan" };
  const wallets = [
    { address: "other", main: false, solscan_url: "" },
    { address: ADDR, main: true, solscan_url: `https://solscan.io/account/${ADDR}` },
  ];

  test("sourceLinks builds outbound links from the main wallet", () => {
    expect(sourceLinks("tekkerrss", links, wallets)).toEqual([
      { id: "x", url: "https://x.com/tekkerrss" },
      { id: "kolexplorer", url: "https://kolexplorer.com/kol/tekkerrss" },
      { id: "solanatracker", url: `https://www.solanatracker.io/wallet/${ADDR}` },
      { id: "kolscan", url: `https://kolscan.io/account/${ADDR}` },
      { id: "fomo", url: "https://fomo.family/profile/tek" },
      { id: "telegram", url: "https://t.me/tekchan" },
    ]);
  });

  test("wallet links need a wallet", () => {
    const ids = sourceLinks(null, links, []).map((l) => l.id);
    expect(ids).toEqual(["kolexplorer", "fomo", "telegram"]);
  });

  test("safeSolscan keeps only Solscan account pages", () => {
    expect(safeSolscan(`https://solscan.io/account/${ADDR}`)).toBe(`https://solscan.io/account/${ADDR}`);
    expect(safeSolscan(`http://solscan.io/account/${ADDR}`)).toBe(null);
    expect(safeSolscan(`https://evil.io/account/${ADDR}`)).toBe(null);
    expect(safeSolscan(`https://solscan.io/account/${ADDR}?x=1`)).toBe(null);
    expect(safeSolscan("javascript:alert(1)")).toBe(null);
  });
});
