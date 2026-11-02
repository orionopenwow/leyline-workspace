"""The release audit's shared parts: the results, the settings, the addresses (new name first, old as fallback) and
HTTP that never raises. Standard library only (docs/AUDIT.md)."""
from __future__ import annotations

import json
import os
import re
import socket
import ssl
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / "hosting" / "kit" / "tools"))   # wunet: the Host kit's 3.3.5a test client

PASS, FAIL, SKIP = "PASS", "FAIL", "SKIP"
UA = "Leyline-Audit/1"

# Never shown to players (docs/WORDS.md); the site's own tests use the same rule (stream/tests/test_site.py).
BANNED = re.compile(r"(?i)world of warcraft|\bwow\b|warcraft|lich king|blizzard")
# Words a server prints when a page broke.
SERVER_ERROR = re.compile(r"Traceback \(most recent call last\)|Internal Server Error|502 Bad Gateway|"
                          r"503 Service Unavailable|\bNoneType\b|\bKeyError\b|\bAttributeError\b")
# A client version in a sentence (version chips are short and stand alone, so they are not sentences).
VERSION = re.compile(r"\b(?:3\.3\.5a?|1\.12(?:\.[0-9])?|2\.4\.3|[0-9]\.[0-9]{1,2}\.[0-9][a-z]?)\b")


@dataclass
class Result:
    feature: str
    check: str
    status: str
    detail: str = ""
    action: str = ""
    secs: float = 0.0


class Recorder:
    """Every check's line, printed as it happens (Leyline Control streams it) and kept for the report."""

    def __init__(self, out=None, secret_words=()):
        self.results: list[Result] = []
        self.out = out if out is not None else sys.stdout
        self.secrets = set(secret_words)
        self.t0 = time.time()

    def hide(self, *words):
        self.secrets.update(w for w in words if w)

    def clean(self, text: str) -> str:
        text = str(text or "")
        for s in self.secrets:
            text = text.replace(s, "[hidden]")
        return text.replace("\r", " ").replace("\n", " ").strip()

    def add(self, feature, check, status, detail="", action="", secs=0.0):
        r = Result(feature, check, status, self.clean(detail)[:600], self.clean(action)[:300], round(secs, 1))
        self.results.append(r)
        line = f"{status:<4}  {feature}: {check}" + (f"  ({r.detail})" if r.detail else "")
        if status == FAIL and r.action:
            line += f"  -> {r.action}"
        try:
            print(line, file=self.out, flush=True)
        except UnicodeEncodeError:
            print(line.encode("ascii", "replace").decode(), file=self.out, flush=True)
        return r

    def note(self, text):
        print("      " + self.clean(text), file=self.out, flush=True)

    def counts(self):
        c = {PASS: 0, FAIL: 0, SKIP: 0}
        for r in self.results:
            c[r.status] = c.get(r.status, 0) + 1
        return c


def load_config(path: Path | None = None) -> dict:
    cfg = json.loads((path or HERE / "audit.json").read_text(encoding="utf-8"))
    shared = ROOT / "tools" / "domain" / "addresses.json"     # the domain move's one list, when it is in this copy
    if shared.exists():
        try:
            for k, v in json.loads(shared.read_text(encoding="utf-8")).items():
                if isinstance(v, dict) and v.get("new") and v.get("old"):
                    cfg["addresses"][k] = {"new": v["new"], "old": v["old"]}
        except (OSError, ValueError):
            pass
    return cfg


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a, **k):
        return None


_CTX = ssl.create_default_context()
_OPENER = urllib.request.build_opener(NoRedirect, urllib.request.HTTPSHandler(context=_CTX))


@dataclass
class Resp:
    status: int            # 0: no answer at all
    url: str
    headers: dict = field(default_factory=dict)
    body: bytes = b""
    error: str = ""
    hops: list = field(default_factory=list)

    @property
    def text(self) -> str:
        return self.body.decode("utf-8", "replace")

    def json(self):
        try:
            return json.loads(self.body or b"null")
        except ValueError:
            return None


class Ctx:
    """What every section gets: the settings, the recorder, HTTP, and the live addresses.

    fake: {"site": "http://127.0.0.1:1234", ...} for CI and the container (bases instead of https://<name>), and
    rewrite: [(prefix, replacement)] so downloads from GitHub go to a local fake server."""

    def __init__(self, cfg: dict, rec: Recorder, fake: dict | None = None, rewrite=(), temp: Path | None = None):
        self.cfg, self.rec, self.fake, self.rewrite = cfg, rec, fake or {}, list(rewrite)
        self.temp = temp
        self._hosts: dict[str, str] = {}
        self.cache: dict = {}             # sections share what they fetched (hostpack, listing, downloads)

    # ------------------------------------------------------------------ addresses
    def names(self, key: str) -> tuple[str, str]:
        a = self.cfg["addresses"][key]
        return a["new"], a["old"]

    def host(self, key: str) -> str:
        """The name that answers: the new one when it does, else the old one (the move may not be on yet)."""
        if key in self._hosts:
            return self._hosts[key]
        new, old = self.names(key)
        pick = old
        if key == "realmlist":
            pick = new if tcp_open(new, 3724, 5) else old
        else:
            r = http_get(f"https://{new}/", timeout=8, max_bytes=2048)
            if r.status:
                pick = new
        self._hosts[key] = pick
        return pick

    def base(self, key: str) -> str:
        if key in self.fake:
            return self.fake[key].rstrip("/")
        return "https://" + self.host(key)

    def url(self, key: str, path: str) -> str:
        return self.base(key) + path

    # ------------------------------------------------------------------ HTTP
    def map(self, url: str) -> str:
        for a, b in self.rewrite:
            if url.startswith(a):
                return b + url[len(a):]
        return url

    def get(self, url, **kw) -> Resp:
        return http_get(self.map(url), **kw)


def http_get(url: str, timeout: float = 20, max_bytes: int = 4 << 20, headers=None, follow: int = 5,
             method: str = "GET", sink=None) -> Resp:
    """GET that never raises. Follows up to `follow` redirects (kept in .hops). sink(chunk) gets the body instead
    of keeping it (for big downloads)."""
    hops = []
    for _ in range(follow + 1):
        req = urllib.request.Request(url, method=method, headers=dict({"User-Agent": UA}, **(headers or {})))
        try:
            r = _OPENER.open(req, timeout=timeout)
            st, hd = r.status, {k.lower(): v for k, v in r.headers.items()}
        except urllib.error.HTTPError as e:
            r, st, hd = e, e.code, {k.lower(): v for k, v in (e.headers or {}).items()}
        except (urllib.error.URLError, OSError, ValueError) as e:
            reason = getattr(e, "reason", e)
            return Resp(0, url, error=f"{type(e).__name__}: {reason}"[:200], hops=hops)
        if st in (301, 302, 303, 307, 308) and hd.get("location") and follow:
            hops.append((st, url))
            url = urllib.parse.urljoin(url, hd["location"])
            try:
                r.close()
            except Exception:  # noqa: BLE001
                pass
            continue
        body = bytearray()
        try:
            while True:
                chunk = r.read(65536)
                if not chunk:
                    break
                if sink:
                    sink(chunk)
                else:
                    body += chunk
                    if len(body) > max_bytes:
                        break
        except (OSError, ValueError) as e:
            return Resp(st, url, hd, bytes(body), f"read: {type(e).__name__}", hops)
        finally:
            try:
                r.close()
            except Exception:  # noqa: BLE001
                pass
        return Resp(st, url, hd, bytes(body), "", hops)
    return Resp(0, url, error="too many redirects", hops=hops)


def tcp_open(host: str, port: int, timeout: float = 5) -> bool:
    try:
        with socket.create_connection((host, port), timeout=timeout):
            return True
    except OSError:
        return False


def is_windows() -> bool:
    return os.name == "nt"


def appdata() -> Path | None:
    v = os.environ.get("APPDATA")
    return Path(v) if v else None


def short(e: BaseException) -> str:
    return f"{type(e).__name__}: {e}"[:300]
