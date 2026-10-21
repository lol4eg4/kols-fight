import { useState } from "react";
import { Bracket, Button, Card, EmptyState, ErrorLine, Heading, Skeleton, buttonClass } from "@arena/design";
import type { BracketEntry, BracketRound as DsRound } from "@arena/design";
import { Link, t, useApi } from "@arena/shell";
import type { SlotProps } from "@arena/shell";
import { bracketPath, lineupsPath } from "./bracket";
import type { Choice, Contestant, Lineups, RoundName, TournamentView } from "./bracket";
import "./tournament.css";

function roundTitle(name: RoundName): string {
  switch (name) {
    case "quarterfinals":
      return t("tournament.round.quarterfinals");
    case "semifinals":
      return t("tournament.round.semifinals");
    case "final":
      return t("tournament.round.final");
  }
}

function entry(c: Contestant, winner: string): BracketEntry {
  return { id: c.kol, name: c.name, score: c.score, winner: c.kol === winner };
}

/** The design system's bracket from the server's view. */
export function toRounds(view: TournamentView): DsRound[] {
  return view.rounds.map((r) => ({
    id: r.name,
    title: roundTitle(r.name),
    matches: r.matches.map((m, i) => ({
      id: `${r.name}-${i}`,
      a: entry(m.a, m.winner),
      b: entry(m.b, m.winner),
    })),
  }));
}

function Bracketed({ view }: { view: TournamentView }) {
  const final = view.rounds[view.rounds.length - 1]?.matches[0];
  return (
    <>
      <Bracket rounds={toRounds(view)} label={t("tournament.bracket")} pendingLabel={t("tournament.pending")} winnerLabel={t("tournament.winner")} />
      <div className="tournament__result">
        <p>
          <strong>{t("tournament.champion", { name: view.champion.name })}</strong>
        </p>
        {final?.decision === "rank_tiebreak" && <p>{t("tournament.rank_tiebreak")}</p>}
        {final && (
          <Link to={`/fight?ids=${encodeURIComponent(final.a.kol)},${encodeURIComponent(final.b.kol)}&date=${view.date}`} className={buttonClass("secondary")}>
            {t("tournament.watch_final")}
          </Link>
        )}
      </div>
    </>
  );
}

function Result({ ids, date }: { ids: string[]; date: string }) {
  const s = useApi<TournamentView>(bracketPath(ids, date));
  if (s.status === "loading") return <Skeleton label={t("tournament.loading")} variant="block" />;
  if (s.status === "error") {
    return s.error.status === 404 ? (
      <EmptyState message={t("tournament.no_data")} />
    ) : (
      <ErrorLine message={t("tournament.error")} onRetry={s.reload} retryLabel={t("tournament.retry")} />
    );
  }
  return <Bracketed view={s.data} />;
}

/** The bracket of 8 below the fight: Top 8 or Our network, run with the fight rules. */
export function Tournament({ date }: SlotProps<"board/below_fight">) {
  const [choice, setChoice] = useState<Choice>("top");
  const lineups = useApi<Lineups>(lineupsPath(date));

  let body;
  if (lineups.status === "loading") {
    body = <Skeleton label={t("tournament.loading")} variant="block" />;
  } else if (lineups.status === "error") {
    body =
      lineups.error.status === 404 ? (
        <EmptyState message={t("tournament.no_data")} />
      ) : (
        <ErrorLine message={t("tournament.error")} onRetry={lineups.reload} retryLabel={t("tournament.retry")} />
      );
  } else if (lineups.data.top.length === 0) {
    body = <EmptyState message={t("tournament.empty")} />;
  } else {
    const l = lineups.data;
    const networkReady = l.network.length > 0;
    // a network lineup that vanished (new day) falls back to the top
    const ids = choice === "network" && networkReady ? l.network : l.top;
    body = (
      <>
        <div role="group" aria-label={t("tournament.lineup")} className="tournament__lineup">
          <Button variant={ids === l.top ? "primary" : "secondary"} aria-pressed={ids === l.top} onClick={() => setChoice("top")}>
            {t("tournament.top")}
          </Button>
          {networkReady ? (
            <Button variant={ids === l.network ? "primary" : "secondary"} aria-pressed={ids === l.network} onClick={() => setChoice("network")}>
              {t("tournament.network")}
            </Button>
          ) : (
            <Button variant="secondary" disabled reason={t("tournament.network_needs")}>
              {t("tournament.network")}
            </Button>
          )}
        </div>
        <Result ids={ids} date={l.date} />
      </>
    );
  }

  return (
    <Card as="section" aria-label={t("tournament.title")}>
      <div className="tournament">
        <Heading level={2} variant="section">
          {t("tournament.title")}
        </Heading>
        {body}
      </div>
    </Card>
  );
}

export default Tournament;
