#!/usr/bin/env python3
"""The kill switch (docs/KILL-SWITCH.md): Leyline Control's card that takes Leyline down, with a farewell message.

Two stages, each after its own read-only preview and a typed phrase, run only from the Control card (never a step,
never the queue, never Autopilot):
  * Pause: the message posted everywhere, then everything stopped or hidden, and all of it can be undone;
  * Delete: only after a Pause, only the parts that Pause took down, only after a full backup that checked out.

Parts (each with preview / pause / resume / delete): message, realms, site, discord, github. The services sit behind
small adapters (GitHub = the gh CLI, Discord = its HTTP API with the bot token, Server = ks_server.py over SSH,
Steps = Control's own step runner), so the tests run every part against fakes (tools/killswitch/fakes.py).
Standard library only.
"""
from __future__ import annotations

import datetime as _dt
import hashlib
import json
import os
import re
import secrets
import shutil
import subprocess
import tarfile
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
SERVER_SCRIPT = HERE / "server" / "ks_server.py"

PAUSE_PHRASE = "SHUT DOWN LEYLINE"
DELETE_PHRASE = "DELETE LEYLINE FOREVER"
RESUME_PHRASE = "BRING LEYLINE BACK"
PHRASES = {"pause": PAUSE_PHRASE, "delete": DELETE_PHRASE, "resume": RESUME_PHRASE}

PARTS = ("message", "realms", "site", "discord", "github")
PART_TITLES = {"message": "Community message", "realms": "Realms and network", "site": "Website",
               "discord": "Discord", "github": "GitHub"}
DELETABLE = ("site", "discord", "github")
SERVER_PARTS = ("message", "realms", "site")

GH_OWNER = "orionopenwow"
GH_REPOS = ("orionopenwow/leyline-workspace", "orionopenwow/Leyline", "orionopenwow/Leyline-Launcher")
# the existing Control steps that take realms and extras off (never rollback.sh or docker compose up)
REALM_STEPS = ("hangout-off", "vr-beacon-off", "build-realm-off", "ov112-realm-off", "sv-site-off", "gamedata-off")
REALM_STEPS_BACK = {"hangout-off": "hangout", "vr-beacon-off": "vr-beacon", "build-realm-off": "build-realm-on",
                    "ov112-realm-off": "ov112-on", "sv-site-off": "sv-site", "gamedata-off": "gamedata-on"}

PREVIEW_FRESH = 3600              # a stage needs a preview from the last hour
DELETE_WAIT = 24 * 3600           # Delete only a day after the Pause (owner decision; docs/KILL-SWITCH.md)
MESSAGE_MAX = 1800
TITLE_MAX = 80
DISCORD_EXPORT_MAX = 20000        # messages a channel in the backup

DEFAULT_TITLE = "Leyline is closing"
DEFAULT_MESSAGE = ("TODO(owner): your farewell to the community, in your own words. What is closing, from when, "
                   "and thank you. Pause won't run while this line is here.")

LEGAL_LINE = "not affiliated with or endorsed by Blizzard Entertainment"
BRAND_RE = re.compile(r"(?i)world\s+of\s+warcraft|\bwow\b|warcraft|lich\s+king|blizzard")

# Discord permission bits that make a channel read-only for @everyone
LOCK_BITS = ((1 << 6) | (1 << 11) | (1 << 20) | (1 << 21) | (1 << 35) | (1 << 36) | (1 << 38) | (1 << 46))
TEXTY = (0, 2, 5, 13, 15)          # text, voice (its chat), announcement, stage, forum
CATEGORY = 4


class Refused(Exception):
    """Stops a stage before it changes anything (or, mid-Delete, before the next change)."""


# --------------------------------------------------------------------------------------------------------- checks
def message_problems(title: str, message: str) -> list[str]:
    out = []
    if not str(message or "").strip():
        out.append("Write the farewell message first.")
    if "TODO(owner)" in str(message) or "TODO(owner)" in str(title):
        out.append("The message still has TODO(owner) in it: write your own words first.")
    if len(str(message or "")) > MESSAGE_MAX:
        out.append(f"The message is over {MESSAGE_MAX} characters.")
    if len(str(title or "")) > TITLE_MAX:
        out.append(f"The title is over {TITLE_MAX} characters.")
    if BRAND_RE.search(str(title or "") + "\n" + re.sub(re.escape(LEGAL_LINE), "", str(message or ""), flags=re.I)):
        out.append("The message has a brand word players must not see (only the legal line may name the maker).")
    return out


def clean_parts(parts) -> list[str]:
    return [p for p in PARTS if p in set(parts or [])]


def clean_options(o) -> dict:
    o = o if isinstance(o, dict) else {}
    return {"private": o.get("private") is True, "dry": o.get("dry") is True}


def preview_key(stage: str, title: str, message: str, parts, options) -> str:
    """What a preview was made for: the stage, the message, the parts and the choices. Go needs the same key."""
    raw = json.dumps([stage, title, message, clean_parts(parts), clean_options(options)], ensure_ascii=False)
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()[:24]


def new_id() -> str:
    return time.strftime("%Y%m%d-%H%M%S") + "-" + secrets.token_hex(2)


def sha256_file(path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


# --------------------------------------------------------------------------------------------------------- the log
class Log:
    """Every line to logs/killswitch-<time>.log and the page, secrets hidden (the bot token by its exact text, and
    whatever Control's scrub() hides)."""

    def __init__(self, path: Path | None, emit=None, scrub=None):
        self.path, self.emit, self.scrub = path, emit, scrub
        self.secrets: list[str] = []
        self.lines: list[str] = []
        if path:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("", encoding="utf-8")

    def hide(self, s: str) -> None:
        if s and len(s) >= 8:
            self.secrets.append(s)

    def clean(self, line: str) -> str:
        for s in self.secrets:
            line = line.replace(s, "[hidden]")
            for part in s.split("."):
                if len(part) >= 6:
                    line = line.replace(part, "[hidden]")
        return self.scrub(line) if self.scrub else line

    def __call__(self, *parts) -> None:
        line = self.clean(" ".join(str(p) for p in parts))
        self.lines.append(line)
        if self.path:
            with open(self.path, "a", encoding="utf-8") as f:
                f.write(line + "\n")
        if self.emit:
            self.emit(line)


# --------------------------------------------------------------------------------------------------------- adapters
class Shell:
    """Runs a program, never through a shell. Output is text (UTF-8)."""

    def run(self, argv, input=None, timeout=900, cwd=None, env=None):
        p = subprocess.run([str(a) for a in argv], input=input, capture_output=True, timeout=timeout, cwd=cwd, env=env,
                           creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        dec = lambda b: (b or b"").decode("utf-8", "replace")   # noqa: E731
        return p.returncode, dec(p.stdout), dec(p.stderr)

    def run_to_file(self, argv, dest, timeout=3600):
        with open(dest, "wb") as f:
            p = subprocess.run([str(a) for a in argv], stdout=f, stderr=subprocess.PIPE, timeout=timeout,
                               creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
        return p.returncode, (p.stderr or b"").decode("utf-8", "replace")


class GitHub:
    """The gh CLI, signed in on this PC. The tool never reads or prints its token."""

    def __init__(self, shell=None, gh="gh", git="git"):
        self.sh, self.gh, self.git = shell or Shell(), gh, git

    def _api(self, *args, input=None):
        rc, out, err = self.sh.run([self.gh, "api", *args], input=input)
        return rc, out, err

    def login(self):
        rc, out, _ = self._api("user", "--jq", ".login")
        return out.strip() if rc == 0 and out.strip() else None

    def scopes(self) -> set:
        rc, out, _ = self._api("-i", "user")
        for line in out.splitlines():
            if line.lower().startswith("x-oauth-scopes:"):
                return {s.strip() for s in line.split(":", 1)[1].split(",") if s.strip()}
        return set()

    def repo(self, full):
        rc, out, err = self._api(f"repos/{full}")
        if rc != 0:
            if "404" in err or "Not Found" in err or "Not Found" in out:
                return None
            raise Refused(f"GitHub didn't answer for {full}: {err.strip()[:200]}")
        try:
            return json.loads(out)
        except ValueError:
            raise Refused(f"GitHub's answer for {full} wasn't JSON") from None

    def _pages(self, path):
        items, page = [], 1
        while page <= 50:
            rc, out, err = self._api(f"{path}{'&' if '?' in path else '?'}per_page=100&page={page}")
            if rc != 0:
                raise Refused(f"GitHub didn't list {path}: {err.strip()[:200]}")
            got = json.loads(out or "[]")
            items += got
            if len(got) < 100:
                break
            page += 1
        return items

    def releases(self, full):
        return [{"tag": r.get("tag_name"), "name": r.get("name"), "draft": r.get("draft"), "published": r.get("published_at"),
                 "assets": [{"name": a.get("name"), "size": a.get("size"), "url": a.get("browser_download_url"),
                             "downloads": a.get("download_count")} for a in r.get("assets") or []]}
                for r in self._pages(f"repos/{full}/releases")]

    def issues(self, full):
        return [{"number": i.get("number"), "title": i.get("title"), "state": i.get("state"), "pr": "pull_request" in i,
                 "user": (i.get("user") or {}).get("login"), "created": i.get("created_at"), "body": i.get("body")}
                for i in self._pages(f"repos/{full}/issues?state=all")]

    def edit(self, full, **fields):
        args = ["-X", "PATCH", f"repos/{full}"]
        for k, v in fields.items():
            args += ["-F", f"{k}={'true' if v is True else 'false' if v is False else v}"]
        rc, _out, err = self._api(*args)
        if rc != 0:
            raise Refused(f"GitHub refused to change {full}: {err.strip()[:200]}")

    def delete(self, full):
        rc, _out, err = self._api("-X", "DELETE", f"repos/{full}")
        if rc != 0:
            raise Refused(f"GitHub refused to delete {full}: {err.strip()[:200]}")

    def mirror(self, full, dest):
        rc, _out, err = self.sh.run([self.gh, "repo", "clone", full, str(dest), "--", "--mirror"], timeout=3600)
        if rc != 0:
            raise Refused(f"couldn't copy {full}: {err.strip()[:200]}")

    def git(self, *args, cwd=None):
        return self.sh.run([self.git, *args], cwd=cwd, timeout=3600)


class DiscordAPI:
    """The Discord API with the bot token (from the Discord card's token.txt). Writes only when allowed."""

    def __init__(self, token, api_base=None, sleep=time.sleep):
        self._token = token
        self.base = (api_base or os.environ.get("LEYLINE_DISCORD_API") or "https://discord.com/api/v10").rstrip("/")
        self.sleep = sleep
        self.writes = False

    def request(self, method, path, body=None, query=None):
        if method != "GET" and not self.writes:
            raise RuntimeError(f"blocked a {method} outside a run")
        url = self.base + path + ("?" + urllib.parse.urlencode(query) if query else "")
        data = json.dumps(body).encode() if body is not None else None
        headers = {"Authorization": "Bot " + self._token, "User-Agent": "LeylineKillSwitch (local, 1)",
                   "X-Audit-Log-Reason": urllib.parse.quote("Leyline kill switch (owner)")}
        if data is not None:
            headers["Content-Type"] = "application/json"
        last = "retries"
        for attempt in range(8):
            try:
                with urllib.request.urlopen(urllib.request.Request(url, data=data, method=method, headers=headers),
                                            timeout=60) as r:
                    raw = r.read()
                    return json.loads(raw) if raw else None
            except urllib.error.HTTPError as e:
                raw = e.read()
                try:
                    info = json.loads(raw) if raw else {}
                except ValueError:
                    info = {}
                if e.code == 429 or e.code >= 500:
                    self.sleep(float(info.get("retry_after") or 0.5 * (attempt + 1)) + 0.05)
                    continue
                raise Refused(f"Discord said {e.code}: {info.get('message') or 'error'}") from None
            except (urllib.error.URLError, TimeoutError, ConnectionError) as e:
                self.sleep(0.5 * (attempt + 1))
                last = type(e).__name__
        raise Refused(f"Discord didn't answer ({last})")

    def guild(self):
        gs = self.request("GET", "/users/@me/guilds") or []
        if len(gs) != 1:
            raise Refused("The bot must be in exactly one server (yours)." if gs else "The bot isn't in any server.")
        return self.request("GET", f"/guilds/{gs[0]['id']}")

    def channels(self, gid):
        return self.request("GET", f"/guilds/{gid}/channels") or []

    def roles(self, gid):
        return self.request("GET", f"/guilds/{gid}/roles") or []

    def messages(self, cid, before=None, limit=100):
        q = {"limit": limit}
        if before:
            q["before"] = before
        return self.request("GET", f"/channels/{cid}/messages", query=q) or []

    def post(self, cid, content):
        return self.request("POST", f"/channels/{cid}/messages", {"content": content, "allowed_mentions": {"parse": []}})

    def pin(self, cid, mid):
        self.request("PUT", f"/channels/{cid}/pins/{mid}")

    def unpin(self, cid, mid):
        self.request("DELETE", f"/channels/{cid}/pins/{mid}")

    def set_perm(self, cid, oid, allow, deny):
        self.request("PUT", f"/channels/{cid}/permissions/{oid}", {"type": 0, "allow": str(allow), "deny": str(deny)})

    def clear_perm(self, cid, oid):
        self.request("DELETE", f"/channels/{cid}/permissions/{oid}")

    def delete_channel(self, cid):
        self.request("DELETE", f"/channels/{cid}")


class Server:
    """ks_server.py on the server, over SSH with the owner's key (the same login the deploy steps use)."""
    REMOTE = "/srv/wrath/killswitch/ks_server.py"

    def __init__(self, shell=None, host=None, key=None):
        self.sh = shell or Shell()
        self.host = host or os.environ.get("KS_SSH_HOST", "root@play.openvanilla.app")
        self.key = key or str(Path(os.environ.get("USERPROFILE") or Path.home()) / ".ssh" / "openwow")
        self.uploaded = False

    def _ssh(self):
        return ["ssh", "-i", self.key, "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=accept-new",
                "-o", "ServerAliveInterval=30", self.host]

    def upload(self):
        if self.uploaded:
            return
        rc, _o, err = self.sh.run(self._ssh() + [f"mkdir -p /srv/wrath/killswitch && cat > {self.REMOTE}.new && "
                                                 f"mv {self.REMOTE}.new {self.REMOTE}"],
                                  input=SERVER_SCRIPT.read_bytes(), timeout=120)
        if rc != 0:
            raise Refused(f"couldn't reach the server over SSH: {err.strip()[:200]}")
        self.uploaded = True

    def call(self, cmd, payload=None, dry=False):
        """(ok, the answer's JSON, its lines)."""
        self.upload()
        rc, out, err = self.sh.run(self._ssh() + [f"python3 {self.REMOTE} {cmd}" + (" --dry-run" if dry else "")],
                                   input=json.dumps(payload or {}).encode("utf-8"), timeout=1800)
        return parse_ks(rc, out, err)

    def fetch(self, remote, local):
        if not re.fullmatch(r"/srv/wrath/killswitch/backups/[0-9A-Za-z._-]+", str(remote)):
            raise Refused("that isn't a kill switch backup")
        rc, err = self.sh.run_to_file(self._ssh() + [f"cat {remote}"], local)
        if rc != 0:
            raise Refused(f"couldn't copy the backup down: {err.strip()[:200]}")


def parse_ks(rc, out, err):
    lines = [l for l in (out or "").splitlines() if l.strip()]
    doc = {}
    for l in lines:
        if l.startswith("KS-JSON "):
            try:
                doc = json.loads(l[8:])
            except ValueError:
                doc = {}
    ok = rc == 0 and any(l.strip() == "KS OK" for l in lines)
    if not ok and not doc.get("error"):
        doc["error"] = (err or "").strip()[:200] or "no answer"
    return ok, doc, [l for l in lines if not l.startswith("KS-JSON ")]


# --------------------------------------------------------------------------------------------------------- context
class Ctx:
    def __init__(self, *, title, message, parts, options, log, github=None, discord=None, server=None, run_step=None,
                 backup_root=None, now=time.time):
        self.title = " ".join(str(title or DEFAULT_TITLE).split())[:TITLE_MAX]
        self.message = str(message or "")
        self.parts = clean_parts(parts)
        self.options = clean_options(options)
        self.dry = self.options["dry"]
        self.log = log
        self.gh = github
        self.dc = discord
        self.srv = server
        self.run_step = run_step
        self.backup_root = Path(backup_root) if backup_root else desktop()
        self.now = now
        self._status = None
        self._guild = None

    def server_status(self):
        if self._status is None:
            if not self.srv:
                self._status = {"error": "no server connection"}
            else:
                try:
                    ok, doc, _ = self.srv.call("status")
                    self._status = doc if ok else {"error": doc.get("error") or "the server didn't answer"}
                except Refused as e:
                    self._status = {"error": str(e)}
        return self._status

    def guild(self):
        if self._guild is None and self.dc:
            self._guild = self.dc.guild()
        return self._guild

    def discord_text(self):
        return f"**{self.title}**\n\n{self.message}"[:2000]


def desktop() -> Path:
    home = Path(os.environ.get("USERPROFILE") or Path.home())
    for d in (Path(os.environ["OneDrive"]) / "Desktop" if os.environ.get("OneDrive") else None, home / "Desktop"):
        if d and d.is_dir():
            return d
    return home


def item(part, kind, text):
    return {"part": part, "kind": kind, "text": text}


def announce_channel(channels, guild):
    texty = sorted([c for c in channels if c.get("type") in (0, 5)], key=lambda c: (c.get("position") or 0, int(c["id"])))
    for c in texty:
        if "announcement" in c.get("name", "").lower():
            return c
    sid = (guild or {}).get("system_channel_id")
    return next((c for c in texty if c["id"] == sid), texty[0] if texty else None)


# --------------------------------------------------------------------------------------------------------- preview
def preview(ctx: Ctx, stage: str, paused: dict | None = None) -> dict:
    """Read-only: what the stage would do, part by part, and what blocks it. Never writes anywhere."""
    if stage not in ("pause", "delete", "resume"):
        raise ValueError(stage)
    items, problems = [], []
    L = ctx.log

    def problem(part, text):
        problems.append(f"{PART_TITLES.get(part, part)}: {text}")
        items.append(item(part, "problem", text))

    if not ctx.parts:
        problems.append("Tick at least one part.")
    paused_parts = set((paused or {}).get("parts") or [])
    if stage == "pause":
        for x in message_problems(ctx.title, ctx.message):
            problem("message", x)
    if stage == "delete":
        if not paused or not paused.get("ok"):
            problems.append("Delete comes only after a finished Pause.")
        else:
            wait = DELETE_WAIT - (ctx.now() - float(paused.get("at") or 0))
            if wait > 0 and ctx.dry:
                items.append(item("all", "keep", "A real Delete opens a day after the Pause; this dry run may go now."))
            elif wait > 0:
                problems.append(f"Delete opens {DELETE_WAIT // 3600} hours after the Pause: {int(wait // 3600)} h "
                                f"{int(wait % 3600 // 60)} min to go.")
        for p in ctx.parts:
            if p not in DELETABLE:
                items.append(item(p, "keep", "Nothing to delete here: Pause already did all of it."))
            elif p not in paused_parts:
                problem(p, "This part wasn't taken down by the Pause. Pause it first.")
    if stage == "resume" and not paused:
        problems.append("Nothing is paused.")

    need_server = any(p in ctx.parts for p in SERVER_PARTS) and not (stage == "delete" and "site" not in ctx.parts)
    st = ctx.server_status() if need_server else {}
    if need_server and st.get("error"):
        problem("site" if "site" in ctx.parts else ctx.parts[0], f"The server didn't answer: {st['error']}")
        st = {}

    guild = chans = None
    if "discord" in ctx.parts or "message" in ctx.parts:
        if not ctx.dc:
            if "discord" in ctx.parts:
                problem("discord", "No Discord bot token: save it in the Discord card first.")
            else:
                items.append(item("message", "keep", "No Discord bot token saved: the message is not posted on Discord."))
        else:
            try:
                guild = ctx.guild()
                chans = ctx.dc.channels(guild["id"])
            except (Refused, KeyError, TypeError) as e:
                problem("discord" if "discord" in ctx.parts else "message", f"Discord: {e}")

    gh_login = None
    if "github" in ctx.parts:
        if not ctx.gh:
            problem("github", "The gh tool isn't there.")
        else:
            gh_login = ctx.gh.login()
            if gh_login is None:
                problem("github", "gh isn't signed in. In a terminal: gh auth login (as orionopenwow).")
            elif gh_login != GH_OWNER:
                problem("github", f"gh is signed in as {gh_login}, not {GH_OWNER}. Nothing will run on GitHub until "
                                  f"you switch (gh auth switch, or gh auth login as {GH_OWNER}).")
                gh_login = None

    for p in ctx.parts:
        if stage == "pause":
            _preview_pause(ctx, p, items, problem, st, guild, chans, gh_login)
        elif stage == "resume":
            _preview_resume(ctx, p, items, paused)
        elif p in DELETABLE and p in paused_parts:
            _preview_delete(ctx, p, items, problem, st, guild, chans, gh_login)

    if stage == "delete":
        items.append(item("backup", "do", f"First a full backup to {ctx.backup_root / 'Leyline-killswitch-backup-<time>'}: "
                                          "git bundles of every repo (all refs), the release and issue lists"
                          + (", the Discord server and its messages" if ctx.dc else "")
                          + (", the website's data" if "site" in ctx.parts else "")
                          + ". Every file checked. If any of it fails, nothing is deleted."))
        if "github" not in ctx.parts and ctx.gh and ctx.gh.login() != GH_OWNER:
            problem("github", "The backup takes the repos too, and gh isn't signed in as orionopenwow.")
    if ctx.dry:
        items.append(item("all", "keep", "Dry run: it checks and logs everything and changes nothing."))
    for i in items:
        L(f"  [{PART_TITLES.get(i['part'], i['part'])}] {i['kind']}: {i['text']}")
    for p in problems:
        L(f"  PROBLEM: {p}")
    return {"stage": stage, "items": items, "problems": problems, "parts": ctx.parts, "options": ctx.options,
            "key": preview_key(stage, ctx.title, ctx.message, ctx.parts, ctx.options), "at": ctx.now(), "id": new_id()}


def _preview_pause(ctx, p, items, problem, st, guild, chans, gh_login):
    if p == "message":
        if guild and chans is not None:
            ch = announce_channel(chans, guild)
            if ch:
                items.append(item(p, "do", f"Discord: post the message in #{ch['name']} of {guild.get('name')} and pin it."))
            else:
                problem(p, "Discord: no text channel to post in.")
        if st:
            items.append(item(p, "do", "Website: a banner with the message on top of every page."
                              + (" (Website is ticked too: the farewell page replaces every page.)" if "site" in ctx.parts else "")))
            items.append(item(p, "do", f"Launcher: the message as the top News post ({st.get('news_posts', 0)} there now) "
                                       "and as the update notice every launcher shows."))
            if st.get("soap"):
                items.append(item(p, "do", "Realm: announce it to everyone online, and make it the message of the day."))
            else:
                items.append(item(p, "manual", "Realm: SOAP didn't answer, so no in-game announcement. "
                                               f"({st.get('soap_said') or 'no answer'})"))
    elif p == "realms":
        if st:
            n = len(st.get("realms") or [])
            if st.get("realms_error"):
                problem(p, st["realms_error"])
            items.append(item(p, "do", "Relay: Stop every node" + (" (already on)" if st.get("nodes_off") else "")
                              + f", and a staff block on all {n} realm(s) in the realm list."))
        items.append(item(p, "do", "Then these Control steps, one by one (a failure is logged and the rest go on): "
                          + ", ".join(REALM_STEPS) + "."))
        items.append(item(p, "manual", "The main realm keeps running on its machine: switch that off at the host if "
                                       "you want it off too. Seeded realms: Stop every node covers them."))
    elif p == "site":
        if st:
            items.append(item(p, "do", "Every page on leylinelauncher.com answers the farewell page. The launcher's "
                                       "feeds, the status page, the art and HQ keep answering."))
        items.append(item(p, "manual", "DNS is yours: it stays as it is."))
    elif p == "discord":
        if chans is not None:
            items.append(item(p, "do", f"Make all {len(chans)} channels read-only for @everyone (posting, threads, "
                                       "reactions, voice). The old permissions are saved for Undo."))
    elif p == "github":
        _preview_repos(ctx, items, problem, gh_login, "pause")


def _preview_repos(ctx, items, problem, gh_login, stage):
    if not gh_login:
        return
    for full in GH_REPOS:
        try:
            r = ctx.gh.repo(full)
        except Refused as e:
            problem("github", str(e))
            continue
        if r is None:
            items.append(item("github", "keep", f"{full}: not there (nothing to do)."))
            continue
        if str(r.get("full_name", "")).lower() != full.lower():
            problem("github", f"{full} now answers as {r.get('full_name')}: check the name first.")
            continue
        state = ("archived" if r.get("archived") else "open") + (", private" if r.get("private") else ", public")
        if stage == "pause":
            act = "archive it (read-only)" + (" and make it private" if ctx.options["private"] else "")
            items.append(item("github", "do", f"{full} ({state}): {act}."))
        else:
            items.append(item("github", "do", f"{full} ({state}): DELETE it, with its releases, issues and settings."))
    if stage == "pause" and not ctx.options["private"]:
        items.append(item("github", "keep", "The repos stay public (read-only). Tick 'make private' to hide them."))
    if stage == "delete":
        if "delete_repo" not in ctx.gh.scopes():
            problem("github", "gh can't delete repos yet. In a terminal: gh auth refresh -h github.com -s delete_repo")
        items.append(item("github", "manual", "openvanilla.app's Pages site goes with Leyline-Launcher."))


def _preview_delete(ctx, p, items, problem, st, guild, chans, gh_login):
    if p == "site":
        if st:
            mb = (st.get("data_bytes") or 0) / 1e6
            items.append(item(p, "do", f"Back up the website's data ({mb:.1f} MB: accounts, Hero Keys, profiles, the "
                                       "forum), copy it here and check it, then wipe it on the server. The farewell "
                                       "page stays."))
        items.append(item(p, "manual", "DNS: remove the names or point them away (Squarespace). Then cancel the "
                                       "server when you're done."))
    elif p == "discord":
        if chans is not None:
            keep = announce_channel(chans, guild)
            n = len([c for c in chans if not keep or c["id"] not in (keep["id"], keep.get("parent_id"))])
            items.append(item(p, "do", f"Delete {n} channel(s), all but #{keep['name'] if keep else '?'} with the farewell."))
        items.append(item(p, "manual", "Delete the Discord server yourself: Discord app > Server Settings > Delete "
                                       "Server. Only its owner can; a bot can't."))
    elif p == "github":
        _preview_repos(ctx, items, problem, gh_login, "delete")


def _preview_resume(ctx, p, items, paused):
    rec = (paused or {}).get("results", {}).get(p) or {}
    if p in ("message", "site", "realms"):
        items.append(item(p, "do", {"message": "Take the banner, the News post and the update notice back.",
                                    "site": "Take the farewell page off: the website answers as before.",
                                    "realms": "Relay: nodes and blocks back as they were."}[p]))
    if p == "realms":
        back = [REALM_STEPS_BACK[s] for s in rec.get("steps_ok") or []]
        items.append(item(p, "manual", "Turn back on what you want, with its own step: " + (", ".join(back) or "none ran")))
    if p == "message":
        items.append(item(p, "manual", "The Discord post stays (delete it yourself if you want). Set the realm's "
                                       "message of the day back."))
    if p == "discord":
        items.append(item(p, "do", f"Put back the saved permissions on {len(rec.get('saved') or {})} channel(s)."))
    if p == "github":
        items.append(item(p, "do", "Unarchive the repos" + (" and make them public again." if rec.get("made_private") else ".")))


# --------------------------------------------------------------------------------------------------------- pause
def run_pause(ctx: Ctx, run_id: str) -> dict:
    """The message everywhere, then everything stopped or hidden. Returns the record that Undo and Delete use."""
    L = ctx.log
    res, problems = {}, []
    if message_problems(ctx.title, ctx.message):
        raise Refused(" ".join(message_problems(ctx.title, ctx.message)))
    if "github" in ctx.parts and (not ctx.gh or ctx.gh.login() != GH_OWNER):
        raise Refused(f"gh isn't signed in as {GH_OWNER}: nothing was done.")
    if ctx.dc:
        ctx.dc.writes = not ctx.dry
    L(f"== Pause {run_id}{' (dry run)' if ctx.dry else ''}: {', '.join(PART_TITLES[p] for p in ctx.parts)}")

    # 1. the message, first, so people read it before things go quiet
    if "message" in ctx.parts and ctx.dc:
        r = res.setdefault("message", {})
        try:
            g = ctx.guild()
            ch = announce_channel(ctx.dc.channels(g["id"]), g)
            if not ch:
                raise Refused("no text channel to post in")
            if ctx.dry:
                L(f"  (dry run) would post and pin the message in #{ch['name']}")
            else:
                msg = ctx.dc.post(ch["id"], ctx.discord_text())
                ctx.dc.pin(ch["id"], msg["id"])
                r.update(channel=ch["id"], post=msg["id"])
                L(f"  Discord: posted and pinned in #{ch['name']}")
            r["channel"] = ch["id"]
        except Refused as e:
            problems.append(f"Discord message: {e}")
            L(f"  !! Discord message: {e}")

    # 2. the server: the banner / farewell page, News, the update notice, the realm, the relay
    sparts = [p for p in ctx.parts if p in SERVER_PARTS]
    if sparts:
        ok, doc, lines = ctx.srv.call("pause", {"id": run_id, "title": ctx.title, "message": ctx.message,
                                                "parts": sparts}, dry=ctx.dry)
        for l in lines:
            L("  server: " + l)
        if not ok:
            problems.append(f"Server: {doc.get('error')}")
        for p in sparts:
            res.setdefault(p, {})["server"] = ok
        problems += [f"Server: {x}" for x in doc.get("problems") or []]

    # 3. realms: the existing Control steps
    if "realms" in ctx.parts:
        r = res.setdefault("realms", {})
        r["steps_ok"], r["steps_failed"] = [], []
        for step in REALM_STEPS:
            if ctx.dry:
                L(f"  (dry run) would run the step {step}")
                continue
            rc = ctx.run_step(step) if ctx.run_step else 1
            (r["steps_ok"] if rc == 0 else r["steps_failed"]).append(step)
            L(f"  step {step}: {'done' if rc == 0 else f'failed (exit {rc}), going on'}")
        if r["steps_failed"]:
            problems.append("Realm steps that failed: " + ", ".join(r["steps_failed"]))

    # 4. Discord read-only
    if "discord" in ctx.parts:
        r = res.setdefault("discord", {"saved": {}})
        try:
            if not ctx.dc:
                raise Refused("no bot token saved")
            g = ctx.guild()
            gid = g["id"]
            for c in ctx.dc.channels(gid):
                old = next((o for o in c.get("permission_overwrites") or [] if o.get("id") == gid), None)
                allow, deny = int((old or {}).get("allow") or 0), int((old or {}).get("deny") or 0)
                if deny & LOCK_BITS == LOCK_BITS and not allow & LOCK_BITS:
                    continue
                r["saved"].setdefault(c["id"], {"allow": str(allow), "deny": str(deny)} if old else None)
                if ctx.dry:
                    L(f"  (dry run) would make #{c['name']} read-only")
                    continue
                ctx.dc.set_perm(c["id"], gid, allow & ~LOCK_BITS, deny | LOCK_BITS)
            L(f"  Discord: {len(r['saved'])} channel(s) read-only for @everyone")
        except Refused as e:
            problems.append(f"Discord lock: {e}")
            L(f"  !! Discord lock: {e}")

    # 5. GitHub: archived (and private if ticked), last
    if "github" in ctx.parts:
        r = res.setdefault("github", {"archived": [], "made_private": []})
        for full in GH_REPOS:
            try:
                info = ctx.gh.repo(full)
                if info is None:
                    L(f"  GitHub: {full} isn't there")
                    continue
                if ctx.options["private"] and not info.get("private"):
                    if ctx.dry:
                        L(f"  (dry run) would make {full} private")
                    else:
                        ctx.gh.edit(full, private=True)
                        r["made_private"].append(full)
                if not info.get("archived"):
                    if ctx.dry:
                        L(f"  (dry run) would archive {full}")
                    else:
                        ctx.gh.edit(full, archived=True)
                        r["archived"].append(full)
                L(f"  GitHub: {full} archived" + (", private" if ctx.options["private"] else ""))
            except Refused as e:
                problems.append(f"GitHub: {e}")
                L(f"  !! GitHub: {e}")

    for p in problems:
        L(f"  PROBLEM: {p}")
    rec = {"id": run_id, "at": ctx.now(), "parts": ctx.parts, "options": ctx.options, "dry": ctx.dry,
           "ok": not problems and not ctx.dry, "problems": problems, "results": res}
    L(f"== Pause {'(dry run) checked' if ctx.dry else 'done' if rec['ok'] else 'done, with problems (see above)'}")
    return rec


# --------------------------------------------------------------------------------------------------------- resume
def run_resume(ctx: Ctx, paused: dict) -> dict:
    L = ctx.log
    if not paused:
        raise Refused("nothing is paused")
    if paused.get("deleted"):
        raise Refused("a Delete already ran: there is nothing to bring back")
    res = paused.get("results") or {}
    problems = []
    if ctx.dc:
        ctx.dc.writes = not ctx.dry
    L(f"== Undo pause {paused['id']}{' (dry run)' if ctx.dry else ''}")
    if any(p in paused.get("parts", []) for p in SERVER_PARTS):
        ok, doc, lines = ctx.srv.call("resume", {"id": paused["id"]}, dry=ctx.dry)
        for l in lines:
            L("  server: " + l)
        if not ok:
            problems.append(f"Server: {doc.get('error')}")
    saved = (res.get("discord") or {}).get("saved") or {}
    if saved:
        try:
            gid = ctx.guild()["id"]
            for cid, old in saved.items():
                if ctx.dry:
                    continue
                try:
                    if old is None:
                        ctx.dc.clear_perm(cid, gid)
                    else:
                        ctx.dc.set_perm(cid, gid, int(old["allow"]), int(old["deny"]))
                except Refused as e:
                    problems.append(f"Discord channel {cid}: {e}")
            L(f"  Discord: permissions back on {len(saved)} channel(s)")
        except (Refused, TypeError) as e:
            problems.append(f"Discord: {e}")
    gh = res.get("github") or {}
    if gh.get("archived") or gh.get("made_private"):
        if not ctx.gh or ctx.gh.login() != GH_OWNER:
            problems.append(f"GitHub: gh isn't signed in as {GH_OWNER}")
        else:
            for full in gh.get("archived") or []:
                try:
                    if not ctx.dry:
                        ctx.gh.edit(full, archived=False)
                    L(f"  GitHub: {full} unarchived")
                except Refused as e:
                    problems.append(f"GitHub: {e}")
            for full in gh.get("made_private") or []:
                try:
                    if not ctx.dry:
                        ctx.gh.edit(full, private=False)
                    L(f"  GitHub: {full} public again")
                except Refused as e:
                    problems.append(f"GitHub: {e}")
    back = [REALM_STEPS_BACK[s] for s in (res.get("realms") or {}).get("steps_ok") or []]
    if back:
        L("  Turn back on what you want, with its own step: " + ", ".join(back))
    for p in problems:
        L(f"  PROBLEM: {p}")
    L("== Undo pause " + ("(dry run) checked" if ctx.dry else "done" if not problems else "done, with problems"))
    return {"ok": not problems and not ctx.dry, "problems": problems, "at": ctx.now(), "turn_back_on": back}


# --------------------------------------------------------------------------------------------------------- backup
def run_backup(ctx: Ctx, run_id: str) -> dict:
    """Everything a Delete could lose, to the Desktop, each file checked. Raises Refused on any failure."""
    L = ctx.log
    out = ctx.backup_root / f"Leyline-killswitch-backup-{run_id}"
    out.mkdir(parents=True, exist_ok=False)
    checks = []
    L(f"== Backup to {out}")

    # GitHub: every repo, all refs, as a checked bundle; its releases and issues
    if not ctx.gh or ctx.gh.login() != GH_OWNER:
        raise Refused(f"backup: gh isn't signed in as {GH_OWNER}")
    gdir = out / "github"
    gdir.mkdir()
    for full in GH_REPOS:
        info = ctx.gh.repo(full)
        if info is None:
            L(f"  {full}: not there, nothing to keep")
            checks.append({"what": full, "ok": True, "note": "not there"})
            continue
        name = full.split("/", 1)[1]
        mdir = gdir / "mirrors" / f"{name}.git"
        mdir.parent.mkdir(exist_ok=True)
        ctx.gh.mirror(full, mdir)
        rc2, refs, _ = ctx.gh.git("for-each-ref", "--format=%(objectname) %(refname)", cwd=str(mdir))
        rs = {l.strip() for l in refs.splitlines() if l.strip()}
        if rc2 == 0 and not rs:
            L(f"  {full}: empty (no commits), nothing to bundle")
            checks.append({"what": full, "ok": True, "note": "empty"})
            shutil.rmtree(mdir, ignore_errors=True)
            continue
        bundle = gdir / f"{name}.bundle"
        rc, _o, err = ctx.gh.git("bundle", "create", str(bundle), "--all", cwd=str(mdir))
        if rc != 0:
            raise Refused(f"backup: git bundle for {full} failed: {err.strip()[:200]}")
        rc, _o, err = ctx.gh.git("bundle", "verify", str(bundle), cwd=str(mdir))
        if rc != 0:
            raise Refused(f"backup: the bundle for {full} didn't verify: {err.strip()[:200]}")
        rc1, heads, _ = ctx.gh.git("bundle", "list-heads", str(bundle), cwd=str(mdir))
        hs = {l.strip() for l in heads.splitlines() if " refs/" in l}
        if rc1 or rc2 or hs != rs:
            raise Refused(f"backup: the bundle for {full} doesn't hold every ref ({len(hs)} of {len(rs)})")
        (gdir / f"{name}-releases.json").write_text(json.dumps(ctx.gh.releases(full), indent=1), encoding="utf-8")
        (gdir / f"{name}-issues.json").write_text(json.dumps(ctx.gh.issues(full), indent=1), encoding="utf-8")
        shutil.rmtree(mdir, ignore_errors=True)
        checks.append({"what": full, "ok": True, "refs": len(rs)})
        L(f"  {full}: bundle checked ({len(rs)} refs), releases and issues listed")
    shutil.rmtree(gdir / "mirrors", ignore_errors=True)

    # Discord: what the bot can read
    if ctx.dc:
        ddir = out / "discord"
        (ddir / "messages").mkdir(parents=True)
        try:
            g = ctx.guild()
            chans = ctx.dc.channels(g["id"])
            for name, obj in (("guild.json", g), ("roles.json", ctx.dc.roles(g["id"])), ("channels.json", chans)):
                (ddir / name).write_text(json.dumps(obj, indent=1, ensure_ascii=False), encoding="utf-8")
            skipped, total = [], 0
            for c in chans:
                if c.get("type") not in TEXTY:
                    continue
                msgs, before = [], None
                try:
                    while len(msgs) < DISCORD_EXPORT_MAX:
                        page = ctx.dc.messages(c["id"], before)
                        if not page:
                            break
                        msgs += page
                        before = page[-1]["id"]
                        if len(page) < 100:
                            break
                except Refused as e:
                    skipped.append({"channel": c.get("name"), "why": str(e)})
                slim = [{"id": m.get("id"), "author": (m.get("author") or {}).get("username"),
                         "author_id": (m.get("author") or {}).get("id"), "time": m.get("timestamp"),
                         "content": m.get("content"), "attachments": [a.get("url") for a in m.get("attachments") or []]}
                        for m in msgs]
                total += len(slim)
                (ddir / "messages" / f"{re.sub(r'[^0-9A-Za-z_-]+', '', c.get('name', '')) or 'channel'}-{c['id']}.json"
                 ).write_text(json.dumps({"channel": c.get("name"), "id": c["id"], "messages": slim}, indent=1,
                                         ensure_ascii=False), encoding="utf-8")
            if skipped:
                (ddir / "skipped.json").write_text(json.dumps(skipped, indent=1), encoding="utf-8")
            checks.append({"what": "discord", "ok": True, "channels": len(chans), "messages": total,
                           "skipped": len(skipped)})
            L(f"  Discord: {len(chans)} channels, {total} messages ({len(skipped)} channel(s) the bot can't read)")
        except (Refused, KeyError, TypeError) as e:
            raise Refused(f"backup: Discord: {e}") from None
    elif "discord" in ctx.parts:
        raise Refused("backup: Discord is ticked and no bot token is saved")

    # the website's data, packed on the server, copied down and checked
    if "site" in ctx.parts:
        ok, doc, lines = ctx.srv.call("backup", dry=ctx.dry)
        for l in lines:
            L("  server: " + l)
        if not ok:
            raise Refused(f"backup: the server couldn't pack the website's data: {doc.get('error')}")
        if ctx.dry:
            checks.append({"what": "website data", "ok": True, "note": "dry run: not packed"})
        else:
            pdir = out / "portal"
            pdir.mkdir()
            local = pdir / Path(doc["path"]).name
            ctx.srv.fetch(doc["path"], local)
            got = sha256_file(local)
            if got != doc.get("sha256"):
                raise Refused("backup: the website's data didn't arrive whole (sha256 differs)")
            try:
                with tarfile.open(local, "r:gz") as t:
                    n = len(t.getmembers())
            except (tarfile.TarError, OSError) as e:
                raise Refused(f"backup: the website's data can't be opened: {e}") from None
            checks.append({"what": "website data", "ok": True, "sha256": got, "files": n, "remote": doc["path"]})
            L(f"  website data: {local.name}, {n} entries, sha256 checked")

    files = []
    for f in sorted(out.rglob("*")):
        if f.is_file():
            if f.suffix == ".json":
                try:
                    json.loads(f.read_text(encoding="utf-8"))
                except ValueError:
                    raise Refused(f"backup: {f.name} isn't readable JSON") from None
            files.append({"file": f.relative_to(out).as_posix(), "bytes": f.stat().st_size, "sha256": sha256_file(f)})
    manifest = {"id": run_id, "at": _dt.datetime.now().isoformat(timespec="seconds"), "checks": checks, "files": files,
                "ok": all(c["ok"] for c in checks)}
    (out / "MANIFEST.json").write_text(json.dumps(manifest, indent=1), encoding="utf-8")
    L(f"== Backup checked: {len(files)} file(s) in {out}")
    return {"dir": str(out), "ok": manifest["ok"], "checks": checks,
            "site_sha256": next((c.get("sha256") for c in checks if c["what"] == "website data"), None)}


# --------------------------------------------------------------------------------------------------------- delete
def run_delete(ctx: Ctx, run_id: str, paused: dict) -> dict:
    """After a Pause and a checked backup only. Any failure stops it before the next deletion."""
    L = ctx.log
    if not paused or not paused.get("ok"):
        raise Refused("Delete comes only after a finished Pause.")
    if ctx.now() - float(paused.get("at") or 0) < DELETE_WAIT and not ctx.dry:
        raise Refused(f"Delete opens {DELETE_WAIT // 3600} hours after the Pause.")
    bad = [p for p in ctx.parts if p in DELETABLE and p not in (paused.get("parts") or [])]
    if bad:
        raise Refused("Not paused first: " + ", ".join(PART_TITLES[p] for p in bad))
    if "github" in ctx.parts:
        if not ctx.gh or ctx.gh.login() != GH_OWNER:
            raise Refused(f"gh isn't signed in as {GH_OWNER}: nothing was deleted.")
        if "delete_repo" not in ctx.gh.scopes():
            raise Refused("gh can't delete repos: gh auth refresh -h github.com -s delete_repo")
    L(f"== Delete {run_id}{' (dry run)' if ctx.dry else ''}: "
      + ", ".join(PART_TITLES[p] for p in ctx.parts if p in DELETABLE))
    backup = run_backup(ctx, run_id)
    if not backup["ok"]:
        raise Refused("the backup didn't check out: nothing was deleted")
    done = []
    if ctx.dc:
        ctx.dc.writes = not ctx.dry

    if "github" in ctx.parts:
        for full in GH_REPOS:
            if ctx.gh.repo(full) is None:
                continue
            if ctx.dry:
                L(f"  (dry run) would delete {full}")
                continue
            ctx.gh.delete(full)
            done.append(full)
            L(f"  GitHub: {full} deleted")

    if "discord" in ctx.parts:
        g = ctx.guild()
        chans = ctx.dc.channels(g["id"])
        keep = announce_channel(chans, g)
        keep_ids = {keep["id"], keep.get("parent_id")} if keep else set()
        for c in sorted(chans, key=lambda c: c.get("type") == CATEGORY):   # channels first, then categories
            if c["id"] in keep_ids:
                continue
            if ctx.dry:
                L(f"  (dry run) would delete #{c['name']}")
                continue
            ctx.dc.delete_channel(c["id"])
            done.append(f"#{c['name']}")
        L(f"  Discord: channels deleted; #{keep['name'] if keep else '?'} kept with the farewell")

    if "site" in ctx.parts:
        if ctx.dry:
            L("  (dry run) would wipe the website's data (the farewell page stays)")
        else:
            ok, doc, lines = ctx.srv.call("wipe", {"sha256": backup["site_sha256"]})
            for l in lines:
                L("  server: " + l)
            if not ok:
                raise Refused(f"the server didn't wipe the website's data: {doc.get('error')}")
            done.append("website data")

    manual = ["DNS: remove leylinelauncher.com and the old names, or point them away (Squarespace)."]
    if "discord" in ctx.parts:
        manual.insert(0, "Discord: delete the server yourself (Server Settings > Delete Server). A bot can't.")
    for m in manual:
        L(f"  YOUR TURN: {m}")
    L("== Delete " + ("(dry run) checked" if ctx.dry else "done"))
    return {"ok": not ctx.dry, "at": ctx.now(), "done": done, "backup": backup["dir"], "manual": manual}
