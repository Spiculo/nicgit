#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Local development: Postgres (docker), the server on :3000 serving web/dist, and one engine worker.
set -e
here="$(cd "$(dirname "$0")/.." && pwd)"
cd "$here"
export NICGIT_DEPLOYMENT="${NICGIT_DEPLOYMENT:-self-hosted}"
export NICGIT_WORKER_TOKEN="${NICGIT_WORKER_TOKEN:-dev-worker-token}"
export NICGIT_DATA_DIR="${NICGIT_DATA_DIR:-$here/data}"
export NICGIT_WEB_DIR="${NICGIT_WEB_DIR:-$here/web/dist}"
docker compose -f deploy/compose.dev.yaml up -d --wait
kim="${NICBIM_HOME:-${KIMBIM_HOME:-$here/../nicbim}}"
pinned="$(grep -E '^[0-9a-f]{40}$' engine.rev | head -1)"
[ "$(git -C "$kim" rev-parse HEAD 2>/dev/null)" = "$pinned" ] ||
  echo "note: $kim is not at the engine commit releases use ($pinned, engine.rev)"
export CARGO_TARGET_DIR="$here/target/dev"
cargo build -p nicgit-server
"$CARGO_TARGET_DIR/debug/nicgit-server" &
server=$!
trap 'kill $server 2>/dev/null' EXIT INT TERM
sleep 2
NICGIT_URL=http://127.0.0.1:3001 ./scripts/engine-python.sh -m nicgit_worker
