#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# End to end on a throw-away database and data folder: server on :3100, one engine worker, scripts/e2e.py.
# Needs the dev Postgres (deploy/compose.dev.yaml) and kimcad's M5 fixture (`pixi run m5` in kimcad).
set -e
here="$(cd "$(dirname "$0")/.." && pwd)"
cd "$here"
kim="${KIMBIM_HOME:-$here/../kimcad}"
db="nicgit_e2e_$$"
data="$(mktemp -d)"
docker compose -f deploy/compose.dev.yaml up -d --wait >/dev/null
docker compose -f deploy/compose.dev.yaml exec -T db psql -U nicgit -q -c "CREATE DATABASE $db" >/dev/null
export CARGO_TARGET_DIR="$here/target/e2e"
cargo build -q -p nicgit-server
export NICGIT_DATABASE_URL="postgres://nicgit:nicgit@127.0.0.1:5434/$db" NICGIT_DATA_DIR="$data" NICGIT_BIND=127.0.0.1:3100 \
       NICGIT_WORKER_TOKEN=e2e-token NICGIT_PUBLIC_URL=http://127.0.0.1:3100
"$CARGO_TARGET_DIR/debug/nicgit-server" > "$data/server.log" 2>&1 &
server=$!
NICGIT_URL=http://127.0.0.1:3100 ./scripts/engine-python.sh -m nicgit_worker --idle 0.3 > "$data/worker.log" 2>&1 &
worker=$!
cleanup() {
  kill $server $worker 2>/dev/null || true
  [ -x scripts/dev-stop.sh ] && true
  docker compose -f deploy/compose.dev.yaml exec -T db psql -U nicgit -q -c "DROP DATABASE IF EXISTS $db WITH (FORCE)" >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM
for i in $(seq 1 60); do curl -sf http://127.0.0.1:3100/api/health >/dev/null && break; sleep 1; done
export NICGIT_E2E_USER="e2e$$"
NICGIT_E2E_TOKEN="$(NICGIT_DEV=1 "$CARGO_TARGET_DIR/debug/nicgit-server" dev-user --slug "$NICGIT_E2E_USER" --email "$NICGIT_E2E_USER@example.com")"
export NICGIT_E2E_TOKEN
python scripts/e2e.py http://127.0.0.1:3100 "$kim/build/m5/project/.git"
