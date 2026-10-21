import type { ReactNode } from "react";
import { cx } from "../cx";
import { formatPoints } from "../format";
import type { Tier } from "./TierBadge";

export interface ScorePart {
  id: string;
  /** Name of the part, for example "Profit". */
  label: string;
  /** Weight of the part, for example "40%". */
  weightLabel?: string;
  /** The raw value as the reader should see it, for example a Money. null means no data. */
  raw: ReactNode | null;
  /** Where the raw value sits on the part's scale, 0 to 1. null means no data. */
  share: number | null;
  /** Points earned. null means no data and counts as 0. */
  points: number | null;
  /** Most points the part can give. */
  max: number;
}

export interface ScoreBreakdownLabels {
  part: string;
  raw: string;
  scale: string;
  points: string;
  /** Row before the total: "Sum of parts". */
  sum: string;
  /** Last row: "Arena Score". */
  score: string;
  /** Shown for a part without data: "no data". */
  noData: string;
}

export interface ScoreBreakdownProps {
  parts: ScorePart[];
  /** Sum of the parts' points. */
  sum: number;
  /** The final Score, 0 to 100. */
  score: number;
  tier: Tier;
  labels: ScoreBreakdownLabels;
}

const tone = { goat: "coin", diamond: "blue", sendoor: "pink", degen: "lavender" } as const;

/** Why a KOL has this Score: four parts with raw value, share of the scale and points, then the sum and the result. */
export function ScoreBreakdown({ parts, sum, score, tier, labels }: ScoreBreakdownProps) {
  return (
    <div className="ds-breakdown">
      <div className="ds-breakdown__head" aria-hidden="true">
        <span>{labels.part}</span>
        <span>{labels.raw}</span>
        <span>{labels.scale}</span>
        <span>{labels.points}</span>
      </div>
      <ul className="ds-breakdown__rows">
        {parts.map((p) => {
          const empty = p.raw === null || p.share === null || p.points === null;
          return (
            <li key={p.id} className={cx("ds-breakdown__row", empty && "ds-breakdown__row--empty")}>
              <span className="ds-breakdown__part">
                {p.label}
                {p.weightLabel && <span className="ds-breakdown__weight">{p.weightLabel}</span>}
              </span>
              <span className="ds-breakdown__raw">
                <span className="ds-sr-only">{labels.raw}: </span>
                {empty ? <span className="ds-breakdown__nodata">{labels.noData}</span> : p.raw}
              </span>
              <span className="ds-breakdown__scale">
                <span className="ds-sr-only">{labels.scale}: </span>
                <span className="ds-breakdown__track" aria-hidden="true">
                  <span
                    className={cx("ds-breakdown__fill", `ds-progress__fill--${tone[tier]}`)}
                    style={{ width: `${Math.min(Math.max(p.share ?? 0, 0), 1) * 100}%` }}
                  />
                </span>
                {!empty && <span className="ds-sr-only">{Math.round(Math.min(Math.max(p.share ?? 0, 0), 1) * 100)}%</span>}
              </span>
              <span className="ds-breakdown__points">
                <span className="ds-sr-only">{labels.points}: </span>
                {formatPoints(empty ? 0 : (p.points ?? 0))}
                <span className="ds-breakdown__max"> / {p.max}</span>
              </span>
            </li>
          );
        })}
      </ul>
      <dl className="ds-breakdown__totals">
        <div className="ds-breakdown__total">
          <dt>{labels.sum}</dt>
          <dd>{formatPoints(sum)}</dd>
        </div>
        <div className="ds-breakdown__total ds-breakdown__total--score">
          <dt>{labels.score}</dt>
          <dd>{score}</dd>
        </div>
      </dl>
    </div>
  );
}
