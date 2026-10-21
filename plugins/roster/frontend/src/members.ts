// Types of GET /api/team/network and the pure logic of the Network screen.

export const FORMATS = ["post", "buy", "thread", "stream", "space", "call"] as const;
export type Format = (typeof FORMATS)[number];

export interface AdminMember {
  kol: string;
  name: string;
  x_handle: string | null;
  x_verified: boolean;
  fomo_handle: string | null;
  pumpfun: string | null;
  sheet_wallets: string[];
  formats: string[];
  keep: boolean;
  active: boolean;
  group: string | null;
  low_usd: number | null;
  high_usd: number | null;
  note: string | null;
  wallet_note: string | null;
  joined_at: string;
  left_at: string | null;
  last_seen_at: string | null;
}

export interface Skipped {
  line: number;
  reason: string;
}

/** The last sheet read. `skipped` is what the server stored: a list of lines. */
export interface SyncRun {
  started_at: string;
  finished_at: string;
  status: "ok" | "failed" | string;
  error: string | null;
  rows: number;
  added: number;
  updated: number;
  removed: number;
  skipped: unknown;
  note: string | null;
}

export interface TeamNetwork {
  members: AdminMember[];
  last_sync: SyncRun | null;
}

/** Skipped lines of a read, whatever shape came back. */
export function skippedLines(run: SyncRun): Skipped[] {
  if (!Array.isArray(run.skipped)) return [];
  return run.skipped.filter((s): s is Skipped => !!s && typeof s === "object" && typeof (s as Skipped).line === "number" && typeof (s as Skipped).reason === "string");
}

/** Distinct groups, sorted; members without a group are not a group. */
export function groupsOf(members: AdminMember[]): string[] {
  return [...new Set(members.map((m) => m.group).filter((g): g is string => !!g && g.trim() !== ""))].sort((a, b) => a.localeCompare(b));
}

/** By group ("" keeps all) and by a fragment of the name, id, X or FOMO handle. */
export function filterMembers(members: AdminMember[], group: string, query: string): AdminMember[] {
  const q = query.trim().toLowerCase().replace(/^@/, "");
  return members.filter((m) => {
    if (group !== "" && m.group !== group) return false;
    if (q === "") return true;
    return [m.name, m.kol, m.x_handle, m.fomo_handle].some((v) => v !== null && v.toLowerCase().includes(q));
  });
}

/** An X handle as the server takes it: up to 15 letters, digits or underscores, without @. */
export function cleanHandle(s: string): string | null {
  const h = s.trim().replace(/^@/, "");
  return /^[A-Za-z0-9_]{1,15}$/.test(h) ? h : null;
}

export interface EditForm {
  handle: string;
  formats: string[];
  x_followers: string;
  telegram_subscribers: string;
  pumpfun_followers: string;
  source: string;
  date: string;
}

export function emptyForm(m: AdminMember, today: string): EditForm {
  return {
    handle: m.x_verified && m.x_handle ? m.x_handle : "",
    formats: [...m.formats],
    x_followers: "",
    telegram_subscribers: "",
    pumpfun_followers: "",
    source: "",
    date: today,
  };
}

export type FormProblem = "handle" | "reach_number" | "reach_source" | "reach_date" | "nothing";

export interface MemberPatch {
  x_handle?: string;
  formats?: string[];
  reach?: { x_followers: number | null; telegram_subscribers: number | null; pumpfun_followers: number | null; source: string; date: string };
}

function count(s: string): number | null | undefined {
  const v = s.trim().replace(/[\s,_]/g, "");
  if (v === "") return null;
  return /^\d+$/.test(v) ? Number(v) : undefined;
}

/**
 * The PATCH body for what changed, or the problems. A handle is sent only when it differs
 * from the verified one (sending it marks it verified); formats only when they changed;
 * reach only when a number is filled, and then with its source and date.
 */
export function buildPatch(m: AdminMember, f: EditForm): { patch: MemberPatch; problems: FormProblem[] } {
  const patch: MemberPatch = {};
  const problems: FormProblem[] = [];
  if (f.handle.trim() !== "") {
    const h = cleanHandle(f.handle);
    if (h === null) problems.push("handle");
    else if (!(m.x_verified && m.x_handle === h)) patch.x_handle = h;
  }
  const formats = FORMATS.filter((x) => f.formats.includes(x));
  const before = FORMATS.filter((x) => m.formats.includes(x));
  if (formats.join() !== before.join()) patch.formats = formats;
  const numbers = [count(f.x_followers), count(f.telegram_subscribers), count(f.pumpfun_followers)];
  if (numbers.some((n) => n === undefined)) problems.push("reach_number");
  else if (numbers.some((n) => n !== null)) {
    if (f.source.trim() === "") problems.push("reach_source");
    if (!/^\d{4}-\d{2}-\d{2}$/.test(f.date)) problems.push("reach_date");
    patch.reach = { x_followers: numbers[0] ?? null, telegram_subscribers: numbers[1] ?? null, pumpfun_followers: numbers[2] ?? null, source: f.source.trim(), date: f.date };
  }
  if (problems.length === 0 && Object.keys(patch).length === 0) problems.push("nothing");
  return { patch, problems };
}
