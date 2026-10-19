# KOL Arena: build spec (epic)

Engineer's ticket for the first release. Contract: `docs/product/system-architecture.md` (blocks, layers, routes, APIs, events, slots, screens). How: `docs/designs/kol-arena-plan.md`. Look: `DESIGN.md` and the Brief section «Дизайн» (`docs/designs/kol-arena.md`). Product rules: `docs/product/scope.md`, `docs/product/user-stories.md`. Requirements from the owner: `data/prototype/` holds the prototype data; the original requirements are summarised in scope and the Brief.

## Context

Clients (Solana token teams) pick KOLs by onchain results. Today the repo is an empty axum + React starter. Done means: the client page, KOL card, fight, network section, games and the team section run on the task site with the day-0 snapshot, the daily collection works against the real sources, and every test below passes.

## Child specs

1. F1 Rust foundation. Priority critical. Depends on nothing.
2. F2 Web foundation and design system. Priority critical. Depends on the committed manifests only (runs beside F1; it never edits `plugin.toml`).
3. A Arena core: registry, snapshots, score, board, fight. Depends on F1, F2.
4. B Daily data: kolexplorer, solanatracker, schedule, radar. Depends on F1, F2.
5. C Network: roster, wallets, fomo, network, picker. Depends on F1, F2.
6. D Auth, journal, games: auth, journal, moves, duel, guess, tournament, streaks. Depends on F1, F2.

```
F1 ──┐
     ├──> merge ──> A ─┐
F2 ──┘          ├──> B ─┤
                ├──> C ─┼──> merge ──> E2E on task site ──> fix
                └──> D ─┘
```

Sequencing: F1 and F2 fix every cross-plugin contract (api crates, plugin.toml, shell registry, design pieces). After that, lanes touch only their own plugin folders, so they run in parallel without conflicts. Reordering lanes before F breaks that.

## Contracts fixed before any child starts

- `plugins/*/plugin.toml` for all 21 plugins are committed (format in F1). They name every `uses`, event, slot, fill, widget and screen. After F1 and F2, each lane owns the manifests of its own plugins and may add to them; a change to another plugin's manifest or `api` crate goes through the coordinator.
- Score formula input: `score(metrics)` exactly as the architecture says. Activity uses `last_min_at_fetch`. Accuracy uses `wr30`, or `wr_all` when `wr30` is null (network KOLs from Solana Tracker), counted only when `d30 > 0`. Expected day-0 ranking for all 122 KOLs: `data/prototype/expected_ranking_2026-10-18.json` (rank, slug, name, score, tier); generated from the formula and checked against the 25 screenshot values below.
- Fixtures: `data/prototype/arena_data_v2.json`, `data/prototype/kolexplorer-2026-10-19.html`, `data/fixtures/solanatracker/st_wallet_tekkerrss.json`, `st_history_tekkerrss_30d.json`, `st_positions_tekkerrss.json`, `st_kols_30d.json`.
- Events (backend, payload): `snapshot_opened { date }` (schedule), `table_read { date }` and `x_handle_verified { kol }` (roster).
- Page events (web, payload): `fighter_added { kol }`, `fight_over { ids, date, winner | null }` (fight).
- API slot `board/in_network`: trait `InNetwork { async fn is_member(&self, person, kol) -> bool }` in `plugins/board/api`; default false; filled by roster.
- Screen slots (host / name: filler, props). Props are ids only.
  - board / network_section: network, `{ date }`.
  - board / radar_section: radar, `{ date }`.
  - board / network_filter: network, `{ date }`; returns the set of network KOL ids for the leaderboard tab.
  - board / rank_change: moves, `{ date, kol }`.
  - board / extra_badges: streaks, `{ date, kol }`.
  - board / ticker: moves, `{ date }`; adds ticker items.
  - board / duel_of_day: duel, `{ date }`.
  - board / below_fight: tournament, `{ date }`.
  - fight / before_fight: guess, `{ ids, date }`.
  - schedule / other_sources: roster, `{}`.
  - roster / wallet_status: wallets, `{ kol }`.
- Widgets: fight / add_to_fight `{ kol }` placed by board and network; fight / fighter_picker `{}` placed by board.
- DTO types: F1 designs the Rust types and errors from the architecture's operations and the metric fields in plan section 4, documents them with rustdoc, and they are frozen at the F merge. Lanes start only after that merge. Each lane writes the TypeScript types for its own routes in its plugin frontend.
- Fight rules (from the owner's requirements, section 7). Rounds in order, winner by:
  1. Arena Score: higher.
  2. PnL 30 days (d30): higher.
  3. PnL 7 days (d7): higher.
  4. PnL 24 hours (d1): higher.
  5. Win rate: higher; `wr30`, or `wr_all` marked with an asterisk when `wr30` is null.
  6. Win rate 7 days (wr7): higher.
  7. Position size (avg_pos): higher.
  8. Freshness of the last trade (minutes since last trade at fetch): lower.
  A round where any fighter has no value is void («not counted: not everyone has data»). The round's best value wins 1 point; several fighters tied on the best value split it equally (1/k each); when every fighter ties, the round is void. Each fighter starts with 8 HP segments; each counted round a fighter does not win (or split) costs 1 segment. Winner: most points; equal points -> higher Arena Score («Tie on rounds — X wins on higher Arena Score»); equal Score too -> Draw. The result lists counted rounds («6 of 8 rounds counted») and a 2-3 line summary naming the rounds each fighter won.
- Metric precedence per KOL per date: a KOL in today's KE top 100 gets KE metrics; other KOLs with a published wallet get Solana Tracker metrics; Solana Tracker skips KOLs KE wrote for that date. A network KOL that is also in the KE top keeps KE metrics; if its confirmed network wallet differs from its KE wallet, wallets shows a conflict and neither is published until staff resolves it.
- Re-running a closed date: schedule reopens it and runs both sources again. A source that succeeds replaces its own rows for that date; a source that fails keeps the rows it wrote earlier that date; KE members are copied from the previous date only when the date has none. Then close and rank again.
- Leak checks: each lane tests its own public routes against an allowlist of public DTO keys. After the merge the coordinator adds `backend/tests/leak.rs`, which walks every public route on the seeded database, and the bundle grep.

## Shared rules for every child

- Work only in the given worktree. Commit on its branch with clear messages; never push, never merge.
- Rust: workspace at repo root; `cargo test -p <crate>` only for crates you touch; never a bare `cargo test`. Format with `cargo fmt`, no warnings in your crates (`cargo clippy -p <crate> -- -D warnings`).
- Web: bun only (no npm, no npx). `bun test` for unit tests, `bun run build` in `web/`.
- SQL in plugin code names its own schema (`snapshots.metric`). Never read another plugin's tables; call its API.
- A plugin backend depends only on its own `api` crate and on `api` crates of plugins in the same or a lower layer that its `plugin.toml` lists in `uses`.
- A plugin frontend imports only `react`, `@arena/design`, `@arena/shell` and its own files.
- Every UI string goes in the plugin's `en.ts` dictionary.
- Public routes return public DTO types only; no rate, group, note, team contact, referral link or hypothesis address in any public response or in the client bundle.
- Secrets come from env (`SOLANA_TRACKER_API_KEY`, `NETWORK_SHEET_CSV_URL`, `FIRST_ADMIN_LOGIN`, `FIRST_ADMIN_PASSWORD`, `PUBLIC_RPC_URL`). Never log them. Never commit anything under `tmp/`.
- External calls in tests: fixtures and fake local servers only. Live Solana Tracker calls: at most the number your brief allows.
- `rm` only with literal paths or `"${VAR:?}"`.
- Test databases: `arena_platform::testing::TestDb` creates and drops its own database from `~/.config/board/postgres.env`.

## F1 Rust foundation

Owns: root `Cargo.toml`, `Cargo.lock`, `backend/`, `platform/core/`, every `plugins/*/plugin.toml`, `plugins/*/api/`, `plugins/*/stub/`, `plugins/*/backend/` skeletons, `plugins/*/migrations/0001_schema.sql` (schema only, no tables), `scripts/site.sh`, `backend/tests/arch.rs`.

Deliver:

- Workspace with members `backend`, `platform/core`, `plugins/*/api`, `plugins/*/backend`. Move `backend/Cargo.lock` to the root. Binary `server` at `target/{debug,release}/server`.
- `arena-platform` as in plan section 3: Person, Tag, Plugin trait, Ctx with Apis type map, Events (sequential async handlers), Slot<T> with default, implementation Registry (app config rules, `off` = stub), page meta hook, per-plugin schema migrations on a detached connection, scheduler with advisory lock on a detached connection, Config with redacted Debug, TestDb.
- `plugin.toml` format (also read by F2's generator):

  ```toml
  name = "snapshots"
  layer = 1
  version = "0.1.0"
  tags = []                       # route tags it serves: public, staff, admin
  uses = []                       # plugin names whose api it calls
  emits = []                      # event names
  listens = []                    # event names
  slots = []                      # slots it declares: "screen:<name>" or "api:<name>"
  fills = []                      # "<plugin>/<slot>"
  widgets = []                    # widgets it offers
  screens = []                    # [{ name, path, tag }]
  ```

  Values copied from the architecture for all 21 plugins: auth, journal, registry, snapshots, score, board, fight, kolexplorer, solanatracker, schedule, radar, roster, wallets, fomo, network, picker, moves, duel, guess, tournament, streaks.
- `api` crate for every plugin that the architecture lists with an API: async trait (`async-trait`) with exactly the agreed operations and DTOs (names in English, meaning from the architecture), plus `stub` module at `plugins/<name>/stub/mod.rs` returning «off» errors.
- Backend skeleton per plugin: implements `Plugin`, registers its api impl (methods `todo!()` are not allowed: return a typed «not built» error), mounts no routes yet, embeds its migrations with a `build.rs`.
- `backend/src/main.rs`: config, plugin list in construction order, apps by tag, scheduler start, SPA fallback with page meta, health route kept (now also reports per-plugin pending migrations).
- `backend/tests/arch.rs`: plugin list equals `plugins/*/plugin.toml`; deps from `cargo metadata` respect layers and `uses`; no cycles; every route registered with a tag; SQL in `plugins/*/backend` names only its own schema.
- `scripts/site.sh` as in plan section 2 and 9 (root bun install, root cargo build, sources `tmp/site.env` for task mode and `$HOME/.config/kol-fight/staging.env` for staging when present).

Acceptance:

- `cargo build` at the root exits 0; `cargo test -p arena-platform -p server` exits 0 with TestDb, events, slots, registry, scheduler lock and migration tests.
- A fresh task database starts the server; `/api/health` returns ok with every plugin's migrations applied.
- `cargo clippy -p arena-platform -p server -- -D warnings` exits 0.

## F2 Web foundation and design system

Owns: root `package.json` and `bun.lock`, `web/`, `platform/design/`, `plugins/*/frontend/` skeletons, `scripts/gen-plugins.ts`, `scripts/gen-tokens.ts`.

Deliver:

- Bun workspaces: `web`, `platform/design`, `plugins/*/frontend`. Package names `@arena/shell` (exports from `web/src/shell`), `@arena/design`, `@arena/<plugin>`.
- Shell: router (history API; overlays `/kol/:id` and `/fight` over `/`; `/team/*` with login redirect `?next=`), frame (ticker slot, sticky header with 4 anchors, phone header per Brief), `<Slot>`, `<Widget>`, page events, dictionary `t()`, `api()` fetch helper that returns `{data} | {error}` and never throws to the screen, session state for `/team`.
- `definePlugin({ name, screens, fills, widgets, dict })`; `scripts/gen-plugins.ts` writes `web/src/plugins.gen.ts` from `plugins/*/plugin.toml`; `bun run build` in `web/` runs it first.
- `@arena/design`: tokens from `DESIGN.md` front matter via `scripts/gen-tokens.ts` into CSS variables; fonts from `@fontsource/press-start-2p`, `@fontsource/golos-text`, `@fontsource/jetbrains-mono` (latin, cyrillic); browser surfaces themed (selection, caret, scrollbar, focus ring); base pieces Button, Field, Select, List, Card, Dialog (focus trap, Esc, focus return), Tabs, Badge, Progress, Tooltip (hover, focus, tap), Skeleton, EmptyState, ErrorLine; KOL Arena pieces PixelAvatar (5x5 mirrored, color from id hash), TierBadge, ScoreBar, Money (short and full, sign, colors, «—» with reason), TimeAgo, SnapshotDate (UTC), ScoreBreakdown, Ticker (pause button, stops on hover and focus, static under reduced motion), HpBar, RoundRow (normal, void, tie), Bracket (tree on desktop, stacked on phone). Gallery screen `/team/design` (staff) renders every piece with sample props.
- Plugin frontend skeleton per plugin: `package.json`, `src/index.ts` with `definePlugin` and an empty `en.ts`.
- Tests (`bun test`): tokens equal `DESIGN.md`; plugins.gen.ts fresh; import rule over `plugins/*/frontend` and `platform/design`; Money, TimeAgo, SnapshotDate formatting; PixelAvatar deterministic; dictionary has every key used.

Acceptance:

- `bun install` at the root and `bun run build` in `web/` exit 0; `bun test` exits 0.
- `/team/design` on the task site shows every piece at 1280 px and 360 px with no horizontal scroll (Playwright screenshot pair saved under `tmp/`).
- The existing `index.html` bug-report widget tag stays.

## A Arena core

Owns: `plugins/{registry,snapshots,score,board,fight}/` (backend, migrations, frontend, tests), `scripts/gen-seed.ts`.

Deliver: plan section 4, layer 1, and the Brief «Дизайн» for Home, KOL card and fight window. Seed migrations generated from `data/prototype/arena_data_v2.json` and `data/prototype/kolexplorer-2026-10-19.html`. Fight image with `resvg`, fonts committed (OFL). Page meta for `/fight`.

Acceptance:

- Score tests on 2026-10-18: Tekkerrss 55, Diamond, rank 18 of 122, and these 25 values from the prototype screenshots (rank, slug or name, Score): 1 Decu 95, 2 Cooker 88, 3 Trunoest 85, 18 Tekkerrss 55, 37 Peter 47, 52 Levis 41, 56 Iced 39, 64 laseasy10 36, 66 BTCFORLIFE 36, 67 juicy 35, 73 Naka 31, 74 trancey 31, 93 shah 23, 101 kook 15, 102 babatooonde 12, 103 TheSOLstice 12, 104 Mrpunk.eth 11, 105 BrazilZZ 11, 107 ocr 10, 108 Smokey 9, 109 Maurits 8, 110 Zagrebian 8, 111 J777Crypto 8, 117 pow 5, 120 NDF_Sonar_trade 0. Full 122-row ranking pinned as a fixture.
- Fight rules tests as in plan section 7.
- Snapshots tests: carry forward, reopen, set_members, copy_members.
- Leak test over `/api/board`, `/api/kol/:id`, `/api/fight`.
- On the task site: `/` shows 122 rows on day 0 with podium Decu, Cooker, Trunoest; `/kol/tekkerrss` shows Score 55 with breakdown; a 3-fighter fight runs by keyboard; `/api/fight/image` returns a 1200x630 PNG.

## B Daily data

Owns: `plugins/{kolexplorer,solanatracker,schedule,radar}/`.

Deliver: plan section 4, layer 2. Fixtures from `data/fixtures/solanatracker/`; KE fixture from `data/prototype/kolexplorer-2026-10-19.html`. Collection screen `/team/collection` with the «другие источники» slot and «Run now» (admin).

Acceptance:

- Parser and client tests with fixtures and a fake server: success, under-50 rows, missing attribute, 401, 403, 429, 5xx, profile failure.
- Schedule test: open, event, KE, ST, close, rank in order; failure in KE copies members; lock held -> «already running»; restart with an open old snapshot closes it.
- Live Solana Tracker calls inside the lane: at most 10, only to confirm a response shape the fixtures lack. The full live run (about 160 calls) happens after the merge, by the coordinator.

## C Network

Owns: `plugins/{roster,wallets,fomo,network,picker}/`.

Deliver: plan section 4, layer 3; team screens Network, Wallets, FOMO match, Picker; client network section.

Acceptance:

- Roster tests: CSV parse with a fixture the lane writes at `plugins/roster/tests/fixtures/sheet.csv`: header `Name,Group,Low $,High $,Deliverable / note,FOMO,Pump.fun`, 6 invented KOLs with invented rates (never real names or rates); header change fails, removed row, keep flag for the 9 prototype KOLs, formats keywords, no rate in any public response or bundle.
- Wallets tests: every transition of the state machine, manual confirm without KOL record refused, conflict on shared wallet, journal rows.
- FOMO tests on a fake RPC: intersection, 0.5% tolerance, Token-2022, error lines.
- On the task site: sheet read works with `tmp/site.env`; Wallets shows counts per status; first wallet pass for the network runs and the counts are recorded in `tmp/wallet-pass.txt` for the Test report.

## D Auth, journal, games

Owns: `plugins/{auth,journal,moves,duel,guess,tournament,streaks}/`.

Deliver: plan section 4, layers 0 and 4; screens Login, Staff; game fillers on Home and in the fight window.

Acceptance:

- Auth tests: argon2 verify, lockout after 5 failures for 15 minutes, session expiry, revoke deletes sessions, role per tag, Origin check on non-GET admin requests.
- Journal tests: record and history.
- Games tests: moves only when both days fresh; duel deterministic per date; tournament with a draw advances the higher rank; streaks appear only with enough history.
- On the task site: login, wrong password message, lockout, logout; staff cannot open `/team/staff`.

## Checks in a lane worktree vs after the merge

- In a lane worktree: the lane's crate tests and `bun test`, `cargo build`, `bun run build`, and a local run of the server binary on a free port against a `TestDb`-style throwaway database to click through its own screens. Other lanes' plugins answer «not built»; screens show their error or empty states.
- After the merge, by the coordinator: everything marked «on the task site», live runs, E2E.

## E2E (after merge)

Playwright against the task site: plan section 7 list. Screenshots at 1280 and 360 px for Home, card, fight, network, Wallets, Collection, saved under `tmp/e2e/`.

## Definition of done

1. `cargo build` and `bun run build` exit 0 at the merged head.
2. Each touched crate's tests and `bun test` exit 0; E2E list passes on the task site.
3. Day-0 board matches the prototype: 122 rows, Tekkerrss 55 Diamond rank 18.
4. One live daily run completed on the task site; collection screen shows both sources ok.
5. Leak tests pass; bundle grep finds no internal field names or fixture rates.
6. No horizontal scroll at 360 px on every screen.

## Rollback

Revert the merge on staging; plugin schemas stay (migrations additive). Nothing outside this repo changes except the staging secrets file, which only the owner's yes creates.

## Out of scope

Light theme, RU strings, Telegram alert, production, own PnL from chain, live streams.
