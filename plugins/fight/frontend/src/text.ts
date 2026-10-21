// Words of the fight: round names, value formats, the "why it won" line, screen reader announcements.

import { formatMinutesAgo, formatPoints, formatSnapshotDate, moneyParts } from "@arena/design";
import { t } from "@arena/shell";
import { MONEY_ROUNDS, RATE_ROUNDS, formatPointsShort, whyOf } from "./logic";
import type { FightResult, PartKind, Round, RoundKind, Source } from "./types";

export const KIND_TITLE: Record<RoundKind, () => string> = {
  score: () => t("fight.kind.score"),
  pnl30d: () => t("fight.kind.pnl30d"),
  pnl7d: () => t("fight.kind.pnl7d"),
  pnl24h: () => t("fight.kind.pnl24h"),
  win_rate: () => t("fight.kind.win_rate"),
  win_rate7d: () => t("fight.kind.win_rate7d"),
  position_size: () => t("fight.kind.position_size"),
  last_trade: () => t("fight.kind.last_trade"),
};

export const KIND_MEASURES: Record<RoundKind, () => string> = {
  score: () => t("fight.measure.score"),
  pnl30d: () => t("fight.measure.pnl30d"),
  pnl7d: () => t("fight.measure.pnl7d"),
  pnl24h: () => t("fight.measure.pnl24h"),
  win_rate: () => t("fight.measure.win_rate"),
  win_rate7d: () => t("fight.measure.win_rate7d"),
  position_size: () => t("fight.measure.position_size"),
  last_trade: () => t("fight.measure.last_trade"),
};

export const PART_NAME: Record<PartKind, () => string> = {
  profit: () => t("fight.part.profit"),
  accuracy: () => t("fight.part.accuracy"),
  form: () => t("fight.part.form"),
  activity: () => t("fight.part.activity"),
};

export const PART_RAW: Record<PartKind, () => string> = {
  profit: () => t("fight.part.profit_raw"),
  accuracy: () => t("fight.part.accuracy_raw"),
  form: () => t("fight.part.form_raw"),
  activity: () => t("fight.part.activity_raw"),
};

export const PART_WEIGHT: Record<PartKind, number> = { profit: 40, accuracy: 25, form: 20, activity: 15 };

export const SOURCE_NAME: Record<Source, () => string> = {
  kolexplorer: () => t("fight.source.kolexplorer"),
  solanatracker: () => t("fight.source.solanatracker"),
};

export function percent(rate: number): string {
  return t("fight.percent", { n: formatPoints(rate * 100) });
}

/** A round value as plain text: "+$565.1K", "$1.1K", "69.7%", "95", "59 min ago". */
export function valueText(kind: RoundKind, v: number): string {
  if (MONEY_ROUNDS.has(kind)) return moneyParts(v, "short", { signed: kind !== "position_size" }).text;
  if (RATE_ROUNDS.has(kind)) return percent(v);
  if (kind === "last_trade") return formatMinutesAgo(v);
  return formatPointsShort(v);
}

/** "A and B", "A, B and C". */
export function listText(items: readonly string[]): string {
  if (items.length <= 1) return items[0] ?? "";
  return t("fight.list_and", { a: items.slice(0, -1).join(", "), b: items[items.length - 1]! });
}

export type Names = (kol: string) => string;

export function namesOf(fight: Pick<FightResult, "fighters">): Names {
  const m = new Map(fight.fighters.map((f) => [f.kol, f.name]));
  return (kol) => m.get(kol) ?? kol;
}

/** The line under a round: why the winner won, why it was a tie, why it did not count. */
export function whyText(round: Round, name: Names): string {
  const w = whyOf(round);
  switch (w.type) {
    case "void_missing":
      return t("fight.why.void_missing", { names: listText(w.kols.map(name)) });
    case "void_tied":
      return t("fight.why.void_tied", { value: w.value === null ? t("fight.round.no_data") : valueText(round.kind, w.value) });
    case "tie":
      return t("fight.why.tie", { names: listText(w.kols.map(name)), value: valueText(round.kind, w.value) });
    case "win": {
      const vars = { winner: name(w.winner), runner: name(w.runnerUp), wv: valueText(round.kind, w.winnerValue), rv: valueText(round.kind, w.runnerValue) };
      const m = w.margin;
      if (m.type === "fresher") return t("fight.why.fresher", vars);
      if (m.type === "times") return t("fight.why.times", { ...vars, n: formatPoints(m.n) });
      if (MONEY_ROUNDS.has(round.kind)) return t("fight.why.more", { ...vars, diff: moneyParts(m.n, "short", { signed: false }).text });
      return t("fight.why.higher", { ...vars, n: formatPoints(m.n) });
    }
  }
}

/** Read out when a round appears: "Round 2, PnL 30 days: Decu wins, +$565.1K vs +$297K vs +$207.1K." */
export function announceText(round: Round, name: Names): string {
  const title = KIND_TITLE[round.kind]();
  const o = round.outcome;
  if (o.type === "void") {
    return t("fight.announce.void", { n: round.number, title, reason: o.reason === "missing_data" ? t("fight.announce.void_missing") : t("fight.announce.void_tied") });
  }
  if (o.type === "tie") return t("fight.announce.tie", { n: round.number, title, names: listText(o.kols.map(name)) });
  const winner = round.values.find((v) => v.kol === o.kol);
  const others = round.values.filter((v) => v.kol !== o.kol);
  const values = [winner, ...others]
    .filter((v) => v !== undefined)
    .map((v) => (v.value === null ? t("fight.round.no_data") : valueText(round.kind, v.value)))
    .join(` ${t("fight.vs_sep")} `);
  return t("fight.announce.win", { n: round.number, title, winner: name(o.kol), values });
}

/** Where the values of a round came from: "Source: Decu and Cooker KolExplorer, Oct 17, 12:00 UTC; Tekkerrss Solana Tracker, ...". */
export function sourceText(round: Round, date: string, name: Names): string {
  if (round.kind === "score") return t("fight.source.board", { date: formatSnapshotDate(date) });
  const groups = new Map<string, { source: Source | null; at: string | null; kols: string[] }>();
  for (const v of round.values) {
    const key = `${v.source ?? ""}|${v.fetched_at ?? ""}`;
    const g = groups.get(key) ?? { source: v.source, at: v.fetched_at, kols: [] };
    g.kols.push(v.kol);
    groups.set(key, g);
  }
  const items = [...groups.values()].map((g) =>
    t("fight.source.item", {
      names: listText(g.kols.map(name)),
      source: g.source ? SOURCE_NAME[g.source]() : t("fight.source.unknown"),
      date: g.at ? formatSnapshotDate(g.at, { year: false }) : t("fight.source.unknown"),
    }),
  );
  return t("fight.source.line", { list: items.join("; ") });
}

/** "Decu 4 · Cooker 1 · Tekkerrss 1", best first. */
export function scoreLine(fight: Pick<FightResult, "fighters">): string {
  return [...fight.fighters]
    .sort((a, b) => b.points - a.points)
    .map((f) => `${f.name} ${formatPointsShort(f.points)}`)
    .join(" · ");
}

/** The text of an X post about the fight. */
export function shareText(fight: Pick<FightResult, "fighters" | "winner">): string {
  const name = namesOf(fight);
  const pts = [...fight.fighters].sort((a, b) => b.points - a.points).map((f) => formatPointsShort(f.points)).join("–");
  if (fight.winner === null) return t("fight.share.text_draw", { names: listText(fight.fighters.map((f) => f.name)) });
  const losers = fight.fighters.filter((f) => f.kol !== fight.winner).map((f) => f.name);
  return t("fight.share.text_win", { winner: name(fight.winner), losers: listText(losers), score: pts });
}
