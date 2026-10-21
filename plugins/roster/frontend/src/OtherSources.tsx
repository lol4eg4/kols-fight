import { useEffect, useState } from "react";
import { Card, ErrorLine, Heading, Skeleton } from "@arena/design";
import { api, t } from "@arena/shell";
import type { ApiError } from "@arena/shell";
import type { TeamNetwork } from "./members";
import { SyncSummary } from "./sync";
import "./roster.css";

type Load = { status: "loading" } | { status: "ready"; data: TeamNetwork } | { status: "error"; error: ApiError };

/** Filler of schedule/other_sources: the network sheet as a source on the Collection screen. */
export default function OtherSources() {
  const [load, setLoad] = useState<Load>({ status: "loading" });
  const [tick, setTick] = useState(0);

  useEffect(() => {
    let alive = true;
    setLoad({ status: "loading" });
    void api<TeamNetwork>("/api/team/network").then((r) => {
      if (!alive) return;
      setLoad(r.error ? { status: "error", error: r.error } : { status: "ready", data: r.data });
    });
    return () => {
      alive = false;
    };
  }, [tick]);

  return (
    <Card>
      <Heading level={3} variant="title">
        {t("roster.source.title")}
      </Heading>
      {load.status === "loading" && <Skeleton label={t("roster.source.loading")} variant="lines" count={2} />}
      {load.status === "error" && <ErrorLine message={t("roster.error.load", { message: load.error.message })} onRetry={() => setTick((n) => n + 1)} retryLabel={t("roster.retry")} />}
      {load.status === "ready" && (
        <>
          <SyncSummary run={load.data.last_sync} />
          <p>{t("roster.source.members", { count: load.data.members.length })}</p>
        </>
      )}
    </Card>
  );
}
