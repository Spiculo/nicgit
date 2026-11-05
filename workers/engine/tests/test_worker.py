# SPDX-License-Identifier: AGPL-3.0-or-later
"""The supervisor and the job's process: one process per job, and the pool token stays out of it."""
import json
import os
import pathlib
import sys
import time

import pytest
from nicgit_worker import __main__ as supervisor
from nicgit_worker import child, client as client_module, handlers


class FakeClient:
    git_url = "unused"

    def __init__(self):
        self.progress_calls, self.finished = [], []

    def progress(self, fraction):
        self.progress_calls.append(fraction)

    def finish(self, *, result=None, error=None):
        self.finished.append((result, error))

    def git_env(self):
        return dict(os.environ)


def job(kind="summary", **extra):
    return {"id": "j1", "kind": kind, "args": {}, "token": "job-token", "fetch": {"mode": "none"}, **extra}


def test_worker_heartbeats_during_a_long_job(monkeypatch):
    monkeypatch.setattr(handlers, "run", lambda kind, repo, args, ctx: time.sleep(0.45) or {"ok": True})
    client = FakeClient()
    child.run_one(client, job(), heartbeat=0.1)
    assert len(client.progress_calls) >= 3, client.progress_calls
    assert client.finished and client.finished[0][1] is None


def test_worker_reports_job_errors(monkeypatch):
    def boom(*a):
        raise handlers.JobError("nope")
    monkeypatch.setattr(handlers, "run", boom)
    client = FakeClient()
    child.run_one(client, job())
    assert client.finished == [(None, "nope")]


def test_crash_reports_the_type_and_not_the_details(monkeypatch):
    def boom(*a):
        raise ValueError("a customer's element name")
    monkeypatch.setattr(handlers, "run", boom)
    client = FakeClient()
    child.run_one(client, job())
    assert client.finished == [(None, "internal error in the engine worker: ValueError")]


@pytest.mark.parametrize("fails", [False, True])
def test_workspace_removed_after_success_and_failure(monkeypatch, fails):
    seen = {}

    def run(kind, repo, args, ctx):
        seen["root"] = pathlib.Path(repo).parent
        assert seen["root"].is_dir()
        if fails:
            raise RuntimeError("boom")
        return {"ok": True}
    monkeypatch.setattr(handlers, "run", run)
    child.run_one(FakeClient(), job())
    assert not seen["root"].exists()


def test_child_env_has_no_pool_token():
    assert supervisor.child_env({"NICGIT_WORKER_TOKEN": "secret", "PATH": "x"}) == {"PATH": "x"}


def _script(monkeypatch, code):
    monkeypatch.setattr(supervisor, "CHILD", [sys.executable, "-c", code])


def test_the_real_child_process_cannot_see_the_pool_token(monkeypatch, tmp_path):
    out = tmp_path / "env.json"
    monkeypatch.setenv("NICGIT_WORKER_TOKEN", "pool-secret")
    _script(monkeypatch, f"import json, os, sys; json.dump({{'env': dict(os.environ), 'stdin': sys.stdin.read()}}, open({str(out)!r}, 'w'))")
    supervisor.run_in_child("http://server", job())
    seen = json.loads(out.read_text())
    assert "NICGIT_WORKER_TOKEN" not in seen["env"]
    assert "pool-secret" not in seen["stdin"]
    assert json.loads(seen["stdin"])["job"]["token"] == "job-token"


def test_each_job_gets_a_new_process(monkeypatch, tmp_path):
    out = tmp_path / "pids"
    _script(monkeypatch, f"import os; open({str(out)!r}, 'a').write(str(os.getpid()) + chr(10))")
    supervisor.run_in_child("http://server", job())
    supervisor.run_in_child("http://server", job())
    pids = out.read_text().split()
    assert len(pids) == 2 and pids[0] != pids[1] and str(os.getpid()) not in pids


def test_supervisor_reports_crashed_child(monkeypatch):
    calls = []

    class Recording:
        def __init__(self, url, job_id, token):
            calls.append(("new", url, job_id, token))

        def finish(self, *, result=None, error=None):
            calls.append(("finish", error))
    monkeypatch.setattr(client_module, "JobClient", Recording)
    _script(monkeypatch, "import sys; sys.exit(3)")
    supervisor.run_in_child("http://server", job())
    assert calls[0] == ("new", "http://server", "j1", "job-token")
    assert calls[1][0] == "finish" and "stopped unexpectedly" in calls[1][1]


def test_child_that_finished_normally_is_left_alone(monkeypatch):
    monkeypatch.setattr(client_module, "JobClient", lambda *a: pytest.fail("no report needed"))
    _script(monkeypatch, "pass")
    supervisor.run_in_child("http://server", job())


def test_a_job_past_its_time_is_stopped_and_reported(monkeypatch):
    calls = []

    class Recording:
        def __init__(self, url, job_id, token):
            pass

        def finish(self, *, result=None, error=None):
            calls.append(error)
    monkeypatch.setattr(client_module, "JobClient", Recording)
    _script(monkeypatch, "import time; time.sleep(60)")
    started = time.monotonic()
    supervisor.run_in_child("http://server", job(), timeout=0.5)
    assert time.monotonic() - started < 30
    assert len(calls) == 1 and "longer than" in calls[0]


@pytest.mark.skipif(os.name != "posix", reason="process groups")
def test_what_a_job_started_stops_with_it(monkeypatch, tmp_path):
    out = tmp_path / "pid"
    code = ("import subprocess, sys; "
            "p = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)']); "
            f"open({str(out)!r}, 'w').write(str(p.pid))")
    _script(monkeypatch, code)
    supervisor.run_in_child("http://server", job())
    pid = int(out.read_text())
    time.sleep(0.3)
    state = pathlib.Path(f"/proc/{pid}/stat")
    assert not state.exists() or state.read_text().split(")")[1].split()[0] == "Z"


@pytest.mark.parametrize("env,pid", [({"NICGIT_WORKER_CONTAINED": "1"}, 4242), ({}, 1), ({}, 4242), ({"NICGIT_WORKER_CONTAINED": "0"}, 1)])
def test_clearing_everything_only_ever_happens_alone_in_a_container(monkeypatch, env, pid):
    # Outside a container it would stop everything this user runs, so it must take both conditions.
    monkeypatch.setattr(supervisor.os, "kill", lambda *a: pytest.fail("killed something"), raising=False)
    assert supervisor.clear_after_job(env, pid) is False


def test_supervisor_serves_every_handler_kind():
    assert sorted(supervisor.KINDS) == handlers.kinds()


@pytest.mark.skipif(not sys.platform.startswith("linux"), reason="/proc and prctl are Linux")
def test_supervisor_not_dumpable_on_linux(tmp_path):
    import subprocess
    code = ("from nicgit_worker import __main__ as s; import ctypes; assert s.lock_down(); "
            "print(ctypes.CDLL(None).prctl(3, 0, 0, 0, 0))")
    assert subprocess.run([sys.executable, "-c", code], capture_output=True, text=True, check=True).stdout.strip() == "0"
