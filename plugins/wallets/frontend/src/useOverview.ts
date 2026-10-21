import { useEffect, useState } from "react";
import { api } from "@arena/shell";
import type { ApiError, ApiResult } from "@arena/shell";
import { createCache } from "./overview";
import type { Overview } from "./overview";

const KEY = "overview";
const TTL_MS = 30_000;

// One overview for the screen and every wallet_status filler of the Network screen.
const cache = createCache<ApiResult<Overview>>(() => api<Overview>("/api/team/wallets"), TTL_MS);
const listeners = new Set<() => void>();

/** After a write: take the overview the server returned, or load it again. Every user redraws. */
export function refreshOverview(fresh?: Overview) {
  if (fresh) cache.put(KEY, { data: fresh });
  else cache.drop(KEY);
  listeners.forEach((fn) => fn());
}

export type OverviewLoad = { status: "loading" } | { status: "ready"; data: Overview } | { status: "error"; error: ApiError };

export function useOverview(): { load: OverviewLoad; reload: () => void } {
  const [load, setLoad] = useState<OverviewLoad>({ status: "loading" });
  const [tick, setTick] = useState(0);

  useEffect(() => {
    const fn = () => setTick((n) => n + 1);
    listeners.add(fn);
    return () => {
      listeners.delete(fn);
    };
  }, []);

  useEffect(() => {
    let alive = true;
    void cache.get(KEY).then((r) => {
      if (!alive) return;
      // a reload keeps the last good overview on screen until the new one comes
      if (r.error) setLoad({ status: "error", error: r.error });
      else setLoad({ status: "ready", data: r.data });
    });
    return () => {
      alive = false;
    };
  }, [tick]);

  return { load, reload: () => refreshOverview() };
}
