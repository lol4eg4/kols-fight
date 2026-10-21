import { useCallback, useEffect, useState } from "react";
import { api } from "@arena/shell";
import type { ApiError } from "@arena/shell";

export type Tier = "goat" | "diamond" | "sendoor" | "degen";

export interface DuelFighter {
  kol: string;
  name: string;
  rank: number | null;
  score: number;
  tier: Tier;
  points: number;
  hp: number;
}

/** Body of GET /api/duel. */
export interface DuelView {
  date: string;
  fighters: DuelFighter[];
  winner: string | null;
  counted_rounds: number;
}

export type DuelState = { status: "loading" } | { status: "ready"; data: DuelView } | { status: "error"; error: ApiError };

const cache = new Map<string, Promise<DuelState>>();

function load(date: string, fresh: boolean): Promise<DuelState> {
  if (fresh) cache.delete(date);
  let p = cache.get(date);
  if (!p) {
    p = api<DuelView>(`/api/duel?date=${encodeURIComponent(date)}`).then((r): DuelState => {
      if (r.error) {
        cache.delete(date); // a failed answer is not kept
        return { status: "error", error: r.error };
      }
      return { status: "ready", data: r.data };
    });
    cache.set(date, p);
  }
  return p;
}

/** The duel of `date`, and a way to ask again after an error. */
export function useDuel(date: string): { state: DuelState; retry: () => void } {
  const [state, setState] = useState<DuelState>({ status: "loading" });
  const [tick, setTick] = useState(0);
  useEffect(() => {
    let live = true;
    setState({ status: "loading" });
    void load(date, tick > 0).then((s) => live && setState(s));
    return () => {
      live = false;
    };
  }, [date, tick]);
  const retry = useCallback(() => setTick((n) => n + 1), []);
  return { state, retry };
}
