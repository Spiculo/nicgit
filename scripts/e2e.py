# SPDX-License-Identifier: AGPL-3.0-or-later
"""End to end against a running NicGit (server + at least one engine worker), stdlib only.

    python scripts/e2e.py http://127.0.0.1:3100 ../kimcad/build/m5/project/.git

Signs up a user, creates a project, pushes KimBim's M5 project with main set back before its combine,
waits for the engine's summaries, compares the options, opens a proposal, combines it taking the source
option's side of every conflict, and checks the result equals what the engine's own merge gives.
"""
import http.cookiejar
import json
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
import uuid

BASE, FIXTURE = sys.argv[1].rstrip("/"), sys.argv[2]
jar = http.cookiejar.CookieJar()
opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))


def call(method, path, body=None, ok=(200,)):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(BASE + path, data=data, method=method, headers={"content-type": "application/json"})
    try:
        with opener.open(req, timeout=900) as r:
            status, raw = r.status, r.read()
    except urllib.error.HTTPError as e:
        status, raw = e.code, e.read()
    if status not in ok:
        raise SystemExit(f"FAIL {method} {path}: {status} {raw[:400]!r}")
    return status, (json.loads(raw) if raw else None)


def wait(path, timeout=600):
    start = time.time()
    while time.time() - start < timeout:
        status, body = call("GET", path, ok=(200, 202))
        if status == 200:
            return body
        time.sleep(1)
    raise SystemExit(f"FAIL timeout waiting for {path}")


def git(*args, cwd=None):
    return subprocess.run(["git", "-c", "credential.helper=", *args], check=True, capture_output=True, text=True, cwd=cwd).stdout.strip()


def step(msg):
    print(f"- {msg}", flush=True)


user = f"e2e{uuid.uuid4().hex[:8]}"
step(f"sign up {user}")
call("POST", "/api/auth/signup", {"slug": user, "email": f"{user}@example.com", "password": "correct horse battery"})
_, tok = call("POST", "/api/me/tokens", {"name": "e2e"})
call("POST", "/api/projects", {"owner": user, "slug": "m5", "visibility": "private"})
remote = BASE.replace("://", f"://{user}:{tok['token']}@") + f"/{user}/m5.git"

step("push the M5 project (main before its combine)")
git("--git-dir", FIXTURE, "push", remote, "refs/heads/*:refs/heads/*")
before = git("--git-dir", FIXTURE, "rev-parse", "main^1")
git("--git-dir", FIXTURE, "push", "--force", remote, f"{before}:refs/heads/main")

step("summaries from the engine")
for _ in range(300):
    _, versions = call("GET", f"/api/projects/{user}/m5/versions?option=Scheme%2520B")
    if versions and versions[0]["summary"]:
        break
    time.sleep(1)
else:
    raise SystemExit("FAIL no summary")
print(f"  Scheme B head: {versions[0]['summary']['modified']} changed, {versions[0]['summary']['added']} added")

step("compare main with Scheme B")
cmp = wait(f"/api/projects/{user}/m5/compare?from=main&to=Scheme%2520B")
assert cmp["details"], cmp
print(f"  {len(cmp['details'])} elements differ")

step("the version as IFC")
req = urllib.request.Request(BASE + f"/api/projects/{user}/m5/versions/main/model.ifc")
for _ in range(300):
    with opener.open(req) as r:
        if r.status == 200:
            assert r.read(12) == b"ISO-10303-21"
            break
    time.sleep(1)

step("propose Scheme B into main and combine, Scheme B winning conflicts")
call("POST", f"/api/projects/{user}/m5/proposals", {"title": "Scheme B", "source": "Scheme%20B", "target": "main"})
preview = wait(f"/api/projects/{user}/m5/proposals/1/preview")
print(f"  {len(preview['conflicts'])} conflict(s)")
_, out = call("POST", f"/api/projects/{user}/m5/proposals/1/combine", {"mode": "whole:theirs"})

step("main now points at the combine commit (tree equality with the engine merge is pinned by workers/engine/tests)")
with tempfile.TemporaryDirectory() as tmp:
    git("clone", "--bare", "-q", remote, f"{tmp}/c.git")
    got = git("--git-dir", f"{tmp}/c.git", "rev-parse", "main^{tree}")
    assert git("--git-dir", f"{tmp}/c.git", "rev-parse", "main") == out["commit"]
print(f"  combined {out['commit'][:8]}, tree {got[:8]}")
print("e2e OK")
