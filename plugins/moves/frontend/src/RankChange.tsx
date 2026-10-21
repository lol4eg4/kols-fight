import { Badge, Skeleton } from "@arena/design";
import { t } from "@arena/shell";
import type { SlotProps } from "@arena/shell";
import { useMoves } from "./moves";

type Period = "day" | "week";

function phrase(n: number, period: Period): string {
  if (period === "day") return n > 0 ? t("moves.up_day", { n }) : n < 0 ? t("moves.down_day", { n: -n }) : t("moves.same_day");
  return n > 0 ? t("moves.up_week", { n }) : n < 0 ? t("moves.down_week", { n: -n }) : t("moves.same_week");
}

function look(n: number): { tone: "plus" | "minus" | "muted"; icon: string } {
  if (n > 0) return { tone: "plus", icon: "▲" };
  if (n < 0) return { tone: "minus", icon: "▼" };
  return { tone: "muted", icon: "=" };
}

/** The arrow in a board row: ▲3 / ▼2 against yesterday, or against 7 days ago when yesterday is not known. */
export function RankChange({ date, kol }: SlotProps<"board/rank_change">) {
  const state = useMoves(date);
  if (state.status === "loading") return <Skeleton label={t("moves.loading")} variant="lines" count={1} />;
  // No snapshot, or a failed load: no arrow. The board shows its own error, not one per row.
  if (state.status === "error") return null;
  const c = state.data.changes.find((x) => x.kol === kol);
  if (!c) return null; // not fresh on both days: no arrow
  const period: Period = c.day !== null ? "day" : "week";
  const n = c.day ?? c.week;
  if (n === null) return null;
  const parts = [c.day === null ? null : phrase(c.day, "day"), c.week === null ? null : phrase(c.week, "week")].filter((x): x is string => x !== null);
  const hint = parts.length === 2 ? t("moves.hint_join", { day: parts[0]!, week: parts[1]! }) : parts[0]!;
  const { tone, icon } = look(n);
  const shown = n === 0 ? t("moves.same") : String(Math.abs(n));
  return (
    <Badge tone={tone} icon={icon} hint={hint}>
      <span aria-hidden="true">{period === "week" ? `${shown} ${t("moves.week_tag")}` : shown}</span>
      <span className="ds-sr-only">{phrase(n, period)}</span>
    </Badge>
  );
}

export default RankChange;
