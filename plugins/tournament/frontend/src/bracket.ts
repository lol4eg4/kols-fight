export type Tier = "goat" | "diamond" | "sendoor" | "degen";

export interface Contestant {
  kol: string;
  name: string;
  rank: number | null;
  score: number;
  tier: Tier;
}

export type MatchDecision = "rounds" | "score_tiebreak" | "rank_tiebreak";

export interface Match {
  a: Contestant;
  b: Contestant;
  points_a: number;
  points_b: number;
  counted_rounds: number;
  winner: string;
  decision: MatchDecision;
}

export type RoundName = "quarterfinals" | "semifinals" | "final";

export interface BracketRound {
  name: RoundName;
  matches: Match[];
}

/** Body of GET /api/tournament. */
export interface TournamentView {
  date: string;
  rounds: BracketRound[];
  champion: Contestant;
}

/** Body of GET /api/tournament/lineups: ids in bracket order; empty = not available. */
export interface Lineups {
  date: string;
  top: string[];
  network: string[];
}

export type Choice = "top" | "network";

/** Path of the bracket for `ids` on `date`. */
export function bracketPath(ids: readonly string[], date: string): string {
  return `/api/tournament?ids=${ids.map(encodeURIComponent).join(",")}&date=${encodeURIComponent(date)}`;
}

export function lineupsPath(date: string): string {
  return `/api/tournament/lineups?date=${encodeURIComponent(date)}`;
}
