import { useState } from "react";
import type { FormEvent } from "react";
import { Badge, Button, Container, Dialog, EmptyState, ErrorLine, Field, Heading, List, ListItem, Select, SnapshotDate, Skeleton } from "@arena/design";
import { api, t, useApi, useSession } from "@arena/shell";
import "./auth.css";

interface StaffRow {
  id: number;
  login: string;
  role: "staff" | "admin";
  created_at: string;
  revoked_at: string | null;
}

function AddDialog({ open, onClose, onDone }: { open: boolean; onClose: () => void; onDone: () => void }) {
  const [login, setLogin] = useState("");
  const [role, setRole] = useState("staff");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);

  function close() {
    setLogin("");
    setRole("staff");
    setPassword("");
    setProblem(null);
    onClose();
  }

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (busy) return;
    if (login.trim() === "" || password === "") {
      setProblem(t("auth.staff.fill_all"));
      return;
    }
    setBusy(true);
    setProblem(null);
    const r = await api("/api/admin/staff", { method: "POST", body: { login: login.trim(), role, password } });
    setBusy(false);
    if (r.error) {
      setProblem(r.error.message);
      return;
    }
    close();
    onDone();
  }

  return (
    <Dialog open={open} onClose={close} title={t("auth.staff.add_title")} closeLabel={t("auth.staff.close")}>
      <form className="auth-form" onSubmit={(e) => void submit(e)} noValidate>
        <Field label={t("auth.staff.field.login")} hint={t("auth.staff.field.login_hint")} name="login" autoComplete="off" autoCapitalize="none" value={login} onChange={(e) => setLogin(e.target.value)} />
        <Select
          label={t("auth.staff.field.role")}
          name="role"
          value={role}
          onChange={(e) => setRole(e.target.value)}
          options={[
            { value: "staff", label: t("auth.staff.role.staff") },
            { value: "admin", label: t("auth.staff.role.admin") },
          ]}
        />
        <Field label={t("auth.staff.field.password")} hint={t("auth.staff.field.password_hint")} name="password" type="text" autoComplete="off" value={password} onChange={(e) => setPassword(e.target.value)} />
        {problem && <ErrorLine message={problem} />}
        <div className="auth-actions">
          {busy ? (
            <Button type="submit" disabled reason={t("auth.staff.creating")}>
              {t("auth.staff.create")}
            </Button>
          ) : (
            <Button type="submit">{t("auth.staff.create")}</Button>
          )}
          <Button variant="secondary" onClick={close}>
            {t("auth.staff.cancel")}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

function RevokeDialog({ row, onClose, onDone }: { row: StaffRow | null; onClose: () => void; onDone: () => void }) {
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);

  function close() {
    setProblem(null);
    onClose();
  }

  async function revoke() {
    if (!row || busy) return;
    setBusy(true);
    setProblem(null);
    const r = await api(`/api/admin/staff/${row.id}/revoke`, { method: "POST" });
    setBusy(false);
    if (r.error) {
      setProblem(r.error.message);
      return;
    }
    close();
    onDone();
  }

  return (
    <Dialog open={row !== null} onClose={close} title={t("auth.staff.revoke_title")} closeLabel={t("auth.staff.close")}>
      <div className="auth-form">
        {row && <p>{t("auth.staff.revoke_confirm", { login: row.login })}</p>}
        {problem && <ErrorLine message={problem} />}
        <div className="auth-actions">
          {busy ? (
            <Button disabled reason={t("auth.staff.revoking")}>
              {t("auth.staff.revoke_yes")}
            </Button>
          ) : (
            <Button onClick={() => void revoke()}>{t("auth.staff.revoke_yes")}</Button>
          )}
          <Button variant="secondary" onClick={close}>
            {t("auth.staff.cancel")}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}

export default function Staff() {
  const list = useApi<StaffRow[]>("/api/admin/staff");
  const { state } = useSession();
  const me = state.status === "ready" ? state.person.login : null;
  const [adding, setAdding] = useState(false);
  const [revoking, setRevoking] = useState<StaffRow | null>(null);

  let body;
  if (list.status === "loading") {
    body = <Skeleton label={t("auth.staff.loading")} variant="rows" />;
  } else if (list.status === "error") {
    body = <ErrorLine message={list.error.message} onRetry={list.reload} retryLabel={t("auth.staff.retry")} />;
  } else if (list.data.length === 0) {
    body = <EmptyState message={t("auth.staff.empty")} />;
  } else {
    body = (
      <List label={t("auth.staff.list")}>
        {list.data.map((row) => {
          const revoked = row.revoked_at !== null;
          const self = row.login === me;
          return (
            <ListItem key={row.id}>
              <div className="auth-row">
                <div className="auth-row__who">
                  <strong>{row.login}</strong>
                  <Badge tone={row.role === "admin" ? "coin" : "neutral"}>{row.role === "admin" ? t("auth.staff.role.admin") : t("auth.staff.role.staff")}</Badge>
                  {revoked ? <Badge tone="minus">{t("auth.staff.revoked")}</Badge> : <Badge tone="plus">{t("auth.staff.active")}</Badge>}
                  {self && <Badge tone="blue">{t("auth.staff.you")}</Badge>}
                  <span className="auth-row__note">
                    {t("auth.staff.since")} <SnapshotDate at={row.created_at} />
                  </span>
                </div>
                {!revoked &&
                  (self ? (
                    <Button variant="secondary" disabled reason={t("auth.staff.revoke_self")}>
                      {t("auth.staff.revoke")}
                    </Button>
                  ) : (
                    <Button variant="secondary" onClick={() => setRevoking(row)}>
                      {t("auth.staff.revoke")}
                    </Button>
                  ))}
              </div>
            </ListItem>
          );
        })}
      </List>
    );
  }

  return (
    <Container>
      <div className="auth-page auth-page--wide">
        <div className="auth-bar">
          <Heading level={1} variant="section">
            {t("auth.staff.title")}
          </Heading>
          <Button onClick={() => setAdding(true)}>{t("auth.staff.add")}</Button>
        </div>
        {body}
      </div>
      <AddDialog open={adding} onClose={() => setAdding(false)} onDone={list.reload} />
      <RevokeDialog row={revoking} onClose={() => setRevoking(null)} onDone={list.reload} />
    </Container>
  );
}
