# KOL Arena: build plan (engineering)

Status: draft for review, 2026-10-19. Inputs, in order of authority: `docs/product/scope.md`, `docs/product/user-stories.md`, `docs/product/system-architecture.md` (agreed), Brief `docs/designs/kol-arena.md` (section «Дизайн»), `DESIGN.md`, engineering notes `docs/designs/kol-arena-scope-review.md`. The architecture is the contract: block names, layers, routes, APIs, slots, events and screens come from it. This plan says how to build it.

## 1. Facts checked today

- Solana Tracker free key reaches PnL v2. `GET https://data.solanatracker.io/v2/pnl/wallets/{wallet}` returns summary (realized, unrealized, total, counts, averages, timing.lastTrade, avgHoldTimeSecs), `analysis.winRate` (all time), `analysis.tokens.winning/losing`, and `identity` (name, twitter, platform profiles incl. FOMO handle). `GET /v2/pnl/leaderboard/kols/period?period=30d` returns 100 KOLs per page with `period.realized`, `period.volume`, `period.tradingDays`, `identity.twitter`, cursor pagination. Header `x-api-key`. v1 `/pnl/{wallet}` is 404. Fixtures saved under `tmp/fixtures/` (public data, no key); copy into plugin `tests/fixtures/`.
- Per-wallet 7-day and 30-day realized PnL: `GET /v2/pnl/wallets/{wallet}/history?period=30d` returns `days[]` (30 entries) with `activity.pnl.realized`, `activity.counts.buys/sells`, `activity.volume.total` per day. d7 and d30 = sums of daily realized over the last 7 and 30 days; vol30 = sum of daily volume; days30 = days with buys + sells > 0. Two calls per network wallet per day (summary + history): about 140 per day for 69 wallets, inside the free 10,000 per month. Budget while building: at most 10 more live calls; tests use fixtures (`st_wallet_*`, `st_history_*`, `st_positions_*`, `st_kols_30d`) and a fake server.
- Public RPC `api.mainnet-beta.solana.com` answers `getProgramAccounts` on the Token program with `dataSize 165` + `memcmp` mint filter and `dataSlice {offset 32, length 40}` (owner + amount): 273,808 holders of one token in 3.8 s. Token-2022 answers too (no dataSize filter there). Wrapped SOL errors (too large).
- Network sheet (secret URL, env only) has one tab `Influencers`, columns: Name, Group, Low $, High $, Deliverable / note, FOMO, Pump.fun. Read as CSV export of that tab. Formats come only from known keywords in «Deliverable / note» (Post, Buy, Thread, Stream, Space, Call); the rest of that cell is a team-only note.
- Prototype JSON: 122 KOLs, 53 with a wallet, 22 network KOLs (`net`), 17 pending, 41 radar (Kolscan, not used). KE rows snapshot 2026-10-17 12:00 UTC; network rows (`src`) Solana Tracker 2026-10-18 10:40 UTC. Day-0 snapshot date: 2026-10-18. The JSON holds no rates; it is committed as `data/prototype/arena_data_v2.json`, the input of `scripts/gen-seed.ts`.
- Day-0 wallets: the JSON has 53 of 122. `gen-seed.ts` fills the rest from one saved KE leaderboard page (`data/prototype/kolexplorer-2026-10-19.html`, fetched once, also the parser fixture) by X handle. A KOL still without a wallet shows «—» with the reason «wallet arrives with the next daily update»; its Score stays (the prototype took it from that KOL's KE wallet row).

## 2. Repository layout

```
Cargo.toml                  workspace: backend, platform/core, plugins/*/api, plugins/*/backend
package.json                bun workspaces: web, platform/design, plugins/*/frontend
backend/                    crate `server`: assembly only (config, plugin list, app tags, jobs)
platform/core/              crate `arena-platform`: plugin trait, context, events, slots, person, tags, scheduler, per-plugin migrations, test DB helper
platform/design/            package `@arena/design`: tokens from DESIGN.md, base and KOL Arena pieces, gallery
web/                        package `kol-fight-web`: shell (router, frame, slot/widget registry, page events, dictionary, fetch)
plugins/<name>/             plugin.toml, api/, backend/, migrations/, frontend/, stub/, tests/
scripts/                    site.sh, gen-plugins.ts (plugin.toml -> web/src/plugins.gen.ts), gen-seed.ts (prototype JSON -> seed migrations)
```

- `site.sh` changes: `bun install --frozen-lockfile` at the repo root, `bun run build` in `web/`, `cargo build` from the root, binary at `target/{debug,release}/server`. It sources `tmp/site.env` when the file exists (task site only; staging secrets are a Release question, see 9).
- Plugin names (folder = schema = crate suffix): auth, journal, registry, snapshots, score, board, fight, kolexplorer, solanatracker, schedule, radar, roster, wallets, fomo, network, picker, moves, duel, guess, tournament, streaks.
- `api/` crate exists only for a plugin others call. It holds the trait, DTOs and a `Stub` that returns «off». `backend/` depends on its own `api/` and on lower or same-layer `api/` crates only.

## 3. Platform

Rust (`arena-platform`):

- `Person { id: Option<i64>, role: Nobody | Staff | Admin }`. Every API call takes `&Person`. Route tags: Public accepts Nobody; Staff needs Staff or Admin; Admin needs Admin.
- `Plugin` trait: `name()`, `migrations() -> &'static Migrator`, `routes(&Ctx) -> Vec<(Tag, Router)>`, `install(&Ctx)` (idempotent, after migrations), `jobs() -> Vec<Job>`.
- `Ctx`: pool, `Config` (secrets, optional values read once at start), `Apis` (type map of `Arc<dyn Trait>`), `Events`, `Slots`.
- Construction order in `backend/src/main.rs`: by layer, and inside a layer by the `uses` list in `plugin.toml` (topological; journal before auth). Each plugin gets the APIs it uses from `Ctx` and registers its own. A missing API or a cycle is a start-up error naming the plugins.
- Events: `Events::on::<E>(handler)`, `emit(&E).await` runs handlers in registration order; a handler error is logged and returned to the emitter. Events in this product: «снимок открыт» (date), «таблица прочитана» (date), «хэндл X проверен» (kol). Page events live in the web shell.
- API slots: `Slot<dyn Trait>` with a default. Board's «в сети?» defaults to false; Schedule's «другие источники» is a screen slot (web).
- Migrations: one schema per plugin. Each plugin's migrator runs on a detached connection with `CREATE SCHEMA IF NOT EXISTS <name>; SET search_path TO <name>`, so its `_sqlx_migrations` and tables live in its schema. Every query in plugin code names the schema (`snapshots.metric`); the shared pool keeps the default search_path. A test greps each plugin's SQL: table names must carry its own schema, never another plugin's. Each backend crate embeds `sqlx::migrate!("../migrations")` and has a `build.rs` with `rerun-if-changed=../migrations`. `public` keeps the app migrator for `/api/health`. Health reports app migration plus per-plugin pending count.
- Scheduler: jobs at fixed UTC times (05:45 roster read, 06:00 collection). Each run takes a connection from the pool, detaches it (`PoolConnection::detach`), takes `pg_try_advisory_lock(job key)` on it, and closes that connection at the end, so the lock ends with the run even on error or panic. A missed run does not run twice; «Run now» uses the same lock and answers «already running» when it is held.
- Config: env `SOLANA_TRACKER_API_KEY`, `NETWORK_SHEET_CSV_URL`, `FIRST_ADMIN_LOGIN`, `FIRST_ADMIN_PASSWORD`, `PUBLIC_RPC_URL` (Solana mainnet public RPC). Missing value: the feature that needs it reports «not configured» on the collection screen and in its run result. No default URL or key in code.
- Implementation registry: `Registry::choose(plugin, &Person) -> Impl` with rules from app config (per app, per role, per person id); values are an implementation name or `off` (the stub). Today every plugin has one implementation plus its stub.
- Page meta: `Plugin::page_meta(path, query) -> Option<Meta>`; the SPA fallback injects the first match into `index.html` (used by fight for `og:` and `twitter:` tags).
- Test helper `arena_platform::testing::TestDb`: creates a throwaway database from the board Postgres env, runs the named plugins' migrations, drops it on drop.

Web shell (`web/`) and design (`@arena/design`):

- Router: history API, routes from plugin screens: `/`, `/kol/:id`, `/fight` (overlays over `/`), `/team/login`, `/team/*`. A closed `/team` route without a session goes to `/team/login?next=`.
- Plugin frontend exports `definePlugin({ name, screens, fills, widgets, dict })`. `scripts/gen-plugins.ts` writes `web/src/plugins.gen.ts` from `plugin.toml`; a test fails if it is stale.
- `<Slot name>` renders fillers or its default; `<Widget name id>`; page events through `usePageEvent(name)` / `emitPageEvent(name, payload)`.
- Dictionary: `en.ts` per plugin plus shell; `t(key)`. No string literals in JSX except proper nouns and numbers (lint test).
- `@arena/design`: CSS variables generated from `DESIGN.md` front matter (`scripts/gen-tokens.ts`; test compares), self-hosted fonts (`@fontsource/press-start-2p`, `@fontsource/golos-text`, `@fontsource/jetbrains-mono`, latin + cyrillic), base pieces (Button, Field, List, Card, Dialog, Tabs, Badge, Progress, Tooltip that opens on hover, focus and tap) and the KOL Arena pieces from architecture section 5. Gallery screen `/team/design` (staff) shows every piece.
- Import rule (bun test): a plugin frontend imports only `react`, `@arena/design`, `@arena/shell` and its own files. `@arena/design` imports no plugin and calls no route.

## 4. Plugins: build notes

Only what the architecture does not already say.

Layer 0

- auth: argon2id hashes; session token 32 random bytes, cookie `arena_session` HttpOnly, Secure, SameSite=Strict, 14 days, stored as SHA-256; login rate limit 5 failures per IP per 15 min, then 15 min lock; first admin created at install from config if no admin exists. Admin adds staff and revokes access; revoking deletes that person's sessions at once (S13). Fills platform «человек запроса».
- journal: exactly the agreed API: `record(person, block, object, action, before, after)` -> id, `history(block, object)`. `before` and `after` are JSON; secrets never written.

Layer 1

- registry: KOL id = prototype slug (stable); new KOLs get a slug from name, deduplicated. Seed migration from prototype JSON (public fields only). `publish_wallets` refuses a wallet held by another KOL.
- snapshots: tables `snapshot(date, status, opened_at, closed_at)`, `member(date, kol, reason)`, `metric(date, kol, source, fetched_at, d1, d7, d30, wr7, wr30, wr_all, avg_pos, hold_s, last_min_at_fetch, traded_24h, unreal, total, vol30, days30, wins, losses, pnl_rank, in_top_today)`; `traded_24h` (KE: last trade within 1,440 min at fetch; ST: today's history entry has buys or sells) feeds streaks. Nulls stay null. `close` copies the latest earlier metric row (with its source and fetched_at) for members without one. `open` on a closed date reopens it. Seed migration: snapshot 2026-10-18 closed, members and metrics from the prototype.
- score: pure `score(metrics, at)` in `score/backend/src/formula.rs` with an ASCII diagram of the four parts; `rank(date)` stores parts, score, tier, badges, rank; order Score desc, d30 desc, name asc. Install ranks 2026-10-18 if not ranked.
- board: `/api/board?date=` returns rows with only allowlisted fields (separate public DTO type). Stale: `now - fetched_at > 48h`. Card `/api/kol/:id?date=` with every published wallet and its Solscan link `https://solscan.io/account/<address>` (S8). Ticker items built here; slot «бегущая строка» adds filler items.
- fight: `resolve(fighters, date)` pure function; rounds per requirements section 7; void, tie split, all-win void, rounds tie -> higher Score, Score tie -> Draw. `/api/fight/image` renders SVG with `resvg` and embedded TTF fonts (OFL files committed under `plugins/fight/backend/fonts/`) to a 1200x630 PNG, cached in memory by (ids, date, snapshot closed_at), so a re-collected date never serves an old image; LRU of 200. The server injects `og:` and `twitter:` meta into `index.html` for `/fight` so X shows the image.

Layer 2

- kolexplorer: fetch `https://kolexplorer.com/` once, parse rows by `data-address`, `data-twitter` and cell order; under 50 rows or missing attributes -> failed run, nothing written, members copied from the previous snapshot. Profile pages `/kol/<slug>` for top 100: avg position and hold, one request every 3 seconds (about 5 minutes per run), results cached per KOL for 24 hours in the plugin's schema, so a re-run the same day reuses them; failures leave those two null. The leaderboard page is cached for the run too. User-Agent names the product and a contact page. Parser tests on saved HTML fixtures.
- solanatracker: client with 3 req/s limiter, retries only on 5xx (2 tries, backoff), never on 4xx. 401/403 -> run failed with «key rejected» or «plan lacks PnL»; 429 -> stop the run, mark partial. `kol_list(date)` stored per date for radar and «📡 В топе дня». `wallet_by_handle` matches `identity.twitter`, case-insensitive, without `@`; also exposes FOMO handle from identity for the wallets block.
- schedule: runs table; 06:00 job: open, emit «снимок открыт», KE run, ST run, close, rank. Screen `/team/collection`.
- radar: list from today's ST KOL list minus board members; if today's list is missing -> «Radar not updated today» with the last list date.

Layer 3

- roster: CSV read at 05:45 and on admin button; header check; rows keyed by FOMO handle; removed rows end membership (history kept); the 9 prototype KOLs absent from the sheet stay members (seed flag `keep`). Rates, group, note stored in roster tables only; public API never returns them; export `/api/admin/network/export` CSV.
- wallets: state machine from `kol-arena-scope-review.md`, plus: auto-confirm when the verified X handle equals the wallet label; FOMO-handle match from Solana Tracker identity counts as a label match on the FOMO handle (hypothesis until the X handle is verified). Manual confirm of a hypothesis requires «where and when KOL confirmed». Address from KOL is confirmed after a trades check through Solana Tracker. Every change journaled. Publishes confirmed main and other wallets to registry.
- fomo: holders by `getProgramAccounts` with `memcmp` on mint at offset 0 and `dataSlice` owner + amount, Token (with `dataSize 165`) and Token-2022; decimals from `getTokenSupply`; amount match within 0.5% of the amount the staff member typed (FOMO rounds). Intersect owners across positions. One RPC call per position per program, 60 s timeout, response over 64 MB or RPC error -> the error line, no hypothesis. Manual tool only: a few calls per KOL, inside public RPC limits.
- network: `/api/network` with members on board and waiting list with safe reasons only.
- picker: filter by Score, tier, rate range, format; export for client (public fields) and with rates (staff).

Layer 4

- moves: rank change vs previous day and 7 days back, only if both days fresh for that KOL; «Взлёт недели» = best 7-day climb.
- duel: deterministic pair per date from the top 20 (hash of date), stored; hero slot.
- guess: browser only (localStorage, wrapped in try/catch).
- tournament: `/api/tournament?ids=8&date=` resolves the bracket with fight rules; draw -> higher rank advances.
- streaks: «7 days with a trade», «3 weeks in profit» from history; appear when history is long enough.

## 5. Data flow

```
05:45 roster CSV ──> roster ──emit «таблица прочитана»──> wallets (auto-match) ──> registry.publish_wallets
06:00 schedule.open(date) ──emit «снимок открыт»──> roster/network add members
      ├─ kolexplorer.run(date) ── leaderboard + profiles ──> snapshots.put_metrics / set_members(top100)
      ├─ solanatracker.run(date) ── KOL list + network wallets ──> snapshots.put_metrics, kol_list
      ├─ snapshots.close(date)  (carry forward missing rows with old fetched_at)
      └─ score.rank(date) ──> board, fight, games read ranking(date)
Client: /api/board, /api/kol/:id, /api/fight, /api/fight/image, /api/network, /api/radar, games routes (public DTOs only)
Team:   /api/admin/* (session cookie, role check, journal on every write)
```

## 6. Security

- Separate public DTO types; a test walks every public route on the seeded database and asserts JSON keys are a subset of an allowlist and contain no rate, group, note or hypothesis address.
- Bundle test: build `web/`, grep `dist/` for internal field names and for rate values from a fixture sheet with fake rates.
- CSRF: SameSite=Strict cookie plus `Origin` check on every non-GET `/api/admin` request.
- Inputs: ids and slugs validated against registry; base58 32-byte addresses; dates `YYYY-MM-DD` within stored range; FOMO amounts as decimal strings.
- External text (names, handles) rendered as text only. Links built from validated handles.
- Secrets never logged; config struct implements `Debug` with redaction.

## 7. Tests

Rust (per crate, `cargo test -p <crate>`):

- score: control example (Tekkerrss 55, Diamond, rank 18 of 122) and the 25 screenshot values; full 122-row recompute pinned; null parts; d30 <= 0 disables accuracy; tie order.
- fight: 2-4 fighters, void, tie split, all-win void, rounds tie -> Score, Score tie -> Draw, counted-rounds line, unknown id, stale snapshot link.
- snapshots: carry forward, reopen same date, set_members replaces a reason, copy_members.
- kolexplorer: parser on fixture HTML, under-50 rows fails, missing attribute fails, profile parse.
- solanatracker: fixtures for wallet and KOL list; 401, 403, 429, 5xx on a fake server.
- roster: CSV parse, header change fails, removed row, keep flag, formats keywords, no rates in public API.
- wallets: every transition, confirm without KOL record refused, journal rows.
- fomo: holder intersection, tolerance, too many holders, Token-2022.
- auth: hash verify, lockout, session expiry, role checks per tag.
- architecture test (`backend/tests/arch.rs`): plugin.toml layers vs `cargo metadata` deps (same or lower layer `api` crates only, no cycles), every route has a tag, plugin list in `main.rs` equals `plugins/*/plugin.toml`.
- leak tests (section 6).

Web (`bun test`): formatting (money short and full, time ago, UTC dates), dictionary completeness, import rule, tokens match DESIGN.md, plugins.gen.ts fresh.

E2E (Playwright, against the task site, chromium, Xvfb not needed headless):

- board renders 122 rows on day 0, podium order, search, tier filter, network tab;
- card by keyboard: open, focus trap, Esc returns focus, direct link closes to `/`;
- fight by keyboard: pick 3, VS, Next through rounds, void round grey, K.O., Share link reopens same fight;
- 360 px: no horizontal scroll on `/`, `/kol/tekkerrss`, `/fight?...`, `/team/wallets`;
- reduced motion: no animation classes, ticker static;
- team: login, wrong password message, lockout after 5, logout; staff cannot open `/team/staff`.

## 8. Failure modes

- KE page changes: parser fails, run failed, members copied, page shows stale marks. Tested.
- ST quota hit mid-run: 429 stops run, partial, old metrics carried. Tested on fake server.
- Sheet unreachable or header changed: roster unchanged, run failed on collection screen. Tested.
- Public RPC rejects `getProgramAccounts` or times out: FOMO match shows the error line, no hypothesis. Tested with fake RPC.
- Server restarts during 06:00 run: advisory lock released, snapshot left open; next start sees open snapshot older than today and closes it with carry-forward, then ranks. Tested.
- Two KOLs claim one wallet: publish refused, conflict shown on Wallets. Tested.

## 9. Delivery and secrets

- Task site: `paneweb up`; `tmp/site.env` (git-ignored) holds the key, sheet URL and first admin for this worktree only.
- Staging: the deploy reads only `scripts/site.sh staging`. `site.sh staging` sources `$HOME/.config/kol-fight/staging.env` when it exists. Writing that file is outside the repo: ask the owner for a yes before Test, so staging gets the key, the sheet URL and the first admin. Without it staging shows the day-0 snapshot, the collection screen says «not configured», and nobody can sign in; the Test report says so.
- Migrations additive; seed migrations idempotent by primary keys.

## 10. Build order and lanes

1. Foundation (one worker, sequential): workspace move, platform core, test DB helper, arch test, site.sh, shell, design package with tokens and fonts, gen scripts, seed generator, all `api/` crates with traits, DTOs and stubs, plugin skeletons with `plugin.toml`, empty migrations. Acceptance: `cargo build`, `cargo test -p arena-platform -p server`, `bun test`, `bun run build`, site starts and `/api/health` is ok.
2. Parallel lanes in separate worktrees, each owning only its plugin folders:
   - Lane A, arena core: registry, snapshots, score, board, fight (incl. image) + client page screens.
   - Lane B, daily data: kolexplorer, solanatracker, schedule, radar + collection screen.
   - Lane C, network: roster, wallets, fomo, network, picker + team screens.
   - Lane D, auth and journal + login and staff screens, then games: moves, duel, guess, tournament, streaks.
   Lanes depend only on `api/` crates from step 1. Conflicts: `backend/src/main.rs` plugin list and `web` shell are touched only in step 1; lanes register through generated files.
3. Merge lanes, run the touched crates' tests, E2E on the task site, fix.

## NOT in scope

- Light theme, RU strings, Telegram alert, production deploy (TODOS.md).
- Own PnL calculation from chain (variant В).
- Live streams from Solana Tracker websockets: daily snapshot is the product.

## What already exists

- `backend/` axum + sqlx starter with health route and SPA fallback: kept as the `server` crate.
- `web/` Vite React starter: becomes the shell.
- `scripts/site.sh`, `.paneweb.json`, `.staging-site.json`: kept, site.sh adapted.
- Score formula verified on the prototype (learning `arena-score-fixture`).
