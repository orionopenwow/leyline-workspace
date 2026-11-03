"""Report a bug, on the website and from the launcher (stream/wrath_portal.py hooks it in).

Owner, 1 Nov 04:18: "submitting a bug report, automatically including bug reports in a manifesto". Every report from
here goes to one intake file the GM console's bugs manifest reads (gmconsole/server/bugs_manifest.py), with the
in-game .bug reports and the Discord's #bug-reports channel: one de-duplicated backlog, docs/BUGS-MANIFEST.md.

    GET  /wrath/bugs[?where=<kind>&page=<text>]   the form (signed in on the website; signed out: how to sign in, and
                                                   the other ways to report: .bug in the game, #bug-reports on Discord)
    POST /wrath/bugs                               the form (fields + the form token "t"); answers with a thank-you page
    POST /wrath/bugs/api/report {text, where?, page?, app?}   the launcher: Authorization: Bearer <its Leyline sign-in
                                                   token, stream/social.py>, never the cookie, no CORS header.
                                                   {ok, id} | {error} with 400/401/403/429

Who is who: the forum's sign-in helpers (stream/forum.py viewer / token_viewer: the website's session, the launcher's
token), so the reporter is the player's Leyline profile name. Nothing else about them is written: no account id, no
email, no IP address (the network is used for a rate limit in memory only). Text is redacted as it arrives (the same
patterns as bugs_manifest.redact: emails, IPs, tokens, keys, passwords, webhook links, home-folder user names).

Limits: per account REPORTS_HOUR an hour and REPORTS_DAY a day, per network NET_HOUR an hour, all in memory; text
MIN_TEXT-TEXT_MAX characters. A site sanction (HQ ban or mute) stops reports too.

Storage: $WU_PORTAL_DATA/bugs/intake.jsonl (0600), one JSON object a line; at ROTATE_BYTES it becomes intake.jsonl.1
(the manifest reads both; its backlog keeps everything for good). WU_BUGS=0 turns the module off.
Standard library only.
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

log = logging.getLogger("wrath-portal.bugs")

PAGE = "/wrath/bugs"
API = "/wrath/bugs/api/report"
ON = os.environ.get("WU_BUGS", "1") != "0"
DATA = os.path.join(os.environ.get("WU_PORTAL_DATA", "/data"), "bugs")
INTAKE = os.path.join(DATA, "intake.jsonl")
ROTATE_BYTES = 4 * 1024 * 1024
MIN_TEXT, TEXT_MAX, PAGE_MAX, APP_MAX = 10, 1500, 120, 40
REPORTS_HOUR, REPORTS_DAY, NET_HOUR = 5, 20, 20
FORM_MAX = 8192
# (value, label): where the bug happened
WHERE = (("site", "The website"), ("launcher", "The launcher"), ("game", "In the game"), ("den", "Your Den"),
         ("host", "Hosting a server"), ("mods", "Mods & Workshop"), ("other", "Somewhere else"))
WHERE_LABEL = dict(WHERE)

_get = None
_lock = threading.Lock()
_key = secrets.token_bytes(32)
_acct_times: dict[str, list[float]] = {}
_net_times: dict[str, list[float]] = {}


def bind(getter) -> None:
    global _get
    _get = getter


def P(name):
    try:
        return _get(name) if _get else None
    except Exception:   # noqa: BLE001
        return None


def e(v) -> str:
    return html.escape(str(v), quote=True)


# ----------------------------------------------------------------------------------------------- redaction
# A copy of gmconsole/server/bugs_manifest.py _RED (the two ship to different places); gmconsole/tests/
# test_bugs_manifest.py checks they redact alike.
_RED = [
    (re.compile(r"(?i)https?://(?:\w+\.)?discord(?:app)?\.com/api/webhooks/\S+"), "[webhook link removed]"),
    (re.compile(r"(?i)\b(bearer|bot|basic)\s+[A-Za-z0-9._~+/=-]{12,}"), r"\1 [redacted]"),
    (re.compile(r"(?i)\b(pass(?:word)?|passwd|pwd|pw|token|secret|api[_ -]?key)(\s*(?:is|=|:)\s*)\S+"), r"\1\2[redacted]"),
    (re.compile(r"(?i)\b(key|auth|session|cookie|code)(\s*[=:]\s*)\S+"), r"\1\2[redacted]"),
    (re.compile(r"(?i)([?&](?:t|token|key|code|sig|secret|session|auth|access_token|password)=)[^&\s#]+"), r"\1[redacted]"),
    (re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+"), "[email]"),
    (re.compile(r"\b(?:(?:25[0-5]|2[0-4]\d|1?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|1?\d?\d)\b"), "[ip]"),
    (re.compile(r"(?i)(?<![\w:])(?:[0-9a-f]{1,4}:){5,7}[0-9a-f]{1,4}(?![\w:])"), "[ip]"),
    (re.compile(r"(?i)(?<![\w:])(?:[0-9a-f]{1,4}:)+:(?:[0-9a-f]{1,4}:?)*[0-9a-f]{0,4}(?![\w:])"), "[ip]"),
    (re.compile(r"(?i)\b[A-Za-z0-9_-]{24,}\.[A-Za-z0-9_-]{6,}\.[A-Za-z0-9_-]{20,}\b"), "[redacted]"),
    (re.compile(r"\b(?=[A-Za-z0-9_-]*\d)(?=[A-Za-z0-9_-]*[A-Za-z])[A-Za-z0-9_-]{32,}\b"), "[redacted]"),
    (re.compile(r"(?i)\b([A-Z]:\\+(?:Users|Documents and Settings)\\+)[^\\/\s]+"), r"\1[user]"),
    (re.compile(r"(/(?:home|Users)/)[^/\s]+"), r"\1[user]"),
    (re.compile(r"<@[!&]?\d+>"), "@someone"),
    (re.compile(r"<#\d+>"), "#channel"),
]
_CTRL = re.compile(r"[\x00-\x08\x0b-\x1f\x7f​-‏‪-‮⁦-⁩]")


def redact(text, n: int = TEXT_MAX) -> str:
    s = _CTRL.sub(" ", str(text or ""))
    for rx, rep in _RED:
        s = rx.sub(rep, s)
    return s.strip()[:n]


# ----------------------------------------------------------------------------------------------- the record
def make_record(source: str, reporter: str, text, where: str = "", page: str = "", app: str = "",
                now: float | None = None) -> tuple[dict | None, str]:
    """(record, "") or (None, why). The record is what the manifest reads; nothing in it names the account."""
    text = redact(re.sub(r"[ \t]+", " ", str(text or "")))
    if len(re.sub(r"\W", "", text)) < MIN_TEXT - 3 or len(text) < MIN_TEXT:
        return None, f"Tell us a little more: at least {MIN_TEXT} characters about what happened."
    where = where if where in WHERE_LABEL else ("launcher" if source == "launcher" else "other")
    page = redact(re.sub(r"\s+", " ", str(page or "")), PAGE_MAX)
    w = {"page": WHERE_LABEL[where] + (f": {page}" if page else "")}
    app = re.sub(r"[^\w .+-]", "", str(app or ""))[:APP_MAX]
    if app:
        w["app"] = app
    rid = secrets.token_hex(6)
    return {"id": f"{source}:{rid}", "source": source, "at": int(now if now is not None else time.time()),
            "reporter": str(reporter or "player")[:32], "text": text, "where": w}, ""


def append(rec: dict, path: str = "") -> None:
    path = path or INTAKE
    line = json.dumps(rec, ensure_ascii=False, separators=(",", ":")) + "\n"
    with _lock:
        os.makedirs(os.path.dirname(path), mode=0o700, exist_ok=True)
        try:
            if os.path.getsize(path) > ROTATE_BYTES:
                os.replace(path, path + ".1")
        except OSError:
            pass
        fd = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o600)
        try:
            os.write(fd, line.encode("utf-8"))
        finally:
            os.close(fd)


def _allow(table: dict, key: str, limits, now: float) -> bool:
    with _lock:
        ts = [t for t in table.get(key, []) if now - t < 86400]
        for n, window in limits:
            if sum(1 for t in ts if now - t < window) >= n:
                table[key] = ts
                return False
        ts.append(now)
        table[key] = ts
        if len(table) > 20000:
            table.clear()
        return True


def allowed(acct: str, net: str, now: float | None = None) -> str:
    now = time.time() if now is None else now
    if not _allow(_net_times, net, ((NET_HOUR, 3600),), now):
        return "A lot of reports came from your network this hour. Please try again later."
    if not _allow(_acct_times, acct, ((REPORTS_HOUR, 3600), (REPORTS_DAY, 86400)), now):
        return "Thanks! You've sent a lot of reports today; please wait a little before the next."
    return ""


def submit(who: dict, source: str, net: str, text, where="", page="", app="") -> tuple[int, dict]:
    """The one path for the form and the launcher: (http status, {ok, id} | {error})."""
    adm = P("admin")
    if adm is not None and hasattr(adm, "blocked"):
        try:
            blk = adm.blocked("post", acct=who.get("acct", ""))
        except Exception:   # noqa: BLE001
            blk = ""
        if blk:
            return 403, {"error": blk}
    rec, why = make_record(source, who.get("name") or "player", text, where, page, app)
    if rec is None:
        return 400, {"error": why}
    why = allowed(who.get("acct", ""), net)
    if why:
        return 429, {"error": why}
    try:
        append(rec)
    except OSError as ex:
        log.warning("bug report not saved: %s", ex.strerror)
        return 503, {"error": "We couldn't save that just now. Please try again shortly."}
    log.info("bug report %s from the %s", rec["id"], source)
    return 200, {"ok": True, "id": rec["id"].split(":", 1)[1]}


# ----------------------------------------------------------------------------------------------- pages
def match(path: str) -> bool:
    return ON and path in (PAGE, API)


def _net(h) -> str:
    ip, rb = h.ip(), P("rl_bucket")
    try:
        return rb(ip) if callable(rb) else ip
    except Exception:   # noqa: BLE001
        return ip


def form_token(who: dict) -> str:
    return hmac.new(_key, f"bug|{who.get('session', '')}|{who.get('acct', '')}".encode(), hashlib.sha256).hexdigest()[:32]


def _discord_line() -> str:
    url = P("DISCORD_URL")
    if isinstance(url, str) and url.startswith("https://"):
        return f'<li>On <a href="{e(url)}" rel="noopener noreferrer">our Discord</a>, in #bug-reports.</li>'
    return "<li>On our Discord, in #bug-reports.</li>"


OTHER_WAYS = ('<h2>Other ways to tell us</h2><ul><li>In the game, type <b>.bug</b> and what happened, for example '
              '<b>.bug the quest giver in Northshire has no ! over his head</b>. It saves where you are, so we can find it.</li>'
              '{discord}</ul><p class="fine">Every report lands in one list the team works through. Please don\'t put '
              'passwords or your email in a report; we take them out if you do.</p>')


def page(who: dict | None, msg: str = "", done: str = "", where: str = "", pg: str = "", text: str = "") -> bytes:
    other = OTHER_WAYS.format(discord=_discord_line())
    if done:
        body = (f'<section class="realm"><h1>Thanks, we have it</h1><p>Your report is in the list as <b>{e(done)}</b>. '
                'We read every one.</p><p><a href="/wrath">Back to Leyline</a> · <a href="' + PAGE + '">Report another</a></p>'
                f'{other}</section>')
    elif who is None:
        body = ('<section class="realm"><h1>Report a bug</h1><p>Sign in first, so we know who to thank and can '
                'tell your report apart from spam.</p>'
                f'<p><a class="btn primary" href="/login?next={urllib.parse.quote(PAGE, safe="")}">Sign in</a></p>'
                f'{other}</section>')
    else:
        opts = "".join(f'<option value="{k}"{" selected" if k == where else ""}>{e(t)}</option>' for k, t in WHERE)
        body = ('<section class="realm"><h1>Report a bug</h1>'
                + (f'<p class="fine" role="alert">{e(msg)}</p>' if msg else '')
                + f'<p>Signed in as <b>{e(who.get("name") or "player")}</b>. Say what you did, what you expected and '
                  'what happened instead.</p>'
                  f'<form method="post" action="{PAGE}"><input type="hidden" name="t" value="{e(form_token(who))}">'
                  f'<p><label for="bug-where">Where</label><br><select id="bug-where" name="where">{opts}</select></p>'
                  f'<p><label for="bug-page">Which page, realm or zone? (optional)</label><br><input id="bug-page" '
                  f'name="page" maxlength="{PAGE_MAX}" size="40" value="{e(pg)}"></p>'
                  f'<p><label for="bug-text">What happened?</label><br><textarea id="bug-text" name="text" required '
                  f'minlength="{MIN_TEXT}" maxlength="{TEXT_MAX}" rows="6" cols="60">{e(text)}</textarea></p>'
                  '<p><button class="btn primary" type="submit">Send report</button> <a href="/wrath">Cancel</a></p>'
                  f'</form>{other}</section>')
    render = P("render")
    if render is None:
        return body.encode("utf-8")
    return render(body, title="Report a bug")


def _viewer(h, token: bool = False):
    """(who, error status) from the forum's sign-in helpers; who None = signed out."""
    fm = P("forum")
    if fm is None:
        return None, 503
    try:
        return (fm.token_viewer(h) if token else fm.viewer(h)), 0
    except OSError:
        return None, 503
    except Exception as ex:   # noqa: BLE001 - forum.ForumError (a revoked token)
        return None, int(getattr(ex, "status", 401) or 401)


def get(h, path: str, query: dict) -> None:
    lim = P("browse_limit")
    if lim is not None and not lim.hit(_net(h), 600):
        h.send(429, b"too many requests\n", "text/plain")
        return
    if path != PAGE:
        h.send(405, b"method not allowed\n", "text/plain")
        return
    who, err = _viewer(h)
    if err == 503:
        h.send(503, page(None, "The website's sign-in isn't answering right now. Please try again shortly."))
        return
    q = lambda k: (query.get(k) or [""])[0]
    h.send(200, page(who, where=q("where")[:16], pg=redact(q("page"), PAGE_MAX)))


def _read(h, kind: str):
    try:
        n = int(h.headers.get("Content-Length", "0"))
    except ValueError:
        return None
    if n < 0 or n > FORM_MAX:
        return None
    try:
        raw = h.rfile.read(n).decode("utf-8")
        if kind == "json":
            d = json.loads(raw or "{}")
            return d if isinstance(d, dict) else None
        return {k: v[0] for k, v in urllib.parse.parse_qs(raw, keep_blank_values=True, max_num_fields=10).items()}
    except (UnicodeDecodeError, ValueError):
        return None


def _jpost(h, status: int, doc: dict) -> None:
    h.send(status, json.dumps(doc, separators=(",", ":")).encode("utf-8"), "application/json; charset=utf-8",
           headers={"Cache-Control": "no-store"})


def post(h, path: str) -> None:
    if path == API:
        doc = _read(h, "json")
        if not h.headers.get("Authorization", "").startswith("Bearer "):
            return _jpost(h, 401, {"error": "Sign in from the launcher first."})
        if doc is None:
            return _jpost(h, 400, {"error": "Bad request."})
        who, err = _viewer(h, token=True)
        if who is None:
            return _jpost(h, err or 401, {"error": "Signed out. Sign in again from the launcher." if err != 503
                                          else "Sign-in isn't available right now."})
        s = lambda k: doc.get(k) if isinstance(doc.get(k), str) else ""
        status, out = submit(who, "launcher", _net(h), s("text"), s("where") or "launcher", s("page"), s("app"))
        return _jpost(h, status, out)
    if path != PAGE:
        h.send(405, b"method not allowed\n", "text/plain")
        return
    fm = P("forum")
    if fm is not None and hasattr(fm, "_same_site") and not fm._same_site(h):
        h.send(403, page(None, "That form came from another site."))
        return
    form = _read(h, "form")
    if form is None:
        h.send(400, page(None, "That form didn't arrive in one piece. Please try again."))
        return
    who, err = _viewer(h)
    if who is None:
        h.send(503 if err == 503 else 401, page(None))
        return
    where, pg, text = form.get("where", "")[:16], form.get("page", ""), form.get("text", "")
    if not hmac.compare_digest(form.get("t", ""), form_token(who)):
        h.send(400, page(who, "The form timed out. Please send it again.", where=where, pg=pg[:PAGE_MAX], text=text[:TEXT_MAX]))
        return
    status, out = submit(who, "site", _net(h), text, where, pg)
    if status != 200:
        h.send(status, page(who, out["error"], where=where, pg=pg[:PAGE_MAX], text=text[:TEXT_MAX]))
        return
    h.send(200, page(who, done=out["id"]))
