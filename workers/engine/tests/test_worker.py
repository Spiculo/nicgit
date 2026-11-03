# SPDX-License-Identifier: AGPL-3.0-or-later
"""The worker loop: it keeps reporting while a long job runs, so the server never hands it out twice."""
import time

from nicgit_worker import __main__ as worker
from nicgit_worker import handlers


class FakeClient:
    def __init__(self):
        self.progress_calls, self.finished = [], []

    def progress(self, job_id, fraction):
        self.progress_calls.append(fraction)

    def finish(self, job_id, *, result=None, error=None):
        self.finished.append((result, error))


def test_worker_heartbeats_during_a_long_job(monkeypatch, tmp_path):
    monkeypatch.setattr(handlers, "run", lambda kind, repo, args, ctx: time.sleep(0.45) or {"ok": True})
    client = FakeClient()
    worker.run_one(client, {"id": "j1", "kind": "import_ifc", "repo": str(tmp_path), "args": {}, "cache_dir": str(tmp_path)},
                   heartbeat=0.1)
    assert len(client.progress_calls) >= 3, client.progress_calls
    assert client.finished and client.finished[0][1] is None


def test_worker_reports_job_errors(monkeypatch, tmp_path):
    def boom(*a):
        raise handlers.JobError("nope")
    monkeypatch.setattr(handlers, "run", boom)
    client = FakeClient()
    worker.run_one(client, {"id": "j2", "kind": "summary", "repo": str(tmp_path), "args": {}, "cache_dir": str(tmp_path)})
    assert client.finished == [(None, "nope")]
