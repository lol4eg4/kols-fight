// Day-0 seed: reads the prototype data and writes the seed migrations of registry and snapshots.
// Run: bun scripts/gen-seed.ts   (commit the generated files with the script)
//
// Inputs:
//   data/prototype/arena_data_v2.json        122 KOLs: public fields and day-0 metrics.
//   data/prototype/kolexplorer-2026-10-19.html one saved KOL Explorer leaderboard page: fills the
//                                              wallets the JSON lacks (matched by X handle) and
//                                              the KOL Explorer profile slugs.
// Outputs:
//   plugins/registry/migrations/0003_seed.sql   KOLs and published wallets.
//   plugins/snapshots/migrations/0003_seed.sql  snapshot 2026-10-18 (closed), members, metrics.
//   plugins/snapshots/backend/tests/fixtures/day0_2026-10-18.json
//       the SnapshotView `snapshot(2026-10-18)` must return (serde form of api_snapshots types);
//       the snapshots tests check the seed against it, the score tests rank it.
//
// Public fields only: the JSON holds no rates, groups or notes. Seeds are idempotent by
// primary key (ON CONFLICT DO NOTHING).
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const root = resolve(import.meta.dir, "..");
const DATE = "2026-10-18";
// KE rows: the JSON's `snapshot` time. Network rows (`src: "st"`): Solana Tracker read on
// 2026-10-18 10:40 UTC (plan section 1). The day-0 snapshot opens and closes at that time.
const ST_FETCHED_AT = "2026-10-18T10:40:00Z";
// When the saved KE page was read: its wallets are published at that date.
const KE_PAGE_AT = "2026-10-19T00:00:00Z";

interface ProtoKol {
  name: string;
  slug: string;
  x: string;
  wallet: string | null;
  alt?: string[];
  tg: boolean;
  tgh?: string;
  tgs?: number;
  kolscan: boolean;
  net?: boolean;
  src?: string;
  fomo?: string;
  xf?: number;
  pumpf?: number;
  formats?: string[];
  d1: number | null;
  d7: number | null;
  d30: number | null;
  wr7: number | null;
  wr30: number | null;
  wrAll?: boolean;
  avg: number | null;
  hold: number | null;
  last: number | null;
  unreal: number | null;
  total: number | null;
  vol30?: number;
  days30?: number;
  w?: number;
  l?: number;
  pnl_rank: number | null;
}

interface KeRow {
  slug: string;
  handle: string;
  address: string;
}

const fail = (msg: string): never => {
  throw new Error(`gen-seed: ${msg}`);
};

/** `<tr class="board-row" data-…>` rows of the KE leaderboard page. */
export function parseKePage(html: string): KeRow[] {
  const rows: KeRow[] = [];
  for (const m of html.matchAll(/<tr class="board-row"([^>]*)>/g)) {
    const attrs: Record<string, string> = {};
    for (const a of m[1].matchAll(/data-([a-z0-9-]+)="([^"]*)"/g)) attrs[a[1]] = a[2];
    if (!attrs.slug || !attrs.address) continue;
    const handle = (attrs.twitter ?? "").replace(/^https?:\/\/(www\.)?(x|twitter)\.com\//, "").replace(/\/.*$/, "");
    rows.push({ slug: attrs.slug, handle, address: attrs.address });
  }
  return rows;
}

const BASE58 = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;

function sql(v: unknown): string {
  if (v === null || v === undefined) return "NULL";
  if (typeof v === "boolean") return v ? "true" : "false";
  if (typeof v === "number") {
    if (!Number.isFinite(v)) fail(`not a finite number: ${v}`);
    return String(v);
  }
  if (Array.isArray(v)) return `ARRAY[${v.map(sql).join(", ")}]::text[]`;
  return `'${String(v).replaceAll("'", "''")}'`;
}

const values = (rows: unknown[][]) => rows.map((r) => `    (${r.map(sql).join(", ")})`).join(",\n");

function main() {
  const data = JSON.parse(readFileSync(resolve(root, "data/prototype/arena_data_v2.json"), "utf8"));
  const kols: ProtoKol[] = data.kols;
  if (kols.length !== 122) fail(`expected 122 KOLs, got ${kols.length}`);
  const keFetchedAt: string = data.snapshot;
  if (!/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$/.test(keFetchedAt)) fail(`bad snapshot time ${keFetchedAt}`);
  if (data.snapshot_net !== DATE) fail(`snapshot_net is ${data.snapshot_net}, expected ${DATE}`);

  const ke = parseKePage(readFileSync(resolve(root, "data/prototype/kolexplorer-2026-10-19.html"), "utf8"));
  if (ke.length < 100) fail(`KE page: only ${ke.length} rows`);
  const keByHandle = new Map<string, KeRow>();
  for (const r of ke) if (r.handle) keByHandle.set(r.handle.toLowerCase(), r);

  // Registry rows and wallets.
  const kolRows: unknown[][] = [];
  const walletRows: unknown[][] = [];
  const owner = new Map<string, string>();
  let fromKe = 0;
  const publish = (address: string, kol: string, main: boolean, pos: number, source: string, at: string) => {
    if (!BASE58.test(address)) fail(`${kol}: not a Solana address ${address}`);
    const held = owner.get(address);
    if (held) fail(`wallet ${address} of ${kol} already belongs to ${held}`);
    owner.set(address, kol);
    walletRows.push([address, kol, main, pos, source, at]);
  };
  for (const k of kols) {
    if (!/^[a-z0-9][a-z0-9-]*$/.test(k.slug)) fail(`bad slug ${k.slug}`);
    const keRow = keByHandle.get(k.x.toLowerCase());
    const hasReach = k.xf != null || k.tgs != null || k.pumpf != null;
    kolRows.push([
      k.slug,
      k.name,
      k.x,
      keRow?.slug ?? null,
      k.src === "st",
      k.kolscan,
      k.fomo ?? null,
      null,
      k.tgh ?? null,
      k.xf ?? null,
      k.tgs ?? null,
      k.pumpf ?? null,
      hasReach ? "prototype" : null,
      hasReach ? DATE : null,
      k.formats ?? [],
    ]);
    if (k.wallet) {
      publish(k.wallet, k.slug, true, 0, "prototype", k.src === "st" ? ST_FETCHED_AT : keFetchedAt);
    } else if (keRow) {
      publish(keRow.address, k.slug, true, 0, "kolexplorer", KE_PAGE_AT);
      fromKe++;
    }
    (k.alt ?? []).forEach((a, i) => publish(a, k.slug, false, i + 1, "prototype", ST_FETCHED_AT));
  }

  // Snapshot members and metrics.
  const members: unknown[][] = [];
  const metrics: unknown[][] = [];
  const view = {
    snapshot: { date: DATE, status: "closed", opened_at: ST_FETCHED_AT, closed_at: ST_FETCHED_AT },
    members: [] as unknown[],
  };
  for (const k of [...kols].sort((a, b) => (a.slug < b.slug ? -1 : a.slug > b.slug ? 1 : 0))) {
    // Members: network KOLs (`net`) and the KE top 100 (the rest). Metric source: `src`
    // "st" is Solana Tracker; "kolex" (two network KOLs) and no `src` are KOL Explorer.
    const network = k.net === true;
    if (k.src !== undefined && k.src !== "st" && k.src !== "kolex") fail(`${k.slug}: unknown src ${k.src}`);
    if (k.src === "st" && !network) fail(`${k.slug}: src st outside the network`);
    const reason = network ? "network" : "top100";
    const st = k.src === "st";
    const source = st ? "solanatracker" : "kolexplorer";
    const fetchedAt = st ? ST_FETCHED_AT : keFetchedAt;
    const allTime = k.wrAll === true;
    const m = {
      d1: k.d1,
      d7: k.d7,
      d30: k.d30,
      wr7: k.wr7,
      wr30: allTime ? null : k.wr30,
      wr_all: allTime ? k.wr30 : null,
      avg_pos: k.avg,
      hold_s: k.hold,
      last_min_at_fetch: k.last,
      // KE: last trade within 1,440 minutes at fetch. ST: from the day's history entry, which
      // the prototype does not hold, so no data.
      traded_24h: st || k.last == null ? null : k.last <= 1440,
      unreal: k.unreal,
      total: k.total,
      vol30: k.vol30 ?? null,
      days30: k.days30 ?? null,
      wins: k.w ?? null,
      losses: k.l ?? null,
      pnl_rank: k.pnl_rank,
      // Not in the prototype data.
      in_top_today: false,
    };
    members.push([DATE, k.slug, reason]);
    metrics.push([DATE, k.slug, source, fetchedAt, ...Object.values(m)]);
    view.members.push({
      kol: k.slug,
      reasons: [reason],
      row: { date: DATE, kol: k.slug, source, fetched_at: fetchedAt, metrics: m },
    });
  }

  const head = (what: string) =>
    `-- Generated by scripts/gen-seed.ts from data/prototype. Do not edit by hand.\n-- ${what}\n-- Idempotent by primary key.\n`;
  const registrySql =
    head(`Day-0 KOLs (${kolRows.length}) and published wallets (${walletRows.length}; ${fromKe} from the KE page).`) +
    `\nINSERT INTO registry.kol (id, name, x_handle, kolexplorer, solanatracker, kolscan, fomo, pumpfun, telegram,\n` +
    `    x_followers, telegram_subscribers, pumpfun_followers, reach_source, reach_date, formats) VALUES\n` +
    values(kolRows) +
    `\nON CONFLICT (id) DO NOTHING;\n\nINSERT INTO registry.wallet (address, kol, main, pos, source, published_at) VALUES\n` +
    values(walletRows) +
    `\nON CONFLICT (address) DO NOTHING;\n`;
  const snapshotsSql =
    head(`Day-0 snapshot ${DATE} (closed): ${members.length} members and their metrics.`) +
    `\nINSERT INTO snapshots.snapshot (date, status, opened_at, closed_at) VALUES\n` +
    values([[DATE, "closed", ST_FETCHED_AT, ST_FETCHED_AT]]) +
    `\nON CONFLICT (date) DO NOTHING;\n\nINSERT INTO snapshots.member (date, kol, reason) VALUES\n` +
    values(members) +
    `\nON CONFLICT (date, kol, reason) DO NOTHING;\n\nINSERT INTO snapshots.metric (date, kol, source, fetched_at, d1, d7, d30, wr7, wr30, wr_all,\n` +
    `    avg_pos, hold_s, last_min_at_fetch, traded_24h, unreal, total, vol30, days30, wins, losses, pnl_rank,\n` +
    `    in_top_today) VALUES\n` +
    values(metrics) +
    `\nON CONFLICT (date, kol) DO NOTHING;\n`;

  const write = (rel: string, text: string) => {
    const path = resolve(root, rel);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, text);
    console.log(`wrote ${rel}`);
  };
  write("plugins/registry/migrations/0003_seed.sql", registrySql);
  write("plugins/snapshots/migrations/0003_seed.sql", snapshotsSql);
  write("plugins/snapshots/backend/tests/fixtures/day0_2026-10-18.json", `${JSON.stringify(view, null, 1)}\n`);
  const noWallet = kols.filter((k) => !k.wallet && !keByHandle.has(k.x.toLowerCase()));
  console.log(`${kols.length} KOLs, ${walletRows.length} wallets (${fromKe} from KE), ${noWallet.length} without a wallet`);
}

if (import.meta.main) main();
