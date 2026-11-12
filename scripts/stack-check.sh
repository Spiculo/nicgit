#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Proves the production compose stack, as a self-hosted server: builds it from this working tree and the kimcad
# working tree as they are (uncommitted changes included, through scripts/snapshot.sh), runs the whole
# flow against the real containers (scripts/e2e.py), and tries from inside a worker everything a compromised job
# would try (decision N30). Everything runs under its own compose project and is removed at the end, volumes
# included, so it never touches a real deployment.
# Needs Docker, a kimcad checkout next to this one, and its M5 fixture (`pixi run m5` in kimcad).
#   KEEP=1 ./scripts/stack-check.sh    leaves the stack running afterwards
set -e
here="$(cd "$(dirname "$0")/.." && pwd)"
cd "$here"
kim="${KIMBIM_HOME:-$here/../kimcad}"
# Docker on Windows needs Windows paths, not Git Bash ones.
native() { case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) cygpath -w "$1" ;; *) printf '%s\n' "$1" ;; esac; }
port="${STACK_PORT:-3200}"
env_file="$(mktemp)"
src="$(mktemp -d)"
"$here/scripts/snapshot.sh" "$here" --work "$src/nicgit"
"$here/scripts/snapshot.sh" "$kim" --work "$src/kimcad"
cat > "$env_file" <<EOF
NICGIT_DEPLOYMENT=self-hosted
NICGIT_VERSION=stack-check
NICGIT_IMAGE_PREFIX=nicgit-stackcheck
NICGIT_SOURCE_CONTEXT=$(native "$src/nicgit")
NICGIT_ENGINE_CONTEXT=$(native "$src/kimcad")
NICGIT_DB_PASSWORD=stack-check-db-$$
NICGIT_WORKER_TOKEN=stack-check-pool-$$
NICGIT_PUBLIC_URL=http://localhost:$port
NICGIT_PORT=$port
NICGIT_WORKERS=2
NICGIT_WORKER_MEMORY=4g
EOF
dc() { docker compose -p nicgit-stackcheck -f deploy/compose.yaml -f deploy/compose.build.yaml --env-file "$env_file" "$@"; }
cleanup() {
  status=$?
  if [ "$status" -ne 0 ]; then
    echo "--- server log"; dc logs --tail 40 server 2>&1 | cut -c1-240 || true
    echo "--- worker log"; dc logs --tail 40 worker 2>&1 | cut -c1-240 || true
  fi
  if [ "${KEEP:-0}" != 1 ]; then dc down -v --remove-orphans >/dev/null 2>&1 || true; fi
  rm -rf "$env_file" "$src"
  exit "$status"
}
trap cleanup EXIT INT TERM

echo "== build and start"
dc up -d --build --quiet-pull > "$src/up.log" 2>&1 || { tail -40 "$src/up.log"; echo "FAIL: the stack did not build or start"; exit 1; }
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

check("the server's workers address is reachable", connects("server", 3001))
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

echo "== the workers' API, from outside"
code="$(curl -s -o /dev/null -w '%{http_code}' -X POST -H "X-Worker-Token: stack-check-pool-$$" -H 'Content-Type: application/json' -d '{"kinds":[]}' "http://localhost:$port/internal/jobs/claim")"
case "$code" in
  404|405) echo "  ok    not on the public port ($code)" ;;
  *) echo "  FAIL  the public port answered $code"; exit 1 ;;
esac

echo "== something a job leaves behind, to see it gone after the next job"
for w in $(dc ps -q worker); do
  docker exec -d "$w" sleep 3000
  docker exec "$w" sh -c 'echo x > /work/left-by-a-job; echo x > /dev/shm/left-by-a-job'
done

echo "== the whole flow, through the containers"
NICGIT_E2E_USER="stack$$"
NICGIT_E2E_TOKEN="$(dc exec -T -e NICGIT_DEV=1 server nicgit-server dev-user --slug "$NICGIT_E2E_USER" --email "$NICGIT_E2E_USER@example.com" | tr -d '\r')"
export NICGIT_E2E_USER NICGIT_E2E_TOKEN
python scripts/e2e.py "http://localhost:$port" "$kim/build/m5/project/.git"

echo "== nothing left behind"
# A worker clears everything when a job ends, and that can stop a command run here: let the queue go quiet first.
sleep 5
worked=0
for w in $(dc ps -q worker); do
  # A worker that never got a job has had no reason to clear.
  docker logs "$w" 2>&1 | grep -q ' done in ' || continue
  worked=$((worked + 1))
  left="$(docker exec "$w" sh -c 'ls -A /work /dev/shm | grep -c left-by-a-job; true' | tr -d '\r ')"
  [ "$left" = 0 ] || { echo "FAIL: a job's files survived into the next job"; exit 1; }
  running="$(docker exec "$w" sh -c 'grep -l "^sleep" /proc/[0-9]*/cmdline 2>/dev/null | wc -l' | tr -d '\r ')"
  [ "$running" = 0 ] || { echo "FAIL: a process left by a job survived into the next job"; exit 1; }
done
[ "$worked" -gt 0 ] || { echo "FAIL: no worker ran a job"; exit 1; }
echo "  ok    $worked worker(s) cleared what a job left: no files, no processes"

echo "== size"
docker images --format '  {{.Repository}} {{.Size}}' | grep nicgit-stackcheck || true
docker stats --no-stream --format '  {{.Name}} {{.MemUsage}}' $(dc ps -q) || true
echo "stack check OK"
