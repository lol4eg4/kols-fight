import { Badge, ErrorLine, Skeleton } from "@arena/design";
import { t } from "@arena/shell";
import type { SlotProps } from "@arena/shell";
import { useStreaks } from "./streaks";
import type { Streak } from "./streaks";

function label(s: Streak): { text: string; hint: string; icon: string } {
  return s === "trade_7d"
    ? { text: t("streaks.trade_7d"), hint: t("streaks.trade_7d_hint"), icon: "🔥" }
    : { text: t("streaks.profit_3w"), hint: t("streaks.profit_3w_hint"), icon: "📈" };
}

/** Streak badges of one KOL, for the board row and the card. A KOL without a streak shows nothing. */
export function ExtraBadges({ date, kol }: SlotProps<"board/extra_badges">) {
  const state = useStreaks(date);
  if (state.status === "loading") return <Skeleton label={t("streaks.loading")} variant="lines" count={1} />;
  if (state.status === "error") {
    // No snapshot for the day: no history, so no badges.
    return state.error.status === 404 ? null : <ErrorLine message={t("streaks.error")} />;
  }
  const mine = state.data.streaks.find((x) => x.kol === kol);
  if (!mine || mine.badges.length === 0) return null;
  return (
    <>
      {mine.badges.map((b) => {
        const l = label(b);
        return (
          <Badge key={b} tone="coin" icon={l.icon} hint={l.hint}>
            {l.text}
          </Badge>
        );
      })}
    </>
  );
}

export default ExtraBadges;
