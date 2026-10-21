import type { ReactNode } from "react";
import { cx } from "../cx";
import { Tooltip } from "./Tooltip";

export type BadgeTone = "neutral" | "coin" | "pink" | "blue" | "lavender" | "plus" | "minus" | "muted";

export interface BadgeProps {
  /** Always carry the meaning in the text too: colour alone is never enough. */
  children: ReactNode;
  tone?: BadgeTone;
  /** "chip" has the 4px corner, "pill" is round (network chip, filters). */
  shape?: "chip" | "pill";
  /** Leading emoji or glyph, hidden from screen readers. */
  icon?: string;
  /** Explanation shown on hover, focus and tap. */
  hint?: ReactNode;
}

export function Badge({ children, tone = "neutral", shape = "chip", icon, hint }: BadgeProps) {
  const badge = (
    <span className={cx("ds-badge", `ds-badge--${tone}`, `ds-badge--${shape}`)}>
      {icon && (
        <span className="ds-badge__icon" aria-hidden="true">
          {icon}
        </span>
      )}
      {children}
    </span>
  );
  return hint ? <Tooltip content={hint}>{badge}</Tooltip> : badge;
}
