import { Card, EmptyState, ErrorLine, PixelAvatar, Skeleton, TierBadge } from "@arena/design";
import { Link, t } from "@arena/shell";
import type { SlotProps } from "@arena/shell";
import { useDuel } from "./duel";
import type { DuelFighter } from "./duel";

function Side({ f, won }: { f: DuelFighter; won: boolean }) {
  return (
    <Link to={`/kol/${encodeURIComponent(f.kol)}`} aria-label={t("duel.fighter", { name: f.name, rank: f.rank ?? 0, score: f.score })}>
      <PixelAvatar id={f.kol} size={48} />
      <strong>{f.name}</strong>
      <TierBadge tier={f.tier} />
      <span>{t("duel.score", { n: f.score })}</span>
      {f.rank !== null && <span>{t("duel.rank", { n: f.rank })}</span>}
      <span>{t("duel.points", { n: f.points })}</span>
      {won && <span>{t("duel.winner")}</span>}
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
      <h3>{t("duel.title")}</h3>
      <Side f={a} won={d.winner === a.kol} />
      <span aria-hidden="true">⚔</span>
      <Side f={b} won={d.winner === b.kol} />
      <Link to={`/fight?ids=${encodeURIComponent(a.kol)},${encodeURIComponent(b.kol)}&date=${d.date}`}>{t("duel.watch")}</Link>
    </Card>
  );
}

export default DuelOfDay;
