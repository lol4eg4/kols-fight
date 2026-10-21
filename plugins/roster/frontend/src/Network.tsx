import { useCallback, useEffect, useRef, useState } from "react";
import type { FormEvent, ReactNode } from "react";
import { Badge, Button, Card, Container, Dialog, EmptyState, ErrorLine, Field, Heading, List, ListItem, Money, NoData, Select, Skeleton, TimeAgo, buttonClass } from "@arena/design";
import { Slot, api, t, useSession } from "@arena/shell";
import type { ApiError } from "@arena/shell";
import { FORMATS, buildPatch, emptyForm, filterMembers, groupsOf } from "./members";
import type { AdminMember, EditForm, FormProblem, SyncRun, TeamNetwork } from "./members";
import { SyncSummary } from "./sync";
import "./roster.css";

type Load = { status: "loading" } | { status: "ready"; data: TeamNetwork } | { status: "error"; error: ApiError };
type Notice = { ok: boolean; text: string } | null;

const EXPORT_PATH = "/api/admin/network/export";

function formatText(f: string): string {
  switch (f) {
    case "post":
      return t("roster.format.post");
    case "buy":
      return t("roster.format.buy");
    case "thread":
      return t("roster.format.thread");
    case "stream":
      return t("roster.format.stream");
    case "space":
      return t("roster.format.space");
    case "call":
      return t("roster.format.call");
    default:
      return f;
  }
}

function problemText(p: FormProblem): string {
  switch (p) {
    case "handle":
      return t("roster.edit.bad_handle");
    case "reach_number":
      return t("roster.edit.bad_number");
    case "reach_source":
      return t("roster.edit.need_source");
    case "reach_date":
      return t("roster.edit.need_date");
    case "nothing":
      return t("roster.edit.nothing");
  }
}

function today(): string {
  return new Date().toISOString().slice(0, 10);
}

function Detail({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="ros-detail">
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}

function XHandle({ m }: { m: AdminMember }) {
  if (!m.x_handle) return <NoData reason={t("roster.member.no_handle")} />;
  return (
    <span className="ros-inline">
      <span className="t-mono">{t("roster.member.handle", { handle: m.x_handle })}</span>
      {m.x_verified ? (
        <Badge tone="plus" icon="✓">
          {t("roster.member.verified")}
        </Badge>
      ) : (
        <Badge tone="muted" hint={t("roster.member.unverified_hint")}>
          {t("roster.member.unverified")}
        </Badge>
      )}
    </span>
  );
}

function MemberCard({ m, onEdit }: { m: AdminMember; onEdit: (m: AdminMember) => void }) {
  return (
    <ListItem>
      <div className="ros-member">
        <div className="ros-member__head">
          <Heading level={2} variant="title">
            {m.name}
          </Heading>
          <span className="ros-muted t-mono">{m.kol}</span>
          {m.group && (
            <Badge tone="lavender" shape="pill">
              {t("roster.member.group", { group: m.group })}
            </Badge>
          )}
          {m.keep && (
            <Badge tone="muted" hint={t("roster.member.keep_hint")}>
              {t("roster.member.keep")}
            </Badge>
          )}
          <Slot name="roster/wallet_status" props={{ kol: m.kol }} loading={null} />
        </div>
        <dl className="ros-details">
          <Detail label={t("roster.member.rate")}>
            <span className="ros-inline">
              <Money value={m.low_usd} signed={false} reason={t("roster.member.no_rate")} />
              <span aria-hidden="true">–</span>
              <Money value={m.high_usd} signed={false} reason={t("roster.member.no_rate")} />
            </span>
          </Detail>
          <Detail label={t("roster.member.x")}>
            <XHandle m={m} />
          </Detail>
          <Detail label={t("roster.member.fomo")}>{m.fomo_handle ? <span className="t-mono">{m.fomo_handle}</span> : <NoData reason={t("roster.member.no_fomo")} />}</Detail>
          <Detail label={t("roster.member.pumpfun")}>{m.pumpfun ? <span className="t-mono ros-wrap">{m.pumpfun}</span> : <NoData reason={t("roster.member.no_pumpfun")} />}</Detail>
          <Detail label={t("roster.member.formats")}>
            {m.formats.length === 0 ? (
              <NoData reason={t("roster.member.no_formats")} />
            ) : (
              <span className="ros-inline">
                {m.formats.map((f) => (
                  <Badge key={f} tone="neutral">
                    {formatText(f)}
                  </Badge>
                ))}
              </span>
            )}
          </Detail>
          <Detail label={t("roster.member.sheet_wallets")}>{t("roster.member.sheet_wallets_count", { count: m.sheet_wallets.length })}</Detail>
          <Detail label={t("roster.member.joined")}>
            <TimeAgo at={m.joined_at} />
          </Detail>
          <Detail label={t("roster.member.seen")}>
            <TimeAgo at={m.last_seen_at} reason={t("roster.member.never_seen")} />
          </Detail>
        </dl>
        {m.note && (
          <p className="ros-note">
            <strong>{t("roster.member.note")}</strong> {m.note}
          </p>
        )}
        {m.wallet_note && (
          <p className="ros-note">
            <strong>{t("roster.member.wallet_note")}</strong> {m.wallet_note}
          </p>
        )}
        <div className="ros-actions">
          <Button variant="secondary" onClick={() => onEdit(m)}>
            {t("roster.edit.button")}
          </Button>
        </div>
      </div>
    </ListItem>
  );
}

function EditDialog({ m, onClose, onSaved }: { m: AdminMember; onClose: () => void; onSaved: (n: Notice) => void }) {
  const [form, setForm] = useState<EditForm>(() => emptyForm(m, today()));
  const [problems, setProblems] = useState<FormProblem[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const set = (patch: Partial<EditForm>) => setForm((f) => ({ ...f, ...patch }));
  const has = (p: FormProblem) => problems.includes(p);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (busy) return;
    const built = buildPatch(m, form);
    setProblems(built.problems);
    if (built.problems.length > 0) return;
    setBusy(true);
    setError(null);
    const r = await api<{ member: AdminMember; warning: string | null }>(`/api/team/network/${encodeURIComponent(m.kol)}`, { method: "PATCH", body: built.patch });
    setBusy(false);
    if (r.error) {
      setError(t("roster.error.save", { message: r.error.message }));
      return;
    }
    onSaved(r.data.warning ? { ok: false, text: t("roster.edit.saved_warning", { name: m.name, warning: r.data.warning }) } : { ok: true, text: t("roster.edit.saved", { name: m.name }) });
  };

  return (
    <Dialog open onClose={onClose} title={t("roster.edit.title", { name: m.name })} closeLabel={t("roster.dialog.close")}>
      <form className="ros-form" onSubmit={submit}>
        <Field
          label={t("roster.edit.handle")}
          hint={m.x_verified ? t("roster.edit.handle_hint_verified") : m.x_handle ? t("roster.edit.handle_hint_fomo", { handle: m.x_handle }) : t("roster.edit.handle_hint")}
          value={form.handle}
          onChange={(e) => set({ handle: e.target.value })}
          error={has("handle") ? problemText("handle") : undefined}
          autoComplete="off"
          spellCheck={false}
        />
        <fieldset className="ros-fieldset">
          <legend>{t("roster.edit.formats")}</legend>
          <div className="ros-checks">
            {FORMATS.map((f) => (
              <label key={f} className="ros-check">
                <input
                  type="checkbox"
                  checked={form.formats.includes(f)}
                  onChange={(e) => set({ formats: e.target.checked ? [...form.formats, f] : form.formats.filter((x) => x !== f) })}
                />{" "}
                {formatText(f)}
              </label>
            ))}
          </div>
        </fieldset>
        <fieldset className="ros-fieldset">
          <legend>{t("roster.edit.reach")}</legend>
          <p className="ros-muted">{t("roster.edit.reach_hint")}</p>
          <div className="ros-grid">
            <Field label={t("roster.edit.x_followers")} inputMode="numeric" value={form.x_followers} onChange={(e) => set({ x_followers: e.target.value })} />
            <Field label={t("roster.edit.telegram")} inputMode="numeric" value={form.telegram_subscribers} onChange={(e) => set({ telegram_subscribers: e.target.value })} />
            <Field label={t("roster.edit.pumpfun")} inputMode="numeric" value={form.pumpfun_followers} onChange={(e) => set({ pumpfun_followers: e.target.value })} />
          </div>
          {has("reach_number") && <p className="ds-error__message">{problemText("reach_number")}</p>}
          <div className="ros-grid">
            <Field label={t("roster.edit.source")} hint={t("roster.edit.source_hint")} value={form.source} onChange={(e) => set({ source: e.target.value })} error={has("reach_source") ? problemText("reach_source") : undefined} />
            <Field label={t("roster.edit.date")} type="date" value={form.date} onChange={(e) => set({ date: e.target.value })} error={has("reach_date") ? problemText("reach_date") : undefined} />
          </div>
        </fieldset>
        {has("nothing") && <p role="status">{problemText("nothing")}</p>}
        {error && <ErrorLine message={error} />}
        <div className="ros-actions">
          {busy ? (
            <Button disabled reason={t("roster.edit.busy")}>
              {t("roster.edit.save")}
            </Button>
          ) : (
            <Button type="submit">{t("roster.edit.save")}</Button>
          )}
          <Button variant="secondary" onClick={onClose}>
            {t("roster.edit.cancel")}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

/** Admin: read the sheet now, download the export. */
function AdminTools({ onRead }: { onRead: (n: Notice) => void }) {
  const [busy, setBusy] = useState(false);
  const read = async () => {
    setBusy(true);
    const r = await api<SyncRun>("/api/admin/network/sync", { method: "POST" });
    setBusy(false);
    if (r.error) onRead({ ok: false, text: t("roster.sync.not_run", { message: r.error.message }) });
    else if (r.data.status !== "ok") onRead({ ok: false, text: t("roster.sync.run_failed", { message: r.data.error ?? "" }) });
    else onRead({ ok: true, text: t("roster.sync.run_ok", { rows: r.data.rows, added: r.data.added, updated: r.data.updated, removed: r.data.removed }) });
  };
  return (
    <div className="ros-actions">
      {busy ? (
        <Button disabled reason={t("roster.sync.busy")}>
          {t("roster.sync.button")}
        </Button>
      ) : (
        <Button onClick={() => void read()}>{t("roster.sync.button")}</Button>
      )}
      <a className={buttonClass("secondary")} href={EXPORT_PATH} download>
        {t("roster.export")}
      </a>
    </div>
  );
}

export default function Network() {
  const session = useSession();
  const isAdmin = session.state.status === "ready" && session.state.person.role === "admin";
  const [load, setLoad] = useState<Load>({ status: "loading" });
  const [group, setGroup] = useState("");
  const [query, setQuery] = useState("");
  const [notice, setNotice] = useState<Notice>(null);
  const [editing, setEditing] = useState<AdminMember | null>(null);
  const alive = useRef(true);

  const fetchNow = useCallback(async (quiet: boolean) => {
    if (!quiet) setLoad({ status: "loading" });
    const r = await api<TeamNetwork>("/api/team/network");
    if (!alive.current) return;
    setLoad(r.error ? { status: "error", error: r.error } : { status: "ready", data: r.data });
  }, []);

  useEffect(() => {
    alive.current = true;
    void fetchNow(false);
    return () => {
      alive.current = false;
    };
  }, [fetchNow]);

  const after = (n: Notice) => {
    setNotice(n);
    setEditing(null);
    void fetchNow(true);
  };

  const data = load.status === "ready" ? load.data : null;
  const groups = data ? groupsOf(data.members) : [];
  const shown = data ? filterMembers(data.members, group, query) : [];

  return (
    <Container>
      <h1 className="t-section-title">{t("roster.title")}</h1>
      <p className="ros-muted">{t("roster.intro")}</p>
      {load.status === "loading" && <Skeleton label={t("roster.loading")} variant="rows" />}
      {load.status === "error" && <ErrorLine message={t("roster.error.load", { message: load.error.message })} onRetry={() => void fetchNow(false)} retryLabel={t("roster.retry")} />}
      {data && (
        <>
          <Card>
            <Heading level={2} variant="title">
              {t("roster.sync.title")}
            </Heading>
            <SyncSummary run={data.last_sync} details />
            {isAdmin ? <AdminTools onRead={after} /> : <p className="ros-muted">{t("roster.sync.admin_only")}</p>}
          </Card>
          {notice && (notice.ok ? <p role="status">{notice.text}</p> : <ErrorLine message={notice.text} />)}
          <Card tone="sunken">
            <div className="ros-filters">
              <Select
                label={t("roster.filter.group")}
                options={[{ value: "", label: t("roster.filter.all_groups") }, ...groups.map((g) => ({ value: g, label: g }))]}
                value={group}
                onChange={(e) => setGroup(e.target.value)}
              />
              <Field label={t("roster.filter.search")} type="search" value={query} onChange={(e) => setQuery(e.target.value)} />
            </div>
          </Card>
          <p role="status">{t("roster.count", { shown: shown.length, total: data.members.length })}</p>
          {data.members.length === 0 ? (
            <EmptyState message={t("roster.empty")} />
          ) : shown.length === 0 ? (
            <EmptyState message={t("roster.empty_filter")} />
          ) : (
            <List label={t("roster.list")} variant="cards">
              {shown.map((m) => (
                <MemberCard key={m.kol} m={m} onEdit={setEditing} />
              ))}
            </List>
          )}
          {editing && <EditDialog key={editing.kol} m={editing} onClose={() => setEditing(null)} onSaved={after} />}
        </>
      )}
    </Container>
  );
}
