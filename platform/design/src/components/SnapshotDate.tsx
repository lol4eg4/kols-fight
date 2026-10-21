import { formatSnapshotDate } from "../format";
import { NoData } from "./NoData";

export interface SnapshotDateProps {
  /** ISO time or a plain day "2026-10-18". null means unknown. */
  at: string | number | Date | null | undefined;
  /** Leave the year out. Default false. */
  short?: boolean;
  reason?: string;
}

/** Snapshot time in UTC: "Oct 18, 2026, 06:00 UTC". */
export function SnapshotDate({ at, short, reason }: SnapshotDateProps) {
  if (at === null || at === undefined) return <NoData reason={reason ?? ""} mode="text" />;
  let text: string;
  try {
    text = formatSnapshotDate(at, { year: !short });
  } catch {
    return <NoData reason={reason ?? ""} mode="text" />;
  }
  const iso = typeof at === "string" && /^\d{4}-\d{2}-\d{2}$/.test(at) ? at : new Date(at).toISOString();
  return (
    <time className="ds-mono-text" dateTime={iso}>
      {text}
    </time>
  );
}
