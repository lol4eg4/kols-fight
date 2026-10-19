import { cx } from "../cx";

export interface SkeletonProps {
  /** Read by screen readers instead of the grey bars, for example "Loading the board". */
  label: string;
  /** lines: text bars. rows: taller bars like list rows. block: one big panel. */
  variant?: "lines" | "rows" | "block";
  /** How many bars. Default 3. */
  count?: number;
}

/** Grey bars in place of the content. No spinner. */
export function Skeleton({ label, variant = "lines", count = 3 }: SkeletonProps) {
  const n = variant === "block" ? 1 : Math.max(1, count);
  return (
    <div className={cx("ds-skeleton", `ds-skeleton--${variant}`)} role="status" aria-live="polite">
      <span className="ds-sr-only">{label}</span>
      {Array.from({ length: n }, (_, i) => (
        <span key={i} className="ds-skeleton__bar" aria-hidden="true" style={variant === "lines" && i === n - 1 && n > 1 ? { width: "60%" } : undefined} />
      ))}
    </div>
  );
}
