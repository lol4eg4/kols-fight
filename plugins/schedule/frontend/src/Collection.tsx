import { useCallback, useEffect, useRef, useState } from "react";
import { Badge, Button, Card, Container, EmptyState, ErrorLine, Field, Heading, List, ListItem, Skeleton, SnapshotDate, TimeAgo } from "@arena/design";
import type { BadgeTone } from "@arena/design";
import { Slot, api, t, useSession } from "@arena/shell";
import type { ApiError } from "@arena/shell";

interface StepView {
  step: string;
  status: string;
  started_at: string;
  finished_at: string;
  rows: number | null;
  error: string | null;
  note: string | null;
}

interface RunView {
  id: number;
  date: string;
  trigger: string;
  person_id: number | null;
  started_at: string;
  finished_at: string | null;
  status: string;
  error: string | null;
  steps: StepView[];
}

interface SourceRunView {
  run_id: number;
  date: string;
  status: string;
  at: string;
  rows: number | null;
  error: string | null;
  note: string | null;
}

interface SourceView {
  source: string;
  last_ok: SourceRunView | null;
  last_error: SourceRunView | null;
}

interface CollectionView {
  today: string;
  running: boolean;
  sources: SourceView[];
  runs: RunView[];
}

type Load = { status: "loading" } | { status: "ready"; data: CollectionView } | { status: "error"; error: ApiError };

const POLL_MS = 5000;

const STATUS_TONES: Record<string, BadgeTone> = { running: "blue", ok: "plus", partial: "coin", failed: "minus", interrupted: "minus" };

// t() keys must be plain literals, so each lookup is a switch
function statusText(status: string): string {
  switch (status) {
    case "running":
      return t("schedule.status.running");
    case "ok":
      return t("schedule.status.ok");
    case "partial":
      return t("schedule.status.partial");
    case "failed":
      return t("schedule.status.failed");
    case "interrupted":
      return t("schedule.status.interrupted");
    default:
      return status;
  }
}

function stepText(step: string): string {
  switch (step) {
    case "open":
      return t("schedule.step.open");
    case "event":
      return t("schedule.step.event");
    case "kolexplorer":
      return t("schedule.step.kolexplorer");
    case "solanatracker":
      return t("schedule.step.solanatracker");
    case "close":
      return t("schedule.step.close");
    case "rank":
      return t("schedule.step.rank");
    default:
      return step;
  }
}

function sourceText(source: string): string {
  if (source === "kolexplorer") return t("schedule.sources.kolexplorer");
  if (source === "solanatracker") return t("schedule.sources.solanatracker");
  return source;
}

function triggerText(trigger: string): string {
  switch (trigger) {
    case "scheduled":
      return t("schedule.trigger.scheduled");
    case "manual":
      return t("schedule.trigger.manual");
    case "recovery":
      return t("schedule.trigger.recovery");
    default:
      return trigger;
  }
}

function StatusBadge({ status }: { status: string }) {
  return <Badge tone={STATUS_TONES[status] ?? "neutral"}>{statusText(status)}</Badge>;
}

function SourceRun({ label, run }: { label: string; run: SourceRunView | null }) {
  return (
    <p>
      <strong>{label}</strong>{" "}
      {run === null ? (
        t("schedule.sources.never")
      ) : (
        <>
          <SnapshotDate at={run.date} short /> <TimeAgo at={run.at} />
          {run.rows !== null && <> {t("schedule.sources.rows", { rows: run.rows })}</>}
          {run.note && <> {run.note}</>}
          {run.error && <span className="ds-error__message"> {run.error}</span>}
        </>
      )}
    </p>
  );
}

function RunRow({ run }: { run: RunView }) {
  return (
    <ListItem>
      <div>
        <p>
          <StatusBadge status={run.status} /> <Badge tone="muted">{triggerText(run.trigger)}</Badge>{" "}
          {t("schedule.runs.date", { date: run.date })} <TimeAgo at={run.started_at} />
          {run.person_id !== null && <> {t("schedule.runs.by", { id: run.person_id })}</>}
        </p>
        {run.error && <p className="ds-error__message">{run.error}</p>}
        <ul>
          {run.steps.map((s) => (
            <li key={s.step}>
              <StatusBadge status={s.status} /> {stepText(s.step)}
              {s.rows !== null && <> {t("schedule.runs.rows", { rows: s.rows })}</>}
              {s.note && <> {s.note}</>}
              {s.error && <span className="ds-error__message"> {s.error}</span>}
            </li>
          ))}
        </ul>
      </div>
    </ListItem>
  );
}

function RunNow({ running, onStarted }: { running: boolean; onStarted: () => void }) {
  const [date, setDate] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  const start = async () => {
    setBusy(true);
    setMessage(null);
    const path = date ? `/api/admin/collection/run?date=${encodeURIComponent(date)}` : "/api/admin/collection/run";
    const r = await api<RunView>(path, { method: "POST" });
    setBusy(false);
    if (r.error) setMessage({ ok: false, text: t("schedule.run.failed", { message: r.error.message }) });
    else setMessage({ ok: true, text: t("schedule.run.started", { id: r.data.id }) });
    onStarted();
  };

  return (
    <Card>
      <Heading level={2} variant="title">
        {t("schedule.run.title")}
      </Heading>
      <Field label={t("schedule.run.date")} hint={t("schedule.run.hint")} type="date" value={date} onChange={(e) => setDate(e.target.value)} />
      {running || busy ? (
        <Button disabled reason={busy ? t("schedule.run.busy") : t("schedule.run.disabled_running")}>
          {t("schedule.run.button")}
        </Button>
      ) : (
        <Button onClick={start}>{t("schedule.run.button")}</Button>
      )}
      {message && (message.ok ? <p role="status">{message.text}</p> : <ErrorLine message={message.text} />)}
    </Card>
  );
}

export default function Collection() {
  const session = useSession();
  const isAdmin = session.state.status === "ready" && session.state.person.role === "admin";
  const [load, setLoad] = useState<Load>({ status: "loading" });
  const alive = useRef(true);

  const fetchNow = useCallback(async (quiet: boolean) => {
    if (!quiet) setLoad({ status: "loading" });
    const r = await api<CollectionView>("/api/admin/collection");
    if (!alive.current) return;
    if (r.error) {
      // a failed background poll keeps the last good screen
      if (!quiet) setLoad({ status: "error", error: r.error });
    } else setLoad({ status: "ready", data: r.data });
  }, []);

  useEffect(() => {
    alive.current = true;
    void fetchNow(false);
    return () => {
      alive.current = false;
    };
  }, [fetchNow]);

  const running = load.status === "ready" && load.data.running;
  useEffect(() => {
    if (!running) return;
    const id = setInterval(() => void fetchNow(true), POLL_MS);
    return () => clearInterval(id);
  }, [running, fetchNow]);

  return (
    <Container>
      <h1 className="t-section-title">{t("schedule.title")}</h1>
      {load.status === "loading" && <Skeleton label={t("schedule.loading")} variant="rows" />}
      {load.status === "error" && (
        <ErrorLine message={t("schedule.error.load", { message: load.error.message })} onRetry={() => void fetchNow(false)} retryLabel={t("schedule.retry")} />
      )}
      {load.status === "ready" && (
        <>
          <p role="status">
            {load.data.running ? t("schedule.running") : t("schedule.idle")} {t("schedule.today", { date: load.data.today })}
          </p>
          {isAdmin && <RunNow running={load.data.running} onStarted={() => void fetchNow(true)} />}
          <Heading level={2} variant="title">
            {t("schedule.sources.title")}
          </Heading>
          {load.data.sources.map((s) => (
            <Card key={s.source}>
              <Heading level={3} variant="title">
                {sourceText(s.source)}
              </Heading>
              <SourceRun label={t("schedule.sources.last_ok")} run={s.last_ok} />
              <SourceRun label={t("schedule.sources.last_error")} run={s.last_error} />
            </Card>
          ))}
          <Slot name="schedule/other_sources" props={{}} loading={null} />
          <Heading level={2} variant="title">
            {t("schedule.runs.title")}
          </Heading>
          {load.data.runs.length === 0 ? (
            <EmptyState message={t("schedule.runs.empty")} />
          ) : (
            <List label={t("schedule.runs.label")}>
              {load.data.runs.map((r) => (
                <RunRow key={r.id} run={r} />
              ))}
            </List>
          )}
        </>
      )}
    </Container>
  );
}
