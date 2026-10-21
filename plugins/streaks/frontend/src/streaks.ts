import { useEffect, useState } from "react";
import { api } from "@arena/shell";
import type { ApiError } from "@arena/shell";

export type Streak = "trade_7d" | "profit_3w";

export interface KolStreaks {
  kol: string;
  badges: Streak[];
}

/** Body of GET /api/streaks. */
export interface StreaksView {
  date: string;
  streaks: KolStreaks[];
}

export type StreaksState = { status: "loading" } | { status: "ready"; data: StreaksView } | { status: "error"; error: ApiError };

// Every row of the board asks for the same day. One request per date serves them all.
const cache = new Map<string, Promise<StreaksState>>();

function load(date: string): Promise<StreaksState> {
  let p = cache.get(date);
  if (!p) {
    p = api<StreaksView>(`/api/streaks?date=${encodeURIComponent(date)}`).then((r): StreaksState => {
      if (r.error) {
        cache.delete(date); // a failed answer is not kept: the next mount asks again
        return { status: "error", error: r.error };
      }
      return { status: "ready", data: r.data };
    });
    cache.set(date, p);
  }
  return p;
}

/** The streak badges of `date`. */
export function useStreaks(date: string): StreaksState {
  const [state, setState] = useState<StreaksState>({ status: "loading" });
  useEffect(() => {
    let live = true;
    setState({ status: "loading" });
    void load(date).then((s) => live && setState(s));
    return () => {
      live = false;
    };
  }, [date]);
  return state;
}
