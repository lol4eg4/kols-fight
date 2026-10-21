import { useCallback, useEffect, useState } from "react";
import { api } from "@arena/shell";
import type { ApiError } from "@arena/shell";
import { createCache, networkPath } from "./view";
import type { NetworkView } from "./view";

const cache = createCache<NetworkView>((path) => api<NetworkView>(path), 60_000);

export type NetworkState = { status: "loading" } | { status: "ready"; data: NetworkView } | { status: "error"; error: ApiError };

/** The «Our network» data of a board date, loaded once for both fillers. */
export function useNetwork(date: string): NetworkState & { reload: () => void } {
  const [state, setState] = useState<NetworkState>({ status: "loading" });
  const [round, setRound] = useState(0);
  const path = networkPath(date);

  useEffect(() => {
    let alive = true;
    setState({ status: "loading" });
    void cache.get(path).then((r) => {
      if (!alive) return;
      if (r.error !== undefined) setState({ status: "error", error: r.error as ApiError });
      else setState({ status: "ready", data: r.data as NetworkView });
    });
    return () => {
      alive = false;
    };
  }, [path, round]);

  const reload = useCallback(() => {
    cache.drop(path);
    setRound((n) => n + 1);
  }, [path]);

  return { ...state, reload };
}
