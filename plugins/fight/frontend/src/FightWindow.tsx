import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import type { CSSProperties, ReactNode, RefObject } from "react";
import {
  Button,
  Card,
  Dialog,
  ErrorLine,
  Field,
  HpBar,
  Money,
  PixelAvatar,
  Progress,
  RoundRow,
  ScoreBar,
  Skeleton,
  SnapshotDate,
  TabPanel,
  Tabs,
  TierBadge,
  buttonClass,
  formatMinutesAgo,
  formatPoints,
} from "@arena/design";
import type { RoundFighter } from "@arena/design";
import { Link, Slot, emitPageEvent, navigate, t, useApi, useLocation, useOverlayClose } from "@arena/shell";
import { byKol, useBoard } from "./board";
import {
  HP_SEGMENTS,
  MONEY_ROUNDS,
  RATE_ROUNDS,
  VS,
  fightApiPath,
  fightImagePath,
  fightPath,
  formatPointsShort,
  hpAfter,
  parseFightQuery,
  pointsAfter,
  roundShares,
  roundWinners,
  roundsShown,
  step,
  xPostUrl,
} from "./logic";
import type { Phase, Step } from "./logic";
import { Picker } from "./Picker";
import { setLastFight, setPending } from "./store";
import { KIND_MEASURES, KIND_TITLE, PART_NAME, PART_RAW, PART_WEIGHT, announceText, namesOf, percent, scoreLine, shareText, sourceText, whyText } from "./text";
import type { Names } from "./text";
import type { FightResult, Fighter, PartKind, Round } from "./types";

/** Time a round stays on screen before the next one, when the visitor lets it play. */
const ROUND_MS = 2800;

// ---------- reduced motion ----------

const MOTION_QUERY = "(prefers-reduced-motion: reduce)";

function subscribeMotion(fn: () => void) {
  const m = window.matchMedia(MOTION_QUERY);
  m.addEventListener("change", fn);
  return () => m.removeEventListener("change", fn);
}

function useReducedMotion(): boolean {
  return useSyncExternalStore(
    subscribeMotion,
    () => window.matchMedia(MOTION_QUERY).matches,
    () => false,
  );
}

// ---------- the window ----------

/** The fight window over the home page: /fight picks fighters, /fight?ids=a,b&date= runs the fight. */
export default function FightWindow() {
  const close = useOverlayClose();
  const { search } = useLocation();
  const q = useMemo(() => parseFightQuery(search), [search]);
  const runnable = q.ids.length >= 2;
  const state = useApi<FightResult>(runnable ? fightApiPath(q.ids, q.date) : null);
  const reduced = useReducedMotion();

  // A link with one fighter, or a fight the server would not run: the picker opens with those fighters in it.
  const badRequest = runnable && state.status === "error" && state.error.status === 400;
  useEffect(() => {
    if ((!runnable && q.ids.length > 0) || badRequest) setPending(q.ids);
  }, [search, badRequest]); // eslint-disable-line react-hooks/exhaustive-deps

  const data = runnable && state.status === "ready" ? state.data : null;

  // One run per loaded result: a new result starts at the VS screen again.
  const [run, setRun] = useState<{ data: FightResult | null; phase: Phase }>({ data: null, phase: VS });
  if (run.data !== data) setRun({ data, phase: VS });
  const phase = run.data === data ? run.phase : VS;
  const total = data?.rounds.length ?? 0;
  const go = (action: Step) => setRun((r) => ({ ...r, phase: step(r.phase, action, total) }));

  const [auto, setAuto] = useState(true);
  const startRef = useRef<HTMLButtonElement>(null);
  const nextRef = useRef<HTMLButtonElement>(null);
  const koRef = useRef<HTMLHeadingElement>(null);
  const [said, setSaid] = useState("");

  // Rounds play by themselves unless the visitor paused them or asked for less motion.
  useEffect(() => {
    if (phase.kind !== "rounds" || !auto || reduced) return;
    const id = window.setTimeout(() => go("next"), ROUND_MS);
    return () => window.clearTimeout(id);
  }, [phase, auto, reduced]); // eslint-disable-line react-hooks/exhaustive-deps

  // Focus follows the fight: Next when it starts, the K.O. title at the end, FIGHT on a rematch.
  const prevKind = useRef<Phase["kind"]>("vs");
  useEffect(() => {
    const before = prevKind.current;
    prevKind.current = phase.kind;
    if (before === phase.kind) return;
    if (phase.kind === "rounds") nextRef.current?.focus();
    else if (phase.kind === "ko") koRef.current?.focus();
    else if (before === "ko") startRef.current?.focus();
  }, [phase.kind]);

  // One announcement per round, and the result at the end.
  const shown = data ? roundsShown(phase, total) : 0;
  useEffect(() => {
    if (!data) return setSaid("");
    if (phase.kind === "rounds") setSaid(announceText(data.rounds[shown - 1]!, namesOf(data)));
    else if (phase.kind === "ko") setSaid(t("fight.announce.ko", { line: resultLine(data) }));
    else setSaid("");
  }, [data, phase.kind, shown]);

  // The end of the fight: tell the page (the guess settles) once per loaded result, and keep it for Rematch.
  const told = useRef<FightResult | null>(null);
  useEffect(() => {
    if (!data || phase.kind !== "ko" || told.current === data) return;
    told.current = data;
    const ids = data.fighters.map((f) => f.kol);
    setLastFight({ ids, date: data.date });
    emitPageEvent("fight_over", { ids, date: data.date, winner: data.winner });
  }, [data, phase.kind]);

  const title = data ? data.fighters.map((f) => f.name).join(` ${t("fight.vs_sep")} `) : t("fight.window.title");

  let footer: ReactNode = undefined;
  if (data && phase.kind === "vs") {
    footer = (
      <div className="fight-controls">
        <Button ref={startRef} variant="fight" onClick={() => go("start")}>
          {t("fight.start")}
        </Button>
      </div>
    );
  } else if (data && phase.kind === "rounds") {
    footer = (
      <div className="fight-controls" role="group" aria-label={t("fight.controls")}>
        <Button ref={nextRef} variant="primary" onClick={() => go("next")}>
          {shown >= total ? t("fight.next_ko") : t("fight.next")}
        </Button>
        {!reduced && (
          <Button variant="secondary" aria-pressed={!auto} onClick={() => setAuto((a) => !a)}>
            {auto ? t("fight.pause") : t("fight.play")}
          </Button>
        )}
        <Button variant="secondary" onClick={() => go("skip")}>
          {t("fight.skip")}
        </Button>
      </div>
    );
  }

  const replaceTo = (path: string) => navigate(path, { replace: true });

  let body: ReactNode;
  if (!runnable) {
    body = <Picker headingLevel={3} replace />;
  } else if (state.status === "loading") {
    body = <Skeleton label={t("fight.window.loading")} variant="block" />;
  } else if (state.status === "error") {
    const e = state.error;
    if (e.status === 400) {
      body = (
        <>
          <p className="fight-line">{t("fight.error.400", { message: e.message })}</p>
          <Picker headingLevel={3} replace />
        </>
      );
    } else if (e.status === 404) {
      body = (
        <div className="fight-error">
          <p className="fight-line" role="alert">
            {t("fight.error.404", { date: q.date ?? "" })}
          </p>
          <Button variant="primary" onClick={() => replaceTo(fightPath(q.ids))}>
            {t("fight.error.404_latest")}
          </Button>
        </div>
      );
    } else if (e.status === 503) {
      body = <ErrorLine message={t("fight.error.503")} onRetry={state.reload} retryLabel={t("fight.retry")} />;
    } else {
      body = <ErrorLine message={t("fight.error.other", { message: e.message })} onRetry={state.reload} retryLabel={t("fight.retry")} />;
    }
  } else if (data) {
    body = (
      <FightView
        fight={data}
        phase={phase}
        shown={shown}
        reduced={reduced}
        koRef={koRef}
        onRematch={() => go("rematch")}
        onFresh={() => (q.date === null ? state.reload() : replaceTo(fightPath(q.ids)))}
        onChange={() => {
          setPending(data.fighters.map((f) => f.kol));
          replaceTo(fightPath());
        }}
      />
    );
  }

  return (
    <Dialog open onClose={close} title={title} closeLabel={t("fight.window.close")} tone="pink" footer={footer}>
      <div className="fight-window">
        {body}
        <p className="ds-sr-only" role="status" aria-live="polite">
          {said}
        </p>
      </div>
    </Dialog>
  );
}

function resultLine(f: FightResult): string {
  const name = namesOf(f);
  const head =
    f.winner === null ? t("fight.ko.draw_line") : f.decision === "score_tiebreak" ? t("fight.ko.tiebreak", { name: name(f.winner) }) : t("fight.ko.winner", { name: name(f.winner) });
  return `${head}. ${t("fight.ko.scoreline", { line: scoreLine(f) })}`;
}

// ---------- one fight ----------

interface FightViewProps {
  fight: FightResult;
  phase: Phase;
  shown: number;
  reduced: boolean;
  koRef: RefObject<HTMLHeadingElement | null>;
  onRematch: () => void;
  onFresh: () => void;
  onChange: () => void;
}

function FightView({ fight, phase, shown, reduced, koRef, onRematch, onFresh, onChange }: FightViewProps) {
  const name = namesOf(fight);
  const ids = fight.fighters.map((f) => f.kol);
  const current = phase.kind === "rounds" ? fight.rounds[shown - 1] : undefined;

  return (
    <div className={`fight fight--${phase.kind}`} data-phase={phase.kind}>
      {fight.missing.length > 0 && <p className="fight-line">{t("fight.missing", { ids: fight.missing.join(", ") })}</p>}

      {phase.kind === "vs" ? <VsScreen fight={fight} /> : <FighterStrip fight={fight} shown={shown} current={current} />}

      {/* The guess stays mounted the whole fight, so it hears fight_over; it is hidden while rounds play. */}
      <div className="fight-before" hidden={phase.kind === "rounds"}>
        <Slot name="fight/before_fight" props={{ ids, date: fight.date }} loading={null} />
      </div>

      {phase.kind === "rounds" && current && (
        <div className="fight-stage">
          <p className="fight-stage__count fight-mono">{t("fight.round.progress", { n: shown, total: fight.rounds.length })}</p>
          <RoundView key={current.number} round={current} fight={fight} name={name} grow={!reduced} />
          {shown > 1 && (
            <section className="fight-played" aria-label={t("fight.round.played")}>
              {fight.rounds
                .slice(0, shown - 1)
                .reverse()
                .map((r) => (
                  <RoundView key={r.number} round={r} fight={fight} name={name} grow={false} compact />
                ))}
            </section>
          )}
        </div>
      )}

      {phase.kind === "ko" && <KoScreen fight={fight} koRef={koRef} onRematch={onRematch} onFresh={onFresh} onChange={onChange} />}
    </div>
  );
}

function rankText(f: Fighter): string {
  return f.rank === null ? t("fight.vs.unranked") : t("fight.vs.rank", { rank: f.rank });
}

function VsScreen({ fight }: { fight: FightResult }) {
  return (
    <section className="fight-vs" aria-label={t("fight.vs.title")}>
      <ol className="fight-vs__list" data-n={fight.fighters.length}>
        {fight.fighters.map((f, i) => (
          <li key={f.kol} className="fight-vs__item" style={{ "--i": i } as CSSProperties}>
            {i > 0 && (
              <span className="fight-vs__mark" aria-hidden="true">
                {t("fight.vs")}
              </span>
            )}
            <Card tone="fighter">
              <div className="fight-vs__card">
                <PixelAvatar id={f.kol} size={64} />
                <h3 className="fight-vs__name">{f.name}</h3>
                <div className="fight-vs__facts">
                  <TierBadge tier={f.card.tier} />
                  <span className="fight-muted fight-mono">{rankText(f)}</span>
                </div>
                <ScoreBar score={f.card.score} tier={f.card.tier} label={t("fight.score_label")} />
                <HpBar hp={HP_SEGMENTS} max={HP_SEGMENTS} label={t("fight.hp", { hp: HP_SEGMENTS, max: HP_SEGMENTS })} />
              </div>
            </Card>
          </li>
        ))}
      </ol>
      <p className="fight-muted">{t("fight.vs.rules")}</p>
      <p className="fight-muted">
        {t("fight.vs.board_of")} <SnapshotDate at={fight.date} />
      </p>
    </section>
  );
}

/** Fighters across the top while rounds play and at the end: points and HP. */
function FighterStrip({ fight, shown, current }: { fight: FightResult; shown: number; current: Round | undefined }) {
  const hp = hpAfter(fight, shown);
  const pts = pointsAfter(fight, shown);
  const won = current ? roundWinners(current) : [];
  return (
    <ul className="fight-strip" aria-label={t("fight.vs.title")}>
      {fight.fighters.map((f) => {
        const hit = current?.damaged.includes(f.kol) ?? false;
        const star = won.includes(f.kol);
        return (
          <li key={f.kol} className={`fight-strip__item${star ? " fight-strip__item--star" : ""}`}>
            <span key={`fx-${shown}`} className={`fight-strip__avatar${hit ? " fight-strip__avatar--hit" : ""}`}>
              <PixelAvatar id={f.kol} size={32} />
            </span>
            <div className="fight-strip__who">
              <span className="fight-strip__name">
                {f.name}
                {star && (
                  <span className="fight-strip__star" aria-hidden="true">
                    {" ★"}
                  </span>
                )}
              </span>
              <HpBar hp={hp[f.kol] ?? 0} max={HP_SEGMENTS} label={t("fight.hp", { hp: hp[f.kol] ?? 0, max: HP_SEGMENTS })} />
            </div>
            <span className="fight-strip__pts fight-mono" aria-label={t("fight.points_label", { n: formatPointsShort(pts[f.kol] ?? 0) })}>
              {t("fight.points", { n: formatPointsShort(pts[f.kol] ?? 0) })}
            </span>
          </li>
        );
      })}
    </ul>
  );
}

// ---------- a round ----------

function valueNode(round: Round, v: number, allTime: boolean): ReactNode {
  let node: ReactNode;
  if (MONEY_ROUNDS.has(round.kind)) node = <Money value={v} signed={round.kind !== "position_size"} />;
  else if (RATE_ROUNDS.has(round.kind)) node = percent(v);
  else if (round.kind === "last_trade") node = formatMinutesAgo(v);
  else node = formatPointsShort(v);
  if (!allTime) return node;
  return (
    <span>
      {node} <span className="fight-muted">{t("fight.round.all_time_mark")}</span>
    </span>
  );
}

function RoundView({ round, fight, name, grow, compact }: { round: Round; fight: FightResult; name: Names; grow: boolean; compact?: boolean }) {
  const [grown, setGrown] = useState(!grow);
  useEffect(() => {
    if (grown) return;
    const id = window.requestAnimationFrame(() => window.requestAnimationFrame(() => setGrown(true)));
    return () => window.cancelAnimationFrame(id);
  }, [grown]);

  const winners = roundWinners(round);
  const shares = roundShares(
    round.kind,
    round.values.map((v) => v.value),
  );
  const fighters: RoundFighter[] = round.values.map((v, i) => ({
    id: v.kol,
    name: name(v.kol),
    value: v.value === null ? null : valueNode(round, v.value, v.all_time),
    share: shares[i] === null ? null : grown ? shares[i]! : 0,
    winner: winners.includes(v.kol),
  }));
  const o = round.outcome;
  const status = o.type === "void" ? "void" : o.type === "tie" ? "tie" : "normal";
  const statusLabel =
    o.type === "void" ? (o.reason === "missing_data" ? t("fight.round.void_missing") : t("fight.round.void_tied")) : o.type === "tie" ? t("fight.round.tie") : undefined;

  return (
    <div className={`fight-round${compact ? " fight-round--compact" : " fight-round--current"}${grow ? " fight-round--new" : ""}`}>
      <RoundRow
        number={round.number}
        numberLabel={t("fight.round.n", { n: round.number })}
        title={KIND_TITLE[round.kind]()}
        measures={compact ? undefined : KIND_MEASURES[round.kind]()}
        status={status}
        statusLabel={statusLabel}
        noDataLabel={t("fight.round.no_data")}
        winnerLabel={t("fight.round.winner")}
        why={whyText(round, name)}
        fighters={fighters}
      />
      {!compact && <p className="fight-source">{sourceText(round, fight.date, name)}</p>}
    </div>
  );
}

// ---------- K.O. ----------

interface KoProps {
  fight: FightResult;
  koRef: RefObject<HTMLHeadingElement | null>;
  onRematch: () => void;
  onFresh: () => void;
  onChange: () => void;
}

function KoScreen({ fight, koRef, onRematch, onFresh, onChange }: KoProps) {
  const name = namesOf(fight);
  const [tab, setTab] = useState("rounds");
  const winner = fight.winner === null ? null : (fight.fighters.find((f) => f.kol === fight.winner) ?? null);
  const head =
    winner === null ? t("fight.ko.draw_line") : fight.decision === "score_tiebreak" ? t("fight.ko.tiebreak", { name: winner.name }) : t("fight.ko.winner", { name: winner.name });

  return (
    <>
      <Card tone="gold" as="section" aria-labelledby="fight-ko-title">
        <div className="fight-ko">
          <h3 id="fight-ko-title" ref={koRef} tabIndex={-1} className="fight-ko__stamp">
            {winner === null ? t("fight.ko.draw") : t("fight.ko")}
          </h3>
          {winner && <PixelAvatar id={winner.kol} size={64} />}
          <p className="fight-ko__winner">{head}</p>
          <p className="fight-ko__score fight-mono">{t("fight.ko.scoreline", { line: scoreLine(fight) })}</p>
          <p className="fight-muted">{fight.counted_line}</p>
          <ul className="fight-ko__summary">
            {fight.summary.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </div>
      </Card>

      <Actions fight={fight} onRematch={onRematch} onFresh={onFresh} onChange={onChange} />

      <div className="fight-details">
        <Tabs
          idBase="fight-ko"
          label={t("fight.ko.tabs")}
          value={tab}
          onChange={setTab}
          items={[
            { id: "rounds", label: t("fight.ko.tab_rounds") },
            { id: "where", label: t("fight.ko.tab_where") },
          ]}
        />
        <TabPanel idBase="fight-ko" tab="rounds" value={tab}>
          <div className="fight-all-rounds">
            {fight.rounds.map((r) => (
              <RoundView key={r.number} round={r} fight={fight} name={name} grow={false} compact />
            ))}
          </div>
        </TabPanel>
        <TabPanel idBase="fight-ko" tab="where" value={tab}>
          <WhereScore fight={fight} />
        </TabPanel>
      </div>

      <Share fight={fight} />
    </>
  );
}

function Actions({ fight, onRematch, onFresh, onChange }: Omit<KoProps, "koRef">) {
  const board = useBoard();
  let freshOff: string | null = null;
  if (board.status === "ready") {
    const rows = byKol(board.data.rows);
    const gone = fight.fighters.find((f) => !rows.has(f.kol));
    if (gone) freshOff = t("fight.rematch_fresh_off", { name: gone.name });
    else if (board.data.date === fight.date) freshOff = t("fight.rematch_fresh_same");
  }
  return (
    <div className="fight-actions" role="group" aria-label={t("fight.ko.actions")}>
      <Button variant="fight" onClick={onRematch}>
        {t("fight.rematch")}
      </Button>
      {freshOff === null ? (
        <Button variant="secondary" onClick={onFresh}>
          {t("fight.rematch_fresh")}
        </Button>
      ) : (
        <Button variant="secondary" disabled reason={freshOff}>
          {t("fight.rematch_fresh")}
        </Button>
      )}
      <Button variant="secondary" onClick={onChange}>
        {t("fight.change")}
      </Button>
      <ul className="fight-cards">
        {fight.fighters.map((f) => (
          <li key={f.kol}>
            <Link to={`/kol/${encodeURIComponent(f.kol)}?date=${fight.date}`} className="fight-link">
              {t("fight.open_card", { name: f.name })}
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
}

const PARTS: PartKind[] = ["profit", "accuracy", "form", "activity"];

function partRaw(kind: PartKind, raw: number, allTime: boolean): ReactNode {
  let node: ReactNode;
  if (kind === "profit" || kind === "form") node = <Money value={raw} />;
  else if (kind === "accuracy") node = percent(raw);
  else node = formatMinutesAgo(raw);
  return allTime ? (
    <>
      {node} <span className="fight-muted">{t("fight.round.all_time_mark")}</span>
    </>
  ) : (
    node
  );
}

/** The four parts of the Score side by side for every fighter (stacked on a phone). */
function WhereScore({ fight }: { fight: FightResult }) {
  const n = fight.fighters.length;
  return (
    <div className="fight-where">
      <p className="fight-muted">{t("fight.where.formula")}</p>
      {PARTS.map((kind) => {
        const best = Math.max(...fight.fighters.map((f) => f.card.parts.find((p) => p.kind === kind)?.points ?? 0));
        return (
          <section key={kind} className="fight-where__part" aria-labelledby={`fight-where-${kind}`}>
            <h4 id={`fight-where-${kind}`} className="fight-where__title">
              {PART_NAME[kind]()} <span className="fight-mono fight-muted">{t("fight.where.weight", { n: PART_WEIGHT[kind] })}</span>{" "}
              <span className="fight-muted">{PART_RAW[kind]()}</span>
            </h4>
            <ul className="fight-where__cells" style={{ "--n": n } as CSSProperties}>
              {fight.fighters.map((f) => {
                const p = f.card.parts.find((x) => x.kind === kind);
                const empty = !p || p.status === "no_data" || p.raw === null;
                const max = PART_WEIGHT[kind];
                return (
                  <li key={f.kol} className="fight-where__cell">
                    <span className="fight-where__name">{f.name}</span>
                    <span className="fight-where__raw">{empty ? t("fight.where.no_data") : partRaw(kind, p.raw!, p.all_time)}</span>
                    <Progress
                      value={empty ? 0 : Math.round(p.share * 100)}
                      label={t("fight.where.share", { name: f.name, part: PART_NAME[kind]() })}
                      valueText={t("fight.where.share_text", { n: empty ? 0 : Math.round(p.share * 100) })}
                      tone={!empty && p.points > 0 && p.points === best ? "coin" : "lavender"}
                    />
                    <span className="fight-where__pts fight-mono">
                      {empty ? t("fight.where.points", { points: 0, max }) : t("fight.where.points", { points: formatPoints(p.points), max })}
                    </span>
                    {p?.status === "needs_profit" && <span className="fight-muted">{t("fight.where.needs_profit")}</span>}
                  </li>
                );
              })}
            </ul>
          </section>
        );
      })}
      <section className="fight-where__part" aria-labelledby="fight-where-total">
        <h4 id="fight-where-total" className="fight-where__title">
          {t("fight.where.total")}
        </h4>
        <ul className="fight-where__cells" style={{ "--n": n } as CSSProperties}>
          {fight.fighters.map((f) => (
            <li key={f.kol} className="fight-where__cell">
              <span className="fight-where__name">{f.name}</span>
              <ScoreBar score={f.card.score} tier={f.card.tier} label={t("fight.score_label")} />
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}

// ---------- share ----------

function Share({ fight }: { fight: FightResult }) {
  const ids = fight.fighters.map((f) => f.kol);
  const url = `${window.location.origin}${fightPath(ids, fight.date)}`;
  const image = fightImagePath(ids, fight.date);
  const [note, setNote] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!note) return;
    const id = window.setTimeout(() => setNote(""), 4000);
    return () => window.clearTimeout(id);
  }, [note]);

  const copy = () => {
    const failed = () => {
      inputRef.current?.focus();
      inputRef.current?.select();
      setNote(t("fight.share.copy_failed"));
    };
    if (!navigator.clipboard) return failed();
    navigator.clipboard.writeText(url).then(() => setNote(t("fight.share.copied")), failed);
  };

  return (
    <section className="fight-share" aria-labelledby="fight-share-title">
      <h3 id="fight-share-title" className="fight-share__title">
        {t("fight.share.title")}
      </h3>
      <Field ref={inputRef} label={t("fight.share.link")} value={url} readOnly onFocus={(e) => e.currentTarget.select()} />
      <div className="fight-share__buttons">
        <Button variant="primary" onClick={copy}>
          {t("fight.share.copy")}
        </Button>
        <a className={buttonClass("secondary")} href={xPostUrl(shareText(fight), url)} target="_blank" rel="noopener noreferrer">
          {t("fight.share.x")}
        </a>
        <a className={buttonClass("secondary")} href={image} target="_blank" rel="noopener">
          {t("fight.share.image")}
        </a>
      </div>
      <p className="fight-muted" role="status">
        {note}
      </p>
    </section>
  );
}
