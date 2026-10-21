import { Card, EmptyState, ErrorLine, PixelAvatar, Skeleton, TierBadge, buttonClass } from "@arena/design";
import { Link, t } from "@arena/shell";
import type { SlotProps } from "@arena/shell";
import { useDuel } from "./duel";
import type { DuelFighter } from "./duel";
import "./duel.css";

function Side({ f, won }: { f: DuelFighter; won: boolean }) {
  return (
    <Link to={`/kol/${encodeURIComponent(f.kol)}`} className="duel__side" aria-label={f.rank === null ? t("duel.fighter_unranked", { name: f.name, score: f.score }) : t("duel.fighter", { name: f.name, rank: f.rank, score: f.score })}>
      <PixelAvatar id={f.kol} size={48} />
      <span className="duel__who">
        <strong className="t-title">{f.name}</strong>
        <TierBadge tier={f.tier} />
      </span>
      <span className="duel__stats t-label">
        <span>{t("duel.score", { n: f.score })}</span>
        {f.rank !== null && <span>{t("duel.rank", { n: f.rank })}</span>}
        <span>{t("duel.points", { n: f.points })}</span>
        {won && <span className="duel__winner">★ {t("duel.winner")}</span>}
      </span>
    </Link>
  );
}

/** The pair of the day from the top 20, with a way into the fight. */
export function DuelOfDay({ date }: SlotProps<"board/duel_of_day">) {
  const { state, retry } = useDuel(date);
  if (state.status === "loading") return <Skeleton label={t("duel.loading")} variant="block" />;
  if (state.status === "error") {
    return state.error.status === 404 ? (
      <EmptyState message={t("duel.empty")} />
    ) : (
      <ErrorLine message={t("duel.error")} onRetry={retry} retryLabel={t("duel.retry")} />
    );
  }
  const d = state.data;
  if (d.fighters.length !== 2) return <EmptyState message={t("duel.empty")} />;
  const [a, b] = d.fighters as [DuelFighter, DuelFighter];
  return (
    <Card as="section" aria-label={t("duel.title")}>
      <div className="duel">
        <h3 className="duel__title t-pixel-label">{t("duel.title")}</h3>
        <Side f={a} won={d.winner === a.kol} />
        <span className="duel__vs t-pixel-label" aria-hidden="true">
          VS
        </span>
        <Side f={b} won={d.winner === b.kol} />
        <Link to={`/fight?ids=${encodeURIComponent(a.kol)},${encodeURIComponent(b.kol)}&date=${encodeURIComponent(d.date)}`} className={buttonClass("primary", "duel__watch")}>
          {t("duel.watch")}
        </Link>
      </div>
    </Card>
  );
}

export default DuelOfDay;
