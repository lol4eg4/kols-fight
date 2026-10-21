// JSON of GET /api/fight, as plugins/fight/api/src/lib.rs serializes it, and the board rows the picker reads.

export type Tier = "goat" | "diamond" | "sendoor" | "degen";
export type Source = "kolexplorer" | "solanatracker";
export type PartKind = "profit" | "accuracy" | "form" | "activity";
export type PartStatus = "counted" | "no_data" | "needs_profit";
export type BadgeKind = "sniper" | "on_fire" | "whale" | "scalper" | "hodl" | "live" | "in_top_today";

export type RoundKind = "score" | "pnl30d" | "pnl7d" | "pnl24h" | "win_rate" | "win_rate7d" | "position_size" | "last_trade";
export type Decision = "rounds" | "score_tiebreak" | "draw";
export type VoidReason = "missing_data" | "all_tied";

export interface Part {
  kind: PartKind;
  raw: number | null;
  all_time: boolean;
  share: number;
  points: number;
  status: PartStatus;
}

export interface ScoreCard {
  score: number;
  tier: Tier;
  parts: Part[];
  badges: BadgeKind[];
}

export interface Fighter {
  kol: string;
  name: string;
  rank: number | null;
  card: ScoreCard;
  points: number;
  hp: number;
  rounds_won: RoundKind[];
}

export interface RoundValue {
  kol: string;
  value: number | null;
  all_time: boolean;
  source: Source | null;
  fetched_at: string | null;
}

export type RoundOutcome =
  | { type: "win"; kol: string }
  | { type: "tie"; kols: string[] }
  | { type: "void"; reason: VoidReason };

export interface Award {
  kol: string;
  points: number;
}

export interface Round {
  number: number;
  kind: RoundKind;
  values: RoundValue[];
  outcome: RoundOutcome;
  awards: Award[];
  damaged: string[];
}

export interface FightResult {
  date: string;
  snapshot_closed_at: string | null;
  fighters: Fighter[];
  rounds: Round[];
  winner: string | null;
  decision: Decision;
  counted_rounds: number;
  counted_line: string;
  summary: string[];
  missing: string[];
}

/** The part of a /api/board row the picker needs. */
export interface BoardKol {
  kol: string;
  name: string;
  x_handle: string | null;
  rank: number;
  score: number;
  tier: Tier;
  in_network: boolean;
}

export interface BoardList {
  date: string;
  dates: string[];
  rows: BoardKol[];
}
