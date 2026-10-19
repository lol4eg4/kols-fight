# KOL Arena: scope review notes (engineering)

Input for Architecture, Design and Plan. The Brief (`kol-arena.md`) is the source of truth for scope; this file records the engineering findings of the scope review. Mode: selective expansion, baseline = Brief variant B.

## System sketch

```
 Google Sheet (network roster, rates) ──daily──┐
 KOL Explorer leaderboard HTML ─────────daily──┤
 Solana Tracker Data API (key) ─────────daily──┤
                                               v
                                   Collector job (06:00 UTC, advisory lock,
                                   idempotent per snapshot date)
                                               │ writes
                                               v
   Postgres: kols, wallets(status, source, sent_by, at), snapshots,
             kol_metrics(snapshot, kol, src, fields...), source_runs,
             network_private(group, rate_lo, rate_hi, note)   <- admin only
                                               │
                         Score engine (pure Rust fn: parts, score, tier, badges, rank)
                                               │
              ┌────────────────────────────────┴───────────────────────────┐
              v                                                            v
   Public API (allowlisted DTOs only)                       Admin API (session auth)
   /api/board, /api/kol/:slug, /api/fight?ids=,             wallets, FOMO helper,
   /api/share/fight.png                                     rates export, source health
              v                                                            v
   React client page (EN)                                   React admin pages
                                                            FOMO helper ──> Solana public RPC
                                                            (getProgramAccounts, mint memcmp,
                                                             Token + Token-2022)
```

- Day 0 snapshot: `arena_data_v2.json` ships as a seeder (public fields only; it holds no rates).
- The sheet URL and the Solana Tracker key are secrets: env only, never in git. Rates live only in the database.
- Fight resolution runs on the server (one implementation) so the page, the share link and the X image agree.

## Data flow shadow paths

- KOL Explorer fetch: timeout or non-200 -> source run failed, previous metrics kept with their old date. HTML changed (fewer than 50 rows parsed, or required attributes missing) -> failed, same handling. Never write a partial top-100.
- Solana Tracker: 401 (bad key), 403 (plan lacks PnL v2), 429 (quota), 5xx -> per-wallet failure recorded; KOL keeps previous metrics; run marked partial. 403 on PnL v2 means the free plan is not enough: surface in admin, ask the CEO about the paid plan.
- Google Sheet: fetch fails or header row changed -> roster unchanged, run failed. Rows removed from the sheet: KOL leaves the network (kept in history), never hard-deleted.
- Wallet statuses: hypothesis never reaches public DTOs. Confirmed wallet shared by two KOLs -> admin conflict, neither published until resolved.
- Stale: last successful metrics older than 48 h -> public "data is stale" mark.

## Wallet state machine

```
  not_found ──(handle match on name only / FOMO helper unique match)──> hypothesis
  not_found ──(exact X handle match | address from KOL | pump.fun/KOL Explorer profile)──> confirmed
  hypothesis ──(KOL confirms, staff records it)──> confirmed
  hypothesis ──(rejected)──> not_found
  confirmed ──(staff revokes)──> not_found      (audit log row on every change)
```

## Security

- Admin: individual accounts, argon2 hashes, HttpOnly SameSite=Strict session cookie, login rate limit, audit log of wallet and roster changes.
- Leak guard: separate public and admin DTO types; test asserts public JSON keys are a subset of an allowlist; test greps the built bundle for internal field names and rate values from fixtures.
- Input: fight ids and slugs validated against known KOLs; FOMO helper takes a base58 mint (32 bytes) and a decimal amount; external strings rendered as text only.
- Secrets: Solana Tracker key, sheet URL, first admin password via env.

## Edge cases to test

- Null metrics in any Score part; KOL with all nulls; d30 <= 0 disables accuracy.
- Fight: 2-4 fighters, duplicate pick refused, unknown id in a share link, all rounds void, tie on rounds -> Score tiebreak, tie on Score too.
- Rank ties: Score desc, then d30 desc, then name for determinism.
- Job runs twice the same day -> second run updates the same snapshot.
- KOL drops out of KOL Explorer top-100 and is not network -> leaves the board.

## Tests

- Score engine: control example (Tekkerrss 55, Diamond, #18) and the 25 screenshot values; full 122-row recompute pinned to the day-0 snapshot.
- Fight rules unit tests (void, tie split, all-win void, tiebreak, counted-rounds line).
- Parsers on saved fixtures: KOL Explorer HTML, Solana Tracker JSON, sheet CSV with fake rates.
- Leak tests (API and bundle).
- E2E (Playwright): board, filters, card, fight modal by keyboard, 360 px without horizontal scroll, reduced motion, share link.

## Observability

- `source_runs`: source, started, finished, status, rows, error text. Admin page shows last success per source.
- Structured log lines per run and per failed wallet.

## Deployment

- Migrations additive only. Seeder idempotent.
- Staging needs env for the key, the sheet URL and the first admin password. The staging start script reads only PORT, DATABASE_URL, WEB_DIST today: decide in Plan how secrets reach staging (ask before touching anything outside the repo).
- Rollback: revert the merge; data tables stay.

## Long term

- Russian later: keep every UI string in one dictionary from day one.
- Variant C (own PnL from chain) stays possible: metrics carry `src`.

## NOT in scope

- Russian UI (CEO, 2026-10-19).
- Automated reading of FOMO, GMGN, Kolscan, Solscan (ToS).
- Paid APIs (Solscan, TwitterScore).
- Push alerts to Telegram or email on collection failure: admin page shows it instead.
- Production deploy.

## Design review (2026-10-19)

Person-facing decisions: Brief, section «Дизайн». Tokens: `DESIGN.md`. Score: 5/10 before, 9/10 after (no mockups: the owner's prototype is the visual reference; no new look).

What already exists: `DESIGN.md` (from the prototype), the 6 prototype screenshots, architecture section 5 (design-system pieces: pixel avatar, tier chip, Score bar, money, Score breakdown, ticker, HP bar, round row, bracket).

NOT in scope: light theme; new mockups; RU strings (one dictionary now, translation later); pink halo behind the fight section (dropped); big hero mascot (replaced by Duel of the day, coin stays as logo).

Implementation tasks:

- T1 (P1) Design tokens: CSS variables from `DESIGN.md` front matter; self-hosted fonts via `@fontsource/press-start-2p`, `@fontsource/golos-text`, `@fontsource/jetbrains-mono` (latin + cyrillic); theme selection, caret, scrollbars, focus ring from the palette.
- T2 (P1) Overlay routing: `/kol/:id` and `/fight?ids=&date=` over `/`; ×, Esc and back close; direct entry closes to `/`; focus trap and focus return.
- T3 (P1) Leaderboard: table at 1100px+, table minus 24h and position at 760–1099px, list of cards below 760px (list semantics, name as heading, sort order in list label); per-row source mark with time tooltip; per-source snapshot line under the table.
- T4 (P1) States: skeleton rows; board load error with Try again; empty search with Clear filters; per-KOL stale chip; whole-snapshot-late banner; radar quiet vs not updated; tooltips on hover, focus and tap; focusable disabled buttons with visible reason.
- T5 (P1) Fight window: VS with optional guess; round card (what and why, values with source and date, bars, ★, why line, HP damage); void, tie, all-win rounds; K.O. with counted rounds, Score tie-break, Draw; guess result and tally (hidden without storage); Where Score comes from side by side, per part on phone; Share panel (Copy link, Post on X, Download image); 1200×630 image; aria-live round results; reduced motion.
- T6 (P2) Phone header: icon logo + Board, Network, Fight, Score in one row at 360px; skip links; ticker pause.
- T7 (P2) Tournament: manual pick, Top-8, Our network (disabled under 8 with reason); tree on desktop, stacked rounds on phone; draw advances the higher-ranked KOL.
- T8 (P2) Team section: left menu, dropdown on phone; start page Wallets with status counters; login error, lockout and expired-session states; FOMO match result lines.
