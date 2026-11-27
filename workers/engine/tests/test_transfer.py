# SPDX-License-Identifier: AGPL-3.0-or-later
"""Importing and exporting whole projects: .nic files and IFC."""
import pathlib
import subprocess
import zipfile

import pytest

from kimbim import kimfile
from kimbim.nicfile import remove_tree
from nicgit_worker import handlers

KIMCAD = pathlib.Path(__file__).resolve().parents[3] / ".." / "nicbim"
M5 = KIMCAD / "build" / "m5" / "project"
HOUSE = KIMCAD / "build" / "house.ifc"


def git(repo, *args):
    return subprocess.run(["git", f"--git-dir={repo / '.git'}", *args], check=True, capture_output=True, text=True).stdout.strip()


def empty_repo(path: pathlib.Path) -> pathlib.Path:
    """A fresh NicGit store: `<path>/.git`, bare, HEAD on main."""
    subprocess.run(["git", "init", "--bare", "-q", str(path / ".git")], check=True)
    (path / ".git" / "HEAD").write_text("ref: refs/heads/main\n")
    return path


def ctx(tmp_path, **kw):
    c = handlers.Context(cache_dir=tmp_path / "cache", progress=lambda f: None)
    c.uploads_dir = tmp_path / "uploads"
    c.uploads_dir.mkdir(exist_ok=True)
    for k, v in kw.items():
        setattr(c, k, v)
    return c


@pytest.fixture
def m5_kim(tmp_path):
    if not (M5 / ".git").is_dir():
        pytest.skip("run `pixi run m5` in nicbim first")
    folder = tmp_path / "m5-folder"
    subprocess.run(["git", "clone", "--bare", "-q", str(M5 / ".git"), str(folder / ".git")], check=True)
    (folder / ".git" / "HEAD").write_text("ref: refs/heads/main\n")
    kim = tmp_path / "uploads" / "m5.nic"
    kim.parent.mkdir(exist_ok=True)
    kimfile.pack(folder, kim)
    remove_tree(folder)
    return kim


def test_import_kim_keeps_history(m5_kim, tmp_path):
    repo = empty_repo(tmp_path / "repo")
    out = handlers.run("import_kim", repo, {"upload": m5_kim.name, "max_bytes": 1 << 30}, ctx(tmp_path))
    assert sorted(out["options"]) == ["Scheme%20B", "main"]
    assert out["default"] == "main"
    assert git(repo, "rev-parse", "main") == git(M5, "rev-parse", "main")
    assert git(repo, "rev-parse", "Scheme%20B") == git(M5, "rev-parse", "Scheme%20B")
    assert not m5_kim.exists(), "the upload is removed once imported"


def test_import_kim_rejects_bomb(tmp_path):
    bomb = tmp_path / "uploads" / "bomb.nic"
    bomb.parent.mkdir(exist_ok=True)
    with zipfile.ZipFile(bomb, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr(kimfile.MANIFEST, '{"format": "kim", "version": 1, "kind": "project"}')
        z.writestr(".git/objects/huge", b"\0" * (50 << 20))
    repo = empty_repo(tmp_path / "repo")
    with pytest.raises(handlers.JobError, match="too large"):
        handlers.run("import_kim", repo, {"upload": bomb.name, "max_bytes": 1 << 30}, ctx(tmp_path))


def test_import_kim_rejects_non_kim(tmp_path):
    junk = tmp_path / "uploads" / "junk.nic"
    junk.parent.mkdir(exist_ok=True)
    junk.write_bytes(b"not a zip")
    repo = empty_repo(tmp_path / "repo")
    with pytest.raises(handlers.JobError, match="NicBim"):
        handlers.run("import_kim", repo, {"upload": junk.name, "max_bytes": 1 << 30}, ctx(tmp_path))


def test_upload_name_cannot_escape(tmp_path):
    repo = empty_repo(tmp_path / "repo")
    with pytest.raises(handlers.JobError):
        handlers.run("import_kim", repo, {"upload": "../../etc/passwd", "max_bytes": 1 << 30}, ctx(tmp_path))


def test_import_ifc_first_version(tmp_path):
    if not HOUSE.is_file():
        pytest.skip("run `pixi run house` in nicbim first")
    up = tmp_path / "uploads" / "house.ifc"
    up.parent.mkdir(exist_ok=True)
    up.write_bytes(HOUSE.read_bytes())
    repo = empty_repo(tmp_path / "repo")
    out = handlers.run("import_ifc", repo, {"upload": "house.ifc", "option": "main", "message": "Imported from house.ifc",
                                           "author": ["Ana", "ana@example.com"]}, ctx(tmp_path))
    assert out["commit"] == git(repo, "rev-parse", "main")
    assert git(repo, "log", "-1", "--format=%s", "main") == "Imported from house.ifc"
    assert out["objects"] > 50


def test_export_kim_full_roundtrip(m5_kim, tmp_path):
    repo = empty_repo(tmp_path / "repo")
    handlers.run("import_kim", repo, {"upload": m5_kim.name, "max_bytes": 1 << 30}, ctx(tmp_path))
    out = handlers.run("export_kim", repo, {"scope": "full", "name": "m5"}, ctx(tmp_path))
    folder = tmp_path / "unpacked"
    kimfile.unpack(out["path"], folder)
    assert git(folder, "rev-parse", "Scheme%20B") == git(repo, "rev-parse", "Scheme%20B")


def test_export_kim_latest_has_one_commit(m5_kim, tmp_path):
    repo = empty_repo(tmp_path / "repo")
    handlers.run("import_kim", repo, {"upload": m5_kim.name, "max_bytes": 1 << 30}, ctx(tmp_path))
    out = handlers.run("export_kim", repo, {"scope": "latest", "option": "main", "name": "m5"}, ctx(tmp_path))
    folder = tmp_path / "unpacked"
    kimfile.unpack(out["path"], folder)
    assert git(folder, "rev-list", "--count", "main") == "1"
    assert git(folder, "rev-parse", "main^{tree}") == git(repo, "rev-parse", "main^{tree}")
    assert git(folder, "branch", "--format=%(refname:short)") == "main"


def test_import_kim_from_before_the_rename_explains(tmp_path):
    old = tmp_path / "uploads" / "old.nic"
    old.parent.mkdir(exist_ok=True)
    with zipfile.ZipFile(old, "w") as z:
        z.writestr("nicbim-file.json", '{"format": "kim", "version": 1, "kind": "project"}')
        z.writestr(".git/HEAD", "ref: refs/heads/main\n")
    repo = empty_repo(tmp_path / "repo")
    with pytest.raises(handlers.JobError, match="older version"):
        handlers.run("import_kim", repo, {"upload": old.name, "max_bytes": 1 << 30}, ctx(tmp_path))
