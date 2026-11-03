"""Fakes for the kill switch's tests (tools/killswitch/test_killswitch.py, tools/control/test_control_killswitch.py).
Nothing here talks to GitHub, Discord or any server.

  FakeGitHub   the gh adapter: real local git repos stand in for the three repos, so the bundles are real and checked
  FakeDiscord  the Discord adapter, in memory, with the same "no writes outside a run" guard as the real one
  LocalServer  the real ks_server.py, run on this machine against a temp portal-data, a fake soap-cmd.sh and a
               file:// realm list
  FakeSteps    Control's step runner: records the steps, answers an exit code
"""
from __future__ import annotations

import copy
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import killswitch as ks  # noqa: E402


def _git(*args, cwd=None):
    env = dict(os.environ, GIT_AUTHOR_NAME="Orion", GIT_AUTHOR_EMAIL="o@example.invalid", GIT_COMMITTER_NAME="Orion",
               GIT_COMMITTER_EMAIL="o@example.invalid", GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_SYSTEM=os.devnull)
    p = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, env=env)
    if p.returncode:
        raise RuntimeError(p.stderr)
    return p.stdout


class FakeGitHub:
    def __init__(self, root, login="orionopenwow", scopes=("repo", "read:org")):
        self.root = Path(root)
        self._login = login
        self._scopes = set(scopes)
        self.ops: list[tuple] = []
        self.backup_root = None          # set by the test: where a backup must exist before any delete
        self.fail_mirror = None          # a repo whose copy fails
        self.fail_verify = False
        self.repos = {}
        for full in ks.GH_REPOS:
            name = full.split("/")[1]
            src = self.root / "src" / name
            src.mkdir(parents=True)
            _git("init", "-q", "-b", "main", cwd=src)
            (src / "README.md").write_text(f"{name}\n")
            _git("add", ".", cwd=src)
            _git("commit", "-q", "-m", "first", cwd=src)
            _git("checkout", "-q", "-b", "feat", cwd=src)
            (src / "x.txt").write_text("x\n")
            _git("add", ".", cwd=src)
            _git("commit", "-q", "-m", "second", cwd=src)
            _git("tag", "v1", cwd=src)
            _git("checkout", "-q", "main", cwd=src)
            self.repos[full] = {"full_name": full, "archived": False, "private": False, "src": str(src)}
        self.shell = ks.Shell()

    def writes(self):
        return [o for o in self.ops if o[0] in ("edit", "delete")]

    def login(self):
        return self._login

    def scopes(self):
        return set(self._scopes)

    def repo(self, full):
        r = self.repos.get(full)
        return {k: v for k, v in r.items() if k != "src"} if r else None

    def releases(self, full):
        return [{"tag": "launcher-latest", "assets": [{"name": "Leyline.msi", "size": 10, "url": "https://example.invalid/x"}]}]

    def issues(self, full):
        return [{"number": 1, "title": "a bug", "state": "open", "pr": False}]

    def edit(self, full, **fields):
        if full not in self.repos:
            raise ks.Refused(f"no {full}")
        self.ops.append(("edit", full, dict(fields)))
        self.repos[full].update(fields)

    def delete(self, full):
        ready = bool(self.backup_root) and any(Path(self.backup_root).glob("Leyline-killswitch-backup-*/MANIFEST.json"))
        self.ops.append(("delete", full, ready))
        self.repos.pop(full)

    def mirror(self, full, dest):
        if full == self.fail_mirror:
            raise ks.Refused(f"couldn't copy {full}: fake failure")
        self.ops.append(("mirror", full))
        _git("clone", "-q", "--mirror", self.repos[full]["src"], str(dest))

    def git(self, *args, cwd=None):
        if self.fail_verify and args[:2] == ("bundle", "verify"):
            return 1, "", "fake: bundle broken"
        return self.shell.run(["git", *args], cwd=cwd)


class FakeDiscord:
    GID = "100"

    def __init__(self, backup_root=None):
        self.writes = False
        self.backup_root = backup_root
        self.ops: list[tuple] = []
        g = self.GID
        self.guild_doc = {"id": g, "name": "Leyline", "system_channel_id": "11"}
        self.chans = [
            {"id": "10", "name": "START", "type": 4, "position": 0, "parent_id": None, "permission_overwrites": []},
            {"id": "11", "name": "👋︱welcome", "type": 0, "position": 1, "parent_id": "10", "permission_overwrites": []},
            {"id": "12", "name": "📣︱announcements", "type": 5, "position": 2, "parent_id": "10",
             "permission_overwrites": [{"id": g, "type": 0, "allow": "0", "deny": str(1 << 11)}]},
            {"id": "20", "name": "PLAY", "type": 4, "position": 3, "parent_id": None, "permission_overwrites": []},
            {"id": "21", "name": "💬︱general", "type": 0, "position": 4, "parent_id": "20",
             "permission_overwrites": [{"id": g, "type": 0, "allow": str(1 << 6), "deny": "0"}]},
            {"id": "22", "name": "Hangout", "type": 2, "position": 5, "parent_id": "20", "permission_overwrites": []},
            {"id": "23", "name": "secret", "type": 0, "position": 6, "parent_id": None, "permission_overwrites": []},
        ]
        self.msgs = {"21": [{"id": str(9000 - i), "content": f"hi {i}", "author": {"id": "7", "username": "p"},
                             "timestamp": "2026-11-01T00:00:00", "attachments": []} for i in range(150)],
                     "11": [{"id": "501", "content": "welcome", "author": {"id": "7", "username": "p"}}]}
        self.pins = []
        self.next = 7000

    def _w(self, what):
        if not self.writes:
            raise RuntimeError(f"blocked a write outside a run: {what}")

    def guild(self):
        return copy.deepcopy(self.guild_doc)

    def channels(self, gid):
        return copy.deepcopy(self.chans)

    def roles(self, gid):
        return [{"id": self.GID, "name": "@everyone"}]

    def chan(self, cid):
        return next((c for c in self.chans if c["id"] == cid), None)

    def messages(self, cid, before=None, limit=100):
        if cid == "23":
            raise ks.Refused("Discord said 403: Missing Access")
        ms = self.msgs.get(cid, [])
        if before:
            ms = [m for m in ms if int(m["id"]) < int(before)]
        return copy.deepcopy(ms[:limit])

    def post(self, cid, content):
        self._w("post")
        self.next += 1
        m = {"id": str(self.next), "content": content, "author": {"id": "bot"}}
        self.msgs.setdefault(cid, []).insert(0, m)
        self.ops.append(("post", cid, content))
        return m

    def pin(self, cid, mid):
        self._w("pin")
        self.pins.append((cid, mid))
        self.ops.append(("pin", cid, mid))

    def unpin(self, cid, mid):
        self._w("unpin")
        self.pins.remove((cid, mid))

    def set_perm(self, cid, oid, allow, deny):
        self._w("perm")
        c = self.chan(cid)
        c["permission_overwrites"] = [o for o in c["permission_overwrites"] if o["id"] != oid] + [
            {"id": oid, "type": 0, "allow": str(allow), "deny": str(deny)}]
        self.ops.append(("perm", cid, allow, deny))

    def clear_perm(self, cid, oid):
        self._w("perm-clear")
        c = self.chan(cid)
        c["permission_overwrites"] = [o for o in c["permission_overwrites"] if o["id"] != oid]
        self.ops.append(("perm-clear", cid))

    def delete_channel(self, cid):
        self._w("delete")
        ready = bool(self.backup_root) and any(Path(self.backup_root).glob("Leyline-killswitch-backup-*/MANIFEST.json"))
        self.chans = [c for c in self.chans if c["id"] != cid]
        self.ops.append(("delete", cid, ready))


class LocalServer:
    """The real ks_server.py against a temp server tree. Same interface as killswitch.Server."""

    def __init__(self, root, realms=("aaaaaaaaaaaaaaaa", "bbbbbbbbbbbbbbbb")):
        self.root = Path(root)
        self.data = self.root / "portal-data"
        self.home = self.root / "killswitch"
        (self.data / "leyline").mkdir(parents=True)
        (self.data / "admin").mkdir(parents=True)
        (self.data / "profiles").mkdir(parents=True)
        (self.data / "profiles" / "accounts.json").write_text(json.dumps({"a": 1}))
        (self.data / "leyline" / "news.json").write_text(json.dumps(
            {"format": 1, "updated_at": "x", "posts": [{"id": "n1", "title": "Old news", "items": []}]}))
        (self.data / "leyline" / "version-policy.json").write_text(json.dumps({"minimum": None, "notice": "", "by": "o"}))
        (self.data / "admin" / "relay-block.json").write_text(json.dumps(
            {"format": 1, "blocked": {"cccccccccccccccc": {"at": "x"}}, "nodes_off": False}))
        self.soap_log = self.root / "soap.txt"
        self.soap = self.root / "soap-cmd.sh"
        self.soap.write_text(f"#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{self.soap_log}'\necho done\n")
        self.soap.chmod(0o755)
        self.realms = self.root / "realms.json"
        self.realms.write_text(json.dumps({"realms": [{"id": r} for r in realms] + [{"id": "not-a-realm"}]}))
        self.calls: list[tuple] = []
        self.fail = set()       # commands that fail
        self.mangle_fetch = False

    def snapshot(self):
        return {str(p.relative_to(self.data)): p.read_bytes() for p in sorted(self.data.rglob("*")) if p.is_file()}

    def call(self, cmd, payload=None, dry=False):
        self.calls.append((cmd, dry))
        if cmd in self.fail:
            return False, {"error": f"fake: {cmd} failed"}, []
        env = dict(os.environ, KS_PORTAL_DATA=str(self.data), KS_HOME=str(self.home), KS_SOAP=str(self.soap),
                   KS_REALMS_URL=self.realms.as_uri())
        p = subprocess.run([sys.executable, "-I", str(ks.SERVER_SCRIPT), cmd] + (["--dry-run"] if dry else []),
                           input=json.dumps(payload or {}), capture_output=True, text=True, env=env, timeout=120)
        return ks.parse_ks(p.returncode, p.stdout, p.stderr)

    def fetch(self, remote, local):
        shutil.copyfile(remote, local)
        if self.mangle_fetch:
            with open(local, "ab") as f:
                f.write(b"x")


class FakeSteps:
    def __init__(self, fail=()):
        self.ran: list[str] = []
        self.fail = set(fail)

    def __call__(self, step):
        self.ran.append(step)
        return 1 if step in self.fail else 0


def world(tmp=None):
    """A fresh set of fakes in a temp folder: (root, github, discord, server, steps, desktop)."""
    root = Path(tmp or tempfile.mkdtemp(prefix="leyline-ks-"))
    desk = root / "Desktop"
    desk.mkdir()
    gh = FakeGitHub(root / "gh")
    gh.backup_root = desk
    dc = FakeDiscord(desk)
    srv = LocalServer(root / "srv")
    return root, gh, dc, srv, FakeSteps(), desk
