#!/usr/bin/env python3
"""The domain move (tools/domain, docs/DOMAIN-MOVE.md): the Caddyfile edits, the redirect rules, sign-in flows through
them (no loops), and domain.sh check/on/off against stand-ins for docker, curl and DNS (nothing reaches a server).

    python3 -I tools/domain/tests/test_domain.py
"""
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
TOOLS = os.path.dirname(HERE)
ROOT = os.path.dirname(os.path.dirname(TOOLS))
sys.path.insert(0, TOOLS)
import domain_caddy as dc  # noqa: E402

A = dc.load()
IP = "203.0.113.7"
NEW = {p: A[p]["new"] for p in dc.PARTS}
OLD = {p: A[p]["old"] for p in dc.PARTS}
STREAM_BEGIN = "# >>> WrathUnleashed streaming (stream-up.sh)"
RELAY_BEGIN = "# >>> WrathUnleashed Beacons relay (relay-up.sh)"

CADDYFILE = f"""{{
\temail ops@example.com
\tservers {{
\t\ttimeouts {{
\t\t\tidle 120s
\t\t}}
\t}}
}}

{OLD['site']} {{
\tencode zstd gzip
\t# WrathUnleashed (deploy.sh): the Wrath sign-in page; the old play page sends players there.
\tredir / /wrath/leyline 302
\tredir /play /wrath 302
\thandle /wrath* {{
\t\treverse_proxy wu-portal:8091
\t}}
\thandle {{
\t\treverse_proxy ovlive-realm:8090
\t}}
}}

{STREAM_BEGIN}: the browser client streams here
{OLD['game']} {{
\tencode zstd gzip
\t@gate path /__gate /__env.js
\thandle @gate {{
\t\treverse_proxy wu-portal:8091
\t}}
\thandle {{
\t\treverse_proxy wu-play:8090 {{
\t\t\tflush_interval -1
\t\t}}
\t}}
}}
# <<< WrathUnleashed streaming

{RELAY_BEGIN}: home-hosted realms
{OLD['relay']} {{
\t@metrics path /metrics
\trespond @metrics 403
\treverse_proxy 172.18.0.1:8099 {{
\t\tflush_interval -1
\t}}
}}
# <<< WrathUnleashed Beacons relay
"""
PAGE = {"Sec-Fetch-Dest": "document", "Sec-Fetch-Mode": "navigate", "Accept": "text/html"}


class Caddy(unittest.TestCase):
    def test_alias_then_redirect_then_remove_round_trips(self):
        s1, notes = dc.apply(CADDYFILE, "alias", A)
        self.assertIn(f"\n{OLD['site']}, {NEW['site']} {{\n", s1)
        self.assertIn(f"\n{OLD['game']}, {NEW['game']} {{\n", s1)
        self.assertIn(f"\n{OLD['relay']}, {NEW['relay']} {{\n", s1)
        self.assertIn(f"\n{A['www']['new']} {{\n\tredir https://{NEW['site']}{{uri}} 301\n}}", s1)
        self.assertEqual(dc.status(s1, A), {"site": "alias", "game": "alias", "relay": "alias", "www": "on"})
        self.assertEqual(dc.apply(s1, "alias", A)[0], s1)                      # idempotent
        s2, _ = dc.apply(s1, "redirect", A)
        self.assertEqual(dc.status(s2, A)["site"], "redirect")
        self.assertEqual(dc.apply(s2, "redirect", A)[0], s2)
        self.assertEqual(s2.count(dc.RD_BEGIN), 3)
        self.assertEqual(dc.apply(s2, "alias", A)[0], s1)                      # back to the alias phase
        self.assertEqual(dc.remove(s2, A), CADDYFILE)                          # off: exactly as it was
        dc.blocks(s2)
        # the snippet sits first in each old block, so it runs before the deploy.sh redir and every handle
        site = s2[s2.index(f"{OLD['site']}, "):]
        self.assertLess(site.index("@ly_moved"), site.index("redir / /wrath/leyline 302"))
        self.assertIn(f"redir @ly_moved https://{NEW['site']}{{uri}} 301", site)
        self.assertIn("not path /auth/* /__*", site)
        game = s2[s2.index(f"{OLD['game']}, "):]
        self.assertIn("not path /__gate /__env.js /__auth /data /data/* /ws/*", game)
        relay = s2[s2.index(f"{OLD['relay']}, "):]
        self.assertIn("not path /agent /r/* /api/* /ws/* /healthz /metrics", relay)

    def test_relay_elsewhere_and_foreign_blocks(self):
        no_relay = CADDYFILE[:CADDYFILE.index(RELAY_BEGIN)]
        s, notes = dc.apply(no_relay, "redirect", A)
        self.assertEqual(dc.status(s, A)["relay"], "no-block")
        self.assertTrue(any("relay" in n and "skipped" in n for n in notes))
        no_stream = CADDYFILE.replace(CADDYFILE[CADDYFILE.index(STREAM_BEGIN):CADDYFILE.index(RELAY_BEGIN)], "")
        with self.assertRaises(LookupError):            # "on" needs the game's block
            dc.apply(no_stream, "alias", A)
        s, notes = dc.apply(no_stream, "redirect", A, strict=False)   # reapply after stream-down.sh: skipped
        self.assertEqual(dc.status(s, A)["game"], "no-block")
        with self.assertRaises(LookupError):
            dc.apply(CADDYFILE.replace(f"{OLD['game']} {{", "elsewhere.example {"), "alias", A)
        with self.assertRaises(LookupError):   # somebody else's block for a new name: never merged
            dc.apply(CADDYFILE + f"\n{NEW['site']} {{\n\trespond hi\n}}\n", "alias", A)
        with self.assertRaises(ValueError):
            dc.blocks("a {\n")

    def test_the_other_scripts_still_find_their_blocks(self):
        s, _ = dc.apply(CADDYFILE, "redirect", A)
        sys.path.insert(0, os.path.join(ROOT, "stream", "server"))
        sys.path.insert(0, os.path.join(ROOT, "tools", "security"))
        import importlib.util
        spec = importlib.util.spec_from_file_location("gdc", os.path.join(ROOT, "stream", "server", "gamedata-caddy.py"))
        gdc = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(gdc)
        g = gdc.add(s, OLD["game"])                      # the killswitch block goes into the aliased stream block
        self.assertIn(gdc.BEGIN, g)
        self.assertEqual(gdc.strip(g), s)
        import caddyfile_harden as h
        out, did = h.harden(s, sites=(OLD["site"],))
        self.assertIn("request_body /wrath*", out[out.index(f"{OLD['site']}, "):])
        # stream-up.sh strips and re-appends its block (old name only): reapply puts the new name back
        start, end = s.index(STREAM_BEGIN), s.index("# <<< WrathUnleashed streaming")
        rewritten = s[:start] + CADDYFILE[CADDYFILE.index(STREAM_BEGIN):CADDYFILE.index("# <<< WrathUnleashed streaming")] + s[end:]
        self.assertEqual(dc.status(rewritten, A)["game"], "old-only")
        self.assertEqual(dc.status(dc.apply(rewritten, "redirect", A)[0], A)["game"], "redirect")

    def test_which_requests_move(self):
        r = lambda host, uri, h=PAGE, m="GET": dc.would_redirect(A, "redirect", host, m, uri, h)
        self.assertEqual(r(OLD["site"], "/wrath/leyline?x=1&y=2"), f"https://{NEW['site']}/wrath/leyline?x=1&y=2")
        self.assertEqual(r(OLD["site"], "/"), f"https://{NEW['site']}/")
        self.assertEqual(r(OLD["site"], "/wrath", {"Accept": "text/html,application/xhtml+xml"}), f"https://{NEW['site']}/wrath")
        self.assertIsNone(r(OLD["site"], "/wrath/mods/all.json", {"Accept": "*/*"}))          # launchers (ureq)
        self.assertIsNone(r(OLD["site"], "/wrath/leyline/herokey.json", {"Accept": "application/json"}))   # host packs
        self.assertIsNone(r(OLD["site"], "/wrath/art/x.webp", {"Sec-Fetch-Dest": "image", "Accept": "text/html"}))
        self.assertIsNone(r(OLD["site"], "/wrath/api/social/me", {"Sec-Fetch-Dest": "empty"}))
        self.assertIsNone(r(OLD["site"], "/auth/google/callback?code=x&state=y"))             # OAuth finishes where it began
        self.assertIsNone(r(OLD["site"], "/key", PAGE, "POST"))                               # forms
        self.assertIsNone(r(OLD["game"], "/__gate?code=abc"))
        self.assertIsNone(r(OLD["game"], "/data/World/x.adt", {"Sec-Fetch-Dest": "empty"}))
        self.assertIsNone(r(OLD["game"], "/warm.html", {"Sec-Fetch-Dest": "iframe", "Accept": "text/html"}))
        self.assertEqual(r(OLD["game"], "/?ownfiles=1"), f"https://{NEW['game']}/?ownfiles=1")
        self.assertIsNone(r(OLD["relay"], "/agent", {"Upgrade": "websocket"}))
        self.assertIsNone(r(OLD["relay"], "/api/listing/realms.json", PAGE))
        self.assertIsNone(r(OLD["relay"], "/join/abc?hop=1", {"Sec-Fetch-Dest": "iframe"}))
        self.assertEqual(r(OLD["relay"], "/join/abc"), f"https://{NEW['relay']}/join/abc")
        for n in NEW.values():
            self.assertIsNone(r(n, "/anything"))
        self.assertEqual(r(A["www"]["new"], "/a?b=1", {}), f"https://{NEW['site']}/a?b=1")
        self.assertIsNone(dc.would_redirect(A, "alias", OLD["site"], "GET", "/", PAGE))

    def test_cli(self):
        with tempfile.TemporaryDirectory() as t:
            cf = os.path.join(t, "Caddyfile")
            open(cf, "w").write(CADDYFILE)
            ino = os.stat(cf).st_ino
            run = lambda *a: subprocess.run([sys.executable, "-I", os.path.join(TOOLS, "domain_caddy.py"), *a],
                                            capture_output=True, text=True)
            self.assertEqual(run("apply", cf, "redirect").returncode, 0)
            self.assertEqual(os.stat(cf).st_ino, ino)                 # edited in place (bind mount)
            self.assertIn("site: redirect", run("status", cf).stdout)
            self.assertIn("off", run("reapply", cf, os.path.join(t, "none.json")).stdout)
            json.dump({"phase": "alias"}, open(os.path.join(t, "d.json"), "w"))
            self.assertEqual(run("reapply", cf, os.path.join(t, "d.json")).returncode, 0)
            self.assertIn("site: alias", run("status", cf).stdout)
            self.assertEqual(run("remove", cf).returncode, 0)
            self.assertEqual(open(cf).read(), CADDYFILE)
            self.assertEqual(run("apply", cf, "sideways").returncode, 2)
            self.assertEqual(run("nope").returncode, 1)


# --- a browser following redirects through Caddy and the portal: the sign-in flow on the new names has no loop
def portal(host: str, uri: str, signed_in: set, phase: str):
    """The portal's answers that matter for sign-in (stream/wrath_portal.py, with LEYLINE_DOMAIN_MOVE=1)."""
    path = uri.split("?")[0]
    site, game = NEW["site"], NEW["game"]
    if host in (OLD["site"], site):
        if path == "/wrath/play":
            if host in signed_in:
                return 302, f"https://{game}/__gate?code=c1", None
            return 302, f"https://{site}/wrath/signin?next=/wrath/play", None
        if path == "/wrath/signin":
            return 302, "/auth/google/start", None
        if path == "/auth/google/start":
            return 302, f"https://accounts.google.com/auth?redirect_uri=https://{site}/auth/google/callback", None
        if path == "/auth/google/callback":
            return 302, "/wrath/play", host      # sets the session cookie on this host, lands on wr_next
        return 200, None, None
    if host in (OLD["game"], game):
        if path == "/__gate":
            return 302, "/", "game:" + host
        if ("game:" + host) in signed_in:
            return 200, None, None
        return 302, f"https://{site}/wrath", None   # /__auth: not signed in on this name
    if host == "accounts.google.com":
        return 302, re.search(r"redirect_uri=(\S+)", uri).group(1) + "?code=g", None
    return 404, None, None


def browse(start: str, phase: str = "redirect", max_hops: int = 20):
    import urllib.parse
    cookies, seen, url = set(), [], start
    for _ in range(max_hops):
        u = urllib.parse.urlsplit(url)
        host, uri = u.hostname, (u.path or "/") + ("?" + u.query if u.query else "")
        seen.append(url)
        loc = dc.would_redirect(A, phase, host, "GET", uri, PAGE)
        if loc is None:
            code, loc, cookie = portal(host, uri, cookies, phase)
            if cookie:
                cookies.add(cookie)
            if code == 200:
                return seen, cookies
        url = loc if loc.startswith("https://") else f"https://{host}{loc}"
        if seen.count(url) > 1:
            raise AssertionError("loop: " + " -> ".join(seen + [url]))
    raise AssertionError("too many hops: " + " -> ".join(seen))


class SignInFlow(unittest.TestCase):
    def test_old_play_link_signs_in_once_on_the_new_name(self):
        seen, cookies = browse(f"https://{OLD['site']}/wrath/play?mini=mini")
        self.assertEqual(seen[-1], f"https://{NEW['game']}/")
        self.assertIn(NEW["site"], cookies)
        self.assertIn("game:" + NEW["game"], cookies)
        self.assertFalse(any(OLD["site"] in c or OLD["game"] in c for c in cookies))
        self.assertEqual(sum("accounts.google.com" in s for s in seen), 1)   # signed in once

    def test_old_game_page_and_bookmarks(self):
        for start in (f"https://{OLD['game']}/", f"https://{OLD['site']}/", f"https://{A['www']['new']}/wrath/play",
                      f"https://{NEW['site']}/wrath/play"):
            seen, _ = browse(start)
            self.assertFalse(any(OLD["site"] in s or OLD["game"] in s for s in seen[1:]), seen)

    def test_an_oauth_started_on_the_old_name_finishes_there(self):
        # a sign-in that was in flight when the switch happened: its callback is not moved, so it completes
        self.assertIsNone(dc.would_redirect(A, "redirect", OLD["site"], "GET", "/auth/google/callback?code=1&state=2", PAGE))


# --- domain.sh with stand-ins
def sh_quote(s):
    return "'" + s.replace("'", "'\\''") + "'"


class Script(unittest.TestCase):
    def setUp(self):
        self.t = tempfile.mkdtemp(prefix="domain-test-")
        j = lambda *p: os.path.join(self.t, *p)
        self.bin, self.wu = j("bin"), j("srv-wrath")
        for d in (self.bin, self.wu, j("caddy"), j("relay"), j("srv-wrath", "stream"), j("srv-wrath", "custom", "lua_scripts")):
            os.makedirs(d, exist_ok=True)
        self.cf, self.log = j("caddy", "Caddyfile"), j("log.txt")
        self.realm_env, self.relay_env, self.portal_env = j("caddy", ".env"), j("relay", "relay.env"), j("srv-wrath", "portal.env")
        open(self.cf, "w").write(CADDYFILE)
        self.realm_orig = ("SECRET_KEY=s3cr3t\nPUBLIC_URL=https://play.openvanilla.app\n"
                           "ALLOWED_ORIGINS=https://play.openvanilla.app,https://openvanilla.app\nQUEUE_ENABLED=0\n")
        open(self.realm_env, "w").write(self.realm_orig)
        self.relay_orig = ("RELAY_NAME=relay.openvanilla.app\nRELAY_HTTP_BIND=172.18.0.1:8099\n"
                           "RELAY_SITE_URL=https://openvanilla.app\nRELAY_SITE_ORIGINS=https://openvanilla.app,https://www.openvanilla.app\n"
                           "RELAY_PLAY_ORIGINS=https://wrath.openvanilla.app,https://play.openvanilla.app\n")
        open(self.relay_env, "w").write(self.relay_orig)
        self.portal_orig = "SOAP_USER=WUSOAP\nPUBLIC_HOST=play.openvanilla.app\nLEYLINE_BRAND=1\n"
        open(self.portal_env, "w").write(self.portal_orig)
        open(j("srv-wrath", "stream", "stream-lib.sh"), "w").write(f'recreate_portal() {{ echo recreate_portal >> {sh_quote(self.log)}; }}\n')
        self.dns = {n: IP for n in list(OLD.values())} | {n: IP for n in list(NEW.values()) + [A["www"]["new"]]}
        self.fake("curl", f'exec {sh_quote(sys.executable)} -I {sh_quote(os.path.join(HERE, "fakebin_curl.py"))} "$@"')
        self.fake("getent", f'''{sh_quote(sys.executable)} -I -c '
import json, sys
d = json.load(open(sys.argv[1]))
ip = d.get(sys.argv[3])
print(f"{{ip}} STREAM {{sys.argv[3]}}") if ip else sys.exit(2)' {sh_quote(j("dns.json"))} "$@"''')
        self.fake("hostname", f'echo "{IP} 10.0.0.2"')
        self.fake("ip", "true")
        self.fake("ss", "true")
        self.fake("ufw", "true")
        self.fake("docker", f'''echo "docker $*" >> {sh_quote(self.log)}
case "$*" in
  *"caddy reload"*) if [ -n "${{FAKE_RELOAD_FAIL_ON:-}}" ] && grep -q "$FAKE_RELOAD_FAIL_ON" {sh_quote(self.cf)}; then exit 1; fi; exit 0 ;;
  inspect*) cat {sh_quote(self.realm_env)} ;;
esac
exit 0''')

    def tearDown(self):
        shutil.rmtree(self.t, ignore_errors=True)

    def fake(self, name, body):
        p = os.path.join(self.bin, name)
        open(p, "w").write("#!/usr/bin/env bash\n" + body + "\n")
        os.chmod(p, os.stat(p).st_mode | stat.S_IEXEC)

    def run_sh(self, mode, **env):
        with open(os.path.join(self.t, "dns.json"), "w") as f:
            json.dump(self.dns, f)
        e = dict(os.environ, PATH=self.bin + os.pathsep + os.environ["PATH"], WU_DIR=self.wu,
                 WU_CADDYFILE=self.cf, WU_CADDY_DIR=os.path.join(self.t, "caddy"), WU_RELAY_DIR=os.path.join(self.t, "relay"),
                 WU_DOMAIN_BACKUPS=os.path.join(self.t, "backups"), WU_DOMAIN_TLS_WAIT="0", WU_DOMAIN_PAUSE="0",
                 FAKE_TOOLS=TOOLS, FAKE_LOG=self.log, FAKE_CF=self.cf, FAKE_REALM_ENV=self.realm_env, FAKE_PORTAL_ENV=self.portal_env)
        e.update(env)
        # a copy of the folder as the bat uploads it (domain.sh, domain_caddy.py, addresses.json)
        up = os.path.join(self.t, "upload")
        os.makedirs(up, exist_ok=True)
        for f in ("domain.sh", "domain_caddy.py", "addresses.json"):
            shutil.copy(os.path.join(TOOLS, f), up)
        return subprocess.run(["bash", os.path.join(up, "domain.sh"), mode], capture_output=True, text=True, env=e, timeout=120)

    def read(self, p):
        with open(p) as f:
            return f.read()

    def test_check_changes_nothing_and_prints_the_records(self):
        self.dns.pop(NEW["relay"])
        r = self.run_sh("check")
        self.assertEqual(r.returncode, 0, r.stderr)
        out = r.stdout
        self.assertTrue(out.rstrip().endswith("DOMAIN CHECK OK"), out)
        self.assertIn(f"public IPv4: {IP}", out)
        for host in ("@", "play", "relay", "www"):
            self.assertRegex(out, rf"Type A +Host {re.escape(host)} +Data {re.escape(IP)}")
        self.assertIn("no AAAA", out)
        self.assertIn(f"{NEW['relay']} -> nothing yet", out)
        self.assertIn("WAITING FOR DNS", out)
        self.assertIn(f"https://{NEW['site']}/auth/google/callback", out)
        self.assertIn("PUBLIC_URL ALLOWED_ORIGINS", out)
        self.assertNotIn("s3cr3t", out + r.stderr)
        self.assertEqual(self.read(self.cf), CADDYFILE)
        self.assertEqual(self.read(self.realm_env), self.realm_orig)
        self.assertFalse(os.path.exists(os.path.join(self.t, "backups")))
        self.dns[NEW["relay"]] = IP
        self.assertIn("READY: all four names point here", self.run_sh("check").stdout)

    def test_on_refuses_before_dns(self):
        self.dns[A["www"]["new"]] = "198.51.100.1"
        r = self.run_sh("on")
        self.assertEqual(r.returncode, 1)
        self.assertIn("Nothing was changed", r.stderr)
        self.assertEqual(self.read(self.cf), CADDYFILE)

    def test_on_then_off(self):
        r = self.run_sh("on")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertTrue(r.stdout.rstrip().endswith("DOMAIN OK"), r.stdout)
        cf = self.read(self.cf)
        self.assertEqual(dc.status(cf, A), {"site": "redirect", "game": "redirect", "relay": "redirect", "www": "on"})
        p = self.read(self.portal_env)
        for line in (f"PUBLIC_HOST={NEW['site']}", f"STREAM_HOST={NEW['game']}", "LEYLINE_DOMAIN_MOVE=1",
                     f"WU_JOIN_BASE=https://{NEW['relay']}", f"WU_RELAY_WS=wss://{NEW['relay']}", "LEYLINE_BRAND=1"):
            self.assertIn(line + "\n", p)
        realm = self.read(self.realm_env)
        self.assertIn(f"PUBLIC_URL=https://{NEW['site']}\n", realm)
        self.assertIn(f"ALLOWED_ORIGINS=https://{OLD['site']},https://openvanilla.app,https://{NEW['site']}\n", realm)
        self.assertIn("SECRET_KEY=s3cr3t\n", realm)
        relay = self.read(self.relay_env)
        self.assertIn(f"RELAY_NAME={NEW['relay']}\n", relay)
        self.assertIn(f"RELAY_SITE_URL=https://{NEW['site']}\n", relay)
        for o in (OLD["game"], OLD["site"], NEW["game"], NEW["site"]):
            self.assertIn("https://" + o, re.search(r"RELAY_PLAY_ORIGINS=(.*)", relay).group(1))
        self.assertIn(f"https://{NEW['site']}", re.search(r"RELAY_SITE_ORIGINS=(.*)", relay).group(1))
        self.assertTrue(os.path.isfile(os.path.join(self.wu, "custom", "lua_scripts", "leyline-domain.on")))
        st = json.load(open(os.path.join(self.wu, "domain", "domain.json")))
        self.assertEqual(st["phase"], "redirect")
        for f in ("domain_caddy.py", "addresses.json"):   # stream-up.sh and relay-up.sh reapply from here
            self.assertTrue(os.path.isfile(os.path.join(self.wu, "domain", f)))
        log = self.read(self.log)
        self.assertIn("recreate_portal", log)
        self.assertIn("up -d --no-deps realm", log)
        self.assertIn("-p wu-relay", log)
        self.assertNotIn("s3cr3t", r.stdout + r.stderr)
        # off: every name back as it was
        r = self.run_sh("off")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertTrue(r.stdout.rstrip().endswith("DOMAIN OK"))
        self.assertEqual(self.read(self.cf), CADDYFILE)
        self.assertEqual(self.read(self.realm_env), self.realm_orig)
        self.assertEqual(self.read(self.relay_env), self.relay_orig)
        p = self.read(self.portal_env)
        self.assertIn(f"PUBLIC_HOST={OLD['site']}\n", p)
        self.assertNotIn("STREAM_HOST=", p)
        self.assertIn("LEYLINE_DOMAIN_MOVE=0\n", p)
        self.assertFalse(os.path.exists(os.path.join(self.wu, "custom", "lua_scripts", "leyline-domain.on")))
        self.assertFalse(os.path.exists(os.path.join(self.wu, "domain", "domain.json")))

    def test_a_refused_redirect_rolls_everything_back(self):
        r = self.run_sh("on", FAKE_RELOAD_FAIL_ON="Leyline domain move (tools")
        self.assertEqual(r.returncode, 1)
        self.assertIn("everything is back as it was", r.stderr)
        self.assertEqual(self.read(self.cf), CADDYFILE)
        self.assertEqual(self.read(self.portal_env), self.portal_orig)
        self.assertEqual(self.read(self.realm_env), self.realm_orig)
        self.assertEqual(self.read(self.relay_env), self.relay_orig)
        self.assertFalse(os.path.exists(os.path.join(self.wu, "custom", "lua_scripts", "leyline-domain.on")))
        self.assertFalse(os.path.exists(os.path.join(self.wu, "domain", "domain.json")))

    def test_a_new_name_that_does_not_answer_rolls_back(self):
        r = self.run_sh("on", FAKE_DOWN=NEW["game"])
        self.assertEqual(r.returncode, 1)
        self.assertIn(f"{NEW['game']} did not answer over HTTPS", r.stderr)
        self.assertEqual(self.read(self.cf), CADDYFILE)
        self.assertEqual(self.read(self.portal_env), self.portal_orig)

    def test_sign_in_still_on_the_old_name_waits_before_the_301(self):
        r = self.run_sh("on", FAKE_GOOGLE="old")
        self.assertEqual(r.returncode, 3, r.stdout + r.stderr)
        self.assertTrue(r.stdout.rstrip().endswith("DOMAIN WAITING"))
        self.assertEqual(dc.status(self.read(self.cf), A)["site"], "alias")      # both names serve; no 301
        # run again once it is fixed: the saved "before" values stay the first run's, so off restores the original
        r = self.run_sh("on")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertEqual(self.run_sh("off").returncode, 0)
        self.assertEqual(self.read(self.realm_env), self.realm_orig)
        self.assertEqual(self.read(self.relay_env), self.relay_orig)
        self.assertEqual(self.read(self.cf), CADDYFILE)


class Bats(unittest.TestCase):
    def test_the_three_steps(self):
        wrap = open(os.path.join(TOOLS, "Domain move.bat"), encoding="utf-8").read()
        self.assertIn("tar -cf - domain.sh domain_caddy.py addresses.json", wrap)
        self.assertIn("root@play.openvanilla.app", wrap)            # the SSH name: works before and after the move
        self.assertIn("-e '^DOMAIN WAITING'", wrap)
        for mode in ("check", "on", "off"):
            with open(os.path.join(TOOLS, f"Domain move - {mode}.bat"), encoding="utf-8", newline="") as f:
                b = f.read()
            self.assertIn(f'call "%~dp0Domain move.bat" {mode}', b)
            self.assertIn("\r\n", b)
        for w in ("World of Warcraft", "Warcraft", "Blizzard", "WoW"):
            self.assertNotIn(w, wrap)


if __name__ == "__main__":
    unittest.main(verbosity=1)
