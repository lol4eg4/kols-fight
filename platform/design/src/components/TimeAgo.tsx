import { formatMinutesAgo, formatTimeAgo } from "../format";
import { NoData } from "./NoData";

export type TimeAgoProps = {
  /** Why there is no time, shown instead when the value is missing. */
  reason?: string;
  reasonMode?: "tooltip" | "text";
} & (
  | { minutes: number | null | undefined; at?: undefined; now?: undefined }
  | { at: string | number | Date | null | undefined; now?: string | number | Date; minutes?: undefined }
);

/** "2h ago", "5d ago". Give minutes since the event, or the time of the event. */
export function TimeAgo({ minutes, at, now, reason, reasonMode }: TimeAgoProps) {
  if (minutes !== undefined) {
    if (minutes === null || !Number.isFinite(minutes)) return <NoData reason={reason ?? ""} mode={reasonMode} />;
    return <span className="ds-mono-text">{formatMinutesAgo(minutes)}</span>;
  }
  if (at === null || at === undefined) return <NoData reason={reason ?? ""} mode={reasonMode} />;
  const date = new Date(at);
  if (Number.isNaN(date.getTime())) return <NoData reason={reason ?? ""} mode={reasonMode} />;
  return (
    <time className="ds-mono-text" dateTime={date.toISOString()}>
      {formatTimeAgo(date, now)}
    </time>
  );
}
