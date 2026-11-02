"""The audit's report: logs/audit-latest.txt and logs/audit-latest.html (green or red per feature, what failed, the
one next action, and the short list only a person can check). MANUAL is the same list as docs/AUDIT.md's
(tools/audit/tests/test_audit.py keeps them equal)."""
from __future__ import annotations

import html
import time

from common import FAIL, PASS, SKIP

ORDER = ["Server", "Site", "Downloads", "Catalog", "Wrath Unleashed", "Bug reports", "Your Den", "Hosting",
         "Launcher", "Looks", "Forum", "Azeroth Survivors"]

# (title, where to click, what good looks like): only what a person must judge, or a second PC must do
MANUAL = [
    ("Play solo with your real game",
     "Leyline > Home > Your Den > Play. At the login screen: account DEN, password den, Login.",
     "No firewall or Defender prompt. Character select shows the Forever UI look; Enter puts you in the white room, "
     "dressed, with the chair the quick start talks about. Close Leyline, open it again, Play: straight in again."),
    ("Jaina looks and walks right",
     "In Your Den, run a few steps, then type: Jaina, come",
     "Jaina is her own model (not a cube or a missing model), walks and runs after you with her feet moving, "
     "and her lines show as \"Jaina says:\" in chat and in a bubble over her head."),
    ("Forge opens on the Den tile",
     "Leyline > Your Den > Shape the ground > Open in Forge. Raise a hill, Ctrl+S, close Forge.",
     "Forge opens on the Den's own tile (not an empty map). After closing: \"Terrain updated\". The next Play shows the hill."),
    ("A friend on another network joins your Den",
     "Leyline > Your Den > Invite friends > copy the link. Your friend (another home, not your Wi-Fi): "
     "Leyline > Realms > Got an invite link? > paste > Play.",
     "Your friend lands at character select on their own character and comes into your Den; you see each other "
     "move and talk. Stop sharing: their next login is refused."),
    ("Wrath Unleashed on your desktop game",
     "Leyline > Play (Wrath Unleashed chosen) > log in > make or pick a character > Enter world. Then Esc > Report a bug.",
     "Nameplates are easy to read and sit right over heads; the bars and frames look finished. Report a bug opens a "
     "box (press Cancel, so nothing is filed)."),
    ("The browser game feels right",
     "On the website: Play in the browser > sign in > Play.",
     "Character select within about 2 minutes on a first visit, much faster on the second. In the world: smooth "
     "turning, about 30 frames a second or better, no long freezes when you run into a new area."),
    ("Host a server, start to finish",
     "Leyline > Host > Host a mode > Classic levelling > Host. Wait for \"You're live\".",
     "The first start finishes (5 to 20 minutes). \"You're live\" shows a link; the realm is on Realms (public) and "
     "a friend outside your network can join it with the link."),
    ("The website when you are signed in",
     "On the website: Sign in (Hero Key or Google) > Forum > New thread (don't post) > Report a bug (don't send) > Host > Start server.",
     "Sign-in brings you back where you were. The forum and bug forms show (not \"sign in\"). Start server gives a "
     "file that runs on your PC and the page turns live."),
    ("Looks: one click changes the game",
     "Leyline > Looks > Classic > Apply, then Play. Afterwards Looks > Balanced > Apply.",
     "The game opens looking different (shorter view, plainer shadows); Balanced puts it back."),
    ("Azeroth Survivors (only when its page shows Play)",
     "Website > Azeroth Survivors > Play.",
     "The game opens and a run starts. The pick screen and the results screen show no franchise name or logo."),
]


def next_action(results) -> str:
    fails = [r for r in results if r.status == FAIL]
    if not fails:
        return "Nothing failed. Do the manual list (about 30 minutes, best with a friend online)."
    fails.sort(key=lambda r: ORDER.index(r.feature) if r.feature in ORDER else len(ORDER))
    r = fails[0]
    return f"{r.feature}: {r.check} failed. {r.action or 'Tell Claude this line.'}"


def features(results):
    out = {}
    for r in results:
        out.setdefault(r.feature, []).append(r)
    return sorted(out.items(), key=lambda kv: ORDER.index(kv[0]) if kv[0] in ORDER else len(ORDER))


def counts(results):
    c = {PASS: 0, FAIL: 0, SKIP: 0}
    for r in results:
        c[r.status] += 1
    return c


def text(results, meta: dict) -> str:
    c = counts(results)
    out = [f"Leyline release audit, {meta.get('when', '')} ({meta.get('secs', 0):.0f} s, {meta.get('mode', 'live')})",
           f"Result: {'ALL GREEN' if not c[FAIL] else str(c[FAIL]) + ' FAILED'}  ({c[PASS]} passed, {c[FAIL]} failed, {c[SKIP]} skipped)",
           f"Next: {next_action(results)}", ""]
    for feat, rs in features(results):
        out.append(f"== {feat}")
        for r in rs:
            out.append(f"{r.status:<4}  {r.check}" + (f"  ({r.detail})" if r.detail else "")
                       + (f"\n      -> {r.action}" if r.status == FAIL and r.action else ""))
        out.append("")
    out.append("== Only you can check these (docs/AUDIT.md)")
    for i, (t, where, good) in enumerate(MANUAL, 1):
        out += [f"{i}. {t}", f"   Where: {where}", f"   Good:  {good}"]
    if meta.get("appendix"):
        out += ["", "== Every page line the site checks flagged"] + [f"  {x}" for x in meta["appendix"]]
    return "\n".join(out) + "\n"


CSS = """
:root{--bg:#f6f5f2;--card:#fff;--text:#1d1f23;--dim:#5f6670;--line:#e3e1dc;--ok:#1d7a44;--okbg:#e5f4ea;--bad:#b3261e;
--badbg:#fbe9e7;--skip:#7a6a2c;--skipbg:#f6f0dc;--accent:#8a5a12}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--bg:#121417;--card:#1a1d21;--text:#e9ebef;--dim:#9aa1ab;
--line:#2c3036;--ok:#5fd18a;--okbg:#14301f;--bad:#ff8378;--badbg:#3a1a17;--skip:#e3c76b;--skipbg:#33290f;--accent:#f1cf86}}
:root[data-theme="dark"]{--bg:#121417;--card:#1a1d21;--text:#e9ebef;--dim:#9aa1ab;--line:#2c3036;--ok:#5fd18a;--okbg:#14301f;
--bad:#ff8378;--badbg:#3a1a17;--skip:#e3c76b;--skipbg:#33290f;--accent:#f1cf86}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text);font:15px/1.5 system-ui,-apple-system,"Segoe UI",sans-serif}
main{max-width:980px;margin:0 auto;padding:24px 16px 60px}
h1{font-size:24px;margin:0 0 4px}h2{font-size:18px;margin:28px 0 10px}.dim{color:var(--dim);font-size:13px}
.verdict{margin:16px 0;padding:16px 18px;border-radius:12px;font-weight:600;font-size:18px}
.verdict.ok{background:var(--okbg);color:var(--ok)}.verdict.bad{background:var(--badbg);color:var(--bad)}
.next{padding:14px 18px;border:2px solid var(--accent);border-radius:12px;background:var(--card)}
.next b{color:var(--accent)}
ol.manual{padding-left:22px}ol.manual li{margin:0 0 12px}ol.manual .w{display:block;color:var(--dim);font-size:14px}
ol.manual .g{display:block;font-size:14px}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(200px,1fr));gap:10px}
.feat{background:var(--card);border:1px solid var(--line);border-left:6px solid var(--ok);border-radius:10px;padding:10px 12px}
.feat.bad{border-left-color:var(--bad)}.feat.skip{border-left-color:var(--skip)}
.feat b{display:block}.feat span{font-size:13px;color:var(--dim)}
details{background:var(--card);border:1px solid var(--line);border-radius:10px;margin:8px 0;padding:8px 12px}
summary{cursor:pointer;font-weight:600}
table{width:100%;border-collapse:collapse;margin-top:8px;font-size:14px}td{padding:6px 6px;border-top:1px solid var(--line);vertical-align:top}
td.s{width:56px}.tag{display:inline-block;font-size:11px;font-weight:700;padding:2px 7px;border-radius:5px}
.tag.PASS{background:var(--okbg);color:var(--ok)}.tag.FAIL{background:var(--badbg);color:var(--bad)}.tag.SKIP{background:var(--skipbg);color:var(--skip)}
.act{color:var(--bad);font-size:13px}.det{color:var(--dim);font-size:13px;overflow-wrap:anywhere}
"""


def page(results, meta: dict) -> str:
    e = html.escape
    c = counts(results)
    ok = not c[FAIL]
    cards, groups = [], []
    for feat, rs in features(results):
        n = counts(rs)
        cls = "bad" if n[FAIL] else ("skip" if not n[PASS] else "")
        cards.append(f'<div class="feat {cls}"><b>{e(feat)}</b><span>{n[PASS]} passed'
                     + (f", {n[FAIL]} failed" if n[FAIL] else "") + (f", {n[SKIP]} skipped" if n[SKIP] else "") + "</span></div>")
        rows = "".join(
            f'<tr><td class="s"><span class="tag {r.status}">{r.status}</span></td><td>{e(r.check)}'
            + (f'<div class="det">{e(r.detail)}</div>' if r.detail else "")
            + (f'<div class="act">Next: {e(r.action)}</div>' if r.status == FAIL and r.action else "") + "</td></tr>"
            for r in rs)
        groups.append(f'<details{" open" if n[FAIL] else ""}><summary>{e(feat)}: '
                      f'{"red" if n[FAIL] else ("green" if n[PASS] else "skipped")}</summary><table>{rows}</table></details>')
    manual = "".join(f'<li><b>{e(t)}</b><span class="w">Where: {e(w)}</span><span class="g">Good: {e(g)}</span></li>'
                     for t, w, g in MANUAL)
    return f"""<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>Release audit</title><style>{CSS}</style></head>
<body><main>
<h1>Leyline release audit</h1>
<div class="dim">{e(meta.get('when', ''))} · {meta.get('secs', 0):.0f} seconds · {e(meta.get('mode', 'live'))} · {e(meta.get('where', ''))}</div>
<div class="verdict {'ok' if ok else 'bad'}">{'Every automated check passed' if ok else f'{c[FAIL]} check{"s" if c[FAIL] != 1 else ""} failed'}
 <span class="dim">({c[PASS]} passed, {c[FAIL]} failed, {c[SKIP]} skipped)</span></div>
<div class="next"><b>Next:</b> {e(next_action(results))}</div>
<h2>Only you can check these</h2>
<p class="dim">The automated run can't judge these, or they need a second PC. About 30 minutes; 4 and 7 need a friend.</p>
<ol class="manual">{manual}</ol>
<h2>Features</h2><div class="grid">{''.join(cards)}</div>
<h2>Every check</h2>{''.join(groups)}
</main></body></html>"""


def write(results, meta: dict, logs) -> tuple:
    logs.mkdir(parents=True, exist_ok=True)
    meta.setdefault("when", time.strftime("%Y-%m-%d %H:%M"))
    t, h = logs / "audit-latest.txt", logs / "audit-latest.html"
    t.write_text(text(results, meta), encoding="utf-8")
    h.write_text(page(results, meta), encoding="utf-8")
    return t, h
