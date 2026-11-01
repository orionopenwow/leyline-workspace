"""Azeroth Survivors' own address (hosting/survivors/): the Caddy site, the killswitch block, the compose service and
sv-site-up.sh --check / --go / --off against fakes (docker, curl, getent) in a temp folder. Nothing real is touched.

    python3 -I hosting/tests/test_survivors_site.py
"""
import hashlib, importlib.util, json, os, re, shutil, stat, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
SV = os.path.join(ROOT, "hosting", "survivors")
fails = count = 0


def check(name, cond, extra=""):
    global fails, count
    count += 1
    print(("PASS " if cond else "FAIL ") + name + (f"  {extra}" if extra else ""))
    fails += 0 if cond else 1


spec = importlib.util.spec_from_file_location("gdc", os.path.join(ROOT, "stream", "server", "gamedata-caddy.py"))
gdc = importlib.util.module_from_spec(spec); spec.loader.exec_module(gdc)
snip = open(os.path.join(SV, "survivors.caddy")).read()
site = snip.replace("__SV_HOST__", "survive.example")

# ------------------------------------------------------------------ the Caddy site
check("caddy: the markers are gamedata-caddy.py's", site.startswith(gdc.SV_SITE_BEGIN) and site.rstrip().endswith(gdc.SV_SITE_END))
check("caddy: the licences for anyone, before the sign-in", site.index("@licenses_sv path /licenses /licenses/*") < site.index("\tforward_auth"))
check("caddy: the gate goes to wu-portal", re.search(r"@gate_sv path /__gate\n\thandle @gate_sv \{\n\t\treverse_proxy wu-portal:8091", site) is not None)
check("caddy: everything else behind forward_auth /__auth?site=survivors", "forward_auth wu-portal:8091 {\n\t\t\turi /__auth?site=survivors" in site)
check("caddy: only sv-play and wu-portal behind it", set(re.findall(r"reverse_proxy (?:@\w+ )?([\w.-]+:\d+)", site)) == {"sv-play:8090", "wu-portal:8091"})
check("caddy: no /__env.js (no game login here)", "__env.js" not in site and "/ws" not in site)
check("caddy: balanced braces", site.count("{") - site.count("{remote_host}") == site.count("}") - site.count("{remote_host}"))
cf = "play.openvanilla.app {\n\treverse_proxy x:1\n}\n\n" + site
check("killswitch: no site, no-site", gdc.sv_status("play.openvanilla.app {\n}\n") == "no-site")
check("killswitch: status off", gdc.sv_status(cf) == "off")
on = gdc.sv_add(cf, "survive.example")
check("killswitch: on, first inside the site", gdc.sv_status(on) == "on"
      and on.index("survive.example {") < on.index(gdc.SV_BEGIN) < on.index("@licenses_sv"))
check("killswitch: 410 JSON for /data and the manifests", "@gamedata_off_sv path /data /data/* /boot-manifest.json /world-manifest.json" in on
      and json.loads(re.search(r"respond `(\{.*?\})` 410", on).group(1))["error"] == "gamedata_off")
check("killswitch: every page but the licences rests (503)", "@resting_sv not path /licenses/*" in on and "` 503\n" in on)
rest = gdc.SV_RESTING
check("killswitch: the resting page has no braces or backticks (Caddy) and no brand words",
      "{" not in rest and "}" not in rest and "`" not in rest
      and not re.search(r"warcraft|blizzard|(^|[^a-z])wow([^a-z]|$)|lich king", rest, re.I) and "resting" in rest)
check("killswitch: idempotent, and out byte for byte", gdc.sv_add(on, "survive.example") == on and gdc.sv_strip(on) == cf)
check("killswitch: the Wrath and 1.12 blocks are untouched by it", gdc.status(on) == "no-site" and gdc.vr_status(on) == "no-site")
T = tempfile.mkdtemp(prefix="sv-site-")
p = os.path.join(T, "Caddyfile"); open(p, "w").write(cf)
r = subprocess.run([sys.executable, "-I", os.path.join(ROOT, "stream", "server", "gamedata-caddy.py"), "add-sv", p, "survive.example"])
r2 = subprocess.run([sys.executable, "-I", os.path.join(ROOT, "stream", "server", "gamedata-caddy.py"), "status-sv", p], capture_output=True, text=True)
check("killswitch: add-sv / status-sv on the command line", r.returncode == 0 and r2.stdout.strip() == "on")
subprocess.run([sys.executable, "-I", os.path.join(ROOT, "stream", "server", "gamedata-caddy.py"), "remove-sv", p])
check("killswitch: remove-sv", open(p).read() == cf)

# ------------------------------------------------------------------ the compose service
comp = open(os.path.join(SV, "docker-compose.yml")).read()
check("compose: our wenilla-host, the slice only (--only-listed), no game connection",
      '"--patch", "/slice", "--only-listed"' in comp and "--upstream" not in comp and "--data" not in comp)
check("compose: locked down", all(x in comp for x in ("read_only: true", 'user: "65534:65534"', "cap_drop: [ALL]",
                                                       "no-new-privileges:true", "mem_limit: 256m", ":/www:ro", ":/slice:ro")))
check("compose: on Caddy's network, no published port", "name: wenilla-realm_default" in comp and "ports:" not in comp)
addr = [l for l in open(os.path.join(SV, "address.txt")) if l.strip() and not l.startswith("#")][0].strip()
check("address.txt: a placeholder with no brand word, marked TODO(owner)", "wow" not in addr.lower()
      and "TODO(owner)" in open(os.path.join(SV, "address.txt")).read())
for bat in sorted(f for f in os.listdir(SV) if f.endswith(".bat")):
    b = open(os.path.join(SV, bat), "rb").read()
    check(f"{bat}: CRLF only, nothing from labs", b.count(b"\n") == b.count(b"\r\n") > 10 and b"labs" not in b)

# ------------------------------------------------------------------ sv-site-up.sh against fakes
src = os.path.join(T, "src")
for rel in ("hosting/survivors/sv-site-up.sh", "hosting/survivors/survivors.caddy", "hosting/survivors/docker-compose.yml",
            "tools/survivors/survivors_check.py", "stream/server/gamedata-caddy.py"):
    os.makedirs(os.path.dirname(os.path.join(src, rel)), exist_ok=True)
    shutil.copy(os.path.join(ROOT, rel), os.path.join(src, rel))
pay = os.path.join(src, "payload")
for rel, body in (("www/index.html", b"<title>Azeroth Survivors</title>"), ("www/wenilla_bg.wasm", b"\0asm"),
                  ("www/licenses/LICENSE-MIT", b"MIT"), ("www/licenses/LICENSE-APACHE", b"Apache"),
                  ("slice/DBFilesClient/Map.dbc", b"WDBC")):
    os.makedirs(os.path.dirname(os.path.join(pay, rel)), exist_ok=True)
    open(os.path.join(pay, rel), "wb").write(body)
open(os.path.join(pay, "names.sha256"), "w").write(hashlib.sha256(b"WDBC").hexdigest() + "  DBFilesClient\\Map.dbc\n")
os.makedirs(os.path.join(src, "stream", "survivors"))
files = {}
for d, _, fs in os.walk(os.path.join(pay, "www")):
    for f in fs:
        q = os.path.join(d, f)
        files[os.path.relpath(q, os.path.join(pay, "www")).replace(os.sep, "/")] = hashlib.sha256(open(q, "rb").read()).hexdigest()
json.dump({"repo": "https://github.com/OutThisLife/azeroth-survivors-re", "sha": "b" * 40, "dist": {"files": files}},
          open(os.path.join(src, "stream", "survivors", "upstream.lock.json"), "w"))
fake, srv = os.path.join(T, "fake"), os.path.join(T, "srv")
os.makedirs(fake); os.makedirs(os.path.join(srv, "wrath", "portal")); os.makedirs(os.path.join(srv, "wrath", "stream", "bin"))
hb = os.path.join(srv, "wrath", "stream", "bin", "wenilla-host"); open(hb, "w").write("#!/bin/sh\n"); os.chmod(hb, 0o755)
shutil.copy(os.path.join(ROOT, "stream", "survivors.py"), os.path.join(srv, "wrath", "portal", "survivors.py"))
caddyfile = os.path.join(T, "Caddyfile"); open(caddyfile, "w").write("play.openvanilla.app {\n\treverse_proxy x:1\n}\n")
state = os.path.join(T, "state"); open(state, "w").write("wenilla-realm-caddy-1\nwu-portal\n")
calls = os.path.join(T, "calls.log")
FAKES = {
    "docker": r'''#!/bin/bash
echo "docker $*" >> "$CALLS"
case "$1" in
  ps) cat "$STATE";;
  network) exit 0;;
  compose) if [[ " $* " == *" up "* ]]; then grep -qx sv-play "$STATE" || echo sv-play >> "$STATE";
           elif [[ " $* " == *" down"* ]]; then sed -i '/^sv-play$/d' "$STATE"; fi; exit 0;;
  rm) sed -i '/^sv-play$/d' "$STATE"; exit 0;;
  logs) exit 0;;
  exec) if [ "$2" = wu-portal ]; then p="${@: -1}"; case "$p" in /index.html|/data/__index) echo 200;; *) echo 404;; esac;
        else [ -n "${CADDY_REFUSE:-}" ] && exit 1; exit 0; fi;;
esac
exit 0
''',
    "curl": r'''#!/bin/bash
u="${@: -1}"
case "$u" in
  */wrath/status.json) echo -n 200;;
  */licenses/LICENSE-MIT|*/licenses/) [ -n "${ON:-}" ] && echo -n 200 || echo -n 200;;
  */data/__index) [ -f "$FLAG" ] && echo -n 410 || echo -n 403;;
  https://survive.example/) [ -f "$FLAG" ] && echo -n 503 || echo -n 302;;
  *) echo -n 000;;
esac
''',
    "getent": "#!/bin/bash\necho \"203.0.113.7 STREAM $2\"\n",
    "hostname": "#!/bin/bash\necho 203.0.113.7\n",
}
for n, body in FAKES.items():
    f = os.path.join(fake, n); open(f, "w").write(body); os.chmod(f, 0o755)
FLAG = os.path.join(srv, "wrath", "portal", "gamedata-off")
env = dict(os.environ, PATH=fake + ":" + os.environ["PATH"], SV_TEST="1", SV_ROOT=os.path.join(srv, "survivors"),
           WU_DIR=os.path.join(srv, "wrath"), SV_CADDYFILE=caddyfile, SV_BACKUPS=os.path.join(T, "bk"),
           SV_SITE_HOST="survive.example", STATE=state, CALLS=calls, FLAG=FLAG)


def run(mode, **extra):
    r = subprocess.run(["bash", os.path.join(src, "hosting", "survivors", "sv-site-up.sh"), src, mode],
                       capture_output=True, text=True, env=dict(env, **extra), timeout=120)
    return r.returncode, r.stdout + r.stderr


rc, out = run("--check")
check("--check: ready, read-only", rc == 0 and out.rstrip().endswith("SV SITE DRY") and "Ready" in out
      and not os.path.exists(os.path.join(srv, "survivors", "www")), out[-600:])
rc, out = run("--check", SV_SITE_HOST="wowsurvive.example")
check("--check: refuses an address with a franchise name in it", rc == 1 and "brand rule" in out)
rc, out = run("--check", SV_SITE_HOST="play.openvanilla.app")
check("--check: refuses another site's address", rc == 1)
rc, out = run("--go")
url = os.path.join(srv, "wrath", "portal", "survivors.url")
check("--go: on", rc == 0 and out.rstrip().endswith("SV SITE ON"), out[-800:])
check("--go: files installed", os.path.isfile(os.path.join(srv, "survivors", "slice", "DBFilesClient", "Map.dbc"))
      and os.path.isfile(os.path.join(srv, "survivors", "www", "licenses", "LICENSE-MIT")))
check("--go: the site block is in, with the address", gdc.sv_status(open(caddyfile).read()) == "off"
      and "survive.example {" in open(caddyfile).read() and "__SV_HOST__" not in open(caddyfile).read())
check("--go: Play on: the portal's survivors.url", open(url).read() == "https://survive.example/\n")
check("--go: sv-play from the compose file", "compose -f " in open(calls).read() and "sv-play" in open(state).read())
rc, out = run("--go")
check("--go again (no new files to swap in twice): one site block", rc == 0 and open(caddyfile).read().count(gdc.SV_SITE_BEGIN) == 1)
open(FLAG, "w").write("{}")
rc, out = run("--go")
check("--go with the killswitch on: the site rests", rc == 0 and gdc.sv_status(open(caddyfile).read()) == "on", out[-500:])
os.remove(FLAG)
before = open(caddyfile).read()
rc, out = run("--go", CADDY_REFUSE="1")
check("--go: Caddy refuses -> rolled back, Play off, the Caddyfile as it was", rc == 1 and "SV SITE FAILED" in out
      and not os.path.exists(url) and open(caddyfile).read() == before)
open(os.path.join(pay, "slice", "DBFilesClient", "Map.dbc"), "ab").write(b"!")
rc, out = run("--go")
check("--go: an upload that fails survivors-check installs nothing", rc == 1 and "survivors-check" in out)
rc, out = run("--off")
check("--off: Play off, the site out, sv-play down", rc == 0 and out.rstrip().endswith("SV SITE OFF") and not os.path.exists(url)
      and gdc.SV_SITE_BEGIN not in open(caddyfile).read() and "sv-play" not in open(state).read())
check("--off: the files are kept", os.path.isdir(os.path.join(srv, "survivors", "slice")))

shutil.rmtree(T, ignore_errors=True)
print(f"\n{count - fails}/{count} passed")
sys.exit(1 if fails else 0)
