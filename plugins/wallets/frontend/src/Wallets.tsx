import { useState } from "react";
import type { FormEvent } from "react";
import { Badge, Button, Card, Container, Dialog, EmptyState, ErrorLine, Field, Heading, List, ListItem, NoData, Select, Skeleton, StatTile, Tabs, TimeAgo } from "@arena/design";
import { api, t } from "@arena/shell";
import type { ApiResult } from "@arena/shell";
import { actionsFor, filled, filterKols, sortCandidates, sortKols } from "./overview";
import type { Action, CandidateView, KolStatus, KolView, Overview } from "./overview";
import { refreshOverview, useOverview } from "./useOverview";
import { CANDIDATE_TONES, KOL_ICONS, KOL_TONES, candidateStatusText, kolStatusHint, kolStatusText, sourceText } from "./text";
import "./wallets.css";

const STATUSES: KolStatus[] = ["conflict", "hypothesis", "not_found", "confirmed"];

type Notice = { ok: boolean; text: string } | null;

function actionText(a: Action): string {
  switch (a) {
    case "confirm":
      return t("wallets.action.confirm");
    case "reject":
      return t("wallets.action.reject");
    case "revoke":
      return t("wallets.action.revoke");
    case "main":
      return t("wallets.action.main");
  }
}

function KolStatusBadge({ status }: { status: KolStatus }) {
  return (
    <Badge tone={KOL_TONES[status]} icon={KOL_ICONS[status]} hint={kolStatusHint(status)}>
      {kolStatusText(status)}
    </Badge>
  );
}

function Address({ value }: { value: string }) {
  return <code className="wal-address t-mono">{value}</code>;
}

function Detail({ label, value }: { label: string; value: string | null }) {
  if (!value) return null;
  return (
    <div className="wal-detail">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function CandidateRow({ c, onAction }: { c: CandidateView; onAction: (a: Action, c: CandidateView) => void }) {
  const actions = actionsFor(c);
  return (
    <li className="wal-cand">
      <div className="wal-cand__head">
        <Address value={c.address} />
        <Badge tone={CANDIDATE_TONES[c.status]}>{candidateStatusText(c.status)}</Badge>
        {c.main && (
          <Badge tone="lavender" icon="★">
            {t("wallets.candidate.main")}
          </Badge>
        )}
        <Badge tone="neutral">{sourceText(c.source)}</Badge>
      </div>
      <dl className="wal-details">
        <Detail label={t("wallets.candidate.evidence")} value={c.evidence} />
        <Detail label={t("wallets.candidate.sent_by")} value={c.sent_by} />
        <Detail label={t("wallets.candidate.confirmation")} value={c.confirmation} />
        <Detail label={t("wallets.candidate.reason")} value={c.reason} />
        <div className="wal-detail">
          <dt>{t("wallets.candidate.last_trade")}</dt>
          <dd>
            <TimeAgo at={c.last_trade_at} reason={c.checked_at ? t("wallets.candidate.no_trades") : t("wallets.candidate.not_checked")} />
          </dd>
        </div>
        <div className="wal-detail">
          <dt>{t("wallets.candidate.checked")}</dt>
          <dd>
            <TimeAgo at={c.checked_at} reason={t("wallets.candidate.not_checked")} />
          </dd>
        </div>
        <div className="wal-detail">
          <dt>{t("wallets.candidate.added")}</dt>
          <dd>
            <TimeAgo at={c.created_at} />
          </dd>
        </div>
      </dl>
      {actions.length > 0 && (
        <div className="wal-actions">
          {actions.map((a) => (
            <Button key={a} variant="secondary" onClick={() => onAction(a, c)}>
              {actionText(a)}
            </Button>
          ))}
        </div>
      )}
    </li>
  );
}

function KolCard({ k, onAction }: { k: KolView; onAction: (a: Action, c: CandidateView) => void }) {
  const candidates = sortCandidates(k.candidates);
  return (
    <ListItem>
      <div className="wal-kol">
        <div className="wal-kol__head">
          <Heading level={2} variant="title">
            {k.name}
          </Heading>
          <span className="wal-muted t-mono">{k.kol}</span>
          <KolStatusBadge status={k.status} />
        </div>
        <dl className="wal-details">
          <div className="wal-detail">
            <dt>{t("wallets.kol.main")}</dt>
            <dd>{k.main_address ? <Address value={k.main_address} /> : <NoData reason={t("wallets.kol.no_main")} />}</dd>
          </div>
          <div className="wal-detail">
            <dt>{t("wallets.kol.last_trade")}</dt>
            <dd>
              <TimeAgo at={k.last_trade_at} reason={t("wallets.kol.no_last_trade")} />
            </dd>
          </div>
        </dl>
        {candidates.length === 0 ? (
          <p className="wal-muted">{t("wallets.kol.no_candidates")}</p>
        ) : (
          <ul className="wal-cands" aria-label={t("wallets.kol.candidates", { name: k.name })}>
            {candidates.map((c) => (
              <CandidateRow key={c.id} c={c} onAction={onAction} />
            ))}
          </ul>
        )}
      </div>
    </ListItem>
  );
}

/** Confirm, reject and revoke: one text and a submit. Make main needs no text. */
function ActionDialog({ action, candidate, kolName, onClose, onDone }: { action: Action; candidate: CandidateView; kolName: string; onClose: () => void; onDone: (n: Notice) => void }) {
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tried, setTried] = useState(false);
  const required = action === "confirm" || action === "revoke";
  const missing = required && !filled(text);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setTried(true);
    if (missing || busy) return;
    setBusy(true);
    setError(null);
    const path = `/api/team/wallets/${candidate.id}/${action}`;
    const body = action === "confirm" ? { where_when: text.trim() } : filled(text) ? { reason: text.trim() } : {};
    const r = await api<CandidateView>(path, { method: "POST", body });
    setBusy(false);
    if (r.error) {
      setError(t("wallets.error.action", { message: r.error.message }));
      return;
    }
    onDone({ ok: true, text: doneText(action, kolName, r.data) });
  };

  let title = "";
  let label = "";
  let hint = "";
  switch (action) {
    case "confirm":
      title = t("wallets.confirm.title", { name: kolName });
      label = t("wallets.confirm.where_when");
      hint = t("wallets.confirm.hint");
      break;
    case "reject":
      title = t("wallets.reject.title", { name: kolName });
      label = t("wallets.reject.reason");
      hint = t("wallets.reject.hint");
      break;
    case "revoke":
      title = t("wallets.revoke.title", { name: kolName });
      label = t("wallets.revoke.reason");
      hint = t("wallets.revoke.hint");
      break;
    case "main":
      title = t("wallets.main.title", { name: kolName });
      break;
  }

  return (
    <Dialog open onClose={onClose} title={title} closeLabel={t("wallets.dialog.close")}>
      <form className="wal-form" onSubmit={submit}>
        <p>
          <Address value={candidate.address} />
        </p>
        {action === "main" ? (
          <p>{t("wallets.main.text")}</p>
        ) : (
          <Field
            multiline
            label={label}
            hint={hint}
            value={text}
            onChange={(e) => setText(e.target.value)}
            error={tried && missing ? t("wallets.form.required") : undefined}
            required={required}
          />
        )}
        {error && <ErrorLine message={error} />}
        <div className="wal-actions">
          {busy ? (
            <Button disabled reason={t("wallets.form.busy")}>
              {actionText(action)}
            </Button>
          ) : (
            <Button type="submit">{actionText(action)}</Button>
          )}
          <Button variant="secondary" onClick={onClose}>
            {t("wallets.form.cancel")}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

function doneText(action: Action, name: string, c: CandidateView): string {
  switch (action) {
    case "confirm":
      return t("wallets.done.confirm", { name, status: candidateStatusText(c.status) });
    case "reject":
      return t("wallets.done.reject", { name });
    case "revoke":
      return t("wallets.done.revoke", { name });
    case "main":
      return t("wallets.done.main", { name });
  }
}

/** «Address from the KOL»: staff types what the KOL sent; the trades check decides. */
function ProposeDialog({ kols, onClose, onDone }: { kols: KolView[]; onClose: () => void; onDone: (n: Notice) => void }) {
  const [kol, setKol] = useState("");
  const [address, setAddress] = useState("");
  const [evidence, setEvidence] = useState("");
  const [tried, setTried] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const options = [...kols].sort((a, b) => a.name.localeCompare(b.name)).map((k) => ({ value: k.kol, label: k.name }));

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setTried(true);
    if (!kol || !filled(address) || !filled(evidence) || busy) return;
    setBusy(true);
    setError(null);
    const r = await api<CandidateView>("/api/team/wallets/propose", { method: "POST", body: { kol, address: address.trim(), evidence: evidence.trim() } });
    setBusy(false);
    if (r.error) {
      setError(t("wallets.error.action", { message: r.error.message }));
      return;
    }
    const name = kols.find((k) => k.kol === kol)?.name ?? kol;
    onDone({ ok: true, text: t("wallets.done.propose", { name, status: candidateStatusText(r.data.status) }) });
  };

  return (
    <Dialog open onClose={onClose} title={t("wallets.propose.title")} closeLabel={t("wallets.dialog.close")}>
      <form className="wal-form" onSubmit={submit}>
        <p>{t("wallets.propose.text")}</p>
        <Select
          label={t("wallets.propose.kol")}
          placeholder={t("wallets.propose.kol_placeholder")}
          options={options}
          value={kol}
          onChange={(e) => setKol(e.target.value)}
          error={tried && !kol ? t("wallets.form.required") : undefined}
          required
        />
        <Field
          label={t("wallets.propose.address")}
          value={address}
          onChange={(e) => setAddress(e.target.value)}
          error={tried && !filled(address) ? t("wallets.form.required") : undefined}
          autoComplete="off"
          spellCheck={false}
          required
        />
        <Field
          multiline
          label={t("wallets.propose.evidence")}
          hint={t("wallets.propose.evidence_hint")}
          value={evidence}
          onChange={(e) => setEvidence(e.target.value)}
          error={tried && !filled(evidence) ? t("wallets.form.required") : undefined}
          required
        />
        {error && <ErrorLine message={error} />}
        <div className="wal-actions">
          {busy ? (
            <Button disabled reason={t("wallets.propose.busy")}>
              {t("wallets.propose.submit")}
            </Button>
          ) : (
            <Button type="submit">{t("wallets.propose.submit")}</Button>
          )}
          <Button variant="secondary" onClick={onClose}>
            {t("wallets.form.cancel")}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

function RunPass({ onDone }: { onDone: (n: Notice) => void }) {
  const [busy, setBusy] = useState(false);
  const run = async () => {
    setBusy(true);
    const r: ApiResult<{ problems: string | null; overview: Overview }> = await api("/api/team/wallets/pass", { method: "POST" });
    setBusy(false);
    if (r.error) {
      onDone({ ok: false, text: t("wallets.error.pass", { message: r.error.message }) });
      return;
    }
    refreshOverview(r.data.overview);
    onDone(r.data.problems ? { ok: false, text: t("wallets.pass.problems", { message: r.data.problems }) } : { ok: true, text: t("wallets.pass.done") });
  };
  return busy ? (
    <Button variant="secondary" disabled reason={t("wallets.pass.busy")}>
      {t("wallets.pass.button")}
    </Button>
  ) : (
    <Button variant="secondary" onClick={() => void run()}>
      {t("wallets.pass.button")}
    </Button>
  );
}

export default function Wallets() {
  const { load, reload } = useOverview();
  const [filter, setFilter] = useState<KolStatus | "all">("all");
  const [query, setQuery] = useState("");
  const [notice, setNotice] = useState<Notice>(null);
  const [open, setOpen] = useState<{ action: Action; candidate: CandidateView } | null>(null);
  const [proposing, setProposing] = useState(false);

  const done = (n: Notice) => {
    setNotice(n);
    setOpen(null);
    setProposing(false);
    if (n?.ok) refreshOverview();
  };

  const onAction = (action: Action, candidate: CandidateView) => {
    setNotice(null);
    setOpen({ action, candidate });
  };

  const data = load.status === "ready" ? load.data : null;
  const counts = (s: KolStatus) => data?.counts[s] ?? 0;
  const shown = data ? sortKols(filterKols(data.kols, filter, query)) : [];
  const openName = open && data ? (data.kols.find((k) => k.kol === open.candidate.kol)?.name ?? open.candidate.kol) : "";

  return (
    <Container>
      <h1 className="t-section-title">{t("wallets.title")}</h1>
      <p className="wal-muted">{t("wallets.intro")}</p>
      {load.status === "loading" && <Skeleton label={t("wallets.loading")} variant="rows" />}
      {load.status === "error" && <ErrorLine message={t("wallets.error.load", { message: load.error.message })} onRetry={reload} retryLabel={t("wallets.retry")} />}
      {data && (
        <>
          <div className="wal-stats">
            <StatTile value={counts("confirmed")} label={t("wallets.count.confirmed")} accent />
            <StatTile value={counts("hypothesis")} label={t("wallets.count.hypothesis")} />
            <StatTile value={counts("not_found")} label={t("wallets.count.not_found")} />
            <StatTile value={counts("conflict")} label={t("wallets.count.conflict")} />
          </div>
          <div className="wal-toolbar">
            <Button
              onClick={() => {
                setNotice(null);
                setProposing(true);
              }}
            >
              {t("wallets.propose.button")}
            </Button>
            <RunPass onDone={setNotice} />
          </div>
          {notice && (notice.ok ? <p role="status">{notice.text}</p> : <ErrorLine message={notice.text} />)}
          <Card tone="sunken">
            <div className="wal-filters">
              <Tabs
                idBase="wal-filter"
                label={t("wallets.filter.label")}
                value={filter}
                onChange={(id) => setFilter(id as KolStatus | "all")}
                items={[{ id: "all", label: t("wallets.filter.all"), count: data.kols.length }, ...STATUSES.map((s) => ({ id: s, label: kolStatusText(s), count: counts(s) }))]}
              />
              <Field label={t("wallets.filter.search")} hideLabel placeholder={t("wallets.filter.search")} type="search" value={query} onChange={(e) => setQuery(e.target.value)} />
            </div>
          </Card>
          <div role="tabpanel" id={`wal-filter-panel-${filter}`} aria-labelledby={`wal-filter-tab-${filter}`}>
            {data.kols.length === 0 ? (
              <EmptyState message={t("wallets.empty")} />
            ) : shown.length === 0 ? (
              <EmptyState message={t("wallets.empty_filter")} />
            ) : (
              <List label={t("wallets.list")} variant="cards">
                {shown.map((k) => (
                  <KolCard key={k.kol} k={k} onAction={onAction} />
                ))}
              </List>
            )}
          </div>
          {open && <ActionDialog key={`${open.action}-${open.candidate.id}`} action={open.action} candidate={open.candidate} kolName={openName} onClose={() => setOpen(null)} onDone={done} />}
          {proposing && <ProposeDialog kols={data.kols} onClose={() => setProposing(false)} onDone={done} />}
        </>
      )}
    </Container>
  );
}
