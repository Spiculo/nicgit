#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Local preview: Postgres, the server serving web/dist, one engine worker, and the demo data.
# Sign in without Google at /api/dev/login (NICGIT_DEV=1, local only).
#   ./scripts/preview.sh          start or restart everything
#   ./scripts/preview.sh stop     stop the server, the worker and the database
set -e
here="$(cd "$(dirname "$0")/.." && pwd)"
cd "$here"

if [ "${1:-}" = stop ]; then
    pkill -f 'target/debug/nicgit-server' 2>/dev/null || true
    pkill -f 'python -m nicgit_worker' 2>/dev/null || true
    docker compose -f deploy/compose.dev.yaml down 2>/dev/null || true
    echo "preview stopped"
    exit 0
fi

pkill -f 'target/debug/nicgit-server' 2>/dev/null || true
pkill -f 'python -m nicgit_worker' 2>/dev/null || true
mkdir -p data
docker compose -f deploy/compose.dev.yaml up -d --wait

export NICGIT_DEPLOYMENT="${NICGIT_DEPLOYMENT:-self-hosted}"
export NICGIT_WORKER_TOKEN="${NICGIT_WORKER_TOKEN:-local-dev-token}"
export NICGIT_DATA_DIR="${NICGIT_DATA_DIR:-$here/data}"
export NICGIT_WEB_DIR="${NICGIT_WEB_DIR:-$here/web/dist}"
export NICGIT_DATABASE_URL="${NICGIT_DATABASE_URL:-postgres://nicgit:nicgit@127.0.0.1:5434/nicgit}"
export NICGIT_DEV=1
export NICGIT_BIND=127.0.0.1:3000
export NICGIT_INTERNAL_BIND=127.0.0.1:3001
export NICGIT_PUBLIC_URL=http://127.0.0.1:3000
export NICGIT_SIGNUP_DOMAINS=example.com
export NICGIT_SERVER_BIN="$here/target/debug/nicgit-server"

cargo build -q -p nicgit-server
nohup "$NICGIT_SERVER_BIN" > data/server.log 2>&1 &
for i in $(seq 1 60); do curl -sf http://127.0.0.1:3000/api/health >/dev/null && break; sleep 1; done

NICGIT_URL=http://127.0.0.1:3001 NICGIT_WORKER_TOKEN="$NICGIT_WORKER_TOKEN" \
    nohup sh scripts/engine-python.sh -m nicgit_worker > data/worker.log 2>&1 &

# Seed once. `dev-user` is idempotent; the project check decides whether to push the fixture.
ANA=$("$NICGIT_SERVER_BIN" dev-user --slug ana --email ana@example.com --name "Ana Ferreira" 2>/dev/null || true)
if [ -n "$ANA" ] && ! curl -sf -H "authorization: Bearer $ANA" \
    http://127.0.0.1:3000/api/projects/studio/sample-house >/dev/null 2>&1; then
    sh scripts/seed-demo.sh > data/seed.log 2>&1 || { echo "seed failed, see data/seed.log"; exit 1; }
fi

echo "NicGit preview: http://127.0.0.1:3000/api/dev/login?slug=ana"
echo "Logs: data/server.log, data/worker.log; stop with: $0 stop"
