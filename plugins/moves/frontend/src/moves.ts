import { useEffect, useState } from "react";
import { api } from "@arena/shell";
import type { ApiError } from "@arena/shell";

/** Rank change of one KOL. Positive = climbed. `null` = not known. */
export interface Change {
  kol: string;
  day: number | null;
  week: number | null;
}

export interface Climb {
  kol: string;
  name: string | null;
  places: number;
}

/** Body of GET /api/moves. */
export interface MovesView {
  date: string;
  changes: Change[];
  day_climb: Climb | null;
  week_climb: Climb | null;
}

export type MovesState = { status: "loading" } | { status: "ready"; data: MovesView } | { status: "error"; error: ApiError };

// Every row of the board asks for the same day. One request per date serves them all.
const cache = new Map<string, Promise<MovesState>>();

function load(date: string | undefined, fresh: boolean): Promise<MovesState> {
  const key = date ?? "";
  if (fresh) cache.delete(key);
  let p = cache.get(key);
  if (!p) {
    p = api<MovesView>(date ? `/api/moves?date=${encodeURIComponent(date)}` : "/api/moves").then((r): MovesState => {
      if (r.error) {
        cache.delete(key); // a failed answer is not kept: the next mount asks again
        return { status: "error", error: r.error };
      }
      return { status: "ready", data: r.data };
    });
    cache.set(key, p);
  }
  return p;
}

/** The moves of `date` (the latest day when absent). */
export function useMoves(date: string | undefined): MovesState {
  const [state, setState] = useState<MovesState>({ status: "loading" });
  useEffect(() => {
    let live = true;
    setState({ status: "loading" });
    void load(date, false).then((s) => live && setState(s));
    return () => {
      live = false;
    };
  }, [date]);
  return state;
}
