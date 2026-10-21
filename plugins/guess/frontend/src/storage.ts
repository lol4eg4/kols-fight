// The visitor's guesses and score live in localStorage only. Every access is inside try/catch:
// private windows, blocked site data and quota errors must never break the page.

export const STORAGE_KEY = "arena-guess-v1";
/** Remembered guesses; the oldest go first. */
export const MAX_PICKS = 50;

/** What happened to a guess once the fight was over. */
export type Verdict = "called" | "missed" | "draw";

export interface Pick {
  /** The KOL the visitor chose. */
  kol: string;
  /** `null` until the fight is over. */
  verdict: Verdict | null;
}

export interface Saved {
  /** Guesses that had a winner. */
  called: number;
  played: number;
  /** By fight key. */
  picks: Record<string, Pick>;
}

export const EMPTY: Saved = { called: 0, played: 0, picks: {} };

/** The same key for the same fighters and date, in any order. */
export function fightKey(ids: readonly string[], date: string): string {
  return `${date}|${[...ids].sort().join(",")}`;
}

/** localStorage, or `null` when the browser refuses it (the getter itself can throw). */
export function browserStorage(): Storage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

function count(v: unknown): number {
  return typeof v === "number" && Number.isInteger(v) && v >= 0 ? v : 0;
}

function parse(raw: string): Saved {
  const j: unknown = JSON.parse(raw);
  if (!j || typeof j !== "object") return { called: 0, played: 0, picks: {} };
  const o = j as Record<string, unknown>;
  const picks: Record<string, Pick> = {};
  if (o.picks && typeof o.picks === "object") {
    for (const [k, v] of Object.entries(o.picks as Record<string, unknown>)) {
      if (!v || typeof v !== "object") continue;
      const p = v as Record<string, unknown>;
      if (typeof p.kol !== "string") continue;
      const verdict = p.verdict === "called" || p.verdict === "missed" || p.verdict === "draw" ? p.verdict : null;
      picks[k] = { kol: p.kol, verdict };
    }
  }
  const played = count(o.played);
  return { called: Math.min(count(o.called), played), played, picks };
}

/** What is saved. `null` when storage is unusable; empty data when nothing (or garbage) is saved. */
export function load(storage: Storage | null = browserStorage()): Saved | null {
  if (!storage) return null;
  let raw: string | null;
  try {
    raw = storage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
  if (raw === null) return { called: 0, played: 0, picks: {} };
  try {
    return parse(raw);
  } catch {
    // JSON that is not ours: start empty, keep working
    return { called: 0, played: 0, picks: {} };
  }
}

/** Writes `saved`; false when the browser refused. */
export function save(saved: Saved, storage: Storage | null = browserStorage()): boolean {
  if (!storage) return false;
  const keys = Object.keys(saved.picks);
  const picks = Object.fromEntries(keys.slice(Math.max(0, keys.length - MAX_PICKS)).map((k) => [k, saved.picks[k]!]));
  try {
    storage.setItem(STORAGE_KEY, JSON.stringify({ ...saved, picks }));
    return true;
  } catch {
    return false;
  }
}

/** Notes the visitor's choice for `key`; a fight that has its verdict keeps it. */
export function choose(saved: Saved, key: string, kol: string): Saved {
  if (saved.picks[key]?.verdict) return saved;
  const { [key]: _old, ...rest } = saved.picks;
  return { ...saved, picks: { ...rest, [key]: { kol, verdict: null } } };
}

/** Settles `key` when the fight is over. Draws and fights without a guess leave the score alone. */
export function settle(saved: Saved, key: string, winner: string | null): { saved: Saved; verdict: Verdict | null } {
  const pick = saved.picks[key];
  if (!pick || pick.verdict) return { saved, verdict: pick?.verdict ?? null };
  const verdict: Verdict = winner === null ? "draw" : winner === pick.kol ? "called" : "missed";
  const counted = verdict !== "draw";
  return {
    verdict,
    saved: {
      called: saved.called + (verdict === "called" ? 1 : 0),
      played: saved.played + (counted ? 1 : 0),
      picks: { ...saved.picks, [key]: { kol: pick.kol, verdict } },
    },
  };
}
