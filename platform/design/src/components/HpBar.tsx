import { cx } from "../cx";

export interface HpBarProps {
  /** Segments left. */
  hp: number;
  /** Segments in all: one per round. Default 8. */
  max?: number;
  /** Read by screen readers, for example "HP 5 of 8". */
  label: string;
}

/** Health as 8 segments. A lost segment turns dark. */
export function HpBar({ hp, max = 8, label }: HpBarProps) {
  const left = Math.min(Math.max(Math.round(hp), 0), max);
  return (
    <div className="ds-hp" role="img" aria-label={label}>
      {Array.from({ length: max }, (_, i) => (
        <span key={i} className={cx("ds-hp__seg", i >= left && "ds-hp__seg--lost")} />
      ))}
    </div>
  );
}
