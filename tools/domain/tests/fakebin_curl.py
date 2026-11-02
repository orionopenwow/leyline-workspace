#!/usr/bin/env python3
"""A stand-in for curl in test_domain.py: answers from the test's Caddyfile (through domain_caddy's model of the
redirects), the portal.env and the website's settings, never the network."""
import json
import os
import sys
import urllib.parse

sys.path.insert(0, os.environ["FAKE_TOOLS"])
import domain_caddy as dc  # noqa: E402

args = sys.argv[1:]
hdrs, fmt, url, fail = {}, "", "", False
i = 0
while i < len(args):
    x = args[i]
    if x == "-H":
        k, _, v = args[i + 1].partition(":")
        hdrs[k.strip()] = v.strip()
        i += 2
        continue
    if x in ("-w", "-o", "-m"):
        if x == "-w":
            fmt = args[i + 1]
        i += 2
        continue
    if x.startswith("-"):
        fail = fail or "f" in x.lstrip("-") and not x.startswith("--")
        i += 1
        continue
    url = x
    i += 1
with open(os.environ["FAKE_LOG"], "a") as f:
    f.write("curl " + url + " " + json.dumps(hdrs) + "\n")
u = urllib.parse.urlsplit(url)
host, path = u.hostname or "", u.path or "/"
uri = path + ("?" + u.query if u.query else "")
a = dc.load(os.path.join(os.environ["FAKE_TOOLS"], "addresses.json"))
cf = open(os.environ["FAKE_CF"]).read()
st = dc.status(cf, a)
down = os.environ.get("FAKE_DOWN", "").split()


def env(path, key):
    try:
        for ln in open(path):
            if ln.startswith(key + "="):
                return ln.split("=", 1)[1].strip()
    except OSError:
        pass
    return ""


def answer():
    if host in down:
        return "000", ""
    if host.startswith("127.") or host.startswith("172."):
        return "200", ""
    news = {a[p]["new"]: p for p in dc.PARTS}
    if host in news and st[news[host]] not in ("alias", "redirect"):
        return "000", ""   # no site block: no certificate
    if host == a["www"]["new"] and st["www"] != "on":
        return "000", ""
    if u.scheme == "http":
        return "308", "https://" + host + uri
    phase = "redirect" if "redirect" in st.values() else "alias"
    loc = dc.would_redirect(a, phase, host, "GET", uri, hdrs)
    if loc:
        return "301", loc
    if path in ("/wrath/status.json", "/warm.html", "/healthz", "/login", "/data/__index", "/wrath/leyline"):
        return "200", ""
    if path == "/auth/google/start":
        mode = os.environ.get("FAKE_GOOGLE", "env")
        back = host if mode == "host" else (urllib.parse.urlsplit(env(os.environ["FAKE_REALM_ENV"], "PUBLIC_URL")).hostname or host)
        if mode == "old":
            back = a["site"]["old"]
        q = urllib.parse.urlencode({"redirect_uri": f"https://{back}/auth/google/callback", "client_id": "x"})
        return "302", "https://accounts.google.com/o/oauth2/v2/auth?" + q
    if path == "/wrath/play":
        return "302", "https://" + (env(os.environ["FAKE_PORTAL_ENV"], "PUBLIC_HOST") or a["site"]["old"]) + "/wrath?go="
    if path == "/":
        return "200", ""
    return "404", ""


code, loc = answer()
if fmt:
    sys.stdout.write(fmt.replace("%{http_code}", code).replace("%{redirect_url}", loc))
if code == "000":
    sys.exit(7)
if fail and int(code) >= 400:
    sys.exit(22)
