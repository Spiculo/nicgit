# SPDX-License-Identifier: AGPL-3.0-or-later
"""A job's temporary folder, with real git against a local bare repository in place of the server."""
import os
import pathlib
import subprocess

import pytest
from nicgit_worker import handlers, workspace

TOKEN = "job-secret-token"


def git(cwd, *args):
    r = subprocess.run(["git", "-c", "user.name=T", "-c", "user.email=t@example.com", *args], cwd=cwd, capture_output=True, text=True, check=True)
    return r.stdout.strip()


class FakeClient:
    def __init__(self, git_url):
        self.git_url = str(git_url)
        self.uploaded, self.downloads, self.files = [], [], {}

    def git_env(self):
        return {**os.environ, "GIT_TERMINAL_PROMPT": "0"}

    def download(self, path, dest):
        self.downloads.append(path)
        if path not in self.files:
            return False
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(self.files[path])
        return True

    def upload(self, path, src):
        self.uploaded.append((path, src.read_bytes()))


@pytest.fixture
def server(tmp_path):
    """A bare repository with two versions on main and a tag."""
    work = tmp_path / "work"
    work.mkdir()
    git(work, "init", "-q", "-b", "main")
    (work / "settings.json").write_text("{}")
    git(work, "add", "-A")
    git(work, "commit", "-qm", "first")
    (work / "settings.json").write_text('{"a":1}')
    git(work, "commit", "-qam", "second")
    git(work, "tag", "v1")
    bare = tmp_path / "server.git"
    git(tmp_path, "clone", "-q", "--bare", str(work), str(bare))
    return bare


def job(kind, fetch, **args):
    return {"id": "j", "kind": kind, "args": args, "token": TOKEN, "fetch": fetch}


def prepared(j, client, tmp_path):
    root = tmp_path / "job"
    root.mkdir()
    return workspace.prepare(j, client, root, lambda f: None)


def test_full_fetch_has_every_version_and_tag(server, tmp_path):
    repo, _ = prepared(job("merge_preview", {"mode": "full"}), FakeClient(server), tmp_path)
    g = f"--git-dir={repo / '.git'}"
    assert git(tmp_path, g, "rev-list", "--count", "main") == "2"
    assert git(tmp_path, g, "tag") == "v1"


def test_commit_fetch_has_only_that_version(server, tmp_path):
    first = git(tmp_path, f"--git-dir={server}", "rev-parse", "main~1")
    repo, _ = prepared(job("export_ifc", {"mode": "commits", "commits": [first], "depth": 1}), FakeClient(server), tmp_path)
    g = f"--git-dir={repo / '.git'}"
    assert git(tmp_path, g, "rev-parse", "HEAD") == first
    assert git(tmp_path, g, "rev-list", "--count", "--all") == "1"
    assert git(tmp_path, g, "tag") == "", "no other versions come along"


def test_two_commit_fetch(server, tmp_path):
    a = git(tmp_path, f"--git-dir={server}", "rev-parse", "main~1")
    b = git(tmp_path, f"--git-dir={server}", "rev-parse", "main")
    repo, _ = prepared(job("compare", {"mode": "commits", "commits": [a, b], "depth": 1}), FakeClient(server), tmp_path)
    g = f"--git-dir={repo / '.git'}"
    assert git(tmp_path, g, "cat-file", "-t", a) == "commit"
    assert git(tmp_path, g, "cat-file", "-t", b) == "commit"


def test_no_fetch_is_an_empty_repository(server, tmp_path):
    repo, _ = prepared(job("import_kim", {"mode": "none"}, upload="u.kim"), _with_upload(server, b"zip"), tmp_path)
    assert git(tmp_path, f"--git-dir={repo / '.git'}", "for-each-ref") == ""


def _with_upload(server, data):
    c = FakeClient(server)
    c.files["upload"] = data
    return c


def test_token_is_nowhere_in_the_repository(server, tmp_path):
    client = FakeClient(server)
    repo, _ = prepared(job("merge_preview", {"mode": "full"}), client, tmp_path)
    for path in (repo / ".git").rglob("*"):
        if path.is_file():
            assert TOKEN.encode() not in path.read_bytes(), path


def test_real_client_passes_the_token_only_through_the_environment():
    from nicgit_worker.client import JobClient
    c = JobClient("http://server:3000/", "j1", TOKEN)
    assert TOKEN not in c.git_url
    env = c.git_env()
    assert env["GIT_CONFIG_VALUE_0"] == f"Authorization: Bearer {TOKEN}"
    assert env["GIT_CONFIG_KEY_0"] == "http.extraHeader"


def test_import_gets_its_upload(server, tmp_path):
    _, ctx = prepared(job("import_ifc", {"mode": "none"}, upload="u.ifc"), _with_upload(server, b"ISO"), tmp_path)
    assert (ctx.uploads_dir / "u.ifc").read_bytes() == b"ISO"


def test_upload_name_cannot_leave_the_folder(server, tmp_path):
    _, ctx = prepared(job("import_ifc", {"mode": "none"}, upload="../../evil.ifc"), _with_upload(server, b"ISO"), tmp_path)
    assert (ctx.uploads_dir / "evil.ifc").is_file()
    assert not (tmp_path / "evil.ifc").exists()


def test_missing_upload_is_a_job_error(server, tmp_path):
    with pytest.raises(handlers.JobError, match="missing"):
        prepared(job("import_ifc", {"mode": "none"}, upload="u.ifc"), FakeClient(server), tmp_path)


def test_checks_reuses_a_cached_export(server, tmp_path):
    c = FakeClient(server)
    c.files["artifacts/ifc/abc.ifc"] = b"ISO"
    _, ctx = prepared(job("checks", {"mode": "none"}, rev="abc"), c, tmp_path)
    assert (ctx.cache_dir / "ifc" / "abc.ifc").read_bytes() == b"ISO"


def test_checks_without_a_cached_export_continues(server, tmp_path):
    _, ctx = prepared(job("checks", {"mode": "none"}, rev="abc"), FakeClient(server), tmp_path)
    assert not (ctx.cache_dir / "ifc" / "abc.ifc").exists()


def test_result_path_becomes_artifact(server, tmp_path):
    c = FakeClient(server)
    j = job("export_ifc", {"mode": "none"})
    repo, ctx = prepared(j, c, tmp_path)
    out = ctx.cache_dir / "ifc" / "abc.ifc"
    out.parent.mkdir()
    out.write_bytes(b"ISO")
    result = workspace.send_back(j, c, repo, ctx, {"path": str(out), "bytes": 3, "commit": "abc"})
    assert result == {"artifact": "ifc/abc.ifc", "bytes": 3, "commit": "abc"}
    assert c.uploaded == [("artifacts/ifc/abc.ifc", b"ISO")]


@pytest.mark.parametrize("kind", ["summary", "compare", "settings_diff", "element_history", "merge_preview", "checks"])
def test_reading_jobs_send_nothing_back(server, tmp_path, kind):
    c = FakeClient(server)
    j = job(kind, {"mode": "full"}, target="main")
    repo, ctx = prepared(j, c, tmp_path)
    before = git(tmp_path, f"--git-dir={server}", "for-each-ref")
    # Moved locally, as if by mistake: it must stay in the temporary folder.
    git(tmp_path, f"--git-dir={repo / '.git'}", "update-ref", "refs/heads/main", git(tmp_path, f"--git-dir={repo / '.git'}", "rev-parse", "main~1"))
    assert workspace.send_back(j, c, repo, ctx, {"x": 1}) == {"x": 1}
    assert git(tmp_path, f"--git-dir={server}", "for-each-ref") == before
    assert c.uploaded == []


def _advance(repo, branch="main"):
    g = f"--git-dir={repo / '.git'}"
    tree = git(repo, g, "rev-parse", f"{branch}^{{tree}}")
    head = git(repo, g, "rev-parse", branch)
    commit = git(repo, g, "commit-tree", tree, "-p", head, "-m", "merged")
    git(repo, g, "update-ref", f"refs/heads/{branch}", commit)
    return commit


def test_combine_pushes_its_target(server, tmp_path):
    c = FakeClient(server)
    j = job("combine", {"mode": "full"}, target="main")
    repo, ctx = prepared(j, c, tmp_path)
    commit = _advance(repo)
    workspace.send_back(j, c, repo, ctx, {"commit": commit, "status": "merge"})
    assert git(tmp_path, f"--git-dir={server}", "rev-parse", "main") == commit


def test_combine_up_to_date_pushes_nothing(server, tmp_path):
    c = FakeClient(server)
    j = job("combine", {"mode": "full"}, target="main")
    repo, ctx = prepared(j, c, tmp_path)
    before = git(tmp_path, f"--git-dir={server}", "rev-parse", "main")
    commit = _advance(repo)
    workspace.send_back(j, c, repo, ctx, {"commit": commit, "status": "up_to_date"})
    assert git(tmp_path, f"--git-dir={server}", "rev-parse", "main") == before


def test_refused_push_shows_the_servers_reason(server, tmp_path):
    hook = pathlib.Path(server) / "hooks" / "pre-receive"
    hook.write_text('#!/bin/sh\necho "NicGit: design option main is protected" >&2\nexit 1\n')
    hook.chmod(0o755)
    c = FakeClient(f"file://{pathlib.Path(server).as_posix()}")
    j = job("combine", {"mode": "full"}, target="main")
    repo, ctx = prepared(j, c, tmp_path)
    commit = _advance(repo)
    with pytest.raises(handlers.JobError, match="main is protected"):
        workspace.send_back(j, c, repo, ctx, {"commit": commit, "status": "merge"})


def test_import_kim_pushes_options_and_milestones(server, tmp_path):
    empty = tmp_path / "empty.git"
    git(tmp_path, "init", "-q", "--bare", str(empty))
    c = _with_upload(empty, b"zip")
    j = job("import_kim", {"mode": "none"}, upload="u.kim")
    repo, ctx = prepared(j, c, tmp_path)
    g = f"--git-dir={repo / '.git'}"
    git(tmp_path, g, "fetch", "-q", str(server), "+refs/heads/*:refs/heads/*", "+refs/tags/*:refs/tags/*")
    workspace.send_back(j, c, repo, ctx, {"options": ["main"]})
    assert git(tmp_path, f"--git-dir={empty}", "tag") == "v1"
    assert git(tmp_path, f"--git-dir={empty}", "rev-parse", "main") == git(tmp_path, f"--git-dir={server}", "rev-parse", "main")
