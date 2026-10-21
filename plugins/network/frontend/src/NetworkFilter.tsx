import { useEffect, useRef } from "react";
import { ErrorLine } from "@arena/design";
import { t } from "@arena/shell";
import { useNetwork } from "./useNetwork";
import { networkIds } from "./view";

/** Fills board/network_filter: tells the leaderboard which KOLs are in our network. Draws only the error line, with a retry, when the network does not load. */
export default function NetworkFilter({ date, onIds }: { date: string; onIds: (ids: string[]) => void }) {
  const state = useNetwork(date);
  const report = useRef(onIds);
  report.current = onIds;
  const data = state.status === "ready" ? state.data : null;

  useEffect(() => {
    if (data) report.current(networkIds(data));
  }, [data]);

  if (state.status === "error") {
    return <ErrorLine message={t("network.error.load", { message: state.error.message })} onRetry={state.reload} retryLabel={t("network.retry")} />;
  }
  return null;
}
