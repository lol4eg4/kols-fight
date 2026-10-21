import { useEffect, useRef } from "react";
import { useNetwork } from "./useNetwork";
import { networkIds } from "./view";

/** Fills board/network_filter: tells the leaderboard which KOLs are in our network. Draws nothing. */
export default function NetworkFilter({ date, onIds }: { date: string; onIds: (ids: string[]) => void }) {
  const state = useNetwork(date);
  const report = useRef(onIds);
  report.current = onIds;
  const data = state.status === "ready" ? state.data : null;

  useEffect(() => {
    if (data) report.current(networkIds(data));
  }, [data]);

  return null;
}
