#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Proves the production compose stack: builds it, runs the whole flow against the real containers (scripts/e2e.py),
# and tries from inside a worker everything a compromised job would try (decision N30). Everything runs under its
# own compose project and is removed at the end, volumes included, so it never touches a real deployment.
# Needs Docker, a kimcad checkout next to this one, and its M5 fixture (`pixi run m5` in kimcad).
#   KEEP=1 ./scripts/stack-check.sh    leaves the stack running afterwards
set -e
here="$(cd "$(dirname "$0")/.." && pwd)"
cd "$here"
kim="${KIMBIM_HOME:-$here/../kimcad}"
port="${STACK_PORT:-3200}"
env_file="$(mktemp)"
cat > "$env_file" <<EOF
NICGIT_DB_PASSWORD=stack-check-db-$$
NICGIT_WORKER_TOKEN=stack-check-pool-$$
NICGIT_PUBLIC_URL=http://localhost:$port
NICGIT_PORT=$port
NICGIT_WORKERS=2
NICGIT_WORKER_MEMORY=4g
EOF
dc() { docker compose -p nicgit-stackcheck -f deploy/compose.yaml --env-file "$env_file" "$@"; }
cleanup() {
  status=$?
  if [ "$status" -ne 0 ]; then
    echo "--- server log"; dc logs --tail 40 server 2>&1 | cut -c1-240 || true
    echo "--- worker log"; dc logs --tail 40 worker 2>&1 | cut -c1-240 || true
  fi
  if [ "${KEEP:-0}" != 1 ]; then dc down -v --remove-orphans >/dev/null 2>&1 || true; fi
  rm -f "$env_file"
  exit "$status"
}
trap cleanup EXIT INT TERM

echo "== build and start"
dc up -d --build --quiet-pull 2>&1 | tail -3
i=0
until curl -sf "http://localhost:$port/api/health" >/dev/null; do
  i=$((i + 1))
  [ "$i" -lt 90 ] || { echo "FAIL: the server did not answer"; exit 1; }
  sleep 1
done

echo "== isolation, from inside a worker"
dc exec -T worker python - <<'EOF'
import os, pathlib, socket, sys

failures = []

def check(name, ok):
    print(("  ok    " if ok else "  FAIL  ") + name)
    if not ok:
        failures.append(name)

def connects(host, port):
    try:
        socket.create_connection((host, port), timeout=4).close()
        return True
    except OSError:
        return False

def unreadable(path):
    try:
        pathlib.Path(path).read_bytes()
        return False
    except OSError:
        return True

def unwritable(path):
    try:
        pathlib.Path(path).write_text("x")
        return False
    except OSError:
        return True

check("the server is reachable", connects("server", 3000))
check("no shared data folder", not pathlib.Path("/var/lib/nicgit/data").exists())
check("the database is not reachable", not connects("db", 5432))
check("the internet is not reachable", not connects("1.1.1.1", 443))
check("the disk is read-only", unwritable("/opt/nicgit/planted.py") and unwritable("/tmp/planted"))
check("the job folder is writable", not unwritable("/work/probe"))
os.unlink("/work/probe")
check("not root", os.getuid() != 0)
check("no capabilities", pathlib.Path("/proc/self/status").read_text().split("CapEff:")[1].split()[0].strip("0") == "")
check("the supervisor's environment (pool token) is not readable", unreadable("/proc/1/environ"))
check("the supervisor's memory is not readable", unreadable("/proc/1/mem"))
sys.exit(1 if failures else 0)
EOF

echo "== the whole flow, through the containers"
NICGIT_E2E_USER="stack$$"
NICGIT_E2E_TOKEN="$(dc exec -T -e NICGIT_DEV=1 server nicgit-server dev-user --slug "$NICGIT_E2E_USER" --email "$NICGIT_E2E_USER@example.com" | tr -d '\r')"
export NICGIT_E2E_USER NICGIT_E2E_TOKEN
python scripts/e2e.py "http://localhost:$port" "$kim/build/m5/project/.git"

echo "== nothing left behind"
for w in $(dc ps -q worker); do
  left="$(docker exec "$w" sh -c 'ls -A /work | wc -l' | tr -d '\r ')"
  [ "$left" = 0 ] || { echo "FAIL: $left item(s) left in a worker's /work"; exit 1; }
done
echo "  ok    every worker's job folder is empty"

echo "== size"
docker images --format '  {{.Repository}} {{.Size}}' | grep nicgit-stackcheck || true
docker stats --no-stream --format '  {{.Name}} {{.MemUsage}}' $(dc ps -q) || true
echo "stack check OK"
