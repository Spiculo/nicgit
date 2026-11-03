# SPDX-License-Identifier: AGPL-3.0-or-later
"""Whole-project transfer: importing .kim and .ifc files into a NicGit store, and exporting .kim files.

A .kim is a transport format here (decision N3): its history is fetched into the store, and the file
itself is never kept. Uploads are untrusted: names are confined to the uploads folder, the zip's total
uncompressed size is bounded before anything is written, and kimfile.unpack refuses unsafe paths.
"""
from __future__ import annotations

import os
import pathlib
import shutil
import subprocess
import tempfile
import zipfile

from kimbim import kimfile
from kimbim.project import KimbimError, Project

from .handlers import Context, JobError

# A .kim compresses records well, but beyond this ratio it is treated as a zip bomb.
MAX_RATIO = 20


def _git(git_dir: pathlib.Path, *args: str, cwd=None) -> str:
    r = subprocess.run(["git", f"--git-dir={git_dir}", *args], capture_output=True, text=True, cwd=cwd,
                       env={**os.environ, "GIT_TERMINAL_PROMPT": "0"})
    if r.returncode != 0:
        raise JobError(f"git {args[0]} failed: {(r.stderr or r.stdout).strip().splitlines()[-1:]}")
    return r.stdout.strip()


def _upload(ctx: Context, name: str) -> pathlib.Path:
    base = pathlib.Path(getattr(ctx, "uploads_dir", ".")).resolve()
    path = (base / name).resolve()
    if path.parent != base or not path.is_file():
        raise JobError("the uploaded file is missing")
    return path


def _branches(git_dir: pathlib.Path) -> list[str]:
    out = _git(git_dir, "for-each-ref", "--format=%(refname:short)", "refs/heads")
    return [b for b in out.splitlines() if b]


def import_kim(repo: pathlib.Path, args: dict, ctx: Context) -> dict:
    kim = _upload(ctx, args["upload"])
    try:
        try:
            with zipfile.ZipFile(kim) as z:
                total = sum(i.file_size for i in z.infolist())
        except (zipfile.BadZipFile, OSError):
            raise JobError("this is not a KimBim .kim file, or it is damaged") from None
        limit = min(int(args.get("max_bytes", 1 << 34)), MAX_RATIO * max(kim.stat().st_size, 1))
        if total > limit:
            raise JobError(f"the .kim unpacks to {total >> 20} MB, which is too large to import")
        tmp = pathlib.Path(tempfile.mkdtemp(prefix="nicgit-kim-"))
        try:
            try:
                kimfile.unpack(kim, tmp / "p")
            except kimfile.KimError as e:
                raise JobError(str(e)) from None
            src = tmp / "p" / ".git"
            # Zip files drop empty folders; git needs these to recognise the repository.
            for d in ("objects", "refs/heads", "refs/tags"):
                (src / d).mkdir(parents=True, exist_ok=True)
            if not (src / "HEAD").is_file():
                raise JobError("this .kim holds no project history")
            ctx.progress(0.3)
            dst = repo / ".git"
            _git(dst, "fetch", "--no-tags", str(src), "+refs/heads/*:refs/heads/*")
            _git(dst, "fetch", "--no-tags", str(src), "+refs/tags/*:refs/tags/*")
            ctx.progress(0.9)
        finally:
            kimfile.remove_tree(tmp)
        options = _branches(repo / ".git")
        if not options:
            raise JobError("this .kim holds no design options")
        default = "main" if "main" in options else options[0]
        (repo / ".git" / "HEAD").write_text(f"ref: refs/heads/{default}\n", encoding="utf-8")
        heads = {b: _git(repo / ".git", "rev-parse", f"refs/heads/{b}") for b in options}
        return {"options": options, "default": default, "heads": heads}
    finally:
        kim.unlink(missing_ok=True)


def import_ifc(repo: pathlib.Path, args: dict, ctx: Context) -> dict:
    """An IFC file becomes a version on `option` (the first version of a new project, or a reconciled
    re-import into an existing one)."""
    ifc = _upload(ctx, args["upload"])
    option = args.get("option") or "main"
    try:
        tmp = pathlib.Path(tempfile.mkdtemp(prefix="nicgit-ifc-"))
        try:
            (tmp / ".kimbim").mkdir()
            (tmp / ".kimbim" / "HEAD").write_text(f"ref: refs/heads/{option}\n", encoding="utf-8")
            project = Project.open_worktree(repo, tmp)
            name, email = args.get("author") or ["NicGit", "nicgit@localhost"]
            try:
                if project.repo.head() is None:
                    commit, _changes = project.commit(ifc, args.get("message") or f"Imported from {ifc.name}", author=(name, email))
                    report = None
                else:
                    result = project.import_ifc(ifc, args.get("message") or f"Re-imported {ifc.name}",
                                                accept_heuristic=bool(args.get("accept_heuristic")))
                    commit = project.head_commit()
                    report = result if isinstance(result, dict) else getattr(result, "__dict__", None)
            except KimbimError as e:
                raise JobError(str(e)) from None
            objects = sum(1 for p in project._entries(commit) if p.startswith("objects/"))
            return {"commit": commit, "option": option, "objects": objects, "report": report}
        finally:
            shutil.rmtree(tmp, ignore_errors=True)
    finally:
        ifc.unlink(missing_ok=True)


def _loosen_refs(git_dir: pathlib.Path) -> None:
    """Write every ref as its own file and drop packed-refs: a zip keeps no empty folders, and a
    repository without refs/ is not recognised once the .kim is unpacked (by NicBim or by git)."""
    refs = _git(git_dir, "for-each-ref", "--format=%(objectname) %(refname)")
    for line in refs.splitlines():
        oid, name = line.split(" ", 1)
        path = git_dir / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(f"{oid}\n", encoding="utf-8")
    (git_dir / "packed-refs").unlink(missing_ok=True)


def export_kim(repo: pathlib.Path, args: dict, ctx: Context) -> dict:
    """A .kim of the project: `full` history, or `latest` (one version of one option, no history)."""
    scope = args.get("scope", "full")
    name = "".join(c for c in args.get("name", "project") if c.isalnum() or c in "-_.") or "project"
    out_dir = ctx.cache_dir / "exports"
    out_dir.mkdir(parents=True, exist_ok=True)
    tmp = pathlib.Path(tempfile.mkdtemp(prefix="nicgit-export-"))
    try:
        folder = tmp / name
        dst = folder / ".git"
        if scope == "full":
            subprocess.run(["git", "clone", "--bare", "-q", "--no-local", str(repo / ".git"), str(dst)], check=True)
            head = (repo / ".git" / "HEAD").read_text(encoding="utf-8")
            (dst / "HEAD").write_text(head, encoding="utf-8")
        elif scope == "latest":
            option = args.get("option") or "main"
            src_commit = _git(repo / ".git", "rev-parse", f"refs/heads/{option}")
            subprocess.run(["git", "init", "--bare", "-q", str(dst)], check=True)
            _git(dst, "fetch", "--depth=1", "--no-tags", str(repo / ".git"), f"{src_commit}")
            tree = _git(dst, "rev-parse", f"{src_commit}^{{tree}}")
            msg = f"{option} as of {src_commit[:7]} (exported from NicGit without history)"
            env = {**os.environ, "GIT_AUTHOR_NAME": "NicGit", "GIT_AUTHOR_EMAIL": "nicgit@localhost",
                   "GIT_COMMITTER_NAME": "NicGit", "GIT_COMMITTER_EMAIL": "nicgit@localhost"}
            commit = subprocess.run(["git", f"--git-dir={dst}", "commit-tree", tree, "-m", msg], check=True,
                                    capture_output=True, text=True, env=env).stdout.strip()
            _git(dst, "update-ref", "refs/heads/main", commit)
            (dst / "shallow").unlink(missing_ok=True)
            _git(dst, "reflog", "expire", "--expire=now", "--all")
            _git(dst, "gc", "-q", "--prune=now")
            (dst / "HEAD").write_text("ref: refs/heads/main\n", encoding="utf-8")
        else:
            raise JobError("scope is full or latest")
        _loosen_refs(dst)
        ctx.progress(0.6)
        target = out_dir / f"{name}-{scope}-{os.getpid()}-{abs(hash(str(tmp))) % 10**8}.kim"
        try:
            kimfile.pack(folder, target)
        except kimfile.KimError as e:
            raise JobError(str(e)) from None
        return {"path": str(target), "bytes": target.stat().st_size, "name": f"{name}.kim"}
    finally:
        kimfile.remove_tree(tmp)


HANDLERS = {
    "import_kim": import_kim,
    "import_ifc": import_ifc,
    "export_kim": export_kim,
}
