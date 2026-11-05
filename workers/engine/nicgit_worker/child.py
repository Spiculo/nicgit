# SPDX-License-Identifier: AGPL-3.0-or-later
"""One job in one process: `python -m nicgit_worker.child`, with {"url", "job"} on stdin.

Engine code parses untrusted IFC and .kim files, so it runs here, holding only this job's token and a
temporary folder. Nothing of it outlives the job, and the next customer's job starts in a new process."""
from __future__ import annotations

import json
import logging
import pathlib
import sys
import tempfile
import threading
import time
import traceback

from kimbim import kimfile

from . import handlers, workspace
from .client import JobClient

log = logging.getLogger("nicgit_worker")

HEARTBEAT_SECONDS = 60.0


def run_one(client, job: dict, heartbeat: float = HEARTBEAT_SECONDS) -> None:
    last = {"fraction": 0.0}

    def report(fraction: float) -> None:
        last["fraction"] = fraction
        client.progress(fraction)

    started = time.monotonic()
    # Report regularly however long the job takes: the server hands out again only jobs that go silent.
    stop = threading.Event()

    def beat() -> None:
        while not stop.wait(heartbeat):
            client.progress(last["fraction"])

    beater = threading.Thread(target=beat, daemon=True)
    beater.start()
    root = pathlib.Path(tempfile.mkdtemp(prefix="nicgit-job-"))
    try:
        repo, ctx = workspace.prepare(job, client, root, report)
        result = handlers.run(job["kind"], repo, job["args"], ctx)
        result = json.loads(json.dumps(result, default=str))
        result = workspace.send_back(job, client, repo, ctx, result)
        if isinstance(result, dict):
            result.setdefault("_seconds", round(time.monotonic() - started, 3))
        client.finish(result=result)
        log.info("%s %s done in %.1fs", job["kind"], job["id"], time.monotonic() - started)
    except handlers.JobError as e:
        client.finish(error=str(e))
        log.info("%s %s failed: %s", job["kind"], job["id"], e)
    except Exception as e:  # noqa: BLE001 (report every failure to the server)
        log.error("%s %s crashed:\n%s", job["kind"], job["id"], traceback.format_exc())
        client.finish(error=f"internal error in the engine worker: {type(e).__name__}")
    finally:
        stop.set()
        beater.join(timeout=5)
        kimfile.remove_tree(root)


def main() -> int:
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(name)s %(message)s")
    work = json.load(sys.stdin)
    job = work["job"]
    run_one(JobClient(work["url"], job["id"], job["token"]), job)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
