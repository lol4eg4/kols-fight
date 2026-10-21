import { cx } from "../cx";

export type ProgressTone = "coin" | "plus" | "pink" | "blue" | "lavender" | "muted";

export interface ProgressProps {
  value: number;
  max?: number;
  /** Names the bar for screen readers, for example "Arena Score". */
  label: string;
  tone?: ProgressTone;
  /** Text read instead of the plain number, for example "55 of 100". */
  valueText?: string;
}

/** 8px bar on a sunken track. The fill grows over 400ms unless the system asks for no motion. */
export function Progress({ value, max = 100, label, tone = "coin", valueText }: ProgressProps) {
  const clamped = Number.isFinite(value) ? Math.min(Math.max(value, 0), max) : 0;
  return (
    <div
      className="ds-progress"
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={max}
      aria-valuenow={clamped}
      aria-valuetext={valueText}
    >
      <div className={cx("ds-progress__fill", `ds-progress__fill--${tone}`)} style={{ width: `${(clamped / max) * 100}%` }} />
    </div>
  );
}
