"""The phone and screenshot sweep (docs/AUDIT.md "Mobile"; owner 3 Nov: "a sweeping smoke test for mobile login and
features and info and refresh of everything, new screenshots if things updated, a full sweep pass").

Section "mobile": the website's main pages on three phones (iPhone 13, Pixel 7, a small 360x640) in headless Chromium
(tools/audit/mobile_sweep.cjs, Playwright), each one: it answers 200, nothing scrolls sideways, primary buttons are at
least 40 px to tap, no console or page errors, no text cut off, no banned words; a screenshot per page and phone in
logs/audit-shots/mobile/. On the iPhone 13 the Hero Key journey: the audit's own test account (made and closed the
way the herokey section does it: account/new {audit: true}, audit/close), its key pasted on /wrath/key, the profile,
Submit a link while logged in, Log out. Then the same pages on a desktop (1440x900) into logs/audit-shots/desktop/,
and every new screenshot against the last run's (kept in logs/audit-shots/previous/): which pages changed, so the
owner knows which marketing and docs screenshots to take again (in the report).

Needs node and Playwright on this PC; without them the section is one SKIP with the fix. LEYLINE_AUDIT_MOBILE=0
skips it. The key goes to the browser on stdin, is hidden from every line, and never reaches a screenshot.
"""
from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import time
from pathlib import Path

from common import BANNED, FAIL, HERE, PASS, SKIP

F = "Mobile"
S = "Screenshots"
FIX = "Install Node.js (winget install OpenJS.NodeJS.LTS), then: npm install -g playwright && npx playwright install chromium"
TELL = "Tell Claude this line and logs\\audit-latest.txt."
SCRIPT = HERE / "mobile_sweep.cjs"
DEVICES = {"iphone13": "iPhone 13", "pixel7": "Pixel 7", "small360": "360x640"}
THRESHOLD = 0.005          # a screenshot "changed" when more than 0.5 % of its pixels did

# (key, title, path): the pages a new player meets first. The forum thread is found when the run starts.
PAGES = [
    ("home", "Home", "/wrath/leyline"),
    ("download", "Download", "/wrath/download"),
    ("login", "Log in (Hero Key)", "/wrath/key"),
    ("forum", "Forum", "/wrath/leyline/forum"),
    ("mods", "Mods", "/wrath/leyline-preview/catalog"),
    ("addons", "Addons", "/wrath/leyline-preview/addons"),
    ("game-modes", "Game modes", "/wrath/modes"),
    ("loadouts", "Loadouts", "/wrath/leyline-preview/loadouts"),
    ("docs-getting-started", "Docs: Getting started", "/wrath/docs/getting-started"),
    ("docs-host-a-realm", "Docs: Host a realm", "/wrath/docs/host-a-realm"),
    ("docs-den-invite", "Docs: Invite a friend to your Den", "/wrath/docs/den-invite"),
    ("docs-den", "Docs: Your Den", "/wrath/docs/den"),
    ("submit", "Submit a link", "/wrath/submit"),
    ("survivors", "Azeroth Survivors", "/wrath/survivors"),
    ("agents", "Agents", "/wrath/leyline-preview/agents"),
    ("forge", "Forge", "/wrath/leyline-preview/forge"),
]
JOURNEY = [
    ("login-signed-in", "Log in on a phone with a pasted Hero Key"),
    ("profile", "Your profile shows on a phone, with Log out"),
    ("submit-signed-in", "Submit a link while logged in, on a phone"),
    ("logged-out", "Log out on a phone"),
]
DISCLAIMER = re.compile(r"(?i)affiliated with or endorsed by\s*$")


def on() -> bool:
    return os.environ.get("LEYLINE_AUDIT_MOBILE", "1") != "0"


# ---------------------------------------------------------------------------------------------- Playwright on this PC
def node_env() -> tuple[str, dict] | tuple[None, str]:
    """(node, env with NODE_PATH) when node can load Playwright here, else (None, why)."""
    node = shutil.which("node")
    if not node:
        return None, "Node.js isn't on this PC"
    env = dict(os.environ)
    paths = [p for p in env.get("NODE_PATH", "").split(os.pathsep) if p]
    npm = shutil.which("npm")
    if npm:
        try:
            root = subprocess.run([npm, "root", "-g"], capture_output=True, text=True, timeout=60, shell=False).stdout.strip()
            if root:
                paths.append(root)
        except (OSError, subprocess.SubprocessError):
            pass
    for d in (HERE.parents[1] / "launcher" / "node_modules", HERE.parents[1] / "node_modules"):
        if d.is_dir():
            paths.append(str(d))
    env["NODE_PATH"] = os.pathsep.join(dict.fromkeys(paths))
    try:
        r = subprocess.run([node, "-e", "require.resolve('playwright')"], capture_output=True, text=True, timeout=60, env=env)
    except (OSError, subprocess.SubprocessError) as e:
        return None, f"node didn't start ({type(e).__name__})"
    if r.returncode:
        return None, "Playwright isn't installed for Node.js on this PC"
    return node, env


# ---------------------------------------------------------------------------------------------- the checks
def banned_in(text: str) -> list[str]:
    """Banned words a player reads (the legal disclaimer stays, as web.page_problems has it)."""
    out = []
    for m in BANNED.finditer(text or ""):
        if DISCLAIMER.search(text[max(0, m.start() - 40):m.start()]):
            continue
        out.append(re.sub(r"\s+", " ", text[max(0, m.start() - 25):m.end() + 25]).strip())
    return out


def problems(r: dict, desktop: bool = False) -> list[str]:
    """Everything wrong with one page on one device (r: a mobile_sweep.cjs result)."""
    if r.get("leak"):
        return ["the test account's key shows on the page (no screenshot taken)"]
    st = r.get("status", 0)
    if st != 200:
        return [f"HTTP {st or r.get('error') or 'no answer'}"]
    out = []
    if not desktop and r.get("scrollWidth", 0) > r.get("innerWidth", 0) + 1:
        out.append(f"scrolls sideways ({r['scrollWidth']} px wide on a {r['innerWidth']} px screen)")
    if not desktop and r.get("small"):
        out.append("buttons under 40 px: " + "; ".join(r["small"][:4]))
    if r.get("errors"):
        out.append("errors: " + "; ".join(r["errors"][:3]))
    if not desktop and r.get("cut"):
        out.append("text cut off: " + "; ".join(r["cut"][:3]))
    if not desktop and r.get("past"):
        out.append("text past the screen's edge: " + "; ".join(r["past"][:3]))
    words = banned_in(r.get("text", "")) + banned_in(r.get("attrs", ""))
    if words:
        out.append("banned words: " + "; ".join(dict.fromkeys(words[:3])))
    return out


def page_lines(results: list[dict], pages) -> list[tuple]:
    """(title, status, detail, action) per page, over the three phones."""
    by = {}
    for r in results:
        by.setdefault(r.get("page"), []).append(r)
    out = []
    for key, title, path in pages:
        rs = by.get(key, [])
        if not rs:
            out.append((f"{title} on phones", SKIP, f"{path}: not reached", ""))
            continue
        bad = [(DEVICES.get(r.get("device"), r.get("device")), p) for r in rs for p in problems(r)]
        if bad:
            detail = f"{path}: " + " | ".join(f"{d}: {p}" for d, p in bad[:6])
            act = ("The page doesn't answer: run the Control step \"signin\" (it deploys the site), then the audit again."
                   if any(p.startswith("HTTP") for _, p in bad) else
                   f"Open {path} on a phone (or the screenshot in logs\\audit-shots\\mobile) and fix the layout. {TELL}")
            out.append((f"{title} on phones", FAIL, detail, act))
        else:
            out.append((f"{title} on phones", PASS, f"{path}: " + ", ".join(DEVICES.get(r.get("device"), "") for r in rs)
                        + ": 200, fits the screen, buttons big enough, no errors", ""))
    return out


# ---------------------------------------------------------------------------------------------- screenshots
def rotate(shots: Path) -> Path | None:
    """The last run's screenshots move to <shots>/previous (the run before that is dropped). Returns it, or None."""
    prev = shots / "previous"
    have = [d for d in ("mobile", "desktop") if (shots / d).is_dir() and any((shots / d).iterdir())]
    if not have:
        return prev if prev.is_dir() else None
    shutil.rmtree(prev, ignore_errors=True)
    prev.mkdir(parents=True, exist_ok=True)
    for d in have:
        shutil.move(str(shots / d), str(prev / d))
    return prev


def thread_path(ctx) -> str:
    """One forum thread to open (the newest), or ''."""
    d = ctx.get(ctx.url("site", "/wrath/forum/api/latest?n=1"), timeout=20).json()
    th = (d or {}).get("threads") if isinstance(d, dict) else None
    if th and isinstance(th, list) and isinstance(th[0], dict) and th[0].get("id"):
        return f"/wrath/leyline/forum/t/{th[0]['id']}"
    return ""


# ---------------------------------------------------------------------------------------------- the test account
def account(ctx, rec):
    """The herokey section's way to make (and later close) the audit's own test account; its lines say "Mobile"."""
    import herokey

    class PhoneKey(herokey.Journey):
        def add(self, check, ok, detail="", action="", status=None, t0=None):
            st = status or (PASS if ok else FAIL)
            self.rec.add(F, "Test account: " + check, st, detail, "" if st != FAIL else (action or TELL),
                         (time.time() - t0) if t0 else 0)
            return st == PASS

    j = PhoneKey(ctx, rec)
    if not j.new_account():
        return j, False
    # saved first, so the signed-in card never shows the key (and no screenshot can hold it)
    j.site.api("POST", "herokey/saved", token=j.token)
    return j, True


# ---------------------------------------------------------------------------------------------- the section
def run(ctx, rec, logs: Path | None = None) -> None:
    if not on():
        rec.add(F, "Phone sweep", SKIP, "LEYLINE_AUDIT_MOBILE=0")
        return
    node, env = node_env()
    if not node:
        rec.add(F, "Phone sweep", SKIP, f"{env}. Fix: {FIX}")
        rec.add(S, "Desktop screenshots and what changed", SKIP, f"{env}. Fix: {FIX}")
        return
    logs = Path(logs or ctx.cache.get("logs") or (HERE.parents[1] / "logs"))
    shots = logs / "audit-shots"
    shots.mkdir(parents=True, exist_ok=True)
    prev = rotate(shots)
    pages = list(PAGES)
    tp = thread_path(ctx)
    if tp:
        pages.insert(4, ("forum-thread", "Forum: a thread", tp))
    j, have_key = (None, False)
    if os.environ.get("LEYLINE_AUDIT_HEROKEY", "1") != "0":
        j, have_key = account(ctx, rec)
    else:
        rec.add(F, "Test account", SKIP, "LEYLINE_AUDIT_HEROKEY=0: the phone Log in journey is left out")
    try:
        job = {"base": ctx.base("site"), "signin": (ctx.fake.get("signin") or ""), "fake": bool(ctx.fake),
               "origin_as": ("https://" + os.environ.get("PUBLIC_HOST", "")) if ctx.fake and os.environ.get("PUBLIC_HOST") else "",
               "shots": str(shots), "previous": str(prev) if prev else "", "threshold": THRESHOLD,
               "pages": [{"key": k, "title": t, "path": p} for k, t, p in pages],
               "key": j.key if have_key else "", "name": (j.me.get("name") if have_key else "") or ""}
        t0 = time.time()
        try:
            p = subprocess.run([node, str(SCRIPT)], input=json.dumps(job), capture_output=True, text=True,
                               timeout=1800, env=env, encoding="utf-8", errors="replace")
        except subprocess.TimeoutExpired:
            rec.add(F, "Phone sweep", FAIL, "the browser sweep took over 30 minutes", TELL)
            return
        line = next((x for x in reversed(p.stdout.splitlines()) if x.startswith("SWEEP_JSON ")), "")
        if p.returncode or not line:
            tail = " ".join((p.stderr or "").strip().splitlines()[-3:])
            if re.search(r"Executable doesn't exist|browserType\.launch", tail):
                rec.add(F, "Phone sweep", SKIP, "Playwright's Chromium isn't installed. Fix: npx playwright install chromium")
                return
            rec.add(F, "Phone sweep", FAIL, f"the browser sweep stopped (exit {p.returncode}): {tail[:300]}", TELL)
            return
        res = json.loads(line[len("SWEEP_JSON "):])
        secs = time.time() - t0
        record(rec, res, pages, have_key, shots, secs)
        ctx.cache["shots"] = summary(res, shots, prev)
    finally:
        if j is not None:
            j.cleanup()


def record(rec, res: dict, pages, have_key: bool, shots: Path, secs: float = 0) -> None:
    for title, st, detail, act in page_lines(res.get("mobile", []), pages):
        rec.add(F, title, st, detail, act)
    if have_key:
        got = {s.get("step"): s for s in res.get("journey", [])}
        for key, title in JOURNEY:
            s = got.get(key)
            if not s:
                rec.add(F, title, SKIP, "not reached (an earlier step failed)")
                continue
            bad = [] if not s.get("ok") else problems(dict(s, status=200))
            ok = bool(s.get("ok")) and not bad
            rec.add(F, title, PASS if ok else FAIL, (s.get("detail") or "") + ("; " + "; ".join(bad) if bad else ""),
                    "" if ok else f"Try it on a phone: /wrath/key, paste a Hero Key, Log in. {TELL}")
    desk = res.get("desktop", [])
    saved = [r for r in desk if r.get("shot")]
    bad = [f"{r.get('page')}: {'; '.join(problems(r, desktop=True))}" for r in desk if problems(r, desktop=True)]
    rec.add(S, "Desktop screenshots at 1440x900", FAIL if bad else PASS,
            f"{len(saved)} saved in logs\\audit-shots\\desktop" + (" | " + " | ".join(bad[:5]) if bad else ""),
            TELL if bad else "", secs)
    n_mobile = len([r for r in res.get("mobile", []) + res.get("journey", []) if r.get("shot")])
    changed = res.get("changed", [])
    if not res.get("compared") and not res.get("fresh"):
        rec.add(S, "Screenshots changed since the last run", PASS,
                f"the first run: {n_mobile} phone and {len(saved)} desktop screenshots kept to compare with next time")
    else:
        rec.add(S, "Screenshots changed since the last run", PASS,
                f"{len(changed)} changed, {res.get('same', 0)} the same, {len(res.get('fresh', []))} new"
                + (": " + ", ".join(c["shot"].replace("\\", "/") for c in changed[:12]) if changed else ""))


def summary(res: dict, shots: Path, prev) -> dict:
    """What the report shows under "Screenshots": the folder, and the pages that changed."""
    return {"dir": str(shots), "previous": str(prev) if prev else "",
            "changed": [{"shot": c["shot"].replace("\\", "/"), "share": c.get("share", 0)} for c in res.get("changed", [])],
            "fresh": [s.replace("\\", "/") for s in res.get("fresh", [])], "same": res.get("same", 0),
            "count": len([r for r in res.get("mobile", []) + res.get("journey", []) + res.get("desktop", []) if r.get("shot")])}
