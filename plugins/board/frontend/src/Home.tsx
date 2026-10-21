import { useMemo } from "react";
import {
  Card,
  Container,
  ErrorLine,
  Heading,
  Money,
  PixelAvatar,
  ScoreBar,
  Select,
  Skeleton,
  SnapshotDate,
  StatTile,
  TierBadge,
  buttonClass,
  formatMoneyShort,
} from "@arena/design";
import type { TickerItem } from "@arena/design";
import { ANCHORS, Link, Slot, Widget, navigate, t, useApi, useSearchParams, useTickerItems } from "@arena/shell";
import { BADGE_HINT, BADGE_NAME, KolBadges, NetworkChip, PART_NAME, PART_SCALE, SOURCE_NAME, TIER_NAME, TIER_RANGE, dateText, hasFill, hasWidget, noDataReason } from "./bits";
import { Leaderboard } from "./Leaderboard";
import { BADGES, BADGE_ICON, PART_WEIGHT, TIERS, cardPath, dateParam, isLate } from "./logic";
import type { Board, BoardRow, BoardTickerItem, PartKind } from "./types";
import "./board.css";

function tickerText(item: BoardTickerItem): string {
  const name = item.name ?? "";
  switch (item.kind) {
    case "top_score":
      return t("board.ticker.top_score", { name, value: item.value });
    case "best_week":
      return t("board.ticker.best_week", { name, value: formatMoneyShort(item.value) });
    case "traded_last_hour":
      return t("board.ticker.traded_last_hour", { value: item.value });
    case "up_30d":
      return t("board.ticker.up_30d", { value: item.value });
  }
}

function BoardTicker({ board }: { board: Board }) {
  const items = useMemo<TickerItem[]>(
    () => board.ticker.map((i, n) => ({ id: `${board.date}-${n}-${i.kind}`, content: tickerText(i) })),
    [board],
  );
  useTickerItems("board", items);
  return null;
}

function Hero({ board }: { board: Board }) {
  const latest = board.dates[board.dates.length - 1];
  const options = [...board.dates].reverse().map((d) => ({
    value: d,
    label: d === latest ? t("board.date.latest", { date: dateText(d) }) : dateText(d),
  }));
  return (
    <section className="board-hero" aria-labelledby="board-hero-title">
      <div className="board-hero__main">
        <Heading level={1} variant="display" id="board-hero-title">
          {t("board.hero.title")}
        </Heading>
        <p className="board-hero__lead">{t("board.hero.lead", { up: board.stats.up_30d, tracked: board.stats.tracked })}</p>
        <div className="board-hero__actions">
          <a href={`#${ANCHORS.leaderboard}`} className={buttonClass("primary")}>
            {t("board.hero.to_board")}
          </a>
          {hasWidget("fight/fighter_picker") && (
            <a href={`#${ANCHORS.fight}`} className={buttonClass("fight")}>
              {t("board.hero.to_fight")}
            </a>
          )}
        </div>
      </div>
      {hasFill("board/duel_of_day") && (
        <div className="board-hero__duel">
          <Slot name="board/duel_of_day" props={{ date: board.date }} loading={null} />
        </div>
      )}
      <div className="board-hero__stats" role="group" aria-label={t("board.stats.label")}>
        <StatTile value={board.stats.tracked} label={t("board.stats.tracked")} accent />
        <StatTile value={board.stats.up_30d} label={t("board.stats.up_30d")} />
        <StatTile value={board.stats.traded_last_hour} label={t("board.stats.traded_last_hour")} />
        <StatTile value={<Money value={board.stats.total_d30} />} label={t("board.stats.total_d30")} />
      </div>
      <div className="board-hero__snapshot">
        <span className="board-muted">
          {t("board.hero.snapshot")} <SnapshotDate at={board.closed_at ?? board.date} />
        </span>
        {board.dates.length > 1 && (
          <div className="board-hero__date">
            <Select
              label={t("board.date.label")}
              value={board.date}
              options={options}
              onChange={(e) => navigate(e.target.value === latest ? "/" : `/?date=${e.target.value}`)}
            />
          </div>
        )}
      </div>
    </section>
  );
}

const PODIUM_TONE = ["gold", "silver", "bronze"] as const;

function Podium({ rows, urlDate }: { rows: BoardRow[]; urlDate: string | null }) {
  const top = rows.filter((r) => 3 >= r.rank).sort((a, b) => a.rank - b.rank);
  if (top.length === 0) return null;
  return (
    <section className="board-section" aria-labelledby="board-podium-title">
      <Heading level={2} id="board-podium-title">
        {t("board.podium.title")}
      </Heading>
      <ol className="board-podium">
        {top.map((r) => (
          <li key={r.kol} className={`board-podium__place board-podium__place--${r.rank}`} data-kol={r.kol}>
            <Card tone={PODIUM_TONE[r.rank - 1]}>
              <div className="board-podium__card">
                <span className="board-podium__rank">{t("board.podium.place", { n: r.rank })}</span>
                <PixelAvatar id={r.kol} size={r.rank === 1 ? 72 : 56} />
                <Link to={cardPath(r.kol, urlDate)} className="board-podium__name">
                  {r.name}
                </Link>
                {r.in_network && <NetworkChip />}
                <TierBadge tier={r.tier} />
                <div className="board-podium__score">
                  <ScoreBar score={r.score} tier={r.tier} label={t("board.score")} />
                </div>
                <span className="board-podium__pnl">
                  <span className="board-muted">{t("board.podium.pnl")}</span>{" "}
                  <Money value={r.metrics?.d30} reason={noDataReason(r.metrics, r.source, "d30")} />
                </span>
                <KolBadges badges={r.badges} />
              </div>
            </Card>
          </li>
        ))}
      </ol>
    </section>
  );
}

const PARTS: PartKind[] = ["profit", "accuracy", "form", "activity"];

function HowItWorks({ board }: { board: Board }) {
  return (
    <section id={ANCHORS.score} className="board-section" aria-labelledby="board-how-title">
      <Heading level={2} id="board-how-title">
        {t("board.how.title")}
      </Heading>
      <p className="board-how__formula">{t("board.how.formula")}</p>
      <div className="board-how">
        <Card>
          <Heading level={3} variant="title">
            {t("board.how.parts")}
          </Heading>
          <dl className="board-how__list">
            {PARTS.map((p) => (
              <div key={p}>
                <dt>
                  {PART_NAME[p]()} <span className="board-muted ds-mono-text">{t("board.part.weight", { n: PART_WEIGHT[p] * 100 })}</span>
                </dt>
                <dd>{PART_SCALE[p]()}</dd>
              </div>
            ))}
          </dl>
        </Card>
        <Card>
          <Heading level={3} variant="title">
            {t("board.how.tiers")}
          </Heading>
          <dl className="board-how__list">
            {TIERS.map((tier) => (
              <div key={tier}>
                <dt>
                  <TierBadge tier={tier} />
                </dt>
                <dd className="ds-mono-text">{TIER_RANGE[tier]()}</dd>
              </div>
            ))}
          </dl>
          <Heading level={3} variant="title">
            {t("board.how.sources")}
          </Heading>
          <ul className="board-how__plain">
            <li>{t("board.how.source_kolexplorer")}</li>
            <li>{t("board.how.source_solanatracker")}</li>
            {board.sources.map((s) => (
              <li key={s.source} className="board-muted">
                {t("board.sources.line", { source: SOURCE_NAME[s.source](), kols: s.kols, date: dateText(s.fetched_at) })}
              </li>
            ))}
          </ul>
        </Card>
        <Card>
          <Heading level={3} variant="title">
            {t("board.how.badges")}
          </Heading>
          <dl className="board-how__list">
            {BADGES.map((b) => (
              <div key={b}>
                <dt>
                  <span aria-hidden="true">{BADGE_ICON[b]}</span> {BADGE_NAME[b]()}
                </dt>
                <dd>{BADGE_HINT[b]()}</dd>
              </div>
            ))}
          </dl>
        </Card>
      </div>
    </section>
  );
}

/** Home: hero, podium, leaderboard and the sections other plugins fill. */
export default function Home() {
  const params = useSearchParams();
  const urlDate = dateParam(params.get("date"));
  const state = useApi<Board>(urlDate ? `/api/board?date=${urlDate}` : "/api/board");

  if (state.status === "loading") {
    return (
      <Container>
        <div className="board-page">
          <Skeleton label={t("board.loading")} variant="rows" count={8} />
        </div>
      </Container>
    );
  }
  if (state.status === "error") {
    return (
      <Container>
        <div className="board-page">
          <ErrorLine message={`${t("board.error")} ${state.error.message}`} onRetry={state.reload} retryLabel={t("board.retry")} />
        </div>
      </Container>
    );
  }

  const board = state.data;
  const latest = board.dates[board.dates.length - 1] ?? board.date;
  const late = isLate(board.closed_at, board.date === latest);

  return (
    <>
      <BoardTicker board={board} />
      {late && (
        <div className="board-late" role="status">
          <Container>{t("board.late", { date: dateText(board.closed_at) })}</Container>
        </div>
      )}
      <Container>
        <div className="board-page">
          <Hero board={board} />
          <Podium rows={board.rows} urlDate={urlDate} />
          <Leaderboard board={board} urlDate={urlDate} />

          {hasFill("board/network_section") && (
            <section id={ANCHORS.network} className="board-section" aria-labelledby="board-network-title">
              <Heading level={2} id="board-network-title">
                {t("board.section.network")}
              </Heading>
              <Slot name="board/network_section" props={{ date: board.date }} />
            </section>
          )}

          {hasWidget("fight/fighter_picker") && (
            <section id={ANCHORS.fight} className="board-section" aria-labelledby="board-fight-title">
              <Heading level={2} tone="pink" id="board-fight-title">
                {t("board.section.fight")}
              </Heading>
              <Widget name="fight/fighter_picker" props={{}} />
            </section>
          )}
          <Slot name="board/below_fight" props={{ date: board.date }} />

          {hasFill("board/radar_section") && (
            <section className="board-section" aria-labelledby="board-radar-title">
              <Heading level={2} id="board-radar-title">
                {t("board.section.radar")}
              </Heading>
              <Slot name="board/radar_section" props={{ date: board.date }} />
            </section>
          )}

          <HowItWorks board={board} />

          <footer className="board-footer">
            <p>{t("board.footer.sources")}</p>
            <p className="ds-mono-text">{t("board.footer.nfa")}</p>
          </footer>
        </div>
      </Container>
    </>
  );
}
