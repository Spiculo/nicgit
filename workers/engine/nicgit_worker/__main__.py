# SPDX-License-Identifier: AGPL-3.0-or-later
"""`python -m nicgit_worker`: claim jobs from a NicGit server and run them with the KimBim engine.

Environment: NICGIT_URL (default http://127.0.0.1:3000), NICGIT_WORKER_TOKEN (required).
Run one worker process per CPU you want to give the engine; each runs one job at a time.
"""
from __future__ import annotations

import argparse
import json
import logging
import os
import pathlib
import time
import traceback
import urllib.error
import urllib.request

from . import handlers

log = logging.getLogger("nicgit_worker")


class Client:
    def __init__(self, url: str, token: str):
        self.url, self.token = url.rstrip("/"), token

    def _post(self, path: str, body: dict):
        req = urllib.request.Request(self.url + path, data=json.dumps(body).encode(), method="POST",
                                     headers={"content-type": "application/json", "x-worker-token": self.token})
        with urllib.request.urlopen(req, timeout=60) as r:
            data = r.read()
            return r.status, (json.loads(data) if data else None)

    def claim(self, kinds: list[str]):
        status, job = self._post("/internal/jobs/claim", {"kinds": kinds})
        return job if status == 200 else None

    def progress(self, job_id: str, fraction: float):
        try:
            self._post(f"/internal/jobs/{job_id}/progress", {"progress": fraction})
        except (urllib.error.URLError, OSError):
            pass

    def finish(self, job_id: str, *, result=None, error=None):
        self._post(f"/internal/jobs/{job_id}/finish", {"result": result} if error is None else {"error": error})


def run_one(client: Client, job: dict) -> None:
    ctx = handlers.Context(cache_dir=pathlib.Path(job["cache_dir"]), progress=lambda f: client.progress(job["id"], f))
    ctx.uploads_dir = pathlib.Path(job.get("uploads_dir") or ".")
    started = time.monotonic()
    try:
        result = handlers.run(job["kind"], pathlib.Path(job["repo"]), job["args"], ctx)
        result = json.loads(json.dumps(result, default=str))
        if isinstance(result, dict):
            result.setdefault("_seconds", round(time.monotonic() - started, 3))
        client.finish(job["id"], result=result)
        log.info("%s %s done in %.1fs", job["kind"], job["id"], time.monotonic() - started)
    except handlers.JobError as e:
        client.finish(job["id"], error=str(e))
        log.info("%s %s failed: %s", job["kind"], job["id"], e)
    except Exception as e:  # noqa: BLE001 (report every failure to the server; keep serving)
        log.error("%s %s crashed:\n%s", job["kind"], job["id"], traceback.format_exc())
        client.finish(job["id"], error=f"internal error in the engine worker: {type(e).__name__}")


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(prog="nicgit_worker")
    ap.add_argument("--url", default=os.environ.get("NICGIT_URL", "http://127.0.0.1:3000"))
    ap.add_argument("--kinds", default=os.environ.get("NICGIT_WORKER_KINDS", ""), help="comma list (default: all)")
    ap.add_argument("--once", action="store_true", help="exit when no job is waiting")
    ap.add_argument("--idle", type=float, default=1.0, help="seconds to wait when the queue is empty")
    args = ap.parse_args(argv)
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(name)s %(message)s")
    token = os.environ.get("NICGIT_WORKER_TOKEN")
    if not token:
        ap.error("NICGIT_WORKER_TOKEN is required")
    kinds = [k for k in args.kinds.split(",") if k] or handlers.kinds()
    client = Client(args.url, token)
    log.info("serving %s for %s", ", ".join(kinds), args.url)
    while True:
        try:
            job = client.claim(kinds)
        except (urllib.error.URLError, OSError) as e:
            log.warning("server unreachable: %s", e)
            job = None
            if args.once:
                return 1
        if job:
            run_one(client, job)
            continue
        if args.once:
            return 0
        time.sleep(args.idle)


if __name__ == "__main__":
    raise SystemExit(main())
