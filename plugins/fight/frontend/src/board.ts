// Today's board, as the picker and the add-to-fight buttons need it: names, ranks, tiers, the network mark.
// Loaded once per page and shared; a failed load is not kept, so "Try again" asks the server again.

import { useCallback, useEffect, useState } from "react";
import { api } from "@arena/shell";
import type { ApiResult, ApiState } from "@arena/shell";
import type { BoardKol, BoardList } from "./types";

let pending: Promise<ApiResult<BoardList>> | null = null;
let done: ApiResult<BoardList> | null = null;

function load(): Promise<ApiResult<BoardList>> {
  if (!pending) {
    pending = api<BoardList>("/api/board").then((r) => {
      if (r.error) pending = null;
      else done = r;
      return r;
    });
  }
  return pending;
}

function toState(r: ApiResult<BoardList>): ApiState<BoardList> {
  return r.error ? { status: "error", error: r.error } : { status: "ready", data: r.data };
}

export function useBoard(): ApiState<BoardList> & { reload: () => void } {
  const [state, setState] = useState<ApiState<BoardList>>(() => (done ? toState(done) : { status: "loading" }));
  const [tick, setTick] = useState(0);
  useEffect(() => {
    let live = true;
    if (done) {
      setState(toState(done));
      return;
    }
    setState({ status: "loading" });
    void load().then((r) => {
      if (live) setState(toState(r));
    });
    return () => {
      live = false;
    };
  }, [tick]);
  const reload = useCallback(() => setTick((n) => n + 1), []);
  return { ...state, reload };
}

/** Rows by KOL id. */
export function byKol(rows: readonly BoardKol[]): Map<string, BoardKol> {
  return new Map(rows.map((r) => [r.kol, r]));
}
