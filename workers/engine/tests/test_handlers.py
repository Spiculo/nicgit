# SPDX-License-Identifier: AGPL-3.0-or-later
"""Engine handlers against a copy of KimBim's M5 project (two design options, combined once)."""
import json
import pathlib
import shutil
import subprocess

import pytest

from kimbim.kimfile import remove_tree
from kimbim.project import Project
from nicgit_worker import handlers

FIXTURE = pathlib.Path(__file__).resolve().parents[3] / ".." / "kimcad" / "build" / "m5" / "project"


def git(repo, *args):
    return subprocess.run(["git", f"--git-dir={repo / '.git'}", *args], check=True, capture_output=True, text=True).stdout.strip()


@pytest.fixture
def repo(tmp_path):
    if not (FIXTURE / ".git").is_dir():
        pytest.skip("run `pixi run m5` in kimcad first")
    dst = tmp_path / "repo"
    dst.mkdir()
    # A bare copy the way NicGit stores it: <dir>/.git
    subprocess.run(["git", "clone", "--bare", "-q", str(FIXTURE / ".git"), str(dst / ".git")], check=True)
    yield dst
    remove_tree(dst)


@pytest.fixture
def unmerged(repo):
    """main and Scheme B before they were combined: main is moved back to its first parent."""
    combined = git(repo, "rev-parse", "main")
    first_parent = git(repo, "rev-parse", "main^1")
    git(repo, "update-ref", "refs/heads/main", first_parent)
    return repo, combined


def ctx(tmp_path):
    return handlers.Context(cache_dir=tmp_path / "cache", progress=lambda f: None)


def test_summary_counts(repo, tmp_path):
    head = git(repo, "rev-parse", "main~1")
    out = handlers.run("summary", repo, {"rev": head}, ctx(tmp_path))
    assert out["added"] == 2 and out["modified"] == 2 and out["removed"] == 0
    assert out["by_class"]["IfcWindow"]["added"] == 1
    assert "Level 1" in out["by_level"]
    assert out["first"] is False


def test_summary_of_first_version_counts_objects(repo, tmp_path):
    root = git(repo, "rev-list", "--max-parents=0", "main")
    out = handlers.run("summary", repo, {"rev": root}, ctx(tmp_path))
    assert out["first"] is True
    assert out["added"] > 0
    assert out["by_class"]["IfcWall"]["added"] >= 4


def test_compare_matches_engine(repo, tmp_path):
    a, b = git(repo, "rev-parse", "main~1"), git(repo, "rev-parse", "Scheme%20B")
    out = handlers.run("compare", repo, {"from": a, "to": b}, ctx(tmp_path))
    want = json.loads(json.dumps(Project.open(repo).diff_details(a, b), default=str))
    assert out == want


def test_export_ifc_writes_file(repo, tmp_path):
    head = git(repo, "rev-parse", "main")
    out = handlers.run("export_ifc", repo, {"rev": head}, ctx(tmp_path))
    path = pathlib.Path(out["path"])
    assert path.is_file() and out["bytes"] == path.stat().st_size
    assert path.read_text(encoding="utf-8", errors="replace").startswith("ISO-10303-21")


def test_element_history(repo, tmp_path):
    out = handlers.run("element_history", repo, {"guid": "2bolvJ3Pv4yw2Pbzq7q4wW", "rev": "main"}, ctx(tmp_path))
    kinds = [h["kind"] for h in out["history"]]
    assert kinds[-1] == "added" and "modified" in kinds
    assert all("message" in h and "time" in h for h in out["history"])


def test_merge_preview_lists_conflict(unmerged, tmp_path):
    repo, _ = unmerged
    out = handlers.run("merge_preview", repo, {"target": "main", "source": "Scheme%20B"}, ctx(tmp_path))
    assert out["status"] == "merge"
    assert len(out["conflicts"]) >= 1
    assert out["unresolved"] == [c["id"] for c in out["conflicts"]]
    # The repository's own HEAD and branches are untouched by a preview.
    assert (repo / ".git" / "HEAD").read_text().strip() == "ref: refs/heads/main"


def test_combine_theirs_gives_cli_tree(unmerged, tmp_path):
    repo, _ = unmerged
    preview = handlers.run("merge_preview", repo, {"target": "main", "source": "Scheme%20B"}, ctx(tmp_path))
    resolutions = {c["id"]: "theirs" for c in preview["conflicts"]}
    before_source = git(repo, "rev-parse", "Scheme%20B")
    out = handlers.run("combine", repo, {"target": "main", "source": "Scheme%20B", "resolutions": resolutions,
                                         "message": "Combine Scheme B into main", "author": ["Ana", "ana@example.com"],
                                         "time": 1700000000}, ctx(tmp_path))
    head = git(repo, "rev-parse", "main")
    assert out["commit"] == head
    assert git(repo, "rev-parse", "main^2") == before_source
    # The engine itself, merging the same commits the same way, gives the same tree.
    other = tmp_path / "other"
    subprocess.run(["git", "clone", "--bare", "-q", str(repo / ".git"), str(other / ".git")], check=True)
    git(other, "update-ref", "refs/heads/main", git(repo, "rev-parse", "main^1"))
    p = Project.open(other)
    p.merge("Scheme%20B", "Combine Scheme B into main", resolutions, author=("Ana", "ana@example.com"), time=1700000000)
    assert git(other, "rev-parse", "main^{tree}") == git(repo, "rev-parse", "main^{tree}")
    remove_tree(other)


def test_combine_with_unresolved_conflicts_fails(unmerged, tmp_path):
    repo, _ = unmerged
    before = git(repo, "rev-parse", "main")
    with pytest.raises(handlers.JobError, match="conflict"):
        handlers.run("combine", repo, {"target": "main", "source": "Scheme%20B", "resolutions": {},
                                       "message": "x", "author": ["Ana", "ana@example.com"]}, ctx(tmp_path))
    assert git(repo, "rev-parse", "main") == before


def test_unknown_kind_rejected(repo, tmp_path):
    with pytest.raises(handlers.JobError):
        handlers.run("rm_rf", repo, {}, ctx(tmp_path))


def test_checks_run_on_a_version(repo, tmp_path):
    out = handlers.run("checks", repo, {"rev": "main", "checks": ["schema", "model"]}, ctx(tmp_path))
    names = [c["check"] for c in out["checks"]]
    assert names == ["schema", "model"]
    for c in out["checks"]:
        assert set(c["counts"]) == {"error", "warning", "info"}
        assert c["seconds"] >= 0
    assert out["passed"] == (not any(c["failed"] for c in out["checks"]))
    assert out["seconds"] >= 0


def test_checks_reject_unknown(repo, tmp_path):
    with pytest.raises(handlers.JobError, match="unknown check"):
        handlers.run("checks", repo, {"rev": "main", "checks": ["rm"]}, ctx(tmp_path))
