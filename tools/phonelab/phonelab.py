#!/usr/bin/env python3
"""Phone Lab sessions, readable (docs/MOBILE-WEB-OVERHAUL.md; the portal side is stream/phonelab.py).

On the server (the Control step "phonelab", tools/phonelab/Phone Lab.bat, sends this file over ssh):
    python3 -I phonelab.py --dir /srv/wrath/portal-data/phonelab --latest [--code ABC123] [--copy /tmp/wu-phonelab-latest]
prints the newest (or the named) session's taps and copies its two files to <copy>.json and <copy>.jsonl.
On the PC:
    py -3 -I phonelab.py --file live/wu-phonelab-latest.jsonl [--shots live/shots]
prints the same from the copy and writes its sketches as JPEG files. Read only; prints no secrets (the portal kept none).
Last line: "PHONELAB OK: <code> <n> taps" or "PHONELAB NONE: <why>".
"""
import argparse
import base64
import json
import os
import re
import shutil
import sys
import time

CODE_RE = re.compile(r"[A-HJ-NP-Z2-9]{6}")


def load(jsonl: str) -> list:
    out = []
    try:
        with open(jsonl, encoding="utf-8") as f:
            for line in f:
                try:
                    out.append(json.loads(line))
                except ValueError:
                    pass
    except OSError:
        pass
    return out


def _seq(seq) -> str:
    return " ".join(f"{s[0]}@{s[1]}{'!' if len(s) > 2 and s[2] else ''}" for s in seq if isinstance(s, list) and len(s) >= 2)


def render(meta: dict, batches: list) -> str:
    """The session as lines a person (or Claude) reads: one block per tap, the other records one line each."""
    out = []
    m = meta or {}
    when = lambda t: time.strftime("%Y-%m-%d %H:%M:%S UTC", time.gmtime(t)) if isinstance(t, (int, float)) and t else "?"
    taps = sum(1 for b in batches for r in b.get("recs", []) if r.get("kind") == "tap")
    shots = sum(len(b.get("shots", [])) for b in batches)
    out.append(f"PHONE LAB {m.get('code', '?')}  started {when(m.get('started'))}  "
               + ("stopped " + when(m.get("stopped")) if m.get("stopped") else "live" if m.get("live") else "ended")
               + f"  {len(batches)} batches, {taps} taps, {shots} sketches")
    out.append("  (! after an event: the page called preventDefault on it; times in ms from the first touch)")
    page = None
    for b in sorted(batches, key=lambda b: (b.get("at", 0), b.get("seq", 0))):
        for r in b.get("recs", []):
            k = r.get("kind")
            if r.get("p") != page:
                page = r.get("p")
                out.append(f"--- {page}")
            t = f"{(r.get('t') or 0) / 1000:7.1f}s"
            if k == "page":
                out.append(f"{t}  PAGE  {r.get('ua', '')[:110]}  dpr {r.get('dpr')} screen {r.get('screen')} view {r.get('w')}x{r.get('h')} "
                           f"{r.get('o', '')} touch {r.get('touch')} {'installed app ' if r.get('standalone') else ''}{'(game page)' if r.get('game') else ''}")
            elif k == "timing":
                out.append(f"{t}  LOAD  first paint {r.get('fcp')} ms, DOM ready {r.get('dcl')} ms, loaded {r.get('load')} ms")
            elif k == "tap":
                bf, af = r.get("before") or {}, r.get("after") or {}
                out.append(f"{t}  TAP   ({r.get('x')},{r.get('y')}) on {r.get('on')}" + (f' "{r.get("text")}"' if r.get("text") else "")
                           + (f"  click on {r.get('click')}" if r.get("click") and r.get("click") != r.get("on") else "")
                           + ("" if r.get("click") else "  NO CLICK"))
                out.append(f"          {_seq(r.get('seq', []))}  moved {r.get('moved', 0)} px, {r.get('fingers', 1)} finger(s), "
                           f"page answered {r.get('resp', 'never')} ms" + (f", sketch #{r.get('shot')}" if r.get("shot") is not None else ""))
                if r.get("fsAsk"):
                    out.append(f"          full screen asked from {r.get('fsAsk')}")
                if bf.get("fs") != af.get("fs") or bf.get("o") != af.get("o") or bf.get("vv") != af.get("vv"):
                    out.append(f"          view {bf.get('w')}x{bf.get('h')} {bf.get('o')} fs={bf.get('fs')} -> {af.get('w')}x{af.get('h')} {af.get('o')} fs={af.get('fs')} zoom {af.get('vv')}")
                if r.get("cut"):
                    out.append(f"          (cut short: {r.get('cut')})")
            elif k == "click":
                out.append(f"{t}  CLICK ({r.get('x')},{r.get('y')}) on {r.get('on')} \"{r.get('text', '')}\" {r.get('pt', '')}")
            elif k == "fs-ask":
                out.append(f"{t}  FULL SCREEN ASK from {r.get('from') or 'no event'} gesture={r.get('gesture')} "
                           + ("ok" if r.get("ok") else f"refused: {r.get('error', '')}"))
            elif k == "fs":
                out.append(f"{t}  FULL SCREEN {'on' if r.get('on') else 'off'}")
            elif k == "view":
                out.append(f"{t}  VIEW  {r.get('why')}: {r.get('w')}x{r.get('h')} {r.get('o')} zoom {r.get('vv')} fs={r.get('fs')}")
            elif k == "scroll":
                out.append(f"{t}  SCROLL to {r.get('y')}")
            elif k in ("error", "console"):
                out.append(f"{t}  {k.upper()} {r.get('msg', '')} {r.get('at', '')}")
            elif k == "game":
                out.append(f"{t}  GAME  sign-in: {r.get('signin')}  first-tap full screen: {r.get('fsFirst')}  stages: {json.dumps(r.get('marks'))[:300]}")
            elif k == "visible":
                out.append(f"{t}  {'BACK' if r.get('on') else 'HIDDEN (another app, the lock button)'}")
            elif k == "stop":
                out.append(f"{t}  STOP pressed")
            else:
                out.append(f"{t}  {str(k).upper()} {json.dumps(r)[:200]}")
    out.append(f"PHONELAB OK: {m.get('code', '?')} {taps} taps")
    return "\n".join(out)


def write_shots(batches: list, folder: str) -> int:
    os.makedirs(folder, exist_ok=True)
    n = 0
    for b in batches:
        for s in b.get("shots", []):
            d = s.get("data", "")
            if not d.startswith("data:image/jpeg;base64,"):
                continue
            try:
                raw = base64.b64decode(d.split(",", 1)[1], validate=True)
            except ValueError:
                continue
            with open(os.path.join(folder, f"sketch-{int(s.get('i', 0)):03d}.jpg"), "wb") as f:
                f.write(raw)
            n += 1
    return n


def newest(folder: str, code: str = "") -> str:
    if code:
        return code if os.path.isfile(os.path.join(folder, code + ".json")) else ""
    best, at = "", -1
    try:
        names = os.listdir(folder)
    except OSError:
        return ""
    for n in names:
        if n.endswith(".json") and CODE_RE.fullmatch(n[:-5]):
            try:
                with open(os.path.join(folder, n), encoding="utf-8") as f:
                    s = json.load(f).get("started", 0)
            except (OSError, ValueError):
                continue
            if s > at:
                best, at = n[:-5], s
    return best


def main(argv=None) -> int:
    a = argparse.ArgumentParser()
    a.add_argument("--dir")
    a.add_argument("--latest", action="store_true")
    a.add_argument("--code", default="")
    a.add_argument("--copy", default="")
    a.add_argument("--file")
    a.add_argument("--shots", default="")
    o = a.parse_args(argv)
    if o.file:
        jsonl = o.file
        meta_f = jsonl[:-1] if jsonl.endswith(".jsonl") else jsonl + ".json"
    else:
        code = newest(o.dir or ".", o.code if CODE_RE.fullmatch(o.code or "") else "")
        if not code:
            print("PHONELAB NONE: no session" + (" " + o.code if o.code else "") + " in the last 24 hours")
            return 1
        jsonl, meta_f = os.path.join(o.dir, code + ".jsonl"), os.path.join(o.dir, code + ".json")
        if o.copy:
            shutil.copyfile(meta_f, o.copy + ".json")
            if os.path.isfile(jsonl):
                shutil.copyfile(jsonl, o.copy + ".jsonl")
            else:
                open(o.copy + ".jsonl", "w").close()
    try:
        with open(meta_f, encoding="utf-8") as f:
            meta = json.load(f)
    except (OSError, ValueError):
        meta = {}
    if meta.get("ends"):
        meta["live"] = not meta.get("stopped") and time.time() < meta["ends"]
    batches = load(jsonl)
    print(render(meta, batches))
    if o.shots:
        print(f"{write_shots(batches, o.shots)} sketches written to {o.shots}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
