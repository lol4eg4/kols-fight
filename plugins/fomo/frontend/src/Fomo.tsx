import { useState } from "react";
import { Badge, Button, Card, Container, ErrorLine, Field, Heading, List, ListItem, Select, Skeleton } from "@arena/design";
import type { BadgeTone } from "@arena/design";
import { Link, api, t, useApi, useSearchParams } from "@arena/shell";
import "./fomo.css";

type Outcome = "not_configured" | "node_down" | "too_many_holders" | "no_match" | "several" | "hypothesis" | "known" | "busy" | "not_saved";

interface PositionResult {
  mint: string;
  amount: string;
  decimals: number | null;
  holders: number | null;
  match_count: number;
  matches: string[];
  error: Outcome | null;
}

interface WalletHit {
  address: string;
  positions: number[];
}

interface CandidateView {
  id: number;
  address: string;
  status: string;
  source: string;
}

interface MatchResult {
  kol: string;
  outcome: Outcome;
  detail: string | null;
  positions: PositionResult[];
  wallets: WalletHit[];
  wallet_count: number;
  candidate: CandidateView | null;
}

interface KolOption {
  kol: string;
  name: string;
}

interface WalletsOverview {
  kols: KolOption[];
}

interface Row {
  mint: string;
  amount: string;
}

const MIN_ROWS = 2;
const MAX_ROWS = 3;
const BASE58 = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;

const OUTCOME_TONES: Record<Outcome, BadgeTone> = {
  not_configured: "minus",
  node_down: "minus",
  too_many_holders: "coin",
  no_match: "muted",
  several: "coin",
  hypothesis: "plus",
  known: "blue",
  busy: "coin",
  not_saved: "minus",
};

// t() keys must be plain literals, so each lookup is a switch
function outcomeWord(o: Outcome): string {
  switch (o) {
    case "not_configured":
      return t("fomo.outcome.not_configured");
    case "node_down":
      return t("fomo.outcome.node_down");
    case "too_many_holders":
      return t("fomo.outcome.too_many_holders");
    case "no_match":
      return t("fomo.outcome.no_match");
    case "several":
      return t("fomo.outcome.several");
    case "hypothesis":
      return t("fomo.outcome.hypothesis");
    case "known":
      return t("fomo.outcome.known");
    case "busy":
      return t("fomo.outcome.busy");
    case "not_saved":
      return t("fomo.outcome.not_saved");
  }
}

function statusWord(status: string): string {
  switch (status) {
    case "hypothesis":
      return t("fomo.status.hypothesis");
    case "confirmed":
      return t("fomo.status.confirmed");
    case "not_found":
      return t("fomo.status.not_found");
    case "rejected":
      return t("fomo.status.rejected");
    case "revoked":
      return t("fomo.status.revoked");
    case "retired":
      return t("fomo.status.retired");
    case "conflict":
      return t("fomo.status.conflict");
    default:
      return status;
  }
}

/** One line: what happened and what to do next. */
function nextStep(r: MatchResult): string {
  switch (r.outcome) {
    case "not_configured":
      return t("fomo.next.not_configured");
    case "node_down":
      return t("fomo.next.node_down");
    case "too_many_holders":
      return t("fomo.next.too_many_holders");
    case "no_match":
      return t("fomo.next.no_match");
    case "several":
      return t("fomo.next.several", { count: r.wallet_count });
    case "hypothesis":
      return t("fomo.next.hypothesis");
    case "known":
      return t("fomo.next.known", { status: statusWord(r.candidate?.status ?? "") });
    case "busy":
      return t("fomo.next.busy");
    case "not_saved":
      return t("fomo.next.not_saved");
  }
}

function positionLine(p: PositionResult): string {
  if (p.error === "too_many_holders") return t("fomo.position.too_many");
  if (p.error) return t("fomo.position.not_read");
  if (p.holders === null) return t("fomo.position.skipped");
  return t("fomo.position.read", { holders: p.holders, matches: p.match_count });
}

function short(address: string): string {
  return address.length > 12 ? `${address.slice(0, 4)}…${address.slice(-4)}` : address;
}

function SolscanLink({ address }: { address: string }) {
  return (
    <a className="fomo-addr t-mono" href={`https://solscan.io/account/${address}`} target="_blank" rel="noopener noreferrer" aria-label={t("fomo.solscan_for", { address })}>
      {short(address)}
    </a>
  );
}

function Result({ result }: { result: MatchResult }) {
  const isError = result.outcome === "not_configured" || result.outcome === "node_down" || result.outcome === "not_saved";
  return (
    <Card aria-label={t("fomo.result.title")}>
      <Heading level={2} variant="title">
        {t("fomo.result.title")}
      </Heading>
      <p className="fomo-outcome">
        <Badge tone={OUTCOME_TONES[result.outcome]}>{outcomeWord(result.outcome)}</Badge>
      </p>
      {isError ? <ErrorLine message={nextStep(result)} /> : <p role="status">{nextStep(result)}</p>}
      {result.detail && <p className="t-small fomo-detail">{t("fomo.detail", { detail: result.detail })}</p>}
      {result.candidate && (
        <p className="fomo-candidate">
          <SolscanLink address={result.candidate.address} /> <Badge tone={result.candidate.status === "confirmed" ? "plus" : "coin"}>{statusWord(result.candidate.status)}</Badge>{" "}
          <Link to="/team/wallets">{t("fomo.open_wallets")}</Link>
        </p>
      )}
      <Heading level={3} variant="title">
        {t("fomo.positions.title")}
      </Heading>
      <List label={t("fomo.positions.title")}>
        {result.positions.map((p, i) => (
          <ListItem key={p.mint}>
            <div className="fomo-position">
              <span className="t-label">{t("fomo.position.n", { n: i + 1 })}</span>
              <SolscanLink address={p.mint} />
              <span className="t-mono">{p.amount}</span>
              <span>{positionLine(p)}</span>
            </div>
          </ListItem>
        ))}
      </List>
      {result.outcome === "several" && result.wallets.length > 0 && (
        <>
          <Heading level={3} variant="title">
            {t("fomo.wallets.title")}
          </Heading>
          <List label={t("fomo.wallets.title")}>
            {result.wallets.map((w) => (
              <ListItem key={w.address}>
                <div className="fomo-position">
                  <SolscanLink address={w.address} />
                  <span>{t("fomo.wallets.holds", { list: w.positions.map((n) => n + 1).join(", ") })}</span>
                </div>
              </ListItem>
            ))}
          </List>
        </>
      )}
    </Card>
  );
}

function rowProblem(rows: Row[]): string | null {
  const seen = new Set<string>();
  for (const [i, r] of rows.entries()) {
    const mint = r.mint.trim();
    const n = i + 1;
    if (!mint || !r.amount.trim()) return t("fomo.reason.fill", { n });
    if (!BASE58.test(mint)) return t("fomo.reason.mint", { n });
    if (seen.has(mint)) return t("fomo.reason.twice", { n });
    seen.add(mint);
  }
  return null;
}

export default function Fomo() {
  const params = useSearchParams();
  const kols = useApi<WalletsOverview>("/api/team/wallets");
  const [kol, setKol] = useState(params.get("kol") ?? "");
  const [rows, setRows] = useState<Row[]>(() => Array.from({ length: MIN_ROWS }, () => ({ mint: "", amount: "" })));
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<MatchResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const setRow = (i: number, patch: Partial<Row>) => setRows((rs) => rs.map((r, j) => (j === i ? { ...r, ...patch } : r)));

  const find = async () => {
    setRunning(true);
    setError(null);
    setResult(null);
    const r = await api<MatchResult>("/api/team/fomo/match", {
      method: "POST",
      body: { kol, positions: rows.map((x) => ({ mint: x.mint.trim(), amount: x.amount.trim() })) },
    });
    setRunning(false);
    if (r.error) setError(t("fomo.error.run", { message: r.error.message }));
    else setResult(r.data);
  };

  const problem = running ? t("fomo.reason.running") : !kol ? t("fomo.reason.kol") : rowProblem(rows);

  return (
    <Container>
      <h1 className="t-section-title">{t("fomo.title")}</h1>
      <p>{t("fomo.intro")}</p>
      {kols.status === "loading" && <Skeleton label={t("fomo.loading")} variant="lines" count={2} />}
      {kols.status === "error" && <ErrorLine message={t("fomo.error.kols", { message: kols.error.message })} onRetry={kols.reload} retryLabel={t("fomo.retry")} />}
      {kols.status === "ready" && (
        <Card>
          <div className="fomo-form">
            <Select
              label={t("fomo.kol")}
              placeholder={t("fomo.kol.placeholder")}
              options={kols.data.kols.map((k) => ({ value: k.kol, label: k.name }))}
              value={kol}
              onChange={(e) => setKol(e.target.value)}
            />
            {rows.map((r, i) => (
              <fieldset key={i} className="fomo-row">
                <legend className="t-label">{t("fomo.position.n", { n: i + 1 })}</legend>
                <Field
                  label={t("fomo.mint")}
                  autoComplete="off"
                  spellCheck={false}
                  value={r.mint}
                  onChange={(e) => setRow(i, { mint: e.target.value })}
                />
                <Field label={t("fomo.amount")} hint={t("fomo.amount.hint")} inputMode="decimal" autoComplete="off" value={r.amount} onChange={(e) => setRow(i, { amount: e.target.value })} />
                {rows.length > MIN_ROWS && (
                  <Button variant="secondary" onClick={() => setRows((rs) => rs.filter((_, j) => j !== i))}>
                    {t("fomo.remove")}
                  </Button>
                )}
              </fieldset>
            ))}
            {rows.length < MAX_ROWS && (
              <Button variant="secondary" onClick={() => setRows((rs) => [...rs, { mint: "", amount: "" }])}>
                {t("fomo.add")}
              </Button>
            )}
            {problem ? (
              <Button disabled reason={problem}>
                {running ? t("fomo.running") : t("fomo.find")}
              </Button>
            ) : (
              <Button onClick={() => void find()}>{t("fomo.find")}</Button>
            )}
          </div>
        </Card>
      )}
      {running && <Skeleton label={t("fomo.running")} variant="rows" count={2} />}
      {error && <ErrorLine message={error} />}
      {result && <Result result={result} />}
    </Container>
  );
}
