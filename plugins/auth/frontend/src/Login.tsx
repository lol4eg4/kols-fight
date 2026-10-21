import { useState } from "react";
import type { FormEvent } from "react";
import { Button, Container, ErrorLine, Field, Heading, Skeleton } from "@arena/design";
import { api, Link, navigate, safeNext, t, useSearchParams, useSession } from "@arena/shell";
import "./auth.css";

const DEFAULT_LOCK_MINUTES = 15;

/** The server writes the wait in its message: "Try again in 12 minutes." */
function lockMinutes(message: string): number {
  const m = /(\d+)\s+minute/.exec(message);
  return m ? Number(m[1]) : DEFAULT_LOCK_MINUTES;
}

export default function Login() {
  const { state, reload, signOut } = useSession();
  const params = useSearchParams();
  const next = safeNext(params.get("next"));
  const [login, setLogin] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (busy) return;
    if (login.trim() === "" || password === "") {
      setProblem(t("auth.login.missing"));
      return;
    }
    setBusy(true);
    setProblem(null);
    const r = await api("/api/auth/login", { method: "POST", body: { login: login.trim(), password } });
    setBusy(false);
    if (r.error) {
      if (r.error.code === "locked") setProblem(t("auth.login.locked", { minutes: lockMinutes(r.error.message) }));
      else if (r.error.code === "bad_credentials") setProblem(t("auth.login.wrong"));
      else setProblem(r.error.message);
      setPassword("");
      return;
    }
    setPassword("");
    reload();
    navigate(next);
  }

  let body;
  if (state.status === "idle" || state.status === "loading") {
    body = <Skeleton label={t("auth.login.checking")} count={2} />;
  } else if (state.status === "ready") {
    body = (
      <div className="auth-form">
        <p>{t("auth.login.signed_in", { login: state.person.login })}</p>
        <div className="auth-actions">
          <Link to={next}>{t("auth.login.continue")}</Link>
          <Button variant="secondary" onClick={() => void signOut()}>
            {t("auth.login.sign_out")}
          </Button>
        </div>
      </div>
    );
  } else {
    body = (
      <form className="auth-form" onSubmit={(e) => void submit(e)} noValidate>
        {state.status === "error" && <ErrorLine message={t("auth.login.error")} onRetry={reload} retryLabel={t("auth.login.retry")} />}
        <Field label={t("auth.login.login")} name="login" autoComplete="username" autoCapitalize="none" value={login} onChange={(e) => setLogin(e.target.value)} />
        <Field label={t("auth.login.password")} name="password" type="password" autoComplete="current-password" value={password} onChange={(e) => setPassword(e.target.value)} />
        {problem && <ErrorLine message={problem} />}
        <div className="auth-actions">
          {busy ? (
            <Button type="submit" block disabled reason={t("auth.login.submitting")}>
              {t("auth.login.submit")}
            </Button>
          ) : (
            <Button type="submit" block>
              {t("auth.login.submit")}
            </Button>
          )}
        </div>
      </form>
    );
  }

  return (
    <Container>
      <div className="auth-page">
        <Heading level={1} variant="section">
          {t("auth.login.title")}
        </Heading>
        {body}
      </div>
    </Container>
  );
}
