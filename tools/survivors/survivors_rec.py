#!/usr/bin/env python3
"""Azeroth Survivors: the recorder, run on the owner's PC by "Record Survivors files.bat" (docs/SURVIVORS.md).

A small logging proxy on loopback in front of the upstream wenilla-host, which serves the game page and the
owner's OWN game files from his own client folder. It passes every request through unchanged and writes down each
/data name the game asks for, so the exporter (survivors_export.py) can copy exactly those files into the slice the
Survivors address serves. Nothing leaves this PC; nothing is uploaded here.

    py -3 -I survivors_rec.py --upstream http://127.0.0.1:8090 --listen 127.0.0.1:8091 --out <folder>
        [--seconds 80] [--plan full|quick] [--no-browser-hint]

Open http://127.0.0.1:8091/__rec/ in Edge or Chrome (WebGPU): the recorder page plays the game's own autopilot
(?survivors_auto=<Class>:<mode>&survivors_map=<0..15>) once per pass in a frame, one pass after another, then asks
for one pass by hand (click every race and both genders on the pick screen: the autopilot always takes a class's
first look), then says done. The proxy exits when the page says done (or on Ctrl+C), keeping what it has.

Written to <folder> (appended, so a second session adds to the first):
    recorded.txt   every /data name served with 200, once (any case), in the order first asked
    missing.txt    names the game asked for that the owner's client does not have (404), once
    requests.log   one line per /data request: time, status, bytes, name
    passes.json    the passes run and when

/data answers go back with Cache-Control: no-store, so the browser's cache never hides a name from a later session.
Standard library only (Python 3.9+).
"""
from __future__ import annotations

import argparse
import http.client
import json
import os
import sys
import threading
import time
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

CLASSES = ("Warrior", "Paladin", "Hunter", "Rogue", "Priest", "Shaman", "Mage", "Warlock", "Druid")
MAPS = 16   # the game's battlegrounds, 0..15 (upstream data::MAPS)
HOP = {"connection", "keep-alive", "proxy-authenticate", "proxy-authorization", "te", "trailers",
       "transfer-encoding", "upgrade"}


def plan(kind: str) -> list[dict]:
    """The passes, as query strings for the game page. full: every zone once with its crowd at its thickest (:late,
    the whole ability pool at its top level) and every class once with its whole pool (:all) on a zone of its own,
    then the results screen (:idle) and the hand pass. quick: two zones, two classes, the results, the hand pass."""
    out = []
    zones = range(MAPS) if kind == "full" else (0, 5)
    for i, m in enumerate(zones):
        c = CLASSES[i % len(CLASSES)]
        out.append({"label": f"zone {m} ({c}, crowd)", "q": f"survivors_auto={c}:late&survivors_map={m}"})
    classes = CLASSES if kind == "full" else CLASSES[:2]
    for i, c in enumerate(classes):
        out.append({"label": f"{c}, every spell", "q": f"survivors_auto={c}:all&survivors_map={(i * 7 + 3) % MAPS}"})
    out.append({"label": "results screen", "q": "survivors_auto=Warrior:idle&survivors_map=0", "seconds": 150})
    out.append({"label": "by hand: the pick screen", "q": "", "hand": True})
    return out


class Log:
    """The name lists, shared by the request threads. Case-insensitive, first spelling kept."""

    def __init__(self, out: str):
        os.makedirs(out, exist_ok=True)
        self.out = out
        self.lock = threading.Lock()
        self.seen: dict[str, set] = {"recorded.txt": set(), "missing.txt": set()}
        for f, s in self.seen.items():
            try:
                with open(os.path.join(out, f), encoding="utf-8") as fh:
                    s.update(line.strip().lower() for line in fh if line.strip())
            except FileNotFoundError:
                pass
        self.counts = {"ok": 0, "missing": 0, "requests": 0}

    def add(self, status: int, size: int, name: str) -> None:
        with self.lock:
            self.counts["requests"] += 1
            with open(os.path.join(self.out, "requests.log"), "a", encoding="utf-8") as f:
                f.write(f"{time.strftime('%Y-%m-%dT%H:%M:%S')}\t{status}\t{size}\t{name}\n")
            if name.startswith("__"):
                return   # __index and the other lists are the host's, not game files
            f = "recorded.txt" if status == 200 else "missing.txt" if status == 404 else ""
            if not f or name.lower() in self.seen[f]:
                return
            self.seen[f].add(name.lower())
            self.counts["ok" if status == 200 else "missing"] += 1
            with open(os.path.join(self.out, f), "a", encoding="utf-8") as fh:
                fh.write(name + "\n")

    def summary(self) -> dict:
        with self.lock:
            return {"names": len(self.seen["recorded.txt"]), "missing": len(self.seen["missing.txt"]),
                    "this_session": dict(self.counts)}


def data_name(path: str) -> str | None:
    """The game file name of a /data request ("DBFilesClient\\Map.dbc"), or None for any other path."""
    p = path.split("?", 1)[0]
    if not p.startswith("/data/"):
        return None
    name = urllib.parse.unquote(p[len("/data/"):])
    return name.replace("/", "\\") if name else None


REC_PAGE = """<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Survivors recorder</title>
<style>html,body{margin:0;height:100%;background:#0b0e1a;color:#e8ecff;font:15px system-ui,sans-serif}
#bar{display:flex;gap:14px;align-items:center;padding:8px 12px;background:#141a33}
#bar b{color:#8fd3ff}#game{border:0;width:100%;height:calc(100% - 44px);display:block}
button{font:inherit;padding:4px 12px}</style></head><body>
<div id="bar"><b>Recorder</b><span id="what">starting</span><span id="left"></span>
<button id="next" hidden>Done with this pass</button><span id="names"></span></div>
<iframe id="game" allow="fullscreen; gamepad; autoplay"></iframe>
<script>
const P = __PLAN__, SECONDS = __SECONDS__;
const $ = id => document.getElementById(id);
let i = 0, timer = 0;
async function stats() { try { const r = await fetch('/__rec/stats'); const s = await r.json();
  $('names').textContent = s.names + ' names so far, ' + s.missing + ' not in your client'; } catch (e) {} }
setInterval(stats, 3000);
async function note(ev) { try { await fetch('/__rec/note', {method: 'POST', body: JSON.stringify(ev)}); } catch (e) {} }
function done() {
  $('game').src = 'about:blank'; $('what').textContent = 'All passes done: you can close this window.';
  $('left').textContent = ''; $('next').hidden = true; note({done: true}); stats();
}
function run() {
  clearInterval(timer);
  if (i >= P.length) return done();
  const p = P[i];
  $('what').textContent = 'Pass ' + (i + 1) + ' of ' + P.length + ': ' + p.label;
  note({pass: i, label: p.label});
  $('game').src = '/' + (p.q ? '?' + p.q : '');
  if (p.hand) {
    $('left').textContent = 'Click every race, both genders and every class on the pick screen, then start one run.';
    $('next').hidden = false; $('next').onclick = () => { i++; run(); };
    return;
  }
  $('next').hidden = true;
  let t = p.seconds || SECONDS;
  $('left').textContent = t + ' s';
  timer = setInterval(() => { t--; $('left').textContent = t + ' s'; if (t <= 0) { i++; run(); } }, 1000);
}
run();
</script></body></html>"""


def make_handler(upstream: str, log: Log, passes: list[dict], seconds: int, done: threading.Event, out: str):
    up = urllib.parse.urlsplit(upstream)
    if up.scheme != "http" or not up.hostname:
        raise SystemExit("--upstream must be http://host:port")
    host, port = up.hostname, up.port or 80

    class H(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def log_message(self, *a):
            pass

        def _send(self, status: int, body: bytes, ctype: str, extra: dict | None = None) -> None:
            self.send_response(status)
            self.send_header("Content-Type", ctype)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            # the game in the frame is cross-origin isolated (COOP/COEP from wenilla-host): so is the page around it
            self.send_header("Cross-Origin-Opener-Policy", "same-origin")
            self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
            for k, v in (extra or {}).items():
                self.send_header(k, v)
            self.end_headers()
            if self.command != "HEAD":
                self.wfile.write(body)

        def _rec(self) -> bool:
            p = self.path.split("?", 1)[0]
            if p in ("/__rec", "/__rec/"):
                page = REC_PAGE.replace("__PLAN__", json.dumps(passes)).replace("__SECONDS__", str(int(seconds)))
                self._send(200, page.encode(), "text/html; charset=utf-8")
                return True
            if p == "/__rec/stats":
                self._send(200, json.dumps(log.summary()).encode(), "application/json")
                return True
            if p == "/__rec/note" and self.command == "POST":
                n = min(int(self.headers.get("Content-Length") or 0), 4096)
                try:
                    ev = json.loads(self.rfile.read(n) or b"{}")
                except ValueError:
                    ev = {}
                if not isinstance(ev, dict):
                    ev = {}
                ev = {k: ev[k] for k in ("pass", "label", "done") if k in ev}
                ev["at"] = time.strftime("%Y-%m-%dT%H:%M:%S")
                with open(os.path.join(out, "passes.json"), "a", encoding="utf-8") as f:
                    f.write(json.dumps(ev) + "\n")
                if ev.get("label"):
                    print(f"  pass {int(ev.get('pass', 0)) + 1}: {str(ev['label'])[:80]}", flush=True)
                self._send(200, b"{}", "application/json")
                if ev.get("done"):
                    done.set()
                return True
            return False

        def _proxy(self) -> None:
            n = int(self.headers.get("Content-Length") or 0)
            body = self.rfile.read(n) if n else None
            hd = {k: v for k, v in self.headers.items() if k.lower() not in HOP and k.lower() not in ("host", "accept-encoding")}
            name = data_name(self.path)
            if name is None:
                # the page, the wasm and the scripts: precompressed answers are fine for the browser
                if self.headers.get("Accept-Encoding"):
                    hd["Accept-Encoding"] = self.headers["Accept-Encoding"]
            try:
                c = http.client.HTTPConnection(host, port, timeout=120)
                c.request(self.command, self.path, body=body, headers=hd)
                r = c.getresponse()
                data = r.read()
            except OSError as e:
                self._send(502, f"the game host on {host}:{port} is not answering: {e}\n".encode(), "text/plain")
                return
            if name is not None:
                log.add(r.status, len(data), name)
            self.send_response(r.status)
            for k, v in r.getheaders():
                kl = k.lower()
                if kl in HOP or kl == "content-length" or (name is not None and kl == "cache-control"):
                    continue
                self.send_header(k, v)
            if name is not None:
                self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            if self.command != "HEAD":
                self.wfile.write(data)
            c.close()

        def do_GET(self):  # noqa: N802
            if not self._rec():
                self._proxy()

        def do_HEAD(self):  # noqa: N802
            self._proxy()

        def do_POST(self):  # noqa: N802
            if not self._rec():
                self._proxy()

    return H


def wait_for(upstream: str, seconds: int) -> bool:
    """True once the game host answers /data/__index (it opens the client's archives first)."""
    u = urllib.parse.urlsplit(upstream)
    end = time.time() + seconds
    while time.time() < end:
        try:
            c = http.client.HTTPConnection(u.hostname, u.port or 80, timeout=10)
            c.request("HEAD", "/index.html")
            if c.getresponse().status == 200:
                return True
        except OSError:
            pass
        time.sleep(1)
    return False


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--upstream", default="http://127.0.0.1:8090")
    ap.add_argument("--listen", default="127.0.0.1:8091")
    ap.add_argument("--out", default="")
    ap.add_argument("--seconds", type=int, default=80, help="how long each autopilot pass runs")
    ap.add_argument("--plan", choices=("full", "quick"), default="full")
    ap.add_argument("--wait-only", type=int, metavar="SECONDS",
                    help="only wait (up to SECONDS) until the game host answers, then exit 0 (1 if it never does)")
    a = ap.parse_args(argv)
    if a.wait_only is not None:
        return 0 if wait_for(a.upstream, a.wait_only) else 1
    if not a.out:
        ap.error("--out is needed")
    lhost, _, lport = a.listen.rpartition(":")
    if lhost not in ("127.0.0.1", "localhost", "::1"):
        print("the recorder listens on this PC only (127.0.0.1)", file=sys.stderr)
        return 2
    log = Log(a.out)
    done = threading.Event()
    passes = plan(a.plan)
    srv = ThreadingHTTPServer((lhost, int(lport)), make_handler(a.upstream, log, passes, a.seconds, done, a.out))
    srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    total = sum(p.get("seconds", a.seconds) for p in passes if not p.get("hand"))
    print(f"Recorder on http://{a.listen}/__rec/  ({len(passes)} passes, about {total // 60 + 2} minutes)", flush=True)
    print(f"Names go to {os.path.abspath(a.out)}", flush=True)
    try:
        while not done.wait(5):
            pass
    except KeyboardInterrupt:
        print("stopped: keeping what was recorded", flush=True)
    srv.shutdown()
    s = log.summary()
    print(f"RECORD OK: {s['names']} names recorded, {s['missing']} not in this client", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
