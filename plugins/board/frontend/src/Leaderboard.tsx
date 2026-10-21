import { useMemo, useState } from "react";
import type { MouseEvent } from "react";
import { Button, EmptyState, Field, Heading, Money, PixelAvatar, ScoreBar, Select, Tabs, TierBadge, TimeAgo } from "@arena/design";
import { Link, Slot, Widget, navigate, t } from "@arena/shell";
import { BADGE_NAME, KolBadges, NetworkChip, SORT_NAME, SOURCE_NAME, SourceChip, StaleMark, TIER_NAME, WinRate, dateText, hasFill, noDataReason, useMedia } from "./bits";
import { BADGES, BADGE_ICON, NO_FILTERS, SORTS, TIERS, applyFilters, cardPath, isFiltered, tabRows } from "./logic";
import type { Filters, SortId, TabId } from "./logic";
import type { Board, BoardRow } from "./types";

const PHONE = "(max-width: 759px)";

/** Opens the card when a click lands on the row outside its own links and buttons. */
function rowClick(e: MouseEvent<HTMLElement>, href: string) {
  const target = e.target as HTMLElement;
  if (target.closest("a, button, input, select, [tabindex]")) return;
  const link = e.currentTarget.querySelector<HTMLAnchorElement>("a.board-name");
  link?.focus(); // the card gives focus back here when it closes
  navigate(href);
}

interface RowProps {
  row: BoardRow;
  date: string;
  href: string;
}

function NameCell({ row, href, date, heading }: RowProps & { heading?: boolean }) {
  const link = (
    <Link to={href} className="board-name" aria-label={t("board.row.open", { name: row.name })}>
      {row.name}
    </Link>
  );
  return (
    <div className="board-who">
      <PixelAvatar id={row.kol} size={36} />
      <div className="board-who__text">
        <div className="board-who__line">
          {heading ? <h3 className="board-item__name">{link}</h3> : link}
          {row.in_network && <NetworkChip />}
        </div>
        <div className="board-who__line board-who__meta">
          {row.x_handle && <span className="board-handle">{t("board.card.handle", { handle: row.x_handle })}</span>}
          <KolBadges badges={row.badges} grouped={heading} />
          <Slot name="board/extra_badges" props={{ date, kol: row.kol }} loading={null} />
          {row.stale && <StaleMark at={row.fetched_at} compact />}
        </div>
      </div>
    </div>
  );
}

function TableRow({ row, date, href }: RowProps) {
  const m = row.metrics;
  const reason = (f: Parameters<typeof noDataReason>[2]) => noDataReason(m, row.source, f);
  return (
    <tr className="board-tr" data-kol={row.kol} onClick={(e) => rowClick(e, href)}>
      <td className="board-td board-td--rank">
        <span className="board-rank">{row.rank}</span>
        <Slot name="board/rank_change" props={{ date, kol: row.kol }} loading={null} />
      </td>
      <th scope="row" className="board-td board-td--kol">
        <NameCell row={row} href={href} date={date} />
      </th>
      <td className="board-td">
        <TierBadge tier={row.tier} />
      </td>
      <td className="board-td board-td--score">
        <ScoreBar score={row.score} tier={row.tier} label={t("board.score")} />
      </td>
      <td className="board-td board-td--num">
        <Money value={m?.d30} reason={reason("d30")} />
      </td>
      <td className="board-td board-td--num">
        <Money value={m?.d7} reason={reason("d7")} />
      </td>
      <td className="board-td board-td--num board-td--wide">
        <Money value={m?.d1} reason={reason("d1")} />
      </td>
      <td className="board-td board-td--num">
        <WinRate metrics={m} reason={reason("wr30")} />
      </td>
      <td className="board-td board-td--num board-td--wide">
        <Money value={m?.avg_pos} signed={false} reason={reason("avg_pos")} />
      </td>
      <td className="board-td board-td--num board-td--wide">
        <TimeAgo minutes={m?.last_min_at_fetch ?? null} reason={reason("last_min_at_fetch")} />
      </td>
      <td className="board-td board-td--wide">
        <SourceChip source={row.source} at={row.fetched_at} />
      </td>
      <td className="board-td board-td--fight">
        <Widget name="fight/add_to_fight" props={{ kol: row.kol }} loading={null} />
      </td>
    </tr>
  );
}

function CardRow({ row, date, href }: RowProps) {
  const m = row.metrics;
  const reason = (f: Parameters<typeof noDataReason>[2]) => noDataReason(m, row.source, f);
  return (
    <li className="board-item" data-kol={row.kol} onClick={(e) => rowClick(e, href)}>
      <div className="board-item__top">
        <span className="board-item__rank">
          <span className="board-rank">{row.rank}</span>
          <Slot name="board/rank_change" props={{ date, kol: row.kol }} loading={null} />
        </span>
        <NameCell row={row} href={href} date={date} heading />
      </div>
      <div className="board-item__score">
        <TierBadge tier={row.tier} />
        <ScoreBar score={row.score} tier={row.tier} label={t("board.score")} />
      </div>
      <dl className="board-item__nums">
        <div>
          <dt>{t("board.col.d30")}</dt>
          <dd>
            <Money value={m?.d30} reason={reason("d30")} />
          </dd>
        </div>
        <div>
          <dt>{t("board.col.wr")}</dt>
          <dd>
            <WinRate metrics={m} reason={reason("wr30")} />
          </dd>
        </div>
        <div>
          <dt>{t("board.col.last")}</dt>
          <dd>
            <TimeAgo minutes={m?.last_min_at_fetch ?? null} reason={reason("last_min_at_fetch")} />
          </dd>
        </div>
      </dl>
      <div className="board-item__foot">
        <SourceChip source={row.source} at={row.fetched_at} />
        <Widget name="fight/add_to_fight" props={{ kol: row.kol }} loading={null} />
      </div>
    </li>
  );
}

export interface LeaderboardProps {
  board: Board;
  /** The date from the address, kept in card links; null on the latest snapshot. */
  urlDate: string | null;
}

export function Leaderboard({ board, urlDate }: LeaderboardProps) {
  const [f, setF] = useState<Filters>(NO_FILTERS);
  const [networkIds, setNetworkIds] = useState<Set<string> | null>(null);
  const phone = useMedia(PHONE);
  const set = (patch: Partial<Filters>) => setF((prev) => ({ ...prev, ...patch }));

  const networkTab = hasFill("board/network_filter");
  const rows = useMemo(() => applyFilters(board.rows, f, networkIds), [board.rows, f, networkIds]);
  const counts = useMemo(
    () => ({
      all: board.rows.length,
      top100: tabRows(board.rows, "top100", null).length,
      network: tabRows(board.rows, "network", networkIds).length,
    }),
    [board.rows, networkIds],
  );

  const tabs = [
    { id: "all", label: t("board.tab.all"), count: counts.all },
    { id: "top100", label: t("board.tab.top100"), count: counts.top100 },
    ...(networkTab ? [{ id: "network", label: t("board.tab.network"), count: counts.network }] : []),
  ];
  const caption = t("board.lb.caption", { count: rows.length, sort: SORT_NAME[f.sort]() });

  let empty: string | null = null;
  if (rows.length === 0) {
    if (f.query.trim()) empty = t("board.lb.empty_query", { query: f.query.trim() });
    else if (f.tab === "network" && counts.network === 0) empty = t("board.lb.empty_network");
    else empty = t("board.lb.empty");
  }

  return (
    <section id="leaderboard" className="board-section" aria-labelledby="board-lb-title">
      <Heading level={2} id="board-lb-title">
        {t("board.lb.title")}
      </Heading>
      {networkTab && (
        <Slot
          name="board/network_filter"
          props={{ date: board.date, onIds: (ids: string[]) => setNetworkIds(new Set(ids)) }}
          loading={null}
        />
      )}

      <div className="board-controls">
        <Tabs items={tabs} value={f.tab} onChange={(id) => set({ tab: id as TabId })} label={t("board.lb.tabs")} idBase="board-lb" />
        <div className="board-controls__row">
          <div className="board-controls__search">
            <Field
              type="search"
              label={t("board.search.label")}
              placeholder={t("board.search.placeholder")}
              value={f.query}
              onChange={(e) => set({ query: e.target.value })}
              autoComplete="off"
            />
          </div>
          <div className="board-controls__pair">
            <Select
              label={t("board.sort.label")}
              value={f.sort}
              options={SORTS.map((s) => ({ value: s, label: SORT_NAME[s]() }))}
              onChange={(e) => set({ sort: e.target.value as SortId })}
            />
            <Select
              label={t("board.filter.badge")}
              value={f.badge}
              options={[
                { value: "all", label: t("board.filter.badge_all") },
                ...BADGES.map((b) => ({ value: b, label: `${BADGE_ICON[b]} ${BADGE_NAME[b]()}` })),
              ]}
              onChange={(e) => set({ badge: e.target.value as Filters["badge"] })}
            />
          </div>
        </div>
        <div className="board-pills" role="group" aria-label={t("board.filter.tier")}>
          {(["all", ...TIERS] as const).map((tier) => (
            <button
              key={tier}
              type="button"
              className="board-pill"
              aria-pressed={f.tier === tier}
              onClick={() => set({ tier })}
            >
              {tier === "all" ? t("board.filter.tier_all") : TIER_NAME[tier]()}
            </button>
          ))}
        </div>
      </div>

      <div className="board-tabpanel" role="tabpanel" id={`board-lb-panel-${f.tab}`} aria-labelledby={`board-lb-tab-${f.tab}`}>
        {f.tab === "network" && networkIds === null ? null : empty ? (
          <EmptyState
            message={empty}
            action={
              isFiltered(f) ? (
                <Button variant="secondary" onClick={() => set({ query: "", tier: "all", badge: "all" })}>
                  {t("board.lb.clear")}
                </Button>
              ) : undefined
            }
          />
        ) : phone ? (
          <ol className="board-list" aria-label={caption}>
            {rows.map((r) => (
              <CardRow key={r.kol} row={r} date={board.date} href={cardPath(r.kol, urlDate)} />
            ))}
          </ol>
        ) : (
          <div className="board-table-wrap">
            <table className="board-table">
              <caption className="ds-sr-only">{caption}</caption>
              <thead>
                <tr>
                  <th scope="col" aria-sort={f.sort === "rank" ? "ascending" : undefined}>
                    {t("board.col.rank")}
                  </th>
                  <th scope="col">{t("board.col.kol")}</th>
                  <th scope="col">{t("board.col.tier")}</th>
                  <th scope="col">{t("board.col.score")}</th>
                  <th scope="col" className="board-th--num" aria-sort={f.sort === "d30" ? "descending" : undefined}>
                    {t("board.col.d30")}
                  </th>
                  <th scope="col" className="board-th--num" aria-sort={f.sort === "d7" ? "descending" : undefined}>
                    {t("board.col.d7")}
                  </th>
                  <th scope="col" className="board-th--num board-td--wide" aria-sort={f.sort === "d1" ? "descending" : undefined}>
                    {t("board.col.d1")}
                  </th>
                  <th scope="col" className="board-th--num" aria-sort={f.sort === "winrate" ? "descending" : undefined}>
                    {t("board.col.wr")}
                  </th>
                  <th scope="col" className="board-th--num board-td--wide">
                    {t("board.col.pos")}
                  </th>
                  <th scope="col" className="board-th--num board-td--wide" aria-sort={f.sort === "last" ? "ascending" : undefined}>
                    {t("board.col.last")}
                  </th>
                  <th scope="col" className="board-td--wide">
                    {t("board.col.source")}
                  </th>
                  <th scope="col">{t("board.col.fight")}</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((r) => (
                  <TableRow key={r.kol} row={r} date={board.date} href={cardPath(r.kol, urlDate)} />
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      <p className="board-sources">
        <span className="ds-sr-only">{t("board.sources.label")}</span>
        {board.sources.map((s, i) => (
          <span key={s.source}>
            {i > 0 && <span aria-hidden="true"> · </span>}
            {t("board.sources.line", { source: SOURCE_NAME[s.source](), kols: s.kols, date: dateText(s.fetched_at) })}
          </span>
        ))}
      </p>
    </section>
  );
}
