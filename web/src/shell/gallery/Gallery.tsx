import { useMemo, useState } from "react";
import type { ReactNode } from "react";
import {
  Badge,
  Bracket,
  Button,
  Card,
  Container,
  Dialog,
  EmptyState,
  ErrorLine,
  Field,
  Heading,
  HpBar,
  List,
  ListItem,
  Money,
  NoData,
  PixelAvatar,
  Progress,
  RoundRow,
  ScoreBar,
  ScoreBreakdown,
  Select,
  Skeleton,
  SnapshotDate,
  StatTile,
  TabPanel,
  Tabs,
  TierBadge,
  TimeAgo,
  Ticker,
  Tooltip,
  buttonClass,
} from "@arena/design";
import type { BracketRound, ScoreBreakdownLabels, ScorePart, Tier } from "@arena/design";
import { t } from "../i18n";

// Sample data: names are proper nouns from the prototype; figures are made up to show every state.

const COLORS = [
  "background",
  "surface",
  "surface-sunken",
  "border",
  "text",
  "text-muted",
  "coin",
  "pink",
  "blue",
  "lavender",
  "plus",
  "minus",
  "podium-silver",
  "podium-bronze",
] as const;

const TYPES = ["display", "section-title", "pixel-label", "title", "body", "small", "label", "mono", "stat"] as const;

const KOLS = ["tekkerrss", "levis", "decu", "cooker", "trunoest", "peter", "kook", "ocr", "iced", "shah", "naka", "juicy"];
const TIERS: Tier[] = ["goat", "diamond", "sendoor", "degen"];

function Section({ name, note, children }: { name: string; note: string; children: ReactNode }) {
  const id = `gallery-${name.toLowerCase().replace(/\W+/g, "-")}`;
  return (
    <section className="gallery__section" aria-labelledby={id}>
      <h2 id={id} className="t-section-title gallery__title">
        {name}
      </h2>
      <p className="gallery__note">{note}</p>
      {children}
    </section>
  );
}

function Sample({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="gallery__sample">
      <p className="gallery__label">{label}</p>
      <div className="gallery__stage">{children}</div>
    </div>
  );
}

/** Every design piece with sample props. Staff-only at /team/design. */
export default function Gallery() {
  const [dialog, setDialog] = useState(false);
  const [tab, setTab] = useState("all");
  const [sel, setSel] = useState("a");

  const breakdownLabels: ScoreBreakdownLabels = useMemo(
    () => ({
      part: t("gallery.breakdown.part"),
      raw: t("gallery.breakdown.raw"),
      scale: t("gallery.breakdown.scale"),
      points: t("gallery.breakdown.points"),
      sum: t("gallery.breakdown.sum"),
      score: t("gallery.breakdown.score"),
      noData: t("gallery.breakdown.no_data"),
    }),
    [],
  );
  const parts: ScorePart[] = [
    { id: "profit", label: t("gallery.breakdown.profit"), weightLabel: "40%", raw: <Money value={207060} mode="full" />, share: 0.858, points: 34.3, max: 40 },
    { id: "accuracy", label: t("gallery.breakdown.accuracy"), weightLabel: "25%", raw: "44.2%*", share: 0.284, points: 7.1, max: 25 },
    { id: "form", label: t("gallery.breakdown.form"), weightLabel: "20%", raw: <Money value={-1600} mode="full" />, share: 0, points: 0, max: 20 },
    { id: "activity", label: t("gallery.breakdown.activity"), weightLabel: "15%", raw: <TimeAgo minutes={120} />, share: 0.89, points: 13.4, max: 15 },
  ];
  const partsNoData: ScorePart[] = parts.map((p) => (p.id === "form" ? { ...p, raw: null, share: null, points: null } : p));

  const tickerItems = [
    { id: "1", content: t("gallery.ticker.one") },
    { id: "2", content: t("gallery.ticker.two") },
    { id: "3", content: t("gallery.ticker.three") },
    { id: "4", content: t("gallery.ticker.four") },
  ];

  const rounds: BracketRound[] = [
    {
      id: "qf",
      title: t("gallery.bracket.qf"),
      matches: [
        { id: "q1", a: { id: "decu", name: "Decu", score: 95, winner: true }, b: { id: "ocr", name: "ocr", score: 10 } },
        { id: "q2", a: { id: "levis", name: "Levis", score: 41 }, b: { id: "cooker", name: "Cooker", score: 88, winner: true } },
        { id: "q3", a: { id: "trunoest", name: "Trunoest", score: 85, winner: true }, b: { id: "kook", name: "kook", score: 15 } },
        { id: "q4", a: { id: "tekkerrss", name: "Tekkerrss", score: 55, winner: true }, b: { id: "peter", name: "Peter", score: 47 } },
      ],
    },
    {
      id: "sf",
      title: t("gallery.bracket.sf"),
      matches: [
        { id: "s1", a: { id: "decu", name: "Decu", score: 95, winner: true }, b: { id: "cooker", name: "Cooker", score: 88 } },
        { id: "s2", a: { id: "trunoest", name: "Trunoest", score: 85 }, b: { id: "tekkerrss", name: "Tekkerrss", score: 55 } },
      ],
    },
    { id: "f", title: t("gallery.bracket.final"), matches: [{ id: "f1", a: { id: "decu", name: "Decu" }, b: null }] },
  ];

  return (
    <Container>
      <div className="gallery">
        <header className="gallery__head">
          <h1 className="t-section-title">{t("gallery.title")}</h1>
          <p className="gallery__note">{t("gallery.intro")}</p>
        </header>

        <Section name="Colors" note={t("gallery.colors.note")}>
          <ul className="gallery__swatches">
            {COLORS.map((c) => (
              <li key={c} className="gallery__swatch">
                <span className="gallery__chip" style={{ background: `var(--color-${c})` }} />
                <code>{c}</code>
              </li>
            ))}
          </ul>
        </Section>

        <Section name="Type" note={t("gallery.type.note")}>
          <ul className="gallery__types">
            {TYPES.map((n) => (
              <li key={n} className="gallery__type">
                <code>{n}</code>
                <span className={`t-${n}`}>{n === "display" || n === "section-title" || n === "pixel-label" ? t("gallery.type.pixel") : t("gallery.type.sample")}</span>
              </li>
            ))}
          </ul>
        </Section>

        <Section name="Heading" note={t("gallery.heading.note")}>
          <div className="gallery__stack">
            <Heading level={3} variant="display">
              KOL ARENA
            </Heading>
            <Heading level={3} variant="section" tone="pink">
              KOL FIGHT
            </Heading>
            <Heading level={3} variant="section">
              {t("gallery.heading.section")}
            </Heading>
            <Heading level={3} variant="title">
              {t("gallery.heading.title")}
            </Heading>
          </div>
        </Section>

        <Section name="Button" note={t("gallery.button.note")}>
          <div className="gallery__row">
            <Button variant="primary">{t("gallery.button.primary")}</Button>
            <Button variant="fight">{t("gallery.button.fight")}</Button>
            <Button variant="secondary">{t("gallery.button.secondary")}</Button>
            <a href="#gallery-button" className={buttonClass("secondary")}>
              {t("gallery.button.link")}
            </a>
          </div>
          <div className="gallery__row">
            <Button variant="fight" disabled reason={t("gallery.button.reason")}>
              {t("gallery.button.fight")}
            </Button>
            <Button variant="secondary" disabled reason={t("gallery.button.reason_network")}>
              {t("gallery.button.network_fight")}
            </Button>
          </div>
        </Section>

        <Section name="Field and Select" note={t("gallery.field.note")}>
          <div className="gallery__grid">
            <Field label={t("gallery.field.label")} placeholder={t("gallery.field.placeholder")} hint={t("gallery.field.hint")} />
            <Field label={t("gallery.field.label_error")} defaultValue="@" error={t("gallery.field.error")} />
            <Select
              label={t("gallery.select.label")}
              value={sel}
              onChange={(e) => setSel(e.target.value)}
              options={[
                { value: "a", label: t("gallery.select.a") },
                { value: "b", label: t("gallery.select.b") },
                { value: "c", label: t("gallery.select.c") },
              ]}
            />
          </div>
        </Section>

        <Section name="List and Card" note={t("gallery.card.note")}>
          <div className="gallery__grid">
            <List label={t("gallery.list.label")}>
              <ListItem onSelect={() => undefined}>{t("gallery.list.one")}</ListItem>
              <ListItem onSelect={() => undefined} current>
                {t("gallery.list.two")}
              </ListItem>
              <ListItem>{t("gallery.list.three")}</ListItem>
            </List>
            <List label={t("gallery.list.label")} variant="cards">
              <ListItem>{t("gallery.list.one")}</ListItem>
              <ListItem>{t("gallery.list.two")}</ListItem>
            </List>
          </div>
          <div className="gallery__grid">
            <Card>{t("gallery.card.panel")}</Card>
            <Card tone="sunken">{t("gallery.card.sunken")}</Card>
            <Card tone="fighter">{t("gallery.card.fighter")}</Card>
            <Card tone="gold">{t("gallery.card.gold")}</Card>
            <Card tone="silver">{t("gallery.card.silver")}</Card>
            <Card tone="bronze">{t("gallery.card.bronze")}</Card>
          </div>
          <div className="gallery__grid">
            <StatTile value={<Money value={5300000} />} accent label={t("gallery.stat.one")} />
            <StatTile value="102 of 160" label={t("gallery.stat.two")} />
            <StatTile value="98" label={t("gallery.stat.three")} />
          </div>
        </Section>

        <Section name="Dialog" note={t("gallery.dialog.note")}>
          <Button onClick={() => setDialog(true)}>{t("gallery.dialog.open")}</Button>
          <Dialog
            open={dialog}
            onClose={() => setDialog(false)}
            title={t("gallery.dialog.title")}
            closeLabel={t("shell.close")}
            footer={<Button onClick={() => setDialog(false)}>{t("gallery.dialog.done")}</Button>}
          >
            <p>{t("gallery.dialog.body")}</p>
            <p className="gallery__note">
              {t("gallery.dialog.tip_before")}{" "}
              <Tooltip content={t("gallery.tooltip.text")}>
                <span className="gallery__hint">{t("gallery.dialog.tip_word")}</span>
              </Tooltip>
            </p>
            <Field label={t("gallery.field.label")} />
          </Dialog>
        </Section>

        <Section name="Tabs" note={t("gallery.tabs.note")}>
          <Tabs
            idBase="gallery-tabs"
            label={t("gallery.tabs.label")}
            value={tab}
            onChange={setTab}
            items={[
              { id: "all", label: t("gallery.tabs.all"), count: 122 },
              { id: "top", label: t("gallery.tabs.top"), count: 100 },
              { id: "net", label: t("gallery.tabs.net"), count: 22 },
              { id: "off", label: t("gallery.tabs.off"), disabled: true },
            ]}
          />
          <TabPanel idBase="gallery-tabs" tab="all" value={tab}>
            {t("gallery.tabs.panel_all")}
          </TabPanel>
          <TabPanel idBase="gallery-tabs" tab="top" value={tab}>
            {t("gallery.tabs.panel_top")}
          </TabPanel>
          <TabPanel idBase="gallery-tabs" tab="net" value={tab}>
            {t("gallery.tabs.panel_net")}
          </TabPanel>
        </Section>

        <Section name="Badge" note={t("gallery.badge.note")}>
          <div className="gallery__row">
            <Badge shape="pill" icon="🤝">
              {t("gallery.badge.network")}
            </Badge>
            <Badge tone="plus">{t("gallery.badge.confirmed")}</Badge>
            <Badge tone="coin">{t("gallery.badge.hypothesis")}</Badge>
            <Badge tone="muted">{t("gallery.badge.not_found")}</Badge>
            <Badge tone="minus">{t("gallery.badge.stale")}</Badge>
            <Badge tone="blue" shape="pill" icon="🐋" hint={t("gallery.badge.whale_hint")}>
              {t("gallery.badge.whale")}
            </Badge>
          </div>
        </Section>

        <Section name="Progress" note={t("gallery.progress.note")}>
          <div className="gallery__grid">
            <Progress value={72} label={t("gallery.progress.label")} tone="coin" />
            <Progress value={40} label={t("gallery.progress.label")} tone="plus" />
            <Progress value={12} label={t("gallery.progress.label")} tone="lavender" />
          </div>
        </Section>

        <Section name="Tooltip" note={t("gallery.tooltip.note")}>
          <Tooltip content={t("gallery.tooltip.text")}>
            <span className="gallery__hint">{t("gallery.tooltip.trigger")}</span>
          </Tooltip>
        </Section>

        <Section name="Skeleton, EmptyState, ErrorLine" note={t("gallery.states.note")}>
          <div className="gallery__grid">
            <Skeleton label={t("shell.loading")} count={3} />
            <Skeleton label={t("shell.loading")} variant="rows" count={2} />
            <EmptyState message={t("gallery.states.empty")} action={<Button variant="secondary">{t("gallery.states.clear")}</Button>} />
            <ErrorLine message={t("gallery.states.error")} onRetry={() => undefined} retryLabel={t("shell.retry")} />
          </div>
        </Section>

        <Section name="PixelAvatar" note={t("gallery.avatar.note")}>
          <div className="gallery__row">
            {KOLS.map((id) => (
              <PixelAvatar key={id} id={id} size={48} />
            ))}
          </div>
          <div className="gallery__row">
            <PixelAvatar id="tekkerrss" size={24} />
            <PixelAvatar id="tekkerrss" size={40} />
            <PixelAvatar id="tekkerrss" size={64} />
          </div>
        </Section>

        <Section name="TierBadge and ScoreBar" note={t("gallery.tier.note")}>
          <div className="gallery__grid">
            {TIERS.map((tier, i) => (
              <div key={tier} className="gallery__stack">
                <TierBadge tier={tier} />
                <ScoreBar score={[95, 55, 41, 12][i]!} tier={tier} label={t("gallery.score.label")} />
              </div>
            ))}
          </div>
          <ScoreBar score={55} tier="diamond" label={t("gallery.score.label")} size="lg" />
        </Section>

        <Section name="Money, TimeAgo, SnapshotDate" note={t("gallery.money.note")}>
          <div className="gallery__grid">
            <Sample label={t("gallery.money.short")}>
              <Money value={207060} />
              <Money value={-1600} />
              <Money value={5300000} />
              <Money value={629} />
              <Money value={0} />
            </Sample>
            <Sample label={t("gallery.money.full")}>
              <Money value={207060} mode="full" />
              <Money value={-298530} mode="full" />
              <Money value={1280000} mode="full" signed={false} />
            </Sample>
            <Sample label={t("gallery.money.nodata")}>
              <Money value={null} reason={t("gallery.money.reason")} />
              <NoData reason={t("gallery.money.reason")} mode="text" />
            </Sample>
            <Sample label={t("gallery.time.label")}>
              <TimeAgo minutes={59} />
              <TimeAgo minutes={120} />
              <TimeAgo minutes={7200} />
              <SnapshotDate at="2026-10-18T06:00:00Z" />
              <SnapshotDate at="2026-10-18" />
            </Sample>
          </div>
        </Section>

        <Section name="ScoreBreakdown" note={t("gallery.breakdown.note")}>
          <div className="gallery__grid gallery__grid--wide">
            <ScoreBreakdown parts={parts} sum={54.8} score={55} tier="diamond" labels={breakdownLabels} />
            <ScoreBreakdown parts={partsNoData} sum={41.8} score={42} tier="sendoor" labels={breakdownLabels} />
          </div>
        </Section>

        <Section name="Ticker" note={t("gallery.ticker.note")}>
          <Ticker items={tickerItems} label={t("shell.ticker.label")} pauseLabel={t("shell.ticker.pause")} resumeLabel={t("shell.ticker.resume")} />
        </Section>

        <Section name="HpBar" note={t("gallery.hp.note")}>
          <div className="gallery__grid">
            {[8, 5, 2].map((hp) => (
              <Card key={hp}>
                <HpBar hp={hp} label={t("gallery.hp.label", { hp, max: 8 })} />
              </Card>
            ))}
          </div>
        </Section>

        <Section name="RoundRow" note={t("gallery.round.note")}>
          <div className="gallery__stack gallery__stack--wide">
            <RoundRow
              number={2}
              numberLabel={t("gallery.round.number", { n: 2 })}
              title={t("gallery.round.pnl30")}
              measures={t("gallery.round.measures")}
              noDataLabel={t("gallery.round.no_data")}
              winnerLabel={t("gallery.round.winner")}
              why={t("gallery.round.why")}
              fighters={[
                { id: "tekkerrss", name: "Tekkerrss", value: <Money value={207060} />, share: 0.37 },
                { id: "levis", name: "Levis", value: <Money value={16100} />, share: 0.03 },
                { id: "decu", name: "Decu", value: <Money value={565100} />, share: 1, winner: true },
              ]}
            />
            <RoundRow
              number={4}
              numberLabel={t("gallery.round.number", { n: 4 })}
              title={t("gallery.round.pnl24")}
              status="void"
              statusLabel={t("gallery.round.void")}
              noDataLabel={t("gallery.round.no_data")}
              winnerLabel={t("gallery.round.winner")}
              fighters={[
                { id: "tekkerrss", name: "Tekkerrss", value: null, share: null },
                { id: "levis", name: "Levis", value: null, share: null },
                { id: "decu", name: "Decu", value: <Money value={12900} />, share: 1 },
              ]}
            />
            <RoundRow
              number={7}
              numberLabel={t("gallery.round.number", { n: 7 })}
              title={t("gallery.round.size")}
              status="tie"
              statusLabel={t("gallery.round.tie")}
              noDataLabel={t("gallery.round.no_data")}
              winnerLabel={t("gallery.round.winner")}
              fighters={[
                { id: "tekkerrss", name: "Tekkerrss", value: <Money value={4000} signed={false} />, share: 1, winner: true },
                { id: "levis", name: "Levis", value: <Money value={4000} signed={false} />, share: 1, winner: true },
                { id: "decu", name: "Decu", value: <Money value={339} signed={false} />, share: 0.08 },
              ]}
            />
          </div>
        </Section>

        <Section name="Bracket" note={t("gallery.bracket.note")}>
          <Bracket rounds={rounds} label={t("gallery.bracket.label")} pendingLabel={t("gallery.bracket.pending")} winnerLabel={t("gallery.round.winner")} />
        </Section>
      </div>
    </Container>
  );
}
