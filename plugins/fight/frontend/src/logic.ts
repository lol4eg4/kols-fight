// Pure fight logic: the address, the pending fighters, quick picks, the round-by-round reveal and the
// "why it won" comparison. No React, no shell: unit-tested.

import type { BoardKol, FightResult, Round, RoundKind } from "./types";

export const MIN_FIGHTERS = 2;
export const MAX_FIGHTERS = 4;
/** HP segments each fighter starts with: one per round. */
export const HP_SEGMENTS = 8;

export const ROUND_KINDS: RoundKind[] = ["score", "pnl30d", "pnl7d", "pnl24h", "win_rate", "win_rate7d", "position_size", "last_trade"];

/** Rounds whose value is dollars. */
export const MONEY_ROUNDS: ReadonlySet<RoundKind> = new Set(["pnl30d", "pnl7d", "pnl24h", "position_size"]);
/** Rounds whose value is a win rate, 0 to 1. */
export const RATE_ROUNDS: ReadonlySet<RoundKind> = new Set(["win_rate", "win_rate7d"]);

/** Only the lower value wins the last-trade round (minutes since the last trade). */
export function higherWins(kind: RoundKind): boolean {
  return kind !== "last_trade";
}

// ---------- address ----------

export interface FightQuery {
  /** KOL ids in link order, without blanks and repeats. */
  ids: string[];
  /** The snapshot day as written in the link, or null for the latest snapshot. */
  date: string | null;
}

/** Reads "?ids=a,b&date=2026-10-18". The server judges the ids and the date; here they are only split and cleaned. */
export function parseFightQuery(search: string): FightQuery {
  const p = new URLSearchParams(search);
  const ids: string[] = [];
  for (const raw of (p.get("ids") ?? "").split(",")) {
    const id = raw.trim();
    if (id && !ids.includes(id)) ids.push(id);
  }
  const date = (p.get("date") ?? "").trim();
  return { ids, date: date === "" ? null : date };
}

/** The board date from the page's `date` parameter ("2026-10-18"), or null for today. */
export function boardDate(raw: string | null): string | null {
  return raw && /^\d{4}-\d{2}-\d{2}$/.test(raw) ? raw : null;
}

function query(ids: readonly string[], date: string | null | undefined): string {
  const list = ids.map(encodeURIComponent).join(",");
  return date ? `ids=${list}&date=${encodeURIComponent(date)}` : `ids=${list}`;
}

/** Address of the fight window. No ids: the fighter picker. */
export function fightPath(ids: readonly string[] = [], date?: string | null): string {
  return ids.length === 0 ? "/fight" : `/fight?${query(ids, date)}`;
}

export function fightApiPath(ids: readonly string[], date: string | null): string {
  return `/api/fight?${query(ids, date)}`;
}

/** The 1200x630 result picture: the preview X shows for the fight link. */
export function fightImagePath(ids: readonly string[], date: string): string {
  return `/api/fight/image?${query(ids, date)}`;
}

/** X's compose page with the text and the link filled in. Opened by the visitor; we call nothing. */
export function xPostUrl(text: string, url: string): string {
  return `https://x.com/intent/post?text=${encodeURIComponent(text)}&url=${encodeURIComponent(url)}`;
}

/** Same fighters (any order) and the same date. */
export function sameFight(a: { ids: readonly string[]; date: string | null }, b: { ids: readonly string[]; date: string | null }): boolean {
  return a.date === b.date && a.ids.length === b.ids.length && a.ids.every((id) => b.ids.includes(id));
}

// ---------- pending fighters ----------

export function addFighter(list: readonly string[], kol: string): string[] {
  if (list.includes(kol) || list.length >= MAX_FIGHTERS) return [...list];
  return [...list, kol];
}

export function removeFighter(list: readonly string[], kol: string): string[] {
  return list.filter((k) => k !== kol);
}

export function isFull(list: readonly string[]): boolean {
  return list.length >= MAX_FIGHTERS;
}

export function canFight(list: readonly string[]): boolean {
  return list.length >= MIN_FIGHTERS && list.length <= MAX_FIGHTERS;
}

// ---------- quick picks ----------

export function matchesQuery(row: Pick<BoardKol, "name" | "x_handle" | "kol">, query: string): boolean {
  const q = query.trim().toLowerCase().replace(/^@/, "");
  if (!q) return true;
  return row.name.toLowerCase().includes(q) || (row.x_handle ?? "").toLowerCase().includes(q) || row.kol.toLowerCase().includes(q);
}

/** Search results: best rank first, at most `limit`. */
export function searchKols(rows: readonly BoardKol[], query: string, limit = 6): BoardKol[] {
  if (!query.trim()) return [];
  return rows
    .filter((r) => matchesQuery(r, query))
    .sort((a, b) => a.rank - b.rank)
    .slice(0, limit);
}

/** #1 vs #2 on the board. */
export function topTwo(rows: readonly BoardKol[]): string[] {
  return [...rows]
    .sort((a, b) => a.rank - b.rank)
    .slice(0, 2)
    .map((r) => r.kol);
}

/** `n` different KOLs at random. `rng` returns 0 <= x < 1. */
export function randomPick(rows: readonly BoardKol[], n: number, rng: () => number = Math.random): string[] {
  const pool = rows.map((r) => r.kol);
  const out: string[] = [];
  while (out.length < n && pool.length > 0) {
    const i = Math.min(Math.floor(rng() * pool.length), pool.length - 1);
    out.push(pool.splice(i, 1)[0]!);
  }
  return out;
}

export function networkKols(rows: readonly BoardKol[]): BoardKol[] {
  return rows.filter((r) => r.in_network);
}

// ---------- the reveal ----------

export type Phase = { kind: "vs" } | { kind: "rounds"; shown: number } | { kind: "ko" };
export type Step = "start" | "next" | "skip" | "rematch";

export const VS: Phase = { kind: "vs" };

/** VS -> round 1 -> ... -> round `total` -> K.O. Skip goes straight to K.O.; rematch back to VS. */
export function step(phase: Phase, action: Step, total: number): Phase {
  switch (action) {
    case "rematch":
      return VS;
    case "skip":
      return { kind: "ko" };
    case "start":
      return phase.kind === "vs" ? (total > 0 ? { kind: "rounds", shown: 1 } : { kind: "ko" }) : phase;
    case "next":
      if (phase.kind === "vs") return step(phase, "start", total);
      if (phase.kind === "rounds") return phase.shown >= total ? { kind: "ko" } : { kind: "rounds", shown: phase.shown + 1 };
      return phase;
  }
}

/** Rounds on screen: none on VS, all at K.O. */
export function roundsShown(phase: Phase, total: number): number {
  if (phase.kind === "vs") return 0;
  if (phase.kind === "ko") return total;
  return Math.min(Math.max(phase.shown, 0), total);
}

/** HP of each fighter after the first `shown` rounds. */
export function hpAfter(fight: Pick<FightResult, "fighters" | "rounds">, shown: number): Record<string, number> {
  const hp: Record<string, number> = {};
  for (const f of fight.fighters) hp[f.kol] = HP_SEGMENTS;
  for (const r of fight.rounds.slice(0, shown)) for (const k of r.damaged) if (k in hp) hp[k] = Math.max(0, hp[k]! - 1);
  return hp;
}

/** Points of each fighter after the first `shown` rounds. */
export function pointsAfter(fight: Pick<FightResult, "fighters" | "rounds">, shown: number): Record<string, number> {
  const pts: Record<string, number> = {};
  for (const f of fight.fighters) pts[f.kol] = 0;
  for (const r of fight.rounds.slice(0, shown)) for (const a of r.awards) if (a.kol in pts) pts[a.kol] = pts[a.kol]! + a.points;
  return pts;
}

/** Fighters who took (or split) the round. */
export function roundWinners(round: Round): string[] {
  if (round.outcome.type === "win") return [round.outcome.kol];
  if (round.outcome.type === "tie") return [...round.outcome.kols];
  return [];
}

/** Points without float noise: 1, 0.5, 0.33. */
export function formatPointsShort(n: number): string {
  return String(Math.round(n * 100) / 100);
}

/**
 * Bar length of each value against the best one, 0 to 1; null for no data. Higher-wins rounds with a
 * loss in them are scaled from the lowest value, so a negative PnL still gets a (short) bar.
 */
export function roundShares(kind: RoundKind, values: readonly (number | null)[]): (number | null)[] {
  const nums = values.filter((v): v is number => v !== null && Number.isFinite(v));
  if (nums.length === 0) return values.map(() => null);
  const max = Math.max(...nums);
  const min = Math.min(...nums);
  const MIN_SHARE = 0.04;
  return values.map((v) => {
    if (v === null || !Number.isFinite(v)) return null;
    if (max === min) return 1;
    if (!higherWins(kind)) return v <= 0 ? 1 : Math.max(min > 0 ? min / v : (max - v) / (max - min), MIN_SHARE);
    if (min >= 0) return Math.max(v / max, MIN_SHARE);
    return Math.max((v - min) / (max - min), MIN_SHARE);
  });
}

// ---------- why it won ----------

/** How the winner beat the runner-up: "2.7× more", "+$358K", "+12 points", or simply "more recent". */
export type Margin = { type: "times"; n: number } | { type: "diff"; n: number } | { type: "fresher" };

export type Why =
  | { type: "win"; winner: string; runnerUp: string; winnerValue: number; runnerValue: number; margin: Margin }
  | { type: "tie"; kols: string[]; value: number }
  | { type: "void_missing"; kols: string[] }
  | { type: "void_tied"; value: number | null };

/** The facts behind the "why" line of a round. The screen turns them into text. */
export function whyOf(round: Round): Why {
  const { outcome, values } = round;
  if (outcome.type === "void") {
    if (outcome.reason === "missing_data") return { type: "void_missing", kols: values.filter((v) => v.value === null).map((v) => v.kol) };
    return { type: "void_tied", value: values[0]?.value ?? null };
  }
  if (outcome.type === "tie") {
    const v = values.find((x) => x.kol === outcome.kols[0])?.value ?? 0;
    return { type: "tie", kols: [...outcome.kols], value: v };
  }
  const up = higherWins(round.kind);
  const winnerValue = values.find((v) => v.kol === outcome.kol)?.value ?? 0;
  let runner: { kol: string; value: number } | null = null;
  for (const v of values) {
    if (v.kol === outcome.kol || v.value === null) continue;
    if (!runner || (up ? v.value > runner.value : v.value < runner.value)) runner = { kol: v.kol, value: v.value };
  }
  if (!runner) runner = { kol: outcome.kol, value: winnerValue };
  return { type: "win", winner: outcome.kol, runnerUp: runner.kol, winnerValue, runnerValue: runner.value, margin: marginOf(round.kind, winnerValue, runner.value) };
}

function marginOf(kind: RoundKind, w: number, r: number): Margin {
  if (kind === "last_trade") return { type: "fresher" };
  if (RATE_ROUNDS.has(kind)) return { type: "diff", n: Math.round((w - r) * 100) };
  if (MONEY_ROUNDS.has(kind) && r > 0 && w / r >= 1.5) return { type: "times", n: Math.round((w / r) * 10) / 10 };
  return { type: "diff", n: w - r };
}
