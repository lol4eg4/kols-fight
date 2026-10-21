// Small pieces the home page and the card share.

import { useSyncExternalStore } from "react";
import { Badge, NoData, Tooltip, formatSnapshotDate } from "@arena/design";
import { getRegistry, t } from "@arena/shell";
import type { SlotName, WidgetName } from "@arena/shell";
import { BADGE_ICON, formatPercent, winRate } from "./logic";
import type { SortId } from "./logic";
import type { BadgeKind, Metrics, PartKind, Source, Tier } from "./types";

/** Keys kept as literals so the dictionary test can check them. */
export const TIER_NAME: Record<Tier, () => string> = {
  goat: () => t("board.tier.goat"),
  diamond: () => t("board.tier.diamond"),
  sendoor: () => t("board.tier.sendoor"),
  degen: () => t("board.tier.degen"),
};
export const TIER_RANGE: Record<Tier, () => string> = {
  goat: () => t("board.tier.goat_range"),
  diamond: () => t("board.tier.diamond_range"),
  sendoor: () => t("board.tier.sendoor_range"),
  degen: () => t("board.tier.degen_range"),
};
export const BADGE_NAME: Record<BadgeKind, () => string> = {
  sniper: () => t("board.badge.sniper"),
  on_fire: () => t("board.badge.on_fire"),
  whale: () => t("board.badge.whale"),
  scalper: () => t("board.badge.scalper"),
  hodl: () => t("board.badge.hodl"),
  live: () => t("board.badge.live"),
  in_top_today: () => t("board.badge.in_top_today"),
};
export const BADGE_HINT: Record<BadgeKind, () => string> = {
  sniper: () => t("board.badge.sniper_hint"),
  on_fire: () => t("board.badge.on_fire_hint"),
  whale: () => t("board.badge.whale_hint"),
  scalper: () => t("board.badge.scalper_hint"),
  hodl: () => t("board.badge.hodl_hint"),
  live: () => t("board.badge.live_hint"),
  in_top_today: () => t("board.badge.in_top_today_hint"),
};
export const SOURCE_NAME: Record<Source, () => string> = {
  kolexplorer: () => t("board.source.kolexplorer"),
  solanatracker: () => t("board.source.solanatracker"),
};
export const SOURCE_SHORT: Record<Source, () => string> = {
  kolexplorer: () => t("board.source.kolexplorer_short"),
  solanatracker: () => t("board.source.solanatracker_short"),
};
export const SORT_NAME: Record<SortId, () => string> = {
  rank: () => t("board.sort.rank"),
  d30: () => t("board.sort.d30"),
  d7: () => t("board.sort.d7"),
  d1: () => t("board.sort.d1"),
  winrate: () => t("board.sort.winrate"),
  last: () => t("board.sort.last"),
};
export const PART_NAME: Record<PartKind, () => string> = {
  profit: () => t("board.part.profit"),
  accuracy: () => t("board.part.accuracy"),
  form: () => t("board.part.form"),
  activity: () => t("board.part.activity"),
};
export const PART_SCALE: Record<PartKind, () => string> = {
  profit: () => t("board.part.profit_scale"),
  accuracy: () => t("board.part.accuracy_scale"),
  form: () => t("board.part.form_scale"),
  activity: () => t("board.part.activity_scale"),
};

/** Snapshot time as text, or "" when it cannot be read. */
export function dateText(at: string | null | undefined): string {
  if (!at) return "";
  try {
    return formatSnapshotDate(at);
  } catch {
    return "";
  }
}

/** Why a metric shows "—". */
export function noDataReason(metrics: Metrics | null, source: Source | null, field?: keyof Metrics): string {
  if (!metrics) return t("board.nodata.none");
  if (field === "d1" && source === "solanatracker") return t("board.nodata.d1");
  if (!source) return t("board.nodata.plain");
  return t("board.nodata.metric", { source: SOURCE_NAME[source]() });
}

export function hasFill(name: SlotName): boolean {
  return (getRegistry().fills.get(name)?.length ?? 0) > 0;
}

export function hasWidget(name: WidgetName): boolean {
  return getRegistry().widgets.has(name);
}

function subscribeMedia(query: string) {
  return (fn: () => void) => {
    const m = window.matchMedia(query);
    m.addEventListener("change", fn);
    return () => m.removeEventListener("change", fn);
  };
}

/** True while the media query matches. */
export function useMedia(query: string): boolean {
  return useSyncExternalStore(
    subscribeMedia(query),
    () => window.matchMedia(query).matches,
    () => false,
  );
}

export function KolBadges({ badges, grouped }: { badges: BadgeKind[]; grouped?: boolean }) {
  if (badges.length === 0) return null;
  // on phone cards one tip holds all badges: one touch target instead of one per icon
  if (grouped)
    return (
      <Tooltip
        content={badges.map((b) => (
          <span key={b} className="board-tip-line">
            {BADGE_ICON[b]} {BADGE_NAME[b]()}: {BADGE_HINT[b]()}
          </span>
        ))}
      >
        <span className="board-badges" role="img" aria-label={badges.map((b) => BADGE_NAME[b]()).join(", ")}>
          {badges.map((b) => (
            <span key={b} className="board-badge-icon" aria-hidden="true">
              {BADGE_ICON[b]}
            </span>
          ))}
        </span>
      </Tooltip>
    );
  return (
    <span className="board-badges">
      {badges.map((b) => (
        <Tooltip key={b} content={`${BADGE_NAME[b]()}: ${BADGE_HINT[b]()}`}>
          <span className="board-badge-icon" role="img" aria-label={BADGE_NAME[b]()}>
            {BADGE_ICON[b]}
          </span>
        </Tooltip>
      ))}
    </span>
  );
}

export function NetworkChip() {
  return (
    <Badge shape="pill" icon="🤝" hint={t("board.network_hint")}>
      {t("board.network")}
    </Badge>
  );
}

export function StaleMark({ at, compact }: { at: string | null; compact?: boolean }) {
  if (compact)
    return (
      <Tooltip content={t("board.stale_hint", { date: dateText(at) })}>
        <span className="board-badge-icon" role="img" aria-label={t("board.stale")}>
          ⏳
        </span>
      </Tooltip>
    );
  return (
    <Badge tone="muted" icon="⏳" hint={t("board.stale_hint", { date: dateText(at) })}>
      {t("board.stale")}
    </Badge>
  );
}

export function SourceChip({ source, at }: { source: Source | null; at: string | null }) {
  if (!source) return <Badge tone="muted">{t("board.source.none")}</Badge>;
  return (
    <Badge tone="muted" hint={t("board.source.fetched", { source: SOURCE_NAME[source](), date: dateText(at) })}>
      {SOURCE_SHORT[source]()}
    </Badge>
  );
}

/** Win rate: wr30, or the all-time rate with an asterisk and its explanation. */
export function WinRate({ metrics, reason }: { metrics: Metrics | null; reason: string }) {
  const wr = winRate(metrics);
  if (wr.value === null) return <NoData reason={reason} />;
  if (!wr.allTime) return <span className="ds-mono-text">{formatPercent(wr.value)}</span>;
  return (
    <Tooltip content={t("board.wr.all_time")}>
      <span className="ds-mono-text">
        {formatPercent(wr.value)}
        <span aria-hidden="true">*</span>
      </span>
    </Tooltip>
  );
}
