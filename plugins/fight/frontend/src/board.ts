// The board the picker and the add-to-fight buttons need: names, ranks, tiers, the network mark.
// Today's board, or the date the home page shows. Loaded once per date and shared; a failed load is
// not kept, so "Try again" asks the server again.

import { useCallback, useEffect, useState } from "react";
import { api } from "@arena/shell";
import type { ApiResult, ApiState } from "@arena/shell";
import type { BoardKol, BoardList } from "./types";

const pending = new Map<string, Promise<ApiResult<BoardList>>>();
const done = new Map<string, ApiResult<BoardList>>();

function load(date: string | null): Promise<ApiResult<BoardList>> {
  const key = date ?? "";
  let p = pending.get(key);
  if (!p) {
    p = api<BoardList>(date ? `/api/board?date=${encodeURIComponent(date)}` : "/api/board").then((r) => {
      if (r.error) pending.delete(key);
      else done.set(key, r);
      return r;
    });
    pending.set(key, p);
  }
  return p;
}

function toState(r: ApiResult<BoardList>): ApiState<BoardList> {
  return r.error ? { status: "error", error: r.error } : { status: "ready", data: r.data };
}

/** The board of `date`, or today's board when it is null. */
export function useBoard(date: string | null = null): ApiState<BoardList> & { reload: () => void } {
  const key = date ?? "";
  const [state, setState] = useState<ApiState<BoardList>>(() => {
    const d = done.get(key);
    return d ? toState(d) : { status: "loading" };
  });
  const [tick, setTick] = useState(0);
  useEffect(() => {
    let live = true;
    const d = done.get(key);
    if (d) {
      setState(toState(d));
      return;
    }
    setState({ status: "loading" });
    void load(date).then((r) => {
      if (live) setState(toState(r));
    });
    return () => {
      live = false;
    };
  }, [key, date, tick]);
  const reload = useCallback(() => setTick((n) => n + 1), []);
  return { ...state, reload };
}

/** Rows by KOL id. */
export function byKol(rows: readonly BoardKol[]): Map<string, BoardKol> {
  return new Map(rows.map((r) => [r.kol, r]));
}
