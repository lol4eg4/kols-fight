// Types of GET /api/team/wallets and the pure logic of the Wallets screen and filler.

export type KolStatus = "confirmed" | "hypothesis" | "not_found" | "conflict";
export type CandidateStatus = "hypothesis" | "confirmed" | "not_found" | "rejected" | "revoked" | "retired" | "conflict";
export type CandidateSource = "kolexplorer" | "solanatracker" | "fomo" | "from_kol" | "sheet" | "pumpfun" | "prototype";

export interface CandidateView {
  id: number;
  kol: string;
  address: string;
  status: CandidateStatus;
  source: CandidateSource;
  evidence: string | null;
  sent_by: string | null;
  confirmation: string | null;
  reason: string | null;
  main: boolean;
  created_at: string;
  updated_at: string;
  last_trade_at: string | null;
  checked_at: string | null;
}

export interface KolView {
  kol: string;
  name: string;
  status: KolStatus;
  main_address: string | null;
  last_trade_at: string | null;
  candidates: CandidateView[];
}

export interface Overview {
  counts: Partial<Record<KolStatus, number>>;
  kols: KolView[];
}

export type Action = "confirm" | "reject" | "revoke" | "main";

/** What staff may do with a candidate, by the server's state machine. */
export function actionsFor(c: CandidateView): Action[] {
  switch (c.status) {
    case "hypothesis":
      return ["confirm", "reject"];
    case "conflict":
      return ["confirm", "reject", "revoke"];
    case "confirmed":
      return c.main ? ["revoke"] : ["main", "revoke"];
    default:
      return [];
  }
}

/** Candidates still in play first (conflict, hypothesis, confirmed), then the rest; newest first inside a group. */
export function sortCandidates(list: CandidateView[]): CandidateView[] {
  const order: Record<CandidateStatus, number> = { conflict: 0, hypothesis: 1, confirmed: 2, not_found: 3, retired: 4, rejected: 5, revoked: 6 };
  return [...list].sort((a, b) => order[a.status] - order[b.status] || (a.main === b.main ? 0 : a.main ? -1 : 1) || b.created_at.localeCompare(a.created_at));
}

/** KOLs that need a person first: conflict, hypothesis, not found, then confirmed; by name inside. */
export function sortKols(list: KolView[]): KolView[] {
  const order: Record<KolStatus, number> = { conflict: 0, hypothesis: 1, not_found: 2, confirmed: 3 };
  return [...list].sort((a, b) => order[a.status] - order[b.status] || a.name.localeCompare(b.name));
}

/** Filter by status ("all" keeps every KOL) and by a name or handle fragment. */
export function filterKols(list: KolView[], status: KolStatus | "all", query: string): KolView[] {
  const q = query.trim().toLowerCase();
  return list.filter((k) => (status === "all" || k.status === status) && (q === "" || k.name.toLowerCase().includes(q) || k.kol.toLowerCase().includes(q)));
}

/** A required text is filled: not only spaces. */
export function filled(s: string): boolean {
  return s.trim() !== "";
}

/**
 * One load per key while it is fresh, shared by every caller (one overview for all the
 * wallet_status fillers of the Network screen). A failed load is not kept.
 */
export function createCache<R extends { error?: unknown }>(load: (key: string) => Promise<R>, ttlMs: number, now: () => number = Date.now) {
  const entries = new Map<string, { at: number; promise: Promise<R> }>();
  return {
    get(key: string): Promise<R> {
      const hit = entries.get(key);
      if (hit && now() - hit.at < ttlMs) return hit.promise;
      const promise: Promise<R> = load(key).then((r) => {
        if (r.error !== undefined && entries.get(key)?.promise === promise) entries.delete(key);
        return r;
      });
      entries.set(key, { at: now(), promise });
      return promise;
    },
    /** Puts a fresh value in, e.g. the overview a write returned. */
    put(key: string, value: R) {
      entries.set(key, { at: now(), promise: Promise.resolve(value) });
    },
    drop(key?: string) {
      if (key === undefined) entries.clear();
      else entries.delete(key);
    },
  };
}
