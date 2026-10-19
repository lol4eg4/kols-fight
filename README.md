# KOL Fight

- `web/` React + Vite + TypeScript SPA (bun). `backend/` Rust axum + sqlx (Postgres) server.
- The server serves `/api/*` and the built SPA (`web/dist`, env `WEB_DIST`). Env: `PORT`, `DATABASE_URL`, `WEB_DIST`.
- `scripts/site.sh staging|task|teardown` (from the repo root): builds web and backend, creates the
  database `kol_fight_staging` (staging) or `kol_fight_wt_<folder>` (task), runs the server on `$PORT`.
  `teardown` drops the task database.
- Migrations: add `backend/migrations/NNNN_name.sql`; they run at server start.
- `GET /api/health` returns `{"ok":true,"db":true}`.
- Tests: `cd backend && cargo test`; `cd web && bun run build`.
- `index.html` carries the board's bug-report widget tag. Keep it.
