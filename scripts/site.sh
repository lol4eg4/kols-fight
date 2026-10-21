#!/usr/bin/env bash
# Usage: scripts/site.sh staging|task|teardown   (run from the repo root; staging and task need env PORT)
set -euo pipefail

mode=${1:-}
case "$mode" in
  staging|task|teardown) ;;
  *) echo "usage: scripts/site.sh staging|task|teardown" >&2; exit 2 ;;
esac

pg_env=${BOARD_PG_ENV:-$HOME/.config/board/postgres.env}
if [ ! -f "$pg_env" ] || [ ! -r "$pg_env" ]; then
  echo "site.sh: no Postgres connection file $pg_env. An administrator runs /srv/projects/vmsetup/board/deploy/boardagent-setup.sh as root once." >&2
  exit 1
fi
set -a
# shellcheck disable=SC1090
. "$pg_env"
set +a

if [ "$mode" = staging ]; then
  db=kol_fight_staging
else
  base=$(basename "$PWD")
  base=${base,,}
  db=kol_fight_wt_${base//[^a-z0-9]/_}
  if [ "${#db}" -gt 63 ]; then
    db=${db:0:54}_$(printf %s "$db" | sha256sum | cut -c1-8)
  fi
fi

if [ "$mode" = teardown ]; then
  dropdb --if-exists "$db"
  exit 0
fi

: "${PORT:?site.sh: env PORT is required}"

if [ -z "$(psql -d postgres -Atc "select 1 from pg_database where datname='$db'")" ]; then
  createdb "$db"
fi

(cd web && bun install --frozen-lockfile && bun run build)
if [ "$mode" = staging ]; then
  cargo build --release --locked -p server
  bin=$PWD/target/release/server
else
  cargo build --locked -p server
  bin=$PWD/target/debug/server
fi

# Plugin settings (API keys, sheet URL, first admin). Optional: missing settings show as
# "not configured" in the blocks that need them.
if [ "$mode" = staging ]; then
  site_env=$HOME/.config/kol-fight/staging.env
else
  site_env=$PWD/tmp/site.env
fi
if [ -f "$site_env" ]; then
  set -a
  # shellcheck disable=SC1090
  . "$site_env"
  set +a
fi

COMMIT=$(git rev-parse HEAD)
export PORT COMMIT
export DATABASE_URL="postgres://$PGUSER:$PGPASSWORD@$PGHOST:$PGPORT/$db"
export WEB_DIST=$PWD/web/dist
exec "$bin"
