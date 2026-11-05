#!/usr/bin/env python3
"""Spike test (owner 4 Nov 2026: "we may see a spike soon"). Runs ON the server, standard library only.

Hits the live services from inside the box for about a minute, at a moderate concurrency, and prints what each one
can take, so the weak spot is known before a crowd finds it:
  A. the website program's game-file check (/__auth, what Caddy asks before every game file)
  B. a website page (/wrath, rendered by the same program)
  C. game files straight from wu-play (the browser slice), MB/s
  D. Caddy itself over TLS on this box (a small public file), requests/s
Nothing is written, no account is used, no game is started. Read-only for data; it only adds load for its minute.

    python3 spike_test.py --portal http://<wu-portal ip>:8091 --play http://<wu-play ip>:8090 --site <public name>
          [--workers 40] [--seconds 15]
"""
import argparse
import http.client
import json
import random
import socket
import ssl
import statistics
import threading
import time
import urllib.parse


def run(name, workers, seconds, one):
    """`one()` -> (ok: bool, bytes: int). Many threads for `seconds`; the summary line."""
    stop = time.time() + seconds
    lat, errs, total_bytes = [], [0], [0]
    codes = {}
    lock = threading.Lock()

    def worker():
        while time.time() < stop:
            t = time.time()
            try:
                ok, n, code = one()
            except Exception as e:  # noqa: BLE001
                ok, n, code = False, 0, type(e).__name__
                time.sleep(0.2)   # never spin on a refusal
            d = time.time() - t
            with lock:
                lat.append(d)
                total_bytes[0] += n
                codes[code] = codes.get(code, 0) + 1
                if not ok:
                    errs[0] += 1

    th = [threading.Thread(target=worker, daemon=True) for _ in range(workers)]
    t0 = time.time()
    for t in th:
        t.start()
    for t in th:
        t.join(seconds + 30)
    el = time.time() - t0
    n = len(lat)
    if not n:
        print(f"{name}: no answers at all")
        return {}
    lat.sort()
    p = lambda q: lat[min(n - 1, int(q * n))] * 1000  # noqa: E731
    res = {"name": name, "workers": workers, "req_s": round(n / el, 1), "p50_ms": round(p(0.5)),
           "p95_ms": round(p(0.95)), "max_ms": round(lat[-1] * 1000), "errors": errs[0], "codes": codes,
           "mb_s": round(total_bytes[0] / el / 1048576, 1)}
    print(f"{name}: {res['req_s']} req/s · p50 {res['p50_ms']} ms · p95 {res['p95_ms']} ms · max {res['max_ms']} ms · "
          f"{res['mb_s']} MB/s · errors {errs[0]} · answers {json.dumps(codes, sort_keys=True)}")
    return res


def http_get(base, path, headers=None, timeout=20):
    u = urllib.parse.urlsplit(base)
    c = http.client.HTTPConnection(u.hostname, u.port or 80, timeout=timeout)
    try:
        c.request("GET", path, headers=headers or {})
        r = c.getresponse()
        n = 0
        while True:
            b = r.read(65536)
            if not b:
                break
            n += len(b)
        return r.status, n
    finally:
        c.close()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--portal", required=True)
    ap.add_argument("--play", required=True)
    ap.add_argument("--site", required=True)
    ap.add_argument("--workers", type=int, default=40)
    ap.add_argument("--seconds", type=int, default=15)
    a = ap.parse_args()
    W, S = max(1, min(a.workers, 200)), max(5, min(a.seconds, 60))
    print(f"spike test: {W} at once, {S} s per part")
    out = []

    # A. the game-file check: no cookie, so the answer is a quick "no" (401); what matters is how many per second
    def a_one():
        st, n = http_get(a.portal, "/__auth", {"Host": a.site, "X-Forwarded-Uri": "/data/world%2Fx.blp",
                                                "X-Forwarded-Method": "GET"})
        return st in (200, 401, 403), n, st
    out.append(run("A. game-file check (/__auth)", W, S, a_one))

    # B. a website page
    def b_one():
        st, n = http_get(a.portal, "/wrath/realms", {"Host": a.site})
        return st == 200, n, st
    out.append(run("B. website page (/wrath/realms)", max(1, W // 2), S, b_one))

    # C. game files straight from wu-play: real names from its list
    names = []
    try:
        u = urllib.parse.urlsplit(a.play)
        c = http.client.HTTPConnection(u.hostname, u.port or 80, timeout=30)
        c.request("GET", "/data/__index")
        idx = json.loads(c.getresponse().read())
        c.close()
        names = [x for x in (idx if isinstance(idx, list) else list(idx)) if isinstance(x, str)
                 and x.lower().endswith((".blp", ".m2", ".skin", ".adt", ".wmo"))]
        random.shuffle(names)
        names = names[:400]
    except Exception as e:  # noqa: BLE001
        print(f"C. game files: could not read the file list ({type(e).__name__})")
    if names:
        def c_one():
            st, n = http_get(a.play, "/data/" + urllib.parse.quote(random.choice(names), safe=""))
            return st == 200, n, st
        out.append(run("C. game files (wu-play)", W, S, c_one))

    # D. Caddy over TLS on this box: a small public file, a new connection each time (the costly case)
    ctx = ssl.create_default_context()
    ctx.check_hostname = False
    ctx.verify_mode = ssl.CERT_NONE

    def d_one():
        s = socket.create_connection(("127.0.0.1", 443), timeout=20)
        ss = ctx.wrap_socket(s, server_hostname=a.site)
        c = http.client.HTTPSConnection(a.site, 443, timeout=20, context=ctx)
        c.sock = ss
        try:
            c.request("GET", "/wu-diag.js", headers={"Host": a.site})
            r = c.getresponse()
            n = len(r.read())
            return r.status in (200, 302, 304), n, r.status
        finally:
            c.close()
    out.append(run("D. Caddy over TLS (new connection each)", W, S, d_one))

    print("SPIKE RESULT " + json.dumps([o for o in out if o], separators=(",", ":")))
    # review 4 Nov: say plainly which part struggled (the step still ends OK: it is a measurement, not a gate)
    for w in verdicts(out):
        print("SPIKE WARN " + w)
    print("SPIKE TEST OK")


def verdicts(out, max_error_share=0.05, slow_p95_ms=2000):
    """Plain warnings for the parts that failed answers or were slow: [] when all is well."""
    warn = []
    for o in out:
        if not o:
            continue
        total = sum(o["codes"].values()) or 1
        if o["errors"] / total > max_error_share:
            warn.append(f"{o['name']}: {round(100 * o['errors'] / total)}% failed answers")
        if o["p95_ms"] > slow_p95_ms:
            warn.append(f"{o['name']}: slow (p95 {o['p95_ms']} ms)")
        if any(str(c) in ("429", "503") for c in o["codes"]):
            warn.append(f"{o['name']}: busy answers (429/503) under this load")
    return warn


if __name__ == "__main__":
    main()
