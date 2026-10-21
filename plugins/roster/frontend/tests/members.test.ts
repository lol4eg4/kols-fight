import { describe, expect, test } from "bun:test";
import { buildPatch, cleanHandle, emptyForm, filterMembers, groupsOf, skippedLines } from "../src/members";
import type { AdminMember, SyncRun } from "../src/members";

function member(kol: string, extra: Partial<AdminMember> = {}): AdminMember {
  return {
    kol,
    name: kol[0]!.toUpperCase() + kol.slice(1),
    x_handle: null,
    x_verified: false,
    fomo_handle: null,
    pumpfun: null,
    sheet_wallets: [],
    formats: [],
    keep: false,
    active: true,
    group: null,
    low_usd: null,
    high_usd: null,
    note: null,
    wallet_note: null,
    joined_at: "2026-10-01T00:00:00Z",
    left_at: null,
    last_seen_at: null,
    ...extra,
  };
}

describe("filter", () => {
  const list = [member("alder", { group: "B", x_handle: "alder_x" }), member("briar", { group: "A", fomo_handle: "briarfomo" }), member("cobalt")];
  test("groups are distinct and sorted, empty ones left out", () => expect(groupsOf([...list, member("dune", { group: " " })])).toEqual(["A", "B"]));
  test("by group and by name, id or handle", () => {
    expect(filterMembers(list, "A", "").map((m) => m.kol)).toEqual(["briar"]);
    expect(filterMembers(list, "", "@ALDER_").map((m) => m.kol)).toEqual(["alder"]);
    expect(filterMembers(list, "", "fomo").map((m) => m.kol)).toEqual(["briar"]);
    expect(filterMembers(list, "B", "briar")).toEqual([]);
  });
});

describe("handle", () => {
  test("same rule as the server", () => {
    expect(cleanHandle(" @alder_x ")).toBe("alder_x");
    expect(cleanHandle("a".repeat(16))).toBeNull();
    expect(cleanHandle("al der")).toBeNull();
    expect(cleanHandle("")).toBeNull();
  });
});

describe("patch body", () => {
  const today = "2026-10-21";

  test("an unverified handle starts empty; a verified one is kept and not resent", () => {
    expect(emptyForm(member("a", { x_handle: "fomoname" }), today).handle).toBe("");
    const m = member("a", { x_handle: "alder", x_verified: true });
    expect(buildPatch(m, emptyForm(m, today)).problems).toEqual(["nothing"]);
  });

  test("a new handle is sent; a bad one is a problem", () => {
    const m = member("a");
    expect(buildPatch(m, { ...emptyForm(m, today), handle: "@alder" }).patch).toEqual({ x_handle: "alder" });
    expect(buildPatch(m, { ...emptyForm(m, today), handle: "not a handle" }).problems).toEqual(["handle"]);
  });

  test("formats only when they changed, in server order", () => {
    const m = member("a", { formats: ["buy", "post"] });
    expect(buildPatch(m, { ...emptyForm(m, today), formats: ["post", "buy"] }).problems).toEqual(["nothing"]);
    expect(buildPatch(m, { ...emptyForm(m, today), formats: ["call", "post"] }).patch).toEqual({ formats: ["post", "call"] });
    expect(buildPatch(m, { ...emptyForm(m, today), formats: [] }).patch).toEqual({ formats: [] });
  });

  test("reach needs whole numbers, a source and a date", () => {
    const m = member("a");
    const f = { ...emptyForm(m, today), x_followers: "12,400", pumpfun_followers: "" };
    expect(buildPatch(m, f).problems).toEqual(["reach_source"]);
    expect(buildPatch(m, { ...f, source: "X profile" }).patch).toEqual({
      reach: { x_followers: 12400, telegram_subscribers: null, pumpfun_followers: null, source: "X profile", date: today },
    });
    expect(buildPatch(m, { ...f, x_followers: "-3", source: "X" }).problems).toEqual(["reach_number"]);
    expect(buildPatch(m, { ...f, source: "X", date: "" }).problems).toEqual(["reach_date"]);
  });
});

test("skipped lines survive a stored value of any shape", () => {
  const run = { skipped: [{ line: 4, reason: "no name" }, { line: "x" }, null] } as unknown as SyncRun;
  expect(skippedLines(run)).toEqual([{ line: 4, reason: "no name" }]);
  expect(skippedLines({ skipped: null } as unknown as SyncRun)).toEqual([]);
});
