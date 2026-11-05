# SPDX-License-Identifier: AGPL-3.0-or-later
"""`python -m nicgit_worker`: the supervisor. It claims jobs from a NicGit server and runs each in a new process.

Environment: NICGIT_URL (default http://127.0.0.1:3000), NICGIT_WORKER_TOKEN (required).
Run one worker per CPU you want to give the engine; each runs one job at a time.

The supervisor holds the pool token, which can claim any customer's job, so it never touches engine code or
job files. The job's process gets neither the token nor a way to read it from this one (decision N30)."""
from __future__ import annotations

import argparse
import json
import logging
import os
import subprocess
import sys
import time
import urllib.error

log = logging.getLogger("nicgit_worker")

CHILD = [sys.executable, "-m", "nicgit_worker.child"]
KINDS = ["checks", "combine", "compare", "element_history", "export_ifc", "export_kim", "import_ifc", "import_kim",
         "merge_preview", "settings_diff", "summary"]


def lock_down() -> bool:
    """Linux: stop a same-user process reading this one's memory or environment through /proc or ptrace.
    An exec resets this, so jobs are unaffected. Elsewhere there is nothing to do; production is Linux."""
    if not sys.platform.startswith("linux"):
        return False
    import ctypes

    pr_set_dumpable = 4
    return ctypes.CDLL(None, use_errno=True).prctl(pr_set_dumpable, 0, 0, 0, 0) == 0


def child_env(env) -> dict:
    return {k: v for k, v in env.items() if k != "NICGIT_WORKER_TOKEN"}


def run_in_child(url: str, job: dict) -> None:
    done = subprocess.run(CHILD, input=json.dumps({"url": url, "job": job}).encode(), env=child_env(os.environ), check=False)
    if done.returncode == 0:
        return
    # It died before it could say so (out of memory, a crash in native code). Its token is still good.
    log.error("%s %s: its process ended with %s", job["kind"], job["id"], done.returncode)
    from .client import JobClient

    try:
        JobClient(url, job["id"], job["token"]).finish(error="the engine stopped unexpectedly while working on this")
    except (urllib.error.URLError, OSError) as e:
        log.warning("could not report it: %s", e)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(prog="nicgit_worker")
    ap.add_argument("--url", default=os.environ.get("NICGIT_URL", "http://127.0.0.1:3000"))
    ap.add_argument("--kinds", default=os.environ.get("NICGIT_WORKER_KINDS", ""), help="comma list (default: all)")
    ap.add_argument("--once", action="store_true", help="exit when no job is waiting")
    ap.add_argument("--idle", type=float, default=1.0, help="seconds to wait when the queue is empty")
    ap.add_argument("--exit-if-unreachable", action="store_true",
                    help="exit when the server goes away, rather than wait for it to return (for tests)")
    args = ap.parse_args(argv)
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(name)s %(message)s")
    token = os.environ.get("NICGIT_WORKER_TOKEN")
    if not token:
        ap.error("NICGIT_WORKER_TOKEN is required")
    lock_down()
    from .client import PoolClient

    kinds = [k for k in args.kinds.split(",") if k] or KINDS
    client = PoolClient(args.url, token)
    log.info("serving %s for %s", ", ".join(kinds), args.url)
    while True:
        try:
            job = client.claim(kinds)
        except (urllib.error.URLError, OSError) as e:
            log.warning("server unreachable: %s", e)
            job = None
            if args.once or args.exit_if_unreachable:
                return 1
        if job:
            run_in_child(args.url, job)
            continue
        if args.once:
            return 0
        time.sleep(args.idle)


if __name__ == "__main__":
    raise SystemExit(main())
