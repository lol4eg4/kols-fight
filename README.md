# KOL Fight

- `web/` React + Vite + TypeScript SPA (bun). Rust cargo workspace at the root (`Cargo.toml`,
  `Cargo.lock`, build output in `target/`):
  - `platform/core` (`arena-platform`): plugin trait, APIs, events, slots, implementation registry,
    per-plugin migrations, scheduler with advisory locks, config, `testing::TestDb`.
  - `plugins/<name>/`: `plugin.toml`, `api/` (crate `api-<name>`: trait, DTOs, errors; `stub/` is the
    `off` implementation), `backend/` (crate `plugin-<name>`), `migrations/` (schema `<name>`).
  - `backend/` (binary `server`): puts the plugins together and serves `/api/*` and the SPA.
- The server serves `/api/*` and the built SPA (`web/dist`, env `WEB_DIST`). Env: `PORT`, `DATABASE_URL`, `WEB_DIST`.
- `scripts/site.sh staging|task|teardown` (from the repo root): builds web and backend, creates the
  database `kol_fight_staging` (staging) or `kol_fight_wt_<folder>` (task), runs the server on `$PORT`.
  `teardown` drops the task database.
- Plugin settings come from env (`SOLANA_TRACKER_API_KEY`, `NETWORK_SHEET_CSV_URL`, `FIRST_ADMIN_LOGIN`,
  `FIRST_ADMIN_PASSWORD`, `PUBLIC_RPC_URL`); `site.sh` sources `tmp/site.env` (task) or
  `~/.config/kol-fight/staging.env` (staging) when present.
- Migrations: a plugin adds `plugins/<name>/migrations/NNNN_name.sql` (runs in schema `<name>`);
  app-wide ones go in `backend/migrations/`. All run at server start.
- `GET /api/health` returns `{"ok":true,"db":true,...}` with pending migration counts per plugin.
- Tests (from the root; always name crates): `cargo test -p arena-platform -p server`
  (needs `~/.config/board/postgres.env`; test databases are `kol_fight_test_<random>`, dropped after),
  `cargo test -p plugin-<name>`; `cd web && bun run build`.
- `index.html` carries the board's bug-report widget tag. Keep it.
