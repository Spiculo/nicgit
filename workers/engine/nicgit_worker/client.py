# SPDX-License-Identifier: AGPL-3.0-or-later
"""Talking to the server. Two clients, on purpose: the supervisor's pool token only claims jobs, and a job's
own token only reaches that job (decision N30)."""
from __future__ import annotations

import json
import os
import pathlib
import shutil
import urllib.error
import urllib.parse
import urllib.request


class PoolClient:
    """Used by the supervisor, which never runs engine code."""

    def __init__(self, url: str, token: str):
        self.url, self.token = url.rstrip("/"), token

    def claim(self, kinds: list[str]):
        req = urllib.request.Request(self.url + "/internal/jobs/claim", data=json.dumps({"kinds": kinds}).encode(), method="POST",
                                     headers={"content-type": "application/json", "x-worker-token": self.token})
        with urllib.request.urlopen(req, timeout=60) as r:
            data = r.read()
            return json.loads(data) if r.status == 200 and data else None


class JobClient:
    """Everything one job may do, under that job's token."""

    def __init__(self, url: str, job_id: str, token: str):
        self.base = f"{url.rstrip('/')}/internal/jobs/{job_id}"
        self.token = token
        self.git_url = self.base + "/git"

    def _headers(self) -> dict:
        return {"authorization": f"Bearer {self.token}"}

    def _post(self, path: str, body: dict) -> None:
        req = urllib.request.Request(self.base + path, data=json.dumps(body).encode(), method="POST",
                                     headers={**self._headers(), "content-type": "application/json"})
        with urllib.request.urlopen(req, timeout=60) as r:
            r.read()

    def progress(self, fraction: float) -> None:
        try:
            self._post("/progress", {"progress": fraction})
        except (urllib.error.URLError, OSError):
            pass

    def finish(self, *, result=None, error=None) -> None:
        self._post("/finish", {"result": result} if error is None else {"error": error})

    def git_env(self) -> dict:
        """The token reaches git through its environment: never in a URL, an argument or a config file."""
        return {
            **os.environ,
            "GIT_TERMINAL_PROMPT": "0",
            "GIT_CONFIG_COUNT": "2",
            "GIT_CONFIG_KEY_0": "http.extraHeader",
            "GIT_CONFIG_VALUE_0": f"Authorization: Bearer {self.token}",
            "GIT_CONFIG_KEY_1": "credential.helper",
            "GIT_CONFIG_VALUE_1": "",
        }

    def download(self, path: str, dest: pathlib.Path) -> bool:
        """Save a file of this job to `dest`; False if the server does not have it."""
        req = urllib.request.Request(self.base + "/" + urllib.parse.quote(path), headers=self._headers())
        try:
            with urllib.request.urlopen(req, timeout=600) as r:
                dest.parent.mkdir(parents=True, exist_ok=True)
                with dest.open("wb") as f:
                    shutil.copyfileobj(r, f, 1 << 20)
        except urllib.error.HTTPError as e:
            if e.code == 404:
                return False
            raise
        return True

    def upload(self, path: str, src: pathlib.Path) -> None:
        with src.open("rb") as f:
            req = urllib.request.Request(self.base + "/" + urllib.parse.quote(path), data=f, method="PUT",
                                         headers={**self._headers(), "content-length": str(src.stat().st_size),
                                                  "content-type": "application/octet-stream"})
            with urllib.request.urlopen(req, timeout=3600) as r:
                r.read()
