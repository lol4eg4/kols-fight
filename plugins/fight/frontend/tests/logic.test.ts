import { describe, expect, test } from "bun:test";
import {
  boardDate,
  MAX_FIGHTERS,
  VS,
  addFighter,
  canFight,
  fightApiPath,
  fightImagePath,
  fightPath,
  hpAfter,
  isFull,
  networkKols,
  parseFightQuery,
  pointsAfter,
  randomPick,
  removeFighter,
  roundShares,
  roundWinners,
  roundsShown,
  sameFight,
  searchKols,
  step,
  topTwo,
  whyOf,
  xPostUrl,
} from "../src/logic";
import type { Phase } from "../src/logic";
import type { BoardKol, FightResult, Round } from "../src/types";

const row = (kol: string, rank: number, extra: Partial<BoardKol> = {}): BoardKol => ({
  kol,
  name: kol[0]!.toUpperCase() + kol.slice(1),
  x_handle: `${kol}_x`,
  rank,
  score: 100 - rank,
  tier: "goat",
  in_network: false,
  ...extra,
});

const ROWS = [row("cooker", 2, { in_network: true }), row("decu", 1), row("tekkerrss", 18, { in_network: true }), row("ansem", 5, { x_handle: "blknoiz06" })];

const round = (number: number, kind: Round["kind"], values: [string, number | null][], outcome: Round["outcome"], awards: Round["awards"], damaged: string[]): Round => ({
  number,
  kind,
  values: values.map(([kol, value]) => ({ kol, value, all_time: false, source: "kolexplorer", fetched_at: "2026-10-17T12:00:00Z" })),
  outcome,
  awards,
  damaged,
});

const FIGHT: Pick<FightResult, "fighters" | "rounds"> = {
  fighters: [
    { kol: "decu", name: "Decu", rank: 1, card: { score: 95, tier: "goat", parts: [], badges: [] }, points: 1.5, hp: 7, rounds_won: [] },
    { kol: "cooker", name: "Cooker", rank: 2, card: { score: 88, tier: "goat", parts: [], badges: [] }, points: 0.5, hp: 6, rounds_won: [] },
  ],
  rounds: [
    round(1, "score", [["decu", 95], ["cooker", 88]], { type: "win", kol: "decu" }, [{ kol: "decu", points: 1 }], ["cooker"]),
    round(2, "pnl24h", [["decu", 5], ["cooker", null]], { type: "void", reason: "missing_data" }, [], []),
    round(3, "win_rate", [["decu", 0.5], ["cooker", 0.5]], { type: "tie", kols: ["decu", "cooker"] }, [{ kol: "decu", points: 0.5 }, { kol: "cooker", points: 0.5 }], []),
    round(4, "pnl7d", [["decu", 10], ["cooker", 20]], { type: "win", kol: "cooker" }, [{ kol: "cooker", points: 1 }], ["decu"]),
  ],
};

describe("fight address", () => {
  test("board date comes from a valid date parameter only", () => {
    expect(boardDate("2026-10-18")).toBe("2026-10-18");
    expect(boardDate(null)).toBeNull();
    expect(boardDate("")).toBeNull();
    expect(boardDate("18.10.2026")).toBeNull();
  });

  test("reads ids in order, drops blanks and repeats, keeps the date", () => {
    expect(parseFightQuery("?ids=decu,cooker&date=2026-10-18")).toEqual({ ids: ["decu", "cooker"], date: "2026-10-18" });
    expect(parseFightQuery("?ids= decu,,cooker,decu ,")).toEqual({ ids: ["decu", "cooker"], date: null });
    expect(parseFightQuery("ids=a%2Cb")).toEqual({ ids: ["a", "b"], date: null });
    expect(parseFightQuery("")).toEqual({ ids: [], date: null });
    expect(parseFightQuery("?ids=decu&date=")).toEqual({ ids: ["decu"], date: null });
  });

  test("builds the window, API and image paths", () => {
    expect(fightPath()).toBe("/fight");
    expect(fightPath(["decu", "cooker"])).toBe("/fight?ids=decu,cooker");
    expect(fightPath(["decu", "cooker"], "2026-10-18")).toBe("/fight?ids=decu,cooker&date=2026-10-18");
    expect(fightPath(["a b", "c&d"])).toBe("/fight?ids=a%20b,c%26d");
    expect(fightApiPath(["decu", "cooker"], null)).toBe("/api/fight?ids=decu,cooker");
    expect(fightImagePath(["decu", "cooker"], "2026-10-18")).toBe("/api/fight/image?ids=decu,cooker&date=2026-10-18");
  });

  test("a built path reads back the same", () => {
    const ids = ["decu", "a b", "c&d"];
    expect(parseFightQuery(fightPath(ids, "2026-10-18").slice("/fight".length))).toEqual({ ids, date: "2026-10-18" });
  });

  test("X intent carries text and link encoded", () => {
    expect(xPostUrl("A & B", "https://x.test/fight?ids=a,b")).toBe("https://x.com/intent/post?text=A%20%26%20B&url=https%3A%2F%2Fx.test%2Ffight%3Fids%3Da%2Cb");
  });

  test("same fight ignores order, not the date", () => {
    expect(sameFight({ ids: ["a", "b"], date: "d" }, { ids: ["b", "a"], date: "d" })).toBe(true);
    expect(sameFight({ ids: ["a", "b"], date: "d" }, { ids: ["a", "b"], date: null })).toBe(false);
    expect(sameFight({ ids: ["a", "b"], date: null }, { ids: ["a", "b", "c"], date: null })).toBe(false);
  });
});

describe("pending fighters", () => {
  test("add keeps order, refuses repeats and a fifth fighter", () => {
    let list: string[] = [];
    list = addFighter(list, "a");
    list = addFighter(list, "b");
    list = addFighter(list, "a");
    expect(list).toEqual(["a", "b"]);
    list = addFighter(addFighter(list, "c"), "d");
    expect(list).toHaveLength(MAX_FIGHTERS);
    expect(isFull(list)).toBe(true);
    expect(addFighter(list, "e")).toEqual(["a", "b", "c", "d"]);
  });

  test("remove takes out one and leaves the rest in order", () => {
    expect(removeFighter(["a", "b", "c"], "b")).toEqual(["a", "c"]);
    expect(removeFighter(["a"], "x")).toEqual(["a"]);
  });

  test("a fight needs 2 to 4", () => {
    expect(canFight([])).toBe(false);
    expect(canFight(["a"])).toBe(false);
    expect(canFight(["a", "b"])).toBe(true);
    expect(canFight(["a", "b", "c", "d"])).toBe(true);
    expect(canFight(["a", "b", "c", "d", "e"])).toBe(false);
  });
});

describe("quick picks and search", () => {
  test("#1 vs #2 goes by rank, not row order", () => {
    expect(topTwo(ROWS)).toEqual(["decu", "cooker"]);
  });

  test("random pick gives different KOLs", () => {
    const seq = [0.99, 0.99, 0.99];
    const ids = randomPick(ROWS, 3, () => seq.shift() ?? 0);
    expect(new Set(ids).size).toBe(3);
    expect(randomPick(ROWS.slice(0, 1), 2)).toEqual(["cooker"]);
    expect(randomPick(ROWS, 2, () => 0)).toEqual(["cooker", "decu"]);
  });

  test("network KOLs come from the board's in_network mark", () => {
    expect(networkKols(ROWS).map((r) => r.kol)).toEqual(["cooker", "tekkerrss"]);
  });

  test("search matches name, handle and id, best rank first", () => {
    expect(searchKols(ROWS, "").map((r) => r.kol)).toEqual([]);
    expect(searchKols(ROWS, "e").map((r) => r.kol)).toEqual(["decu", "cooker", "ansem", "tekkerrss"]);
    expect(searchKols(ROWS, "@BLK").map((r) => r.kol)).toEqual(["ansem"]);
    expect(searchKols(ROWS, "e", 2)).toHaveLength(2);
  });
});

describe("round reveal", () => {
  test("VS, then rounds one by one, then K.O.", () => {
    let p: Phase = VS;
    expect(roundsShown(p, 4)).toBe(0);
    p = step(p, "next", 4);
    expect(p).toEqual({ kind: "rounds", shown: 1 });
    p = step(step(step(p, "next", 4), "next", 4), "next", 4);
    expect(p).toEqual({ kind: "rounds", shown: 4 });
    p = step(p, "next", 4);
    expect(p).toEqual({ kind: "ko" });
    expect(roundsShown(p, 4)).toBe(4);
    expect(step(p, "next", 4)).toEqual({ kind: "ko" });
  });

  test("skip goes to K.O. from anywhere; rematch back to VS; start only from VS", () => {
    expect(step(VS, "skip", 8)).toEqual({ kind: "ko" });
    expect(step({ kind: "rounds", shown: 3 }, "skip", 8)).toEqual({ kind: "ko" });
    expect(step({ kind: "ko" }, "rematch", 8)).toEqual(VS);
    expect(step({ kind: "rounds", shown: 3 }, "start", 8)).toEqual({ kind: "rounds", shown: 3 });
    expect(step(VS, "start", 0)).toEqual({ kind: "ko" });
  });

  test("HP and points follow the rounds shown", () => {
    expect(hpAfter(FIGHT, 0)).toEqual({ decu: 8, cooker: 8 });
    expect(hpAfter(FIGHT, 1)).toEqual({ decu: 8, cooker: 7 });
    expect(hpAfter(FIGHT, 4)).toEqual({ decu: 7, cooker: 7 });
    expect(pointsAfter(FIGHT, 3)).toEqual({ decu: 1.5, cooker: 0.5 });
    expect(pointsAfter(FIGHT, 4)).toEqual({ decu: 1.5, cooker: 1.5 });
  });

  test("winners of a round", () => {
    expect(roundWinners(FIGHT.rounds[0]!)).toEqual(["decu"]);
    expect(roundWinners(FIGHT.rounds[1]!)).toEqual([]);
    expect(roundWinners(FIGHT.rounds[2]!)).toEqual(["decu", "cooker"]);
  });
});

describe("bars and why", () => {
  test("bars are shares of the best value; no data stays null", () => {
    expect(roundShares("pnl30d", [565, 207, null])).toEqual([1, 207 / 565, null]);
    const neg = roundShares("pnl7d", [124873, 27384, -1600]);
    expect(neg[0]).toBe(1);
    expect(neg[2]).toBe(0.04);
    expect(roundShares("last_trade", [59, 300])).toEqual([1, 59 / 300]);
    expect(roundShares("score", [50, 50])).toEqual([1, 1]);
    expect(roundShares("score", [null, null])).toEqual([null, null]);
  });

  test("money win: times when the runner-up is positive and far behind, else the difference", () => {
    const r = round(2, "pnl30d", [["decu", 565129], ["cooker", 400000], ["tekkerrss", 207060]], { type: "win", kol: "decu" }, [], []);
    expect(whyOf(r)).toEqual({ type: "win", winner: "decu", runnerUp: "cooker", winnerValue: 565129, runnerValue: 400000, margin: { type: "diff", n: 165129 } });
    const far = round(3, "pnl7d", [["decu", 124873], ["cooker", 27384]], { type: "win", kol: "decu" }, [], []);
    expect(whyOf(far)).toMatchObject({ margin: { type: "times", n: 4.6 } });
    const neg = round(4, "pnl24h", [["decu", 100], ["cooker", -50]], { type: "win", kol: "decu" }, [], []);
    expect(whyOf(neg)).toMatchObject({ margin: { type: "diff", n: 150 } });
  });

  test("rates in points, last trade by recency, ties and voids", () => {
    const rate = round(5, "win_rate", [["decu", 0.697], ["cooker", 0.848]], { type: "win", kol: "cooker" }, [], []);
    expect(whyOf(rate)).toMatchObject({ runnerUp: "decu", margin: { type: "diff", n: 15 } });
    const last = round(8, "last_trade", [["decu", 59], ["cooker", 300], ["t", 120]], { type: "win", kol: "decu" }, [], []);
    expect(whyOf(last)).toMatchObject({ runnerUp: "t", runnerValue: 120, margin: { type: "fresher" } });
    expect(whyOf(FIGHT.rounds[1]!)).toEqual({ type: "void_missing", kols: ["cooker"] });
    expect(whyOf(FIGHT.rounds[2]!)).toEqual({ type: "tie", kols: ["decu", "cooker"], value: 0.5 });
    const tied = round(6, "pnl7d", [["a", 0], ["b", 0]], { type: "void", reason: "all_tied" }, [], []);
    expect(whyOf(tied)).toEqual({ type: "void_tied", value: 0 });
  });
});
