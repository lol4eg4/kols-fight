import { Badge, TimeAgo } from "@arena/design";
import { t } from "@arena/shell";
import { skippedLines } from "./members";
import type { SyncRun } from "./members";

/** The last sheet read: when, how it went, what changed. Never the sheet URL. */
export function SyncSummary({ run, details }: { run: SyncRun | null; details?: boolean }) {
  if (run === null) return <p>{t("roster.sync.never")}</p>;
  const ok = run.status === "ok";
  const skipped = skippedLines(run);
  return (
    <div className="ros-sync">
      <p>
        <Badge tone={ok ? "plus" : "minus"} icon={ok ? "✓" : "✕"}>
          {ok ? t("roster.sync.ok") : t("roster.sync.failed")}
        </Badge>{" "}
        {t("roster.sync.at")} <TimeAgo at={run.finished_at} />
      </p>
      {ok && <p>{t("roster.sync.counts", { rows: run.rows, added: run.added, updated: run.updated, removed: run.removed })}</p>}
      {run.error && <p className="ds-error__message">{run.error}</p>}
      {run.note && <p className="ds-error__message">{t("roster.sync.note", { note: run.note })}</p>}
      {skipped.length > 0 &&
        (details ? (
          <details>
            <summary>{t("roster.sync.skipped", { count: skipped.length })}</summary>
            <ul>
              {skipped.map((s) => (
                <li key={`${s.line}-${s.reason}`}>{t("roster.sync.skipped_line", { line: s.line, reason: s.reason })}</li>
              ))}
            </ul>
          </details>
        ) : (
          <p>{t("roster.sync.skipped", { count: skipped.length })}</p>
        ))}
    </div>
  );
}
