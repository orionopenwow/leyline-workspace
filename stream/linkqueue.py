"""Submit a link: one review queue for mods, addons, looks and realms, from the website and from the Discord.

Owner, 3 Nov: "Easy way for people to submit a link, whether through Discord or the site, with an approval process."

Every link lands here as PENDING. Nothing goes live until the owner or a moderator approves it in HQ > Review >
Submissions (stream/staffdesk.py); every decision is in the admin audit log, and the owner can undo any of them.

    GET  /wrath/submit                the form (logged in on the website; logged out: how to log in, and /submit in
                                      the Discord). ?kind=mod|addon|look|realm picks the kind.
    POST /wrath/submit                the form (fields + the form token "t"); a thank-you page or the form with why
    GET  /wrath/leyline/approved-links.json   what was approved: {format, updated_at, links: [{id, kind, url,
                                      approved_at}]}, no names or notes (CORS *, a minute's cache): the index tools and
                                      the launcher may read it
In the Discord: /submit link:<https://...> kind:<mod|addon|look|realm> [note:<text>] (stream/forum_discord.py, the
forum bot's Interactions Endpoint), answered privately. The hub's "Submit a mod" form (stream/hub.py) puts a copy of
each repo it queues here too, so the one queue shows everything; approving one of those also approves it in the
community index (decisions.jsonl there).

The rules a link must pass (validate()), the same from everywhere:
  - https only, no login or port in it, at most MAX_URL characters, no spaces or hidden characters;
  - a site on the allowed list: the code forges and mod sites in HOSTS (WU_LINK_HOSTS adds more, ".example.org" for
    its subdomains); a realm may also be one of our own pages (OWN_HOSTS, PUBLIC_HOST);
  - a page, never a file: links to .exe, .zip, .mpq and the like, and forge release downloads, are refused (nothing
    is downloaded for players from here, and no game files ever);
  - not from a blocklisted account or repo (tools/hub-index/blocklist.json: lure and fake-star accounts, wowdecomp,
    Chumpton; on the server the copy the signin step puts beside this file, link-blocklist.json; WU_LINK_BLOCKLIST
    names another) and nothing naming decompiled, leaked or repacked clients;
  - not already pending, approved or turned down.
A link that passes but is not a forge repo gets the warning "not a repo: check the page by hand"; links with a
warning, and every look, need an admin or the owner to approve (staffdesk.py has the role rules).

Who: a logged-in website account (the forum's sign-in, stream/forum.py viewer) or a Discord user (their linked
Leyline account when they have one). Kept per submission: the account's game account name and display name, or the
Discord display name and whether it is linked. No email, no IP address, no Discord id on disk (a hash for the
rate limit only, in memory). Site sanctions apply (admin.blocked("submit")). Limits: PER_HOUR per person,
NET_PER_HOUR per network, DAY_CAP from everyone a day, MAX_QUEUE waiting at once.

Files ($WU_PORTAL_DATA/linkqueue, WU_LINKQUEUE_DIR overrides): queue.jsonl (one submission a line), decisions.jsonl
(one decision a line: approve, reject, reopen; the latest per id wins). Standard library only; never raises to the
portal.
"""
from __future__ import annotations

import hashlib
import hmac
import html
import json
import logging
import os
import re
import secrets
import threading
import time
import urllib.parse

log = logging.getLogger("wrath-portal.linkqueue")
e = html.escape
HERE = os.path.dirname(os.path.abspath(__file__))

PAGE = "/wrath/submit"
FEED = "/wrath/leyline/approved-links.json"
KINDS = (("mod", "A mod", "changes the game itself: models, graphics, maps, realm scripts, tools"),
         ("addon", "An addon", "an in-game interface helper (it goes in the AddOns folder)"),
         ("look", "A look", "a graphics preset or HD pack for the launcher's Looks"),
         ("realm", "A realm", "a community realm on the Leyline network"))
KIND_IDS = tuple(k for k, _, _ in KINDS)
DECISIONS = ("approve", "reject", "reopen")
MAX_URL, MAX_NOTE = 300, 280
PER_HOUR, NET_PER_HOUR, DAY_CAP, MAX_QUEUE = 5, 20, 200, 2000
FORM_MAX = 4096
ID_RE = re.compile(r"^[0-9a-f]{12}$")
CTRL_RE = re.compile(r"[\x00-\x1f\x7f​-‏‪-‮⁠-⁤﻿]")
HOST_RE = re.compile(r"^(?=.{1,253}$)[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+$")
SEG_RE = re.compile(r"^[A-Za-z0-9._-]{1,100}$")
FORGES = {"github.com": "github", "gitlab.com": "gitlab", "codeberg.org": "codeberg"}
# the allowed sites (stream/projects.py DEFAULT_HOSTS, the Workshop's instant links, is the same list)
HOSTS = ("github.com", "gitlab.com", "codeberg.org", "bitbucket.org", "git.sr.ht", "sr.ht", "gitea.com", "sourceforge.net",
         "notabug.org", "framagit.org", "itch.io", "curseforge.com", "www.curseforge.com", "wago.io", "addons.wago.io",
         ".github.io", ".gitlab.io", ".codeberg.page", ".itch.io")
OWN_HOSTS = ("leylinelauncher.com", "play.openvanilla.app")
FORGE_RESERVED = {"topics", "orgs", "users", "explore", "search", "settings", "marketplace", "sponsors", "features",
                  "collections", "trending", "about", "login", "api", "new"}
FILE_RE = re.compile(r"(?i)\.(?:exe|msi|zip|rar|7z|tar|gz|tgz|bz2|xz|mpq|dll|bat|cmd|ps1|scr|jar|apk|dmg|pkg|appimage|deb|iso|bin)$")
# never listed, whatever the link (tools/hub-index/hub_index.py EXTERNAL_BAD says the same for the index)
BAD_RE = re.compile(r"(?i)chumpton|wowdecomp|waystone|decompil|leaked|repack|cracked|full[ -]?client|client[ -]?download")
BRAND_RE = re.compile(r"(?i)world\s*of\s*warcraft|warcraft|lich\s*king|blizzard|\bwow\b")

_get = None
_lock = threading.Lock()
_key = secrets.token_bytes(32)
_times: dict[str, list[float]] = {}


def bind(getter) -> None:
    global _get
    _get = getter


def P(name):
    try:
        return _get(name) if _get else None
    except KeyError:
        return None


def qdir() -> str:
    return os.environ.get("WU_LINKQUEUE_DIR") or os.path.join(os.environ.get("WU_PORTAL_DATA", "/data"), "linkqueue")


def now_iso(t: float | None = None) -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(time.time() if t is None else t))


def clean_line(v, n: int) -> str:
    return " ".join(CTRL_RE.sub(" ", str(v or "")).split())[:n]


# ----------------------------------------------------------------------------------------------- the rules

def allowed_hosts() -> tuple[str, ...]:
    extra = tuple(h.strip().lower() for h in os.environ.get("WU_LINK_HOSTS", "").split(",") if h.strip())
    return HOSTS + extra


def own_hosts() -> tuple[str, ...]:
    pub = str(P("PUBLIC_HOST") or "").lower()
    return OWN_HOSTS + ((pub,) if pub and pub not in OWN_HOSTS else ())


def host_ok(host: str, hosts) -> bool:
    for h in hosts:
        if h.startswith("."):
            if host.endswith(h) and len(host) > len(h):
                return True
        elif host == h:
            return True
    return False


_block_cache: dict = {"at": 0.0, "doc": {}}


def blocklist() -> dict:
    """The index's blocklist ({owners: [{owner, reason}], repos: [{repo, reason}]}), read at most once a minute."""
    if time.monotonic() - _block_cache["at"] < 60:
        return _block_cache["doc"]
    doc = {}
    for p in (os.environ.get("WU_LINK_BLOCKLIST", ""), os.path.join(HERE, "link-blocklist.json"),
              os.path.join(HERE, "..", "tools", "hub-index", "blocklist.json")):
        if p and os.path.isfile(p):
            try:
                with open(p, encoding="utf-8") as f:
                    d = json.load(f)
                if isinstance(d, dict):
                    doc = d
                    break
            except (OSError, ValueError):
                continue
    _block_cache.update(at=time.monotonic(), doc=doc)
    return doc


def blocked_reason(host: str, parts: list[str]) -> str:
    """Why a link is refused by the blocklist, or ""."""
    doc = blocklist()
    owner = parts[0].lower() if parts else ""
    if host.endswith(".github.io") or host.endswith(".gitlab.io") or host.endswith(".codeberg.page"):
        owner = host.split(".")[0]
    pair = "/".join(parts[:2]).lower()
    for x in doc.get("owners", []) if isinstance(doc.get("owners"), list) else []:
        if isinstance(x, dict) and str(x.get("owner", "")).lower() == owner and owner:
            return "That account is on our blocklist, so its links can't be listed."
    for x in doc.get("repos", []) if isinstance(doc.get("repos"), list) else []:
        r = str(x.get("repo", "")).lower() if isinstance(x, dict) else ""
        r = r.split("://", 1)[-1].split("/", 1)[-1] if "://" in r or r.count("/") > 1 else r
        if r and r.strip("/") == pair:
            return "That project is on our blocklist, so it can't be listed."
    return ""


def validate(raw, kind: str) -> dict:
    """{id, url, host, forge, owner, repo, warnings} for a link that passes the rules, else ValueError(why)."""
    if kind not in KIND_IDS:
        raise ValueError("Pick what it is: a mod, an addon, a look or a realm.")
    if not isinstance(raw, str) or not raw.strip():
        raise ValueError("Paste the link.")
    raw = raw.strip()
    if len(raw) > MAX_URL:
        raise ValueError(f"That link is longer than {MAX_URL} characters.")
    if CTRL_RE.search(raw) or re.search(r"\s", raw):
        raise ValueError("That link has spaces or hidden characters in it.")
    try:
        u = urllib.parse.urlsplit(raw)
        port = u.port
    except ValueError:
        raise ValueError("That doesn't look like a link.") from None
    if u.scheme.lower() != "https":
        raise ValueError("Links must start with https://.")
    if u.username is not None or u.password is not None or "@" in u.netloc:
        raise ValueError("Links can't carry a login.")
    if port not in (None, 443):
        raise ValueError("Links can't name a port.")
    host = (u.hostname or "").lower().rstrip(".")
    if host.startswith("www.") and host[4:] in FORGES:
        host = host[4:]
    if not HOST_RE.fullmatch(host):
        raise ValueError("That link's site name isn't a normal web address.")
    path = urllib.parse.unquote(u.path or "/")
    if CTRL_RE.search(path) or ".." in path.split("/"):
        raise ValueError("That link's path isn't valid.")
    if BAD_RE.search(raw):
        raise ValueError("That can't be listed here: no decompiled, leaked or repacked game clients.")
    parts = [p for p in path.split("/") if p]
    if (parts and FILE_RE.search(parts[-1])) or "releases/download" in path.lower() or host in (
            "raw.githubusercontent.com", "objects.githubusercontent.com", "codeload.github.com"):
        raise ValueError("Link the project's page, not a file to download.")
    own = host_ok(host, own_hosts())
    if own and kind != "realm":
        raise ValueError("Links to our own site are only for realms.")
    if not own and not host_ok(host, allowed_hosts()):
        raise ValueError(f"Links from {host} aren't accepted yet. Use the project's page on GitHub, GitLab, "
                         "Codeberg, CurseForge or Wago, or ask us to add the site.")
    why = blocked_reason(host, parts)
    if why:
        raise ValueError(why)
    forge, owner, repo, warnings = FORGES.get(host, ""), "", "", []
    if forge:
        if "-" in parts:
            parts = parts[:parts.index("-")]
        parts = parts[:3] if forge == "gitlab" and len(parts) >= 3 and parts[2] not in ("tree", "blob", "issues") else parts[:2]
        if len(parts) < 2 or parts[0].lower() in FORGE_RESERVED or not all(SEG_RE.fullmatch(p) for p in parts):
            raise ValueError(f"Link the repo itself: https://{host}/<owner>/<project>.")
        if parts[-1].lower().endswith(".git"):
            parts[-1] = parts[-1][:-4]
            if not parts[-1]:
                raise ValueError(f"Link the repo itself: https://{host}/<owner>/<project>.")
        owner, repo = "/".join(parts[:-1]), parts[-1]
        url = f"https://{host}/" + "/".join(parts)
    else:
        url = f"https://{host}/" + "/".join(urllib.parse.quote(p, safe="-._~!$&'()*+,;=:@") for p in parts)
        if u.query and own:   # our realm links may carry ?r=<id>
            url += "?" + urllib.parse.quote(u.query[:100], safe="=&-_.")
        warnings.append("not a repo: check the page by hand")
    if kind == "look":
        warnings.append("a look: the owner approves looks and HD packs")
    key = url.lower()
    return {"id": hashlib.sha256(key.encode()).hexdigest()[:12], "url": url, "host": host, "forge": forge,
            "owner": owner, "repo": repo, "warnings": warnings}


# ----------------------------------------------------------------------------------------------- the store

def _path(name: str) -> str:
    return os.path.join(qdir(), name)


def _read(name: str, limit: int = 8_000_000) -> list[dict]:
    out = []
    try:
        with open(_path(name), "rb") as f:
            f.seek(0, 2)
            f.seek(max(0, f.tell() - limit))
            data = f.read().decode("utf-8", "replace")
    except OSError:
        return out
    for ln in data.splitlines():
        try:
            d = json.loads(ln)
        except ValueError:
            continue
        if isinstance(d, dict) and isinstance(d.get("id"), str) and ID_RE.fullmatch(d["id"]):
            out.append(d)
    return out


def _append(name: str, rec: dict) -> None:
    os.makedirs(qdir(), exist_ok=True)
    line = json.dumps(rec, ensure_ascii=False, sort_keys=True) + "\n"
    with _lock:
        fd = os.open(_path(name), os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o640)
        with os.fdopen(fd, "a", encoding="utf-8") as f:
            f.write(line)
            f.flush()
            os.fsync(f.fileno())


def submissions() -> dict[str, dict]:
    """The first submission per id (later copies of the same link are ignored)."""
    out = {}
    for r in _read("queue.jsonl"):
        out.setdefault(r["id"], r)
    return out


def decisions() -> dict[str, dict]:
    out = {}
    for r in _read("decisions.jsonl"):
        if r.get("decision") in DECISIONS:
            out[r["id"]] = r
    return out


def history(iid: str) -> list[dict]:
    return [r for r in _read("decisions.jsonl") if r["id"] == iid and r.get("decision") in DECISIONS]


def _hub_decisions() -> dict:
    """The community index's own decisions (an Index-page decision on a mirrored repo counts here too)."""
    hub = P("hub_mod")
    try:
        return hub.hub.store.decisions() if hub is not None and getattr(hub, "hub", None) is not None else {}
    except Exception:   # noqa: BLE001
        return {}


def status_of(rec: dict, dec: dict | None, hubdec: dict | None = None) -> str:
    if dec is not None and dec.get("decision") != "reopen":
        return "approved" if dec["decision"] == "approve" else "rejected"
    if dec is None and hubdec is not None and rec.get("via") == "index":
        return {"approve": "approved", "reject": "rejected", "delist": "rejected"}.get(hubdec.get("decision"), "pending")
    return "pending"


def items(status: str = "") -> list[dict]:
    """Every submission with its status and latest decision, oldest first (pending) / newest decision first (others)."""
    subs, dec, hd = submissions(), decisions(), _hub_decisions()
    out = []
    for i, r in subs.items():
        d = dec.get(i)
        st = status_of(r, d, hd.get(i))
        if status and st != status:
            continue
        out.append({**r, "status": st, "decision": d})
    if status == "pending":
        out.sort(key=lambda r: r.get("at", ""))
    else:
        out.sort(key=lambda r: ((r.get("decision") or {}).get("at") or r.get("at", "")), reverse=True)
    return out


def counts() -> dict:
    c = {"pending": 0, "approved": 0, "rejected": 0}
    for r in items():
        c[r["status"]] = c.get(r["status"], 0) + 1
    return c


def item(iid: str) -> dict | None:
    if not ID_RE.fullmatch(iid or ""):
        return None
    r = submissions().get(iid)
    if r is None:
        return None
    d = decisions().get(iid)
    return {**r, "status": status_of(r, d, _hub_decisions().get(iid)), "decision": d}


def decide(iid: str, decision: str, by: dict, reason: str = "") -> tuple[dict, dict]:
    """Write one decision (staffdesk.py checks who may). (before, after) for the audit log."""
    if decision not in DECISIONS:
        raise ValueError("Not a decision.")
    cur = item(iid)
    if cur is None:
        raise ValueError("No such submission.")
    before = {"status": cur["status"], "by": (cur.get("decision") or {}).get("by_name", "")}
    rec = {"id": iid, "decision": decision, "at": now_iso(), "by": str(by.get("account", ""))[:64],
           "by_name": str(by.get("name", ""))[:40], "role": str(by.get("role", ""))[:10], "reason": clean_line(reason, 200)}
    _append("decisions.jsonl", rec)
    after = {"status": "pending" if decision == "reopen" else ("approved" if decision == "approve" else "rejected"),
             "url": cur.get("url"), "kind": cur.get("kind")}
    return before, after


def approved_doc() -> dict:
    links = [{"id": r["id"], "kind": r.get("kind"), "url": r.get("url"),
              "approved_at": (r.get("decision") or {}).get("at") or ""} for r in items("approved")]
    return {"format": 1, "updated_at": now_iso(), "links": links}


# ----------------------------------------------------------------------------------------------- submitting

def _allow(key: str, limits, now: float) -> bool:
    with _lock:
        if len(_times) > 20000:
            _times.clear()
        ts = [t for t in _times.get(key, []) if now - t < max(w for _, w in limits)]
        if any(sum(1 for t in ts if now - t < w) >= n for n, w in limits):
            _times[key] = ts
            return False
        _times[key] = ts + [now]
        return True


def _hkey(kind: str, v: str) -> str:
    return kind + ":" + hmac.new(_key, str(v).encode(), hashlib.sha256).hexdigest()[:24]


def submit(url, kind: str, note: str, via: str, who: dict, net: str = "", now: float | None = None) -> tuple[bool, str, str]:
    """Validates and queues one link: (ok, message for the person, id). who: {acct (game account) | discord, name,
    linked}. via: site | discord | index."""
    now = time.time() if now is None else now
    adm = P("admin")
    acct = str(who.get("acct") or "").upper()
    if adm is not None and acct and hasattr(adm, "blocked"):
        try:
            blk = adm.blocked("submit", acct=acct, site=str(who.get("site") or ""))
        except Exception:   # noqa: BLE001
            blk = ""
        if blk:
            return False, blk, ""
    try:
        v = validate(url, kind)
    except ValueError as ex:
        return False, str(ex), ""
    note = clean_line(note, MAX_NOTE)
    if BAD_RE.search(note):
        return False, "That can't be listed here: no decompiled, leaked or repacked game clients.", ""
    cur = item(v["id"])
    if cur is not None:
        return False, {"pending": "That link is already waiting for review: thanks.",
                       "approved": "That link is already approved.",
                       "rejected": "That link was reviewed and isn't listed here."}[cur["status"]], v["id"]
    person = _hkey("a", acct) if acct else _hkey("d", who.get("discord") or "") if who.get("discord") else ""
    if not person:
        return False, "Log in first.", ""
    if not _allow(person, ((PER_HOUR, 3600),), now) or (net and not _allow(_hkey("n", net), ((NET_PER_HOUR, 3600),), now)):
        return False, "That's a lot of links this hour. Please try again later.", ""
    if not _allow("*", ((DAY_CAP, 86400),), now):
        return False, "The queue is busy today. Please try again tomorrow.", ""
    if counts()["pending"] >= MAX_QUEUE:
        return False, "The queue is full right now. Please try again later.", ""
    rec = {"id": v["id"], "url": v["url"], "host": v["host"], "kind": kind, "note": note, "via": via,
           "warnings": v["warnings"], "at": now_iso(now), "by_name": clean_line(who.get("name") or "a player", 40),
           "acct": acct, "linked": bool(acct) if via == "discord" else None}
    if v["forge"]:
        rec.update(forge=v["forge"], owner=v["owner"], repo=v["repo"])
    rec = {k: x for k, x in rec.items() if x not in (None, "")}
    try:
        _append("queue.jsonl", rec)
    except OSError as ex:
        log.warning("link not queued: %s", ex.strerror)
        return False, "We couldn't save that just now. Please try again shortly.", ""
    log.info("link %s queued (%s, via %s)", v["id"], kind, via)
    return True, "Thanks! Your link is in the review queue. A moderator checks it before it shows anywhere.", v["id"]


def add_from_hub(rec: dict) -> None:
    """A copy of a submission the hub's "Submit a mod" form queued (stream/hub.py), so the one queue shows it.
    Never raises; a link the queue already has is left alone."""
    try:
        iid = str(rec.get("id") or "")
        if not ID_RE.fullmatch(iid) or item(iid) is not None:
            return
        kind = "addon" if rec.get("category") == "addons" else "mod"
        try:
            v = validate(str(rec.get("repo") or ""), kind)
        except ValueError as ex:
            v = {"warnings": ["check by hand: " + str(ex)[:80]], "host": "", "forge": ""}
        out = {"id": iid, "url": str(rec.get("repo") or "")[:MAX_URL], "host": v.get("host", ""), "kind": kind,
               "note": clean_line(rec.get("note"), MAX_NOTE), "via": "index", "warnings": v["warnings"],
               "at": str(rec.get("at") or now_iso())[:20], "by_name": "the Submit a mod form",
               "build": str(rec.get("build") or "")[:8], "category": str(rec.get("category") or "")[:24]}
        _append("queue.jsonl", {k: x for k, x in out.items() if x not in (None, "")})
    except Exception:   # noqa: BLE001
        log.exception("hub submission not mirrored into the link queue")


def discord_submit(did: str, name: str, linked_acct: str, options: dict) -> str:
    """The Discord's /submit (stream/forum_discord.py): the private answer's text."""
    if not re.fullmatch(r"\d{5,22}", str(did or "")):
        return "Discord didn't say who you are. Try again."
    ok, msg, _ = submit(options.get("link"), str(options.get("kind") or ""), str(options.get("note") or ""), "discord",
                        {"discord": did, "acct": linked_acct, "name": name})
    return msg


# ----------------------------------------------------------------------------------------------- the page

def _net(h) -> str:
    ip, rb = h.ip(), P("rl_bucket")
    try:
        return rb(ip) if callable(rb) else ip
    except Exception:   # noqa: BLE001
        return ip


def form_token(who: dict) -> str:
    return hmac.new(_key, f"link|{who.get('session', '')}|{who.get('acct', '')}".encode(), hashlib.sha256).hexdigest()[:32]


def _signin(nxt: str) -> str:
    fn = P("signin_url")
    try:
        return fn(nxt) if callable(fn) else "/wrath/signin?next=" + urllib.parse.quote(nxt, safe="")
    except Exception:   # noqa: BLE001
        return "/wrath/signin"


def page(who: dict | None, msg: str = "", done: bool = False, kind: str = "", url: str = "", note: str = "") -> bytes:
    discord = ('<p class="fine">In our Discord, type <b>/submit</b> with the link and what it is. It goes to the same '
               'queue.</p>')
    if done:
        body = ('<section class="realm"><h1>Thanks, it\'s in the queue</h1><p>A moderator checks every link before it '
                'shows anywhere. Approved links appear in the catalog, the Looks or the realm list.</p>'
                f'<p><a href="{PAGE}">Submit another</a> · <a href="/wrath">Back to Leyline</a></p></section>')
    elif who is None:
        body = ('<section class="realm"><h1>Submit a link</h1><p>Share a mod, an addon, a look or a realm with '
                'everyone. Log in first, so we know who to thank and can keep spam out.</p>'
                f'<p><a class="btn primary" href="{e(_signin(PAGE))}">Log in</a></p>{discord}</section>')
    else:
        kind = kind if kind in KIND_IDS else "mod"
        opts = "".join(f'<label class="lq-kind"><input type="radio" name="kind" value="{k}"{" checked" if k == kind else ""}> '
                       f'<b>{e(t)}</b>: {e(d)}</label><br>' for k, t, d in KINDS)
        body = ('<section class="realm"><h1>Submit a link</h1>'
                + (f'<p class="fine" role="alert">{e(msg)}</p>' if msg else '')
                + f'<p>Logged in as <b>{e(who.get("name") or "player")}</b>. Paste the project\'s page: a repo on GitHub, '
                  'GitLab or Codeberg, or its page on CurseForge or Wago. A realm can be its page on our site. Links to '
                  'files to download aren\'t accepted, and never game files.</p>'
                  f'<form method="post" action="{PAGE}"><input type="hidden" name="t" value="{e(form_token(who))}">'
                  f'<fieldset><legend>What is it?</legend>{opts}</fieldset>'
                  f'<p><label for="lq-url">The link</label><br><input id="lq-url" name="url" type="url" required '
                  f'maxlength="{MAX_URL}" size="50" placeholder="https://github.com/you/your-mod" value="{e(url)}"></p>'
                  f'<p><label for="lq-note">A short note for the reviewers (optional)</label><br><textarea id="lq-note" '
                  f'name="note" maxlength="{MAX_NOTE}" rows="3" cols="50">{e(note)}</textarea></p>'
                  '<p><button class="btn primary" type="submit">Submit for review</button> <a href="/wrath">Cancel</a></p>'
                  f'</form><p class="fine">Nothing goes live until a moderator approves it. Please don\'t put emails or '
                  f'other personal details in the note.</p>{discord}</section>')
    render = P("render")
    if render is None:
        return body.encode("utf-8")
    return render(body, title="Submit a link")


def _viewer(h):
    fm = P("forum")
    if fm is None:
        return None, 503
    try:
        return fm.viewer(h), 0
    except OSError:
        return None, 503
    except Exception:   # noqa: BLE001
        return None, 401


def match(path: str) -> bool:
    return path in (PAGE, FEED)


def get(h, path: str, query: dict) -> None:
    lim = P("browse_limit")
    if lim is not None and not lim.hit(_net(h), 600):
        h.send(429, b"too many requests\n", "text/plain")
        return
    if path == FEED:
        h.send(200, (json.dumps(approved_doc(), indent=1, ensure_ascii=False) + "\n").encode("utf-8"),
               "application/json; charset=utf-8", headers={"Access-Control-Allow-Origin": "*", "Cache-Control": "public, max-age=60"})
        return
    who, err = _viewer(h)
    if err == 503:
        h.send(503, page(None, "The website's sign-in isn't answering right now. Please try again shortly."))
        return
    h.send(200, page(who, kind=(query.get("kind") or [""])[0][:8]))


def _read_form(h):
    try:
        n = int(h.headers.get("Content-Length", "0"))
    except ValueError:
        return None
    if n < 0 or n > FORM_MAX:
        return None
    try:
        raw = h.rfile.read(n).decode("utf-8")
        return {k: v[0] for k, v in urllib.parse.parse_qs(raw, keep_blank_values=True, max_num_fields=10).items()}
    except (UnicodeDecodeError, ValueError):
        return None


def post(h, path: str) -> None:
    if path != PAGE:
        h.send(405, b"method not allowed\n", "text/plain")
        return
    fm = P("forum")
    if fm is not None and hasattr(fm, "_same_site") and not fm._same_site(h):
        h.send(403, page(None, "That form came from another site."))
        return
    form = _read_form(h)
    if form is None:
        h.send(400, page(None, "That form didn't arrive in one piece. Please try again."))
        return
    who, err = _viewer(h)
    if who is None:
        h.send(503 if err == 503 else 401, page(None))
        return
    kind, url, note = form.get("kind", "")[:8], form.get("url", "")[:MAX_URL + 50], form.get("note", "")[:MAX_NOTE]
    if not hmac.compare_digest(form.get("t", ""), form_token(who)):
        h.send(400, page(who, "The form timed out. Please send it again.", kind=kind, url=url, note=note))
        return
    ok, msg, _ = submit(url, kind, note, "site", {"acct": who.get("acct"), "name": who.get("name")}, _net(h))
    if not ok:
        h.send(400, page(who, msg, kind=kind, url=url, note=note))
        return
    h.send(200, page(who, done=True))
