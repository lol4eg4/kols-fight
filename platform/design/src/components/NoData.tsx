import { Tooltip } from "./Tooltip";

export interface NoDataProps {
  /** Why there is no figure, for example "Solana Tracker does not show 24-hour PnL". */
  reason: string;
  /**
   * tooltip: the reason opens on hover, focus and tap (tables).
   * text: the reason is written under the dash (KOL card).
   */
  mode?: "tooltip" | "text";
}

/** "—" in muted colour with its reason. Never a zero. */
export function NoData({ reason, mode = "tooltip" }: NoDataProps) {
  if (mode === "text") {
    return (
      <span className="ds-nodata ds-nodata--text">
        <span className="ds-nodata__dash">—</span>
        <span className="ds-nodata__reason">{reason}</span>
      </span>
    );
  }
  return (
    <Tooltip content={reason}>
      <span className="ds-nodata__dash ds-nodata__dash--tip">
        <span aria-hidden="true">—</span>
        <span className="ds-sr-only">{reason}</span>
      </span>
    </Tooltip>
  );
}
