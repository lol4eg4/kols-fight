import type { ReactNode } from "react";
import { cx } from "../cx";

export interface ListProps {
  /** Names the list for screen readers. */
  label: string;
  /** "cards" draws each item as a separate card with a gap (phone layouts). */
  variant?: "rows" | "cards";
  children: ReactNode;
}

/** Dense list of rows for team screens. Items are ListItem. */
export function List({ label, variant = "rows", children }: ListProps) {
  return (
    <ul className={cx("ds-list", `ds-list--${variant}`)} aria-label={label}>
      {children}
    </ul>
  );
}

export interface ListItemProps {
  children: ReactNode;
  /** Makes the whole row one button. */
  onSelect?: () => void;
  /** Marks the row that is open now. */
  current?: boolean;
}

export function ListItem({ children, onSelect, current }: ListItemProps) {
  return (
    <li className={cx("ds-list__item", current && "ds-list__item--current")}>
      {onSelect ? (
        <button type="button" className="ds-list__hit" onClick={onSelect} aria-current={current || undefined}>
          {children}
        </button>
      ) : (
        <div className="ds-list__body">{children}</div>
      )}
    </li>
  );
}
