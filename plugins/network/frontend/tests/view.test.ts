import { describe, expect, test } from "bun:test";
import { createCache, networkIds, networkPath, waitText } from "../src/view";
import type { NetworkView, Waiting } from "../src/view";

const wait = (reason: Waiting["reason"], quiet_days: number | null): Waiting => ({ kol: "k", name: "K", reason, quiet_days });

describe("waiting list text", () => {
  test("only two kinds of reason reach the reader", () => {
    expect(waitText(wait("wallet_not_confirmed", null))).toBe("not_confirmed");
    // a stray number on a not-confirmed KOL changes nothing
    expect(waitText(wait("wallet_not_confirmed", 5))).toBe("not_confirmed");
    expect(waitText(wait("quiet", 12))).toBe("quiet_days");
    expect(waitText(wait("quiet", 1))).toBe("quiet_one_day");
    expect(waitText(wait("quiet", 0))).toBe("quiet");
    expect(waitText(wait("quiet", null))).toBe("quiet");
  });
});

describe("network ids and path", () => {
  test("ids are the members on the board, not the waiting list", () => {
    const v: NetworkView = {
      date: "2026-10-20",
      members: [
        { kol: "briar", name: "Briar", rank: 3, score: 74, tier: "goat", pnl_30d: 1, volume_30d: 2, days_in_market: 3 },
        { kol: "alder", name: "Alder", rank: null, score: null, tier: null, pnl_30d: null, volume_30d: null, days_in_market: null },
      ],
      waiting: [wait("quiet", 3)],
    };
    expect(networkIds(v)).toEqual(["briar", "alder"]);
  });

  test("path carries the date, escaped", () => {
    expect(networkPath("2026-10-20")).toBe("/api/network?date=2026-10-20");
    expect(networkPath("")).toBe("/api/network");
    expect(networkPath("a&b")).toBe("/api/network?date=a%26b");
  });
});

describe("shared cache", () => {
  test("two callers share one load while it is fresh", async () => {
    let calls = 0;
    let clock = 0;
    const cache = createCache<number>(
      async () => {
        calls++;
        return { data: calls };
      },
      1000,
      () => clock,
    );
    const [a, b] = await Promise.all([cache.get("d"), cache.get("d")]);
    expect(a.data).toBe(1);
    expect(b.data).toBe(1);
    expect(calls).toBe(1);
    clock = 999;
    expect((await cache.get("d")).data).toBe(1);
    clock = 1000;
    expect((await cache.get("d")).data).toBe(2);
    expect((await cache.get("other")).data).toBe(3);
    cache.drop("d");
    expect((await cache.get("d")).data).toBe(4);
    cache.drop();
    expect((await cache.get("other")).data).toBe(5);
  });

  test("a failed load is not kept", async () => {
    let calls = 0;
    const cache = createCache<string>(async () => (++calls === 1 ? { error: "down" } : { data: "ok" }), 60_000);
    expect((await cache.get("d")).error).toBe("down");
    expect((await cache.get("d")).data).toBe("ok");
    expect(calls).toBe(2);
  });
});
