// JSON of the board routes, as plugins/board/backend/src/dto.rs serializes it.

export type Tier = "goat" | "diamond" | "sendoor" | "degen";
export type BadgeKind = "sniper" | "on_fire" | "whale" | "scalper" | "hodl" | "live" | "in_top_today";
export type Source = "kolexplorer" | "solanatracker";
export type PartKind = "profit" | "accuracy" | "form" | "activity";
export type PartStatus = "counted" | "no_data" | "needs_profit";

/** USD; win rates 0..1; null = no data. */
export interface Metrics {
  d1: number | null;
  d7: number | null;
  d30: number | null;
  wr7: number | null;
  wr30: number | null;
  wr_all: number | null;
  avg_pos: number | null;
  /** Median hold, seconds. */
  hold_s: number | null;
  /** Minutes since the last trade at fetch. */
  last_min_at_fetch: number | null;
  unreal: number | null;
  total: number | null;
  vol30: number | null;
  days30: number | null;
}

export interface BoardRow {
  kol: string;
  name: string;
  x_handle: string | null;
  rank: number;
  score: number;
  tier: Tier;
  badges: BadgeKind[];
  top100: boolean;
  in_network: boolean;
  source: Source | null;
  fetched_at: string | null;
  stale: boolean;
  metrics: Metrics | null;
}

export interface BoardStats {
  tracked: number;
  up_30d: number;
  traded_last_hour: number;
  total_d30: number;
}

export interface SourceDate {
  source: Source;
  fetched_at: string;
  kols: number;
}

export type TickerKind = "top_score" | "best_week" | "traded_last_hour" | "up_30d";

export interface BoardTickerItem {
  kind: TickerKind;
  kol: string | null;
  name: string | null;
  value: number;
}

export interface Board {
  date: string;
  closed_at: string | null;
  dates: string[];
  stats: BoardStats;
  sources: SourceDate[];
  ticker: BoardTickerItem[];
  rows: BoardRow[];
}

export interface Part {
  kind: PartKind;
  raw: number | null;
  all_time: boolean;
  share: number;
  points: number;
  status: PartStatus;
}

export interface Links {
  kolexplorer: string | null;
  solanatracker: boolean;
  kolscan: boolean;
  fomo: string | null;
  pumpfun: string | null;
  telegram: string | null;
}

export interface Wallet {
  address: string;
  main: boolean;
  solscan_url: string;
}

export interface KolCard {
  kol: string;
  name: string;
  x_handle: string | null;
  date: string;
  rank: number;
  ranked: number;
  top100: boolean;
  in_network: boolean;
  score: number;
  tier: Tier;
  parts: Part[];
  badges: BadgeKind[];
  source: Source | null;
  fetched_at: string | null;
  stale: boolean;
  metrics: Metrics | null;
  formats: string[];
  links: Links;
  wallets: Wallet[];
}
