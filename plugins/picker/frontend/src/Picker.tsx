import { useEffect, useMemo, useState } from "react";
import { Badge, Button, Card, Container, EmptyState, ErrorLine, Field, List, ListItem, Money, NoData, ScoreBar, Select, Skeleton, SnapshotDate, TierBadge, buttonClass } from "@arena/design";
import type { Tier } from "@arena/design";
import { t, useApi } from "@arena/shell";
import "./picker.css";

interface Row {
  kol: string;
  name: string;
  x_handle: string | null;
  fomo: string | null;
  x_followers: number | null;
  score: number | null;
  tier: Tier | null;
  rank: number | null;
  formats: string[];
  low_usd: number | null;
  high_usd: number | null;
}

interface PickList {
  date: string | null;
  no_ranking: string | null;
  total: number;
  rows: Row[];
  formats: string[];
}

interface Filters {
  score_min: string;
  score_max: string;
  tier: string;
  rate_min: string;
  rate_max: string;
  format: string;
}

const EMPTY: Filters = { score_min: "", score_max: "", tier: "", rate_min: "", rate_max: "", format: "" };
const DEBOUNCE_MS = 300;
const TIERS: Tier[] = ["goat", "diamond", "sendoor", "degen"];
const followers = new Intl.NumberFormat("en-US", { notation: "compact", maximumFractionDigits: 1 });

// t() keys must be plain literals, so each lookup is a switch
function tierText(tier: Tier): string {
  switch (tier) {
    case "goat":
      return t("picker.tier.goat");
    case "diamond":
      return t("picker.tier.diamond");
    case "sendoor":
      return t("picker.tier.sendoor");
    case "degen":
      return t("picker.tier.degen");
  }
}

function formatText(format: string): string {
  switch (format) {
    case "post":
      return t("picker.format.post");
    case "buy":
      return t("picker.format.buy");
    case "thread":
      return t("picker.format.thread");
    case "stream":
      return t("picker.format.stream");
    case "space":
      return t("picker.format.space");
    case "call":
      return t("picker.format.call");
    default:
      return format;
  }
}

function queryOf(f: Filters): string {
  const q = new URLSearchParams();
  for (const [k, v] of Object.entries(f)) if (v.trim()) q.set(k, v.trim());
  return q.toString();
}

function exportHref(kols: string[], date: string | null, rates: boolean): string {
  const q = new URLSearchParams({ kols: kols.join(",") });
  if (date) q.set("date", date);
  if (rates) q.set("rates", "1");
  return `/api/team/picker/export?${q.toString()}`;
}

function Rate({ row }: { row: Row }) {
  if (row.low_usd === null && row.high_usd === null) return <NoData reason={t("picker.rate.none")} />;
  if (row.low_usd === null || row.high_usd === null || row.low_usd === row.high_usd) {
    return <Money value={row.low_usd ?? row.high_usd} signed={false} />;
  }
  return (
    <span className="picker-rate">
      <Money value={row.low_usd} signed={false} />–<Money value={row.high_usd} signed={false} />
    </span>
  );
}

function KolRow({ row, checked, onToggle }: { row: Row; checked: boolean; onToggle: () => void }) {
  return (
    <div className="picker-row">
      <label className="picker-check">
        <input type="checkbox" checked={checked} onChange={onToggle} />
        <span className="picker-name">{row.name}</span>
      </label>
      <div className="picker-facts">
        {row.tier !== null && row.score !== null ? (
          <>
            <TierBadge tier={row.tier} />
            <ScoreBar score={row.score} tier={row.tier} label={t("picker.score")} size="sm" />
            {row.rank !== null && <span className="t-mono">{t("picker.rank", { rank: row.rank })}</span>}
          </>
        ) : (
          <NoData reason={t("picker.score.none")} />
        )}
      </div>
      <div className="picker-facts">
        {row.x_handle ? (
          <a href={`https://x.com/${encodeURIComponent(row.x_handle)}`} target="_blank" rel="noopener noreferrer" aria-label={t("picker.x_for", { name: row.name })}>
            @{row.x_handle}
          </a>
        ) : (
          <NoData reason={t("picker.x.none")} />
        )}
        {row.x_followers !== null && <span className="t-mono">{t("picker.followers", { count: followers.format(row.x_followers) })}</span>}
        {row.fomo?.startsWith("https://") && (
          <a href={row.fomo} target="_blank" rel="noopener noreferrer" aria-label={t("picker.fomo_for", { name: row.name })}>
            {t("picker.fomo")}
          </a>
        )}
      </div>
      <div className="picker-facts">
        {row.formats.length > 0 ? row.formats.map((f) => <Badge key={f}>{formatText(f)}</Badge>) : <NoData reason={t("picker.formats.none")} />}
        <span className="picker-rate-wrap">
          <span className="t-label">{t("picker.rate")}</span> <Rate row={row} />
        </span>
      </div>
    </div>
  );
}

export default function Picker() {
  const [filters, setFilters] = useState<Filters>(EMPTY);
  const [query, setQuery] = useState("");
  const [checked, setChecked] = useState<Set<string>>(() => new Set());

  useEffect(() => {
    const id = setTimeout(() => setQuery(queryOf(filters)), DEBOUNCE_MS);
    return () => clearTimeout(id);
  }, [filters]);

  const list = useApi<PickList>(query ? `/api/team/picker?${query}` : "/api/team/picker");
  const set = (key: keyof Filters) => (e: { target: { value: string } }) => setFilters((f) => ({ ...f, [key]: e.target.value }));

  const rows = list.status === "ready" ? list.data.rows : [];
  // Every checked KOL, also those the current filter hides.
  const picked = useMemo(() => [...checked], [checked]);
  const toggle = (kol: string) =>
    setChecked((c) => {
      const next = new Set(c);
      if (next.has(kol)) next.delete(kol);
      else next.add(kol);
      return next;
    });

  const formats = list.status === "ready" ? list.data.formats : [];
  const date = list.status === "ready" ? list.data.date : null;

  return (
    <Container>
      <h1 className="t-section-title">{t("picker.title")}</h1>
      <Card>
        <div className="picker-filters">
          <Field label={t("picker.filter.score_min")} inputMode="numeric" value={filters.score_min} onChange={set("score_min")} />
          <Field label={t("picker.filter.score_max")} inputMode="numeric" value={filters.score_max} onChange={set("score_max")} />
          <Select
            label={t("picker.filter.tier")}
            options={[{ value: "", label: t("picker.filter.any") }, ...TIERS.map((x) => ({ value: x, label: tierText(x) }))]}
            value={filters.tier}
            onChange={set("tier")}
          />
          <Field label={t("picker.filter.rate_min")} inputMode="decimal" value={filters.rate_min} onChange={set("rate_min")} />
          <Field label={t("picker.filter.rate_max")} inputMode="decimal" value={filters.rate_max} onChange={set("rate_max")} />
          <Select
            label={t("picker.filter.format")}
            options={[{ value: "", label: t("picker.filter.any") }, ...formats.map((x) => ({ value: x, label: formatText(x) }))]}
            value={filters.format}
            onChange={set("format")}
          />
        </div>
        <Button variant="secondary" onClick={() => setFilters(EMPTY)}>
          {t("picker.filter.clear")}
        </Button>
      </Card>

      {list.status === "loading" && <Skeleton label={t("picker.loading")} variant="rows" />}
      {list.status === "error" && <ErrorLine message={t("picker.error.load", { message: list.error.message })} onRetry={list.reload} retryLabel={t("picker.retry")} />}
      {list.status === "ready" && (
        <>
          <p role="status">
            {t("picker.shown", { shown: list.data.rows.length, total: list.data.total })}{" "}
            {list.data.date ? (
              <>
                {t("picker.snapshot")} <SnapshotDate at={list.data.date} />
              </>
            ) : (
              t("picker.no_ranking", { reason: list.data.no_ranking ?? "" })
            )}
          </p>
          <div className="picker-actions">
            {picked.length > 0 ? (
              <>
                <a className={buttonClass("primary")} href={exportHref(picked, date, false)} download>
                  {t("picker.export.client")}
                </a>
                <a className={buttonClass("secondary")} href={exportHref(picked, date, true)} download>
                  {t("picker.export.rates")}
                </a>
              </>
            ) : (
              <>
                <Button disabled reason={t("picker.export.reason")}>
                  {t("picker.export.client")}
                </Button>
                <Button variant="secondary" disabled reason={t("picker.export.reason")}>
                  {t("picker.export.rates")}
                </Button>
              </>
            )}
          </div>
          <p className="t-small">{t("picker.picked", { count: picked.length })}</p>
          {rows.length > 0 && (
            <div className="picker-actions">
              <Button variant="secondary" onClick={() => setChecked(new Set(rows.map((r) => r.kol)))}>
                {t("picker.select_all")}
              </Button>
              {picked.length > 0 ? (
                <Button variant="secondary" onClick={() => setChecked(new Set())}>
                  {t("picker.select_none")}
                </Button>
              ) : (
                <Button variant="secondary" disabled reason={t("picker.select_none.reason")}>
                  {t("picker.select_none")}
                </Button>
              )}
            </div>
          )}
          {rows.length === 0 ? (
            <EmptyState message={list.data.total === 0 ? t("picker.empty.network") : t("picker.empty.filters")} />
          ) : (
            <List label={t("picker.list")} variant="cards">
              {rows.map((r) => (
                <ListItem key={r.kol} current={checked.has(r.kol)}>
                  <KolRow row={r} checked={checked.has(r.kol)} onToggle={() => toggle(r.kol)} />
                </ListItem>
              ))}
            </List>
          )}
        </>
      )}
    </Container>
  );
}
