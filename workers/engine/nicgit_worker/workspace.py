# SPDX-License-Identifier: AGPL-3.0-or-later
"""One job's files. A worker mounts no shared storage (decision N30): it fetches what the job needs into a
temporary folder, the unchanged handlers work on that as if it were the store, and what they produced goes
back over HTTP. The server checks everything that comes back."""
from __future__ import annotations

import pathlib
import subprocess
from typing import Callable

from . import handlers
from .handlers import JobError


def _git(client, *args: str, cwd: pathlib.Path | None = None) -> str:
    r = subprocess.run(["git", *args], capture_output=True, text=True, cwd=cwd, env=client.git_env())
    if r.returncode != 0:
        # What a pre-receive hook refused is the useful part; nothing here holds the token.
        lines = [line.removeprefix("remote: ") for line in (r.stderr or "").splitlines() if line.startswith("remote: NicGit:")]
        # The server can also refuse before git starts, for example when the project is full.
        lines += [line.split("remote error: ", 1)[1] for line in (r.stderr or "").splitlines() if "remote error: NicGit:" in line]
        raise JobError(" ".join(lines) or f"git {args[0]} failed: {(r.stderr or r.stdout).strip().splitlines()[-1:]}")
    return r.stdout.strip()


def _fetch(job: dict, client, git_dir: pathlib.Path) -> None:
    plan = job.get("fetch") or {"mode": "full"}
    mode = plan.get("mode")
    if mode == "full":
        _git(client, "clone", "--bare", "-q", client.git_url, str(git_dir))
        return
    _git(client, "init", "--bare", "-q", str(git_dir))
    if mode == "none":
        return
    if mode != "commits":
        raise JobError(f"unknown fetch mode {mode!r}")
    # The engine needs a branch to open a project; only this job's copy ever has these names.
    for i, commit in enumerate(plan["commits"]):
        _git(client, f"--git-dir={git_dir}", "fetch", "-q", f"--depth={int(plan['depth'])}", "--no-tags", client.git_url, commit)
        _git(client, f"--git-dir={git_dir}", "update-ref", f"refs/heads/nicgit-job-{i}", commit)
    _git(client, f"--git-dir={git_dir}", "symbolic-ref", "HEAD", "refs/heads/nicgit-job-0")


def prepare(job: dict, client, root: pathlib.Path, progress: Callable[[float], None]) -> tuple[pathlib.Path, handlers.Context]:
    """Lay out `root` for this job. Returns the project folder and the handlers' context."""
    repo = root / "repo"
    repo.mkdir()
    _fetch(job, client, repo / ".git")
    ctx = handlers.Context(cache_dir=root / "cache", progress=progress)
    ctx.cache_dir.mkdir()
    ctx.uploads_dir = root / "uploads"
    ctx.uploads_dir.mkdir()
    kind, args = job["kind"], job["args"]
    if kind in ("import_kim", "import_ifc"):
        # Under the name the handler expects; a name with a path in it never comes out of the uploads folder.
        name = pathlib.PurePosixPath(args["upload"]).name
        if not client.download("upload", ctx.uploads_dir / name):
            raise JobError("the uploaded file is missing")
    if kind == "checks" and isinstance(args.get("rev"), str):
        # A cached export saves the work; without one the handler exports it.
        name = pathlib.PurePosixPath(args["rev"]).name
        client.download(f"artifacts/ifc/{name}.ifc", ctx.cache_dir / "ifc" / f"{name}.ifc")
    return repo, ctx


def send_back(job: dict, client, repo: pathlib.Path, ctx: handlers.Context, result):
    """Send what the job produced to the server. Returns the result to report."""
    kind, args = job["kind"], job["args"]
    git_dir = f"--git-dir={repo / '.git'}"
    if kind == "combine" and isinstance(result, dict) and result.get("status") != "up_to_date":
        target = f"refs/heads/{args['target']}"
        _git(client, git_dir, "push", "-q", client.git_url, f"{target}:{target}")
    elif kind == "import_ifc":
        ref = f"refs/heads/{args.get('option') or 'main'}"
        _git(client, git_dir, "push", "-q", client.git_url, f"{ref}:{ref}")
    elif kind == "import_kim":
        _git(client, git_dir, "push", "-q", client.git_url, "refs/heads/*:refs/heads/*", "refs/tags/*:refs/tags/*")
    elif kind in ("export_ifc", "export_kim") and isinstance(result, dict) and result.get("path"):
        # A path means nothing outside this folder: the server stores the file, under a name relative to the cache.
        file = pathlib.Path(result.pop("path"))
        artifact = file.relative_to(ctx.cache_dir).as_posix()
        client.upload(f"artifacts/{artifact}", file)
        result["artifact"] = artifact
    return result
