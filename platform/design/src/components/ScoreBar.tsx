import { cx } from "../cx";
import { Progress } from "./Progress";
import type { Tier } from "./TierBadge";

export interface ScoreBarProps {
  /** 0 to 100. */
  score: number;
  /** Colours the bar: goat yellow, diamond blue, sendoor pink, degen lavender. */
  tier: Tier;
  /** Names the bar for screen readers, for example "Arena Score". */
  label: string;
  /** sm: number then bar, for rows. lg: big number over the bar, for the card. */
  size?: "sm" | "lg";
}

/** Score as a number and a bar on a 0 to 100 scale. */
export function ScoreBar({ score, tier, label, size = "sm" }: ScoreBarProps) {
  return (
    <span className={cx("ds-scorebar", `ds-scorebar--${size}`)}>
      <span className="ds-scorebar__value">{score}</span>
      <Progress value={score} max={100} label={label} tone={tierTone[tier]} valueText={`${score}`} />
    </span>
  );
}

const tierTone = { goat: "coin", diamond: "blue", sendoor: "pink", degen: "lavender" } as const;
