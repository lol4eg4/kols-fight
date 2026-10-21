import type { ElementType, ReactNode } from "react";
import { cx } from "../cx";

export type CardTone = "panel" | "sunken" | "fighter" | "gold" | "silver" | "bronze";

export interface CardProps {
  /** Element to render. Default div. */
  as?: ElementType;
  /**
   * panel: surface with border. sunken: cell inside a panel. fighter: pink frame and hard shadow.
   * gold, silver, bronze: podium frames; gold is the first place.
   */
  tone?: CardTone;
  /** Hard shadow, 4px. Default on for fighter and the podium tones. */
  shadow?: boolean;
  children: ReactNode;
  [key: `aria-${string}`]: string | undefined;
  id?: string;
}

export function Card({ as: As = "div", tone = "panel", shadow, children, ...rest }: CardProps) {
  const lifted = shadow ?? (tone !== "panel" && tone !== "sunken");
  return (
    <As {...rest} className={cx("ds-card", `ds-card--${tone}`, lifted && "ds-card--lifted")}>
      {children}
    </As>
  );
}

export interface StatTileProps {
  /** The big figure. Usually a number or a Money. */
  value: ReactNode;
  /** Caption under the figure. */
  label: string;
  /** Colour of the figure: coin for the lead stat. */
  accent?: boolean;
}

/** Big number with a muted caption. */
export function StatTile({ value, label, accent }: StatTileProps) {
  return (
    <div className="ds-card ds-card--panel ds-stat">
      <div className={cx("ds-stat__value t-stat", accent && "ds-stat__value--accent")}>{value}</div>
      <div className="ds-stat__label">{label}</div>
    </div>
  );
}
