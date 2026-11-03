# SPDX-License-Identifier: AGPL-3.0-or-later
"""Job handlers: each turns (project repository, arguments) into a JSON result using the KimBim engine.

The repository is NicGit's bare store, `<repo>/.git`, which `kimbim.project.Project.open` reads as a
project. Jobs that need a current option (merge preview, combine) use a throw-away worktree HEAD
(`Project.open_worktree`, D111) so the store's own HEAD never moves.
"""
from __future__ import annotations

import json
import pathlib
import shutil
import tempfile
from dataclasses import dataclass
from typing import Callable

from kimbim import settings as S
from kimbim.layout import key_to_path, path_to_key
from kimbim.project import KimbimError, Project
from kimbim.records import compose


class JobError(Exception):
    """A failure the user should see (bad input, unresolved conflicts)."""


@dataclass
class Context:
    cache_dir: pathlib.Path
    progress: Callable[[float], None]


def _jsonable(value):
    return json.loads(json.dumps(value, default=str))


def _commit(project: Project, rev: str) -> str:
    try:
        return project._rev(rev)
    except KimbimError as e:
        raise JobError(str(e)) from None


# ---- summaries and compare ----

def summary(project: Project, args: dict, ctx: Context) -> dict:
    """What a version changed against its first parent; the first version counts its objects as added."""
    rev = _commit(project, args["rev"])
    info = project.log(rev, 1)[0]
    if not info.parents:
        by_class: dict = {}
        for key, blob in project._records(rev).items():
            if not key.startswith("o/"):
                continue
            t = json.loads(blob).get("t", "")
            if t.startswith("IfcRel") or t in ("IfcPropertySet", "IfcElementQuantity", "IfcOwnerHistory"):
                continue
            by_class.setdefault(t, {"added": 0, "modified": 0, "removed": 0})["added"] += 1
        return {"first": True, "added": sum(c["added"] for c in by_class.values()), "modified": 0, "removed": 0,
                "by_class": by_class, "by_level": {}, "settings_changed": project.settings_at(rev) is not None,
                "other": 0}
    d = project.diff_details(info.parents[0], rev)
    changes = d["changes"]
    return {"first": False, "added": len(changes["added"]), "modified": len(changes["modified"]),
            "removed": len(changes["removed"]), "by_class": d["categories"], "by_level": d["tree"],
            "settings_changed": bool(d["settings"]), "other": d["other"]}


def compare(project: Project, args: dict, ctx: Context) -> dict:
    return _jsonable(project.diff_details(_commit(project, args["from"]), _commit(project, args["to"])))


def settings_diff(project: Project, args: dict, ctx: Context) -> dict:
    """Project settings keys that differ between two versions, old and new values."""
    a = S.from_bytes(project.settings_at(_commit(project, args["from"])))
    b = S.from_bytes(project.settings_at(_commit(project, args["to"])))
    keys = sorted(k for k in set(a) | set(b) if a.get(k) != b.get(k))
    return {"changed": [{"key": k, "old": a.get(k), "new": b.get(k)} for k in keys]}


def export_ifc(project: Project, args: dict, ctx: Context) -> dict:
    rev = _commit(project, args["rev"])
    out = ctx.cache_dir / "ifc" / f"{rev}.ifc"
    out.parent.mkdir(parents=True, exist_ok=True)
    tmp = out.with_suffix(".ifc.part")
    project.export(tmp, rev, overwrite=True, one_owner=True)
    tmp.replace(out)
    return {"path": str(out), "bytes": out.stat().st_size, "commit": rev}


def element_history(project: Project, args: dict, ctx: Context) -> dict:
    guid = args["guid"]
    if not guid or "/" in guid or len(guid) > 64:
        raise JobError("not an element id")
    history = project.history_of(key_to_path(f"o/{guid}"), rev=_commit(project, args.get("rev", "HEAD")),
                                 limit=int(args.get("limit", 200)))
    return {"guid": guid, "history": [
        {"commit": c.id, "kind": kind, "message": c.message.strip(), "time": c.time, "author": c.author_name}
        for c, kind in history]}


# ---- checks (Actions) ----

def _ids_files(settings: dict) -> dict:
    """The IDS files a version's settings name, as {name: xml}. Files linked from a folder on someone's
    machine (a dict with a path) cannot be read on the server and are skipped."""
    out = {}
    for name, entry in (settings.get("ids_files") or {}).items():
        if isinstance(entry, str):
            out[name] = entry
        elif isinstance(entry, dict) and isinstance(entry.get("xml"), str):
            out[name] = entry["xml"]
    return out


def checks(project: Project, args: dict, ctx: Context) -> dict:
    """Run the engine's checks (schema, IDS, clash, model) on a version, as an Action."""
    import time

    import ifcopenshell
    from kimbim import checks as C

    names = list(args.get("checks") or [])
    unknown = [n for n in names if n not in C.RUNNERS]
    if unknown:
        raise JobError(f"unknown check: {', '.join(unknown)}")
    rev = _commit(project, args["rev"])
    t0 = time.perf_counter()
    path = ctx.cache_dir / "ifc" / f"{rev}.ifc"
    if not path.is_file():
        export_ifc(project, {"rev": rev}, ctx)
    model = ifcopenshell.open(str(path))
    settings = S.from_bytes(project.settings_at(rev))
    results = []
    for i, name in enumerate(names):
        options = {"ids": _ids_files(settings)} if name == "ids" else {}
        if name == "clash":
            options = {"clash_sets": settings.get("clash_sets") or {}}
        try:
            out = C.run(model, name, options)
            results.append({"check": name, "title": out["title"], "ok": True, "total": out["total"], "counts": out["counts"],
                            "issues": _jsonable(out["issues"][:200]), "truncated": out["total"] > 200, "seconds": out["seconds"],
                            "detail": out.get("detail", ""), "failed": out["total"] > 0 and out["counts"].get("error", 0) > 0})
        except Exception as ex:  # noqa: BLE001 (a broken checker fails the check, never passes it)
            results.append({"check": name, "title": name, "ok": False, "error": f"{type(ex).__name__}: {ex}", "total": 1,
                            "counts": {"error": 1, "warning": 0, "info": 0}, "issues": [], "truncated": False,
                            "seconds": 0.0, "failed": True})
        ctx.progress((i + 1) / max(len(names), 1))
    return {"rev": rev, "checks": results, "passed": not any(r["failed"] for r in results),
            "seconds": round(time.perf_counter() - t0, 3)}


# ---- combining options ----

class _OnOption:
    """The project with `option` as its current option, in a temporary worktree."""

    def __init__(self, repo: pathlib.Path, option: str):
        self.repo, self.option = repo, option

    def __enter__(self) -> Project:
        self.tmp = pathlib.Path(tempfile.mkdtemp(prefix="nicgit-wt-"))
        (self.tmp / ".kimbim").mkdir()
        (self.tmp / ".kimbim" / "HEAD").write_text(f"ref: refs/heads/{self.option}\n", encoding="utf-8")
        project = Project.open_worktree(self.repo, self.tmp)
        if project.repo.head() is None:
            raise JobError(f"design option {self.option!r} does not exist")
        return project

    def __exit__(self, *exc):
        shutil.rmtree(self.tmp, ignore_errors=True)


def _preview_json(preview) -> dict:
    out = {"status": preview.status, "base": preview.base, "ours": preview.ours, "theirs": preview.theirs,
           "conflicts": [], "unresolved": [], "warnings": [], "stats": {},
           "settings_conflicts": list(preview.settings_conflicts)}
    if preview.result is not None:
        out["conflicts"] = _jsonable([c.to_dict() for c in preview.result.conflicts])
        out["unresolved"] = list(preview.result.unresolved)
        out["warnings"] = _jsonable(preview.result.warnings)
        out["stats"] = _jsonable(preview.result.stats)
    return out


def merge_preview(project_unused, args: dict, ctx: Context, *, repo: pathlib.Path) -> dict:
    with _OnOption(repo, args["target"]) as p:
        try:
            preview = p.merge_preview(args["source"], args.get("resolutions") or None)
        except KimbimError as e:
            raise JobError(str(e)) from None
        out = _preview_json(preview)
        if preview.settings_conflicts:
            ours = S.from_bytes(p.settings_at(preview.ours))
            theirs = S.from_bytes(p.settings_at(preview.theirs))
            base = S.from_bytes(p.settings_at(preview.base)) if preview.base else {}
            out["settings_conflicts"] = _jsonable([
                {"key": k, "base": base.get(k), "ours": ours.get(k), "theirs": theirs.get(k)}
                for k in preview.settings_conflicts])
        return out


def combine(project_unused, args: dict, ctx: Context, *, repo: pathlib.Path) -> dict:
    """Combine `source` into `target` with the given resolutions; settings keys in conflict take
    `settings_resolutions[key]` ("ours" default, or "theirs").

    The engine's `Project.merge` keeps ours for every settings conflict, so this is `Project.merge` with
    that one step added (engine gap, docs/future-work.md: accept settings resolutions in the engine)."""
    resolutions = args.get("resolutions") or None
    settings_picks = args.get("settings_resolutions") or {}
    name, email = args["author"]
    when = args.get("time")
    with _OnOption(repo, args["target"]) as p:
        try:
            preview = p.merge_preview(args["source"], resolutions)
            if preview.status == "up_to_date":
                return {"commit": preview.ours, "status": "up_to_date"}
            if preview.status == "fast_forward":
                return {"commit": p.repo.fast_forward(preview.theirs), "status": "fast_forward"}
            if not preview.result.clean:
                raise JobError(f"{len(preview.result.unresolved)} conflict(s) still need a side")
            if not any(v == "theirs" for v in settings_picks.values()):
                commit = p.merge(args["source"], args["message"], resolutions, author=(name, email), time=when)
                return {"commit": commit, "status": "merge"}
            merged = S.from_bytes(preview.settings)
            theirs = S.from_bytes(p.settings_at(preview.theirs))
            for key, side in settings_picks.items():
                if side != "theirs" or key not in preview.settings_conflicts:
                    continue
                if key in theirs:
                    merged[key] = theirs[key]
                else:
                    merged.pop(key, None)
            settings = S.to_bytes(merged)
            compose(preview.result.records)  # must be writable as IFC, as Project.merge checks
            _changes, writes = p._changes(p._head_entries(), preview.result.records, settings)
            import time as _time
            commit = p.repo.commit_merge(writes, args["message"], name, email,
                                         int(when if when is not None else _time.time()), [preview.theirs])
            return {"commit": commit, "status": "merge"}
        except KimbimError as e:
            raise JobError(str(e)) from None


# ---- dispatch ----

HANDLERS = {
    "summary": summary,
    "compare": compare,
    "settings_diff": settings_diff,
    "export_ifc": export_ifc,
    "element_history": element_history,
    "checks": checks,
}
WORKTREE_HANDLERS = {
    "merge_preview": merge_preview,
    "combine": combine,
}


def kinds() -> list[str]:
    from . import transfer
    return sorted([*HANDLERS, *WORKTREE_HANDLERS, *transfer.HANDLERS])


def run(kind: str, repo: pathlib.Path, args: dict, ctx: Context) -> dict:
    repo = pathlib.Path(repo)
    if kind in WORKTREE_HANDLERS:
        return WORKTREE_HANDLERS[kind](None, args, ctx, repo=repo)
    if kind in HANDLERS:
        return HANDLERS[kind](Project.open(repo), args, ctx)
    from . import transfer
    if kind in transfer.HANDLERS:
        return transfer.HANDLERS[kind](repo, args, ctx)
    raise JobError(f"unknown job kind {kind!r}")
