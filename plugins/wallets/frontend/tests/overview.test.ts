import { describe, expect, test } from "bun:test";
import { actionsFor, createCache, filled, filterKols, sortCandidates, sortKols } from "../src/overview";
import type { CandidateView, KolView } from "../src/overview";

function cand(id: number, status: CandidateView["status"], extra: Partial<CandidateView> = {}): CandidateView {
  return {
    id,
    kol: "alder",
    address: `addr${id}`,
    status,
    source: "kolexplorer",
    evidence: null,
    sent_by: null,
    confirmation: null,
    reason: null,
    main: false,
    created_at: `2026-10-${String(10 + id).padStart(2, "0")}T00:00:00Z`,
    updated_at: "2026-10-20T00:00:00Z",
    last_trade_at: null,
    checked_at: null,
    ...extra,
  };
}

function kol(kolId: string, name: string, status: KolView["status"]): KolView {
  return { kol: kolId, name, status, main_address: null, last_trade_at: null, candidates: [] };
}

describe("actions follow the server's state machine", () => {
  test("hypothesis: confirm or reject", () => expect(actionsFor(cand(1, "hypothesis"))).toEqual(["confirm", "reject"]));
  test("conflict: confirm, reject or revoke", () => expect(actionsFor(cand(1, "conflict"))).toEqual(["confirm", "reject", "revoke"]));
  test("confirmed: make main unless it is, revoke", () => {
    expect(actionsFor(cand(1, "confirmed"))).toEqual(["main", "revoke"]);
    expect(actionsFor(cand(1, "confirmed", { main: true }))).toEqual(["revoke"]);
  });
  test("closed states: nothing", () => {
    for (const s of ["not_found", "rejected", "revoked", "retired"] as const) expect(actionsFor(cand(1, s))).toEqual([]);
  });
});

describe("order and filter", () => {
  test("candidates in play first, main first, then newest", () => {
    const list = [cand(1, "rejected"), cand(2, "confirmed"), cand(3, "confirmed", { main: true }), cand(4, "hypothesis"), cand(5, "conflict"), cand(6, "hypothesis")];
    expect(sortCandidates(list).map((c) => c.id)).toEqual([5, 6, 4, 3, 2, 1]);
  });

  test("KOLs needing work first, then by name", () => {
    const list = [kol("a", "Alder", "confirmed"), kol("b", "Briar", "hypothesis"), kol("c", "Cobalt", "conflict"), kol("d", "Dune", "not_found"), kol("e", "Ash", "hypothesis")];
    expect(sortKols(list).map((k) => k.kol)).toEqual(["c", "e", "b", "d", "a"]);
  });

  test("filter by status and name or id", () => {
    const list = [kol("alder", "Alder Vane", "confirmed"), kol("briar", "Briar Quill", "hypothesis")];
    expect(filterKols(list, "all", "").length).toBe(2);
    expect(filterKols(list, "hypothesis", "").map((k) => k.kol)).toEqual(["briar"]);
    expect(filterKols(list, "all", " vane ").map((k) => k.kol)).toEqual(["alder"]);
    expect(filterKols(list, "all", "BRI").map((k) => k.kol)).toEqual(["briar"]);
    expect(filterKols(list, "confirmed", "briar")).toEqual([]);
  });

  test("a required text needs more than spaces", () => {
    expect(filled("  ")).toBe(false);
    expect(filled(" DM on X, Oct 19 ")).toBe(true);
  });
});

describe("shared overview cache", () => {
  test("one load for many fillers; put replaces; a failure is not kept", async () => {
    let calls = 0;
    let fail = false;
    const cache = createCache<{ data?: number; error?: string }>(async () => (fail ? { error: "down" } : { data: ++calls }), 30_000);
    const all = await Promise.all([cache.get("o"), cache.get("o"), cache.get("o")]);
    expect(all.map((r) => r.data)).toEqual([1, 1, 1]);
    cache.put("o", { data: 42 });
    expect((await cache.get("o")).data).toBe(42);
    cache.drop();
    fail = true;
    expect((await cache.get("o")).error).toBe("down");
    fail = false;
    expect((await cache.get("o")).data).toBe(2);
  });
});
