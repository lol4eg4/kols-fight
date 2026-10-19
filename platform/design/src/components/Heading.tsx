import type { ReactNode } from "react";
import { cx } from "../cx";

export interface HeadingProps {
  /** Document level: 1 to 6. Pick it for the outline of the page, not for the size. */
  level: 1 | 2 | 3 | 4 | 5 | 6;
  /**
   * display: hero title, big pixel type; coin yellow with the hard pink shadow (KOL ARENA).
   * section: pixel section title (How Arena Score works, KOL FIGHT).
   * title: Golos Text 600, for card and panel names.
   */
  variant?: "display" | "section" | "title";
  /** coin (default for display and section) or pink (the fight section, K.O.). */
  tone?: "coin" | "pink" | "text";
  children: ReactNode;
  id?: string;
}

/** Headings in the three heading voices. Pixel type is for titles only, never for paragraphs or figures. */
export function Heading({ level, variant = "section", tone, children, id }: HeadingProps) {
  const Tag = `h${level}` as const;
  const colour = tone ?? (variant === "title" ? "text" : "coin");
  return (
    <Tag id={id} className={cx("ds-heading", `ds-heading--${variant}`, `ds-heading--${colour}`)}>
      {children}
    </Tag>
  );
}
