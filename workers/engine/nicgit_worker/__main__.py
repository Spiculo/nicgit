# SPDX-License-Identifier: AGPL-3.0-or-later
"""`python -m nicgit_worker`: the supervisor. It claims jobs from a NicGit server and runs each in a new process.

Environment: NICGIT_URL (default http://127.0.0.1:3001, the server's internal address), NICGIT_WORKER_TOKEN
(required), NICGIT_JOB_TIMEOUT_MINUTES (default 60), NICGIT_WORKER_CONTAINED=1 when alone in a container.
Run one worker per CPU you want to give the engine; each runs one job at a time.

The supervisor holds the pool token, which can claim any customer's job, so it never touches engine code or
job files. The job's process gets neither the token nor a way to read it from this one (decision N30)."""
from __future__ import annotations

import argparse
import json
import logging
import os
import pathlib
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import urllib.error

log = logging.getLogger("nicgit_worker")

CHILD = [sys.executable, "-m", "nicgit_worker.child"]
JOB_TIMEOUT_SECONDS = float(os.environ.get("NICGIT_JOB_TIMEOUT_MINUTES", "60")) * 60
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


def _kill_group(pid: int) -> None:
    if os.name == "posix":
        try:
            os.killpg(pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass


def run_in_child(url: str, job: dict, timeout: float = JOB_TIMEOUT_SECONDS) -> None:
    # Its own process group, so what it started can be stopped with it.
    group = {"start_new_session": True} if os.name == "posix" else {}
    proc = subprocess.Popen(CHILD, stdin=subprocess.PIPE, env=child_env(os.environ), **group)
    message = "the engine stopped unexpectedly while working on this"
    try:
        proc.communicate(json.dumps({"url": url, "job": job}).encode(), timeout=timeout)
    except subprocess.TimeoutExpired:
        # It reports in for as long as it runs, so a model the engine never finishes would hold this worker forever.
        message = f"this took longer than the {round(timeout / 60)} minutes a job may run"
        proc.kill()
        proc.wait()
    _kill_group(proc.pid)
    if proc.returncode == 0:
        return
    # It died before it could say so (out of memory, a crash in native code). Its token is still good.
    log.error("%s %s: its process ended with %s", job["kind"], job["id"], proc.returncode)
    from .client import JobClient

    try:
        JobClient(url, job["id"], job["token"]).finish(error=message)
    except (urllib.error.URLError, OSError) as e:
        log.warning("could not report it: %s", e)


def clear_after_job(env=None, pid: int | None = None) -> bool:
    """Leave nothing of one job for the next, which may be another customer's. A process a job left running, under
    the same user, could otherwise read the next job's token and files from /proc. A new process group is not enough,
    since a process can leave its group, so in a container, where this is process 1 and everything else is a job's,
    everything else stops. Elsewhere this would stop everything the user is running, so it needs both conditions."""
    env = os.environ if env is None else env
    pid = os.getpid() if pid is None else pid
    if env.get("NICGIT_WORKER_CONTAINED") != "1" or pid != 1:
        return False
    try:
        os.kill(-1, signal.SIGKILL)
    except ProcessLookupError:
        pass
    try:
        while os.waitpid(-1, 0)[0] > 0:
            pass
    except ChildProcessError:
        pass
    # All that a job can write: the rest of the disk is read-only.
    for folder in (tempfile.gettempdir(), "/dev/shm"):
        for entry in pathlib.Path(folder).iterdir():
            if entry.is_dir() and not entry.is_symlink():
                shutil.rmtree(entry, ignore_errors=True)
            else:
                entry.unlink(missing_ok=True)
    return True


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(prog="nicgit_worker")
    ap.add_argument("--url", default=os.environ.get("NICGIT_URL", "http://127.0.0.1:3001"))
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
            clear_after_job()
            continue
        if args.once:
            return 0
        time.sleep(args.idle)


if __name__ == "__main__":
    raise SystemExit(main())
