import { cx } from "../cx";

export type Tier = "goat" | "diamond" | "sendoor" | "degen";

const TIERS: Record<Tier, { icon: string; name: string }> = {
  goat: { icon: "🐐", name: "GOAT" },
  diamond: { icon: "💎", name: "DIAMOND" },
  sendoor: { icon: "🚀", name: "SENDOOR" },
  degen: { icon: "🧪", name: "DEGEN" },
};

export interface TierBadgeProps {
  tier: Tier;
}

/** Tier chip: icon, the tier name in pixel type, tier colour. The word always shows, so colour is never alone. */
export function TierBadge({ tier }: TierBadgeProps) {
  const { icon, name } = TIERS[tier];
  return (
    <span className={cx("ds-tier", `ds-tier--${tier}`)}>
      <span aria-hidden="true">{icon}</span>
      {name}
    </span>
  );
}
