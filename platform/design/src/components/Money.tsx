import { cx } from "../cx";
import { moneyParts } from "../format";
import type { MoneyMode } from "../format";
import { NoData } from "./NoData";

export interface MoneyProps {
  /** Dollars. null or undefined means no data. */
  value: number | null | undefined;
  /** short: +$207.1K (tables). full: +$207,060 (card). */
  mode?: MoneyMode;
  /** false for an amount that is not a result (position size, volume): no plus, no colour. Default true. */
  signed?: boolean;
  /** Required when the value can be missing: shown instead of the figure. */
  reason?: string;
  /** Where the reason goes when there is no data. Default tooltip. */
  reasonMode?: "tooltip" | "text";
}

/** Dollar amount in mono type. Profit is green with +, loss is red with −: the sign always carries the meaning. */
export function Money({ value, mode = "short", signed = true, reason, reasonMode = "tooltip" }: MoneyProps) {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return <NoData reason={reason ?? ""} mode={reasonMode} />;
  }
  const { text, sign } = moneyParts(value, mode, { signed });
  return <span className={cx("ds-money", `ds-money--${sign}`)}>{text}</span>;
}
