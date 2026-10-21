import { useEffect, useRef, useState } from "react";
import type { ReactNode, RefObject } from "react";
import {
  Badge,
  Button,
  Dialog,
  ErrorLine,
  Heading,
  Money,
  NoData,
  PixelAvatar,
  ScoreBar,
  ScoreBreakdown,
  Skeleton,
  SnapshotDate,
  TierBadge,
  TimeAgo,
  formatMinutesAgo,
} from "@arena/design";
import type { ScorePart } from "@arena/design";
import { Slot, Widget, t, useApi, useOverlayClose, useParams, useSearchParams } from "@arena/shell";
import { BADGE_HINT, BADGE_NAME, NetworkChip, PART_NAME, SOURCE_NAME, StaleMark, dateText, hasWidget, noDataReason } from "./bits";
import { BADGE_ICON, PART_WEIGHT, dateParam, formatHold, formatPercent, safeSolscan, sourceLinks } from "./logic";
import type { LinkId } from "./logic";
import type { KolCard, Metrics, Part } from "./types";
import "./board.css";

const LINK_NAME: Record<LinkId, () => string> = {
  x: () => t("board.link.x"),
  kolexplorer: () => t("board.link.kolexplorer"),
  solanatracker: () => t("board.link.solanatracker"),
  kolscan: () => t("board.link.kolscan"),
  fomo: () => t("board.link.fomo"),
  pumpfun: () => t("board.link.pumpfun"),
  telegram: () => t("board.link.telegram"),
};

const FORMAT_NAME: Record<string, () => string> = {
  post: () => t("board.format.post"),
  tg_post: () => t("board.format.tg_post"),
  buy: () => t("board.format.buy"),
  fomo_buy: () => t("board.format.fomo_buy"),
  post_on_request: () => t("board.format.post_on_request"),
};

function partRaw(p: Part): ReactNode | null {
  if (p.status === "no_data" || p.raw === null) return null;
  switch (p.kind) {
    case "profit":
    case "form":
      return <Money value={p.raw} mode="full" />;
    case "accuracy":
      return (
        <span className="ds-mono-text">
          {formatPercent(p.raw)}
          {p.all_time && <span title={t("board.wr.all_time")}>*</span>}
        </span>
      );
    case "activity":
      return <span className="ds-mono-text">{formatMinutesAgo(p.raw)}</span>;
  }
}

function toScoreParts(parts: Part[]): ScorePart[] {
  return parts.map((p) => {
    const empty = p.status === "no_data" || p.raw === null;
    return {
      id: p.kind,
      label: PART_NAME[p.kind](),
      weightLabel: t("board.part.weight", { n: PART_WEIGHT[p.kind] * 100 }),
      raw: partRaw(p),
      share: empty ? null : p.share,
      points: empty ? null : p.points,
      max: PART_WEIGHT[p.kind] * 100,
    };
  });
}

function Metric({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="board-metric">
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}

function MetricsGrid({ card }: { card: KolCard }) {
  const m = card.metrics;
  const why = (f: keyof Metrics) => noDataReason(m, card.source, f);
  const money = (f: keyof Metrics, signed = true) => <Money value={m?.[f] ?? null} mode="full" signed={signed} reason={why(f)} reasonMode="text" />;
  const plain = (f: keyof Metrics, fmt: (v: number) => string) => {
    const v = m?.[f] ?? null;
    return v === null ? <NoData reason={why(f)} mode="text" /> : <span className="ds-mono-text">{fmt(v)}</span>;
  };
  const allTime = m !== null && m.wr30 === null && m.wr_all !== null;
  return (
    <dl className="board-metrics">
      <Metric label={t("board.metric.d30")}>{money("d30")}</Metric>
      <Metric label={t("board.metric.d7")}>{money("d7")}</Metric>
      <Metric label={t("board.metric.d1")}>{money("d1")}</Metric>
      <Metric label={t("board.metric.unreal")}>{money("unreal")}</Metric>
      <Metric label={t("board.metric.total")}>{money("total")}</Metric>
      {allTime ? (
        <Metric label={t("board.metric.wr_all")}>
          {plain("wr_all", formatPercent)}
          <span className="board-note">{t("board.wr.all_time")}</span>
        </Metric>
      ) : (
        <Metric label={t("board.metric.wr30")}>{plain("wr30", formatPercent)}</Metric>
      )}
      <Metric label={t("board.metric.wr7")}>{plain("wr7", formatPercent)}</Metric>
      <Metric label={t("board.metric.avg_pos")}>{money("avg_pos", false)}</Metric>
      <Metric label={t("board.metric.hold")}>{plain("hold_s", formatHold)}</Metric>
      <Metric label={t("board.metric.last")}>
        <TimeAgo minutes={m?.last_min_at_fetch ?? null} reason={why("last_min_at_fetch")} reasonMode="text" />
      </Metric>
      <Metric label={t("board.metric.vol30")}>{money("vol30", false)}</Metric>
      <Metric label={t("board.metric.days30")}>{plain("days30", String)}</Metric>
    </dl>
  );
}

function CopyButton({ address, target }: { address: string; target: RefObject<HTMLElement | null> }) {
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle");
  useEffect(() => {
    if (state !== "copied") return;
    const id = window.setTimeout(() => setState("idle"), 2000);
    return () => window.clearTimeout(id);
  }, [state]);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(address);
      setState("copied");
    } catch {
      const el = target.current;
      if (el) {
        const range = document.createRange();
        range.selectNodeContents(el);
        const sel = window.getSelection();
        sel?.removeAllRanges();
        sel?.addRange(range);
      }
      setState("failed");
    }
  };
  return (
    <>
      <Button variant="secondary" onClick={() => void copy()} aria-label={t("board.card.copy_label", { address })}>
        {state === "copied" ? t("board.card.copied") : t("board.card.copy")}
      </Button>
      <span className="board-note" role="status">
        {state === "failed" ? t("board.card.copy_failed") : ""}
      </span>
    </>
  );
}

function WalletRow({ address, main, solscan }: { address: string; main: boolean; solscan: string | null }) {
  const ref = useRef<HTMLElement>(null);
  return (
    <li className="board-wallet">
      <div className="board-wallet__addr">
        {main && <Badge tone="coin">{t("board.card.main_wallet")}</Badge>}
        <code ref={ref} className="ds-mono-text">
          {address}
        </code>
      </div>
      <div className="board-wallet__actions">
        <CopyButton address={address} target={ref} />
        {solscan && (
          <a href={solscan} target="_blank" rel="noopener noreferrer" className="board-out">
            {t("board.card.solscan")}
            <span className="ds-sr-only"> ({t("board.card.new_tab")})</span>
            <span aria-hidden="true"> ↗</span>
          </a>
        )}
      </div>
    </li>
  );
}

function CardBody({ card }: { card: KolCard }) {
  const parts = toScoreParts(card.parts);
  const sum = card.parts.reduce((s, p) => s + p.points, 0);
  const links = sourceLinks(card.x_handle, card.links, card.wallets);
  const needsProfit = card.parts.some((p) => p.status === "needs_profit");
  return (
    <div className="board-card">
      <div className="board-card__head">
        <PixelAvatar id={card.kol} size={64} />
        <div className="board-card__who">
          {card.x_handle && <span className="board-handle">{t("board.card.handle", { handle: card.x_handle })}</span>}
          <div className="board-card__chips">
            <TierBadge tier={card.tier} />
            {card.in_network && <NetworkChip />}
            <span className="board-card__rank ds-mono-text">{t("board.card.rank", { rank: card.rank, ranked: card.ranked })}</span>
            {card.stale && <StaleMark at={card.fetched_at} />}
          </div>
        </div>
      </div>

      <div className="board-card__score">
        <ScoreBar score={card.score} tier={card.tier} label={t("board.score")} size="lg" />
      </div>

      <section className="board-card__block" aria-labelledby="board-card-metrics">
        <Heading level={3} variant="title" id="board-card-metrics">
          {t("board.card.metrics")}
        </Heading>
        <MetricsGrid card={card} />
        <p className="board-muted board-card__source">
          {card.source
            ? t("board.card.source", { source: SOURCE_NAME[card.source](), date: dateText(card.fetched_at) })
            : t("board.source.none")}
          {" · "}
          {t("board.card.snapshot")} <SnapshotDate at={card.date} />
        </p>
      </section>

      <section className="board-card__block" aria-labelledby="board-card-breakdown">
        <Heading level={3} variant="title" id="board-card-breakdown">
          {t("board.card.breakdown")}
        </Heading>
        <ScoreBreakdown
          parts={parts}
          sum={sum}
          score={card.score}
          tier={card.tier}
          labels={{
            part: t("board.breakdown.part"),
            raw: t("board.breakdown.raw"),
            scale: t("board.breakdown.scale"),
            points: t("board.breakdown.points"),
            sum: t("board.breakdown.sum"),
            score: t("board.breakdown.score"),
            noData: t("board.breakdown.no_data"),
          }}
        />
        {needsProfit && <p className="board-note">{t("board.card.needs_profit")}</p>}
      </section>

      <section className="board-card__block" aria-labelledby="board-card-badges">
        <Heading level={3} variant="title" id="board-card-badges">
          {t("board.card.badges")}
        </Heading>
        {card.badges.length === 0 ? (
          <p className="board-muted">{t("board.card.no_badges")}</p>
        ) : (
          <ul className="board-card__badges">
            {card.badges.map((b) => (
              <li key={b}>
                <span aria-hidden="true">{BADGE_ICON[b]}</span> <strong>{BADGE_NAME[b]()}</strong>{" "}
                <span className="board-muted">{BADGE_HINT[b]()}</span>
              </li>
            ))}
          </ul>
        )}
        <Slot name="board/extra_badges" props={{ date: card.date, kol: card.kol }} loading={null} />
        {card.formats.length > 0 && (
          <div className="board-card__formats">
            <span className="board-muted">{t("board.card.formats")}</span>
            {card.formats.map((f) => (
              <Badge key={f}>{Object.hasOwn(FORMAT_NAME, f) ? FORMAT_NAME[f]!() : f}</Badge>
            ))}
          </div>
        )}
      </section>

      <section className="board-card__block" aria-labelledby="board-card-wallets">
        <Heading level={3} variant="title" id="board-card-wallets">
          {t("board.card.wallets")}
        </Heading>
        {card.wallets.length === 0 ? (
          <p className="board-muted">{t("board.card.no_wallet")}</p>
        ) : (
          <ul className="board-wallets">
            {card.wallets.map((w) => (
              <WalletRow key={w.address} address={w.address} main={w.main} solscan={safeSolscan(w.solscan_url)} />
            ))}
          </ul>
        )}
      </section>

      {links.length > 0 && (
        <section className="board-card__block" aria-labelledby="board-card-links">
          <Heading level={3} variant="title" id="board-card-links">
            {t("board.card.links")}
          </Heading>
          <ul className="board-links">
            {links.map((l) => (
              <li key={l.id}>
                <a href={l.url} target="_blank" rel="noopener noreferrer" className="board-out">
                  {LINK_NAME[l.id]()}
                  <span className="ds-sr-only"> ({t("board.card.new_tab")})</span>
                  <span aria-hidden="true"> ↗</span>
                </a>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}

/** KOL card over the home page: /kol/:id?date=. */
export default function KolCardScreen() {
  const { id = "" } = useParams<{ id: string }>();
  const params = useSearchParams();
  const date = dateParam(params.get("date"));
  const close = useOverlayClose();
  const path = `/api/kol/${encodeURIComponent(id)}${date ? `?date=${date}` : ""}`;
  const state = useApi<KolCard>(id ? path : null);

  const title = state.status === "ready" ? state.data.name : t("board.card.title");
  const footer =
    state.status === "ready" && hasWidget("fight/add_to_fight") ? <Widget name="fight/add_to_fight" props={{ kol: state.data.kol }} /> : undefined;

  return (
    <Dialog open onClose={close} title={title} closeLabel={t("board.card.close")} footer={footer}>
      {state.status === "loading" && <Skeleton label={t("board.card.loading")} variant="lines" count={6} />}
      {state.status === "error" && <ErrorLine message={state.error.message} onRetry={state.reload} retryLabel={t("board.retry")} />}
      {state.status === "ready" && <CardBody card={state.data} />}
    </Dialog>
  );
}
