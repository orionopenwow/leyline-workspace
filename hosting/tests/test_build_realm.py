"""The Builders' Realm steps (hosting/build-realm/build-realm.sh and its three .bat files; build-realm-check, -on, -off in
tools/control/control.py) against stub docker / curl / getent (no Docker daemon, no database, no network). The stub
database answers the script's SQL and logs it; the stub runs the REAL `setup.py official` for the listing config.

Covers: the dry run changes nothing; a clean --go (the login database as views of acore_auth except its own realm list
and the main realm's script tables, the characters database cloned empty, the settings files 0600 with this realm's
databases and switches and none of the main realm's game switches, no published port, the main realm never touched,
the listing, the realm list address through the relay, the id files); low memory stops before anything; a world
server that never greets rolls everything back; a main world server that restarted during the step rolls back; --off
with builders online sends the in-game countdown; the .bat and step wiring.

    python3 -I hosting/tests/test_build_realm.py
"""
import ast
import json
import os
import re
import shutil
import stat
import subprocess
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
HOSTING = os.path.dirname(HERE)
ROOT = os.path.dirname(HOSTING)
SCRIPT = os.path.join(HOSTING, "build-realm", "build-realm.sh")
BATS = {m: os.path.join(HOSTING, "build-realm", f"Builders realm - {m}.bat") for m in ("check", "on", "off")}

DOCKER = r"""#!/usr/bin/env bash
S="$STUB_STATE"; echo "docker $*" >> "$S/docker.log"
up() { [ -f "$S/up_$1" ]; }
case "$1" in
  ps) for c in ov-ac-database ov-ac-worldserver ov-ac-authserver wu-portal wu-relay; do [ -f "$S/down_$c" ] || echo $c; done
      for c in ov-ac-build-world wu-build-auth wu-build-agent; do up $c && echo $c; done; exit 0 ;;
  inspect)
    last="${@: -1}"
    case "$*" in
      *leyline.build*) [ -f "$S/foreign_$last" ] && echo "" || echo 1 ;;
      *Config.Image*) case "$last" in ov-ac-worldserver) echo ov/ac-wotlk-worldserver:custom ;; *) echo ov/ac-wotlk-authserver:custom ;; esac ;;
      *Config.Env*) case "$last" in
          ov-ac-worldserver) printf 'PATH=/usr/bin\nAC_LOGIN_DATABASE_INFO=ac-database;3306;acore;DBSECRET-sentinel;acore_auth\nAC_WORLD_DATABASE_INFO=ac-database;3306;acore;DBSECRET-sentinel;acore_world\nAC_CHARACTER_DATABASE_INFO=ac-database;3306;acore;DBSECRET-sentinel;acore_characters\nAC_PLAYERBOTS_DATABASE_INFO=ac-database;3306;acore;DBSECRET-sentinel;acore_playerbots\nAC_RATE_XP_KILL=3\nAC_PLAYER_LIMIT=1000\nAC_ALE_ENABLED=true\nOV_EVENTS=1\nOV_DUELS=1\nOV_MAIN=1\n' ;;
          *) printf 'PATH=/usr/bin\nAC_LOGIN_DATABASE_INFO=ac-database;3306;acore;DBSECRET-sentinel;acore_auth\nAC_WRONG_PASS_MAX_COUNT=10\n' ;;
        esac ;;
      *RestartCount*State.Running*) [ -f "$S/oom" ] && echo "0 true true" || echo "0 true false" ;;
      *RestartCount*) echo 0 ;;
      *StartedAt*) if [ "$last" = ov-ac-worldserver ] && [ -f "$S/main_restarts" ]; then date +%s%N; else echo 2026-10-31T10:00:00Z; fi ;;
    esac; exit 0 ;;
  image) case "$3" in python:3.12-alpine) exit 0 ;; *) [ -f "$S/img_${3//[:\/]/_}" ]; exit $? ;; esac ;;
  tag) cp "$S/img_${2//[:\/]/_}" "$S/img_${3//[:\/]/_}" 2>/dev/null; exit 0 ;;
  build) echo built > "$S/img_wu-build-tools_1"; exit 0 ;;
  network|volume) [ "$2" = connect ] && echo "$*" >> "$S/connect.log"; exit 0 ;;
  stats) echo "ov-ac-worldserver 2.1GiB / 62GiB"; echo "ov-ac-database 900MiB / 62GiB"; up ov-ac-build-world && echo "ov-ac-build-world 812MiB / 1GiB"; exit 0 ;;
  update|logs) exit 0 ;;
  stop) rm -f "$S/up_$4" "$S/up_${@: -1}"; exit 0 ;;
  restart) echo "$*" >> "$S/restarts.log"; exit 0 ;;
  rm) for a in "$@"; do rm -f "$S/up_$a"; done; exit 0 ;;
  exec)
    if [[ "$*" == *mysqldump* ]]; then
      echo "-- dump $*"; head -c 4000 /dev/urandom | base64; echo; exit 0
    fi
    if [[ "$*" == *'exec mysql -uroot "$1"'* ]]; then cat >> "$S/sql_into.log"; exit 0; fi
    q=$(cat); printf '%s\n' "$q" >> "$S/sql.log"
    while IFS= read -r ln; do
      case "$ln" in
        *"FROM information_schema.schemata WHERE schema_name = '"*)
          d=$(sed -n "s/.*schema_name = '\([^']*\)'.*/\1/p" <<<"$ln"); case "$d" in acore_playerbots|acore_characters) echo 1 ;; *) [ -f "$S/db_$d" ] && echo 1 || echo 0 ;; esac ;;
        "CREATE DATABASE IF NOT EXISTS"*) d=$(sed -n 's/.*EXISTS `\([^`]*\)`.*/\1/p' <<<"$ln"); touch "$S/db_$d" ;;
        "DROP DATABASE IF EXISTS"*) d=$(sed -n 's/.*EXISTS `\([^`]*\)`.*/\1/p' <<<"$ln"); rm -f "$S/db_$d"; echo "$d" >> "$S/dropped.log" ;;
        *"table_schema = 'acore_auth' AND table_type"*) printf 'account\naccount_access\naccount_banned\nrealmlist\nuptime\nov_go_intents\nmod_x\n' ;;
        *"COUNT(*) FROM information_schema.tables WHERE table_schema = '"*"' AND table_name = '"*) echo 1 ;;
        *"COUNT(*) FROM information_schema.tables WHERE table_schema = '"*) echo 0 ;;
        *"FROM mysql.user WHERE user = 'acore'"*) echo % ;;
      esac
    done <<<"$q"
    exit 0 ;;
  run)
    shift
    if [ "$1" = -d ]; then
      name=""; prev=""; for a in "$@"; do [ "$prev" = --name ] && name=$a; prev=$a; done
      echo "$*" > "$S/args_$name"; touch "$S/up_$name"
      if [ "$name" = wu-build-agent ]; then
        st="$WU_BUILD_DIR/beacon/state"
        if [ -f "$S/fail_connect" ]; then printf '{"connected": false, "last_error": "relay refused: test"}' > "$st/relay-state.json"
        else printf '{"connected": true, "address": "relay.openvanilla.app", "world_port": 40011}' > "$st/relay-state.json"; fi
      fi
      exit 0
    fi
    if [[ " $* " == *" /p/buildprobe.py "* ]]; then
      args=(); seen=0; for a in "$@"; do [ $seen = 1 ] && args+=("$a"); [ "$a" = /p/buildprobe.py ] && seen=1; done
      echo "probe ${args[*]}" >> "$S/probe.log"
      case "${args[0]}" in
        logon) [ -f "$S/fail_logon_${args[1]}" ] && { echo "no answer: OSError"; exit 1; }; echo "logon answer 0x04 (ok)"; exit 0 ;;
        world) [ -f "$S/fail_world_${args[1]}" ] && { echo "no answer: OSError"; exit 1; }; echo "world greeting 0x01ec (ok)"; exit 0 ;;
        players) echo "players $(cat "$S/players" 2>/dev/null || echo 0)"; exit 0 ;;
        soap) echo "${args[*]:2}" >> "$S/soap.log"; [[ "${args[*]}" == *"server shutdown"* ]] && rm -f "$S/up_ov-ac-build-world"; echo ok; exit 0 ;;
      esac
    fi
    # docker run --rm ... IMAGE python -m wuhost.setup ARGS: the real setup.py, /kit = the beacon folder
    args=(); seen=0
    for a in "$@"; do
      if [ $seen = 1 ]; then args+=("${a//\/kit\//$WU_BUILD_DIR/beacon/}"); fi
      [ "$a" = wuhost.setup ] && seen=1
    done
    WU_KIT="$WU_BUILD_DIR/beacon" PYTHONPATH="$STUB_SRC/hosting/kit/tools" python3 -m wuhost.setup "${args[@]}"; exit $? ;;
esac
exit 0
"""
CURL = r"""#!/usr/bin/env bash
S="$STUB_STATE"; for a in "$@"; do u="$a"; done
rid=$(python3 -I -c 'import json,sys;print(json.load(open(sys.argv[1]))["realm_id"])' "$WU_BUILD_DIR/beacon/state/host.json" 2>/dev/null)
case "$u" in
  http://*/api/realms/*) [ -f "$S/up_wu-build-agent" ] || exit 22; echo '{"online": true}' ;;
  http://*/api/listing/realms.json) [ -f "$S/up_wu-build-agent" ] && printf '{"realms": [{"community": {"realmId": "%s"}}]}' "$rid" || echo '{"realms": []}' ;;
  *) exit 6 ;;
esac
"""
GETENT = "#!/usr/bin/env bash\necho '203.0.113.5     STREAM relay.openvanilla.app'\n"


def control_steps():
    tree = ast.parse(open(os.path.join(ROOT, "tools", "control", "control.py"), encoding="utf-8").read())
    out = {}
    for node in tree.body:
        if isinstance(node, ast.Assign):
            for t in node.targets:
                if getattr(t, "id", "") in ("STEPS", "READ_ONLY"):
                    out[t.id] = ast.literal_eval(node.value)
    return out


class BuildRealm(unittest.TestCase):
    def setUp(self):
        self.t = tempfile.mkdtemp(prefix="wu-build-realm-")
        self.bin, self.st = os.path.join(self.t, "bin"), os.path.join(self.t, "stub")
        os.makedirs(self.bin); os.makedirs(self.st)
        for name, body in (("docker", DOCKER), ("curl", CURL), ("getent", GETENT)):
            p = os.path.join(self.bin, name)
            with open(p, "w") as f:
                f.write(body)
            os.chmod(p, 0o755)
        self.src = os.path.join(self.t, "src")
        for d in ("hosting/build-realm", "hosting/kit/tools/wunet", "hosting/kit/tools/wuhost"):
            shutil.copytree(os.path.join(ROOT, d), os.path.join(self.src, d), ignore=shutil.ignore_patterns("__pycache__"))
        for f in ("hosting/kit/tools/Dockerfile", "realm/profiles/build.env", "realm/custom/lua_scripts/ov_buildrealm.lua",
                  "realm/custom/lua_scripts/ov_den.lua"):
            os.makedirs(os.path.dirname(os.path.join(self.src, f)), exist_ok=True)
            shutil.copy(os.path.join(ROOT, f), os.path.join(self.src, f))
        p = os.path.join(self.src, "hosting/build-realm/build-realm.sh")   # a Windows checkout: CRLF
        open(p, "wb").write(open(p, "rb").read().replace(b"\r\n", b"\n").replace(b"\n", b"\r\n"))
        self.relay = os.path.join(self.t, "relay"); os.makedirs(self.relay)
        with open(os.path.join(self.relay, "relay.env"), "w") as f:
            f.write("RELAY_NAME=relay.openvanilla.app\nRELAY_HTTP_BIND=172.18.0.1:8100\n")
        self.wrath = os.path.join(self.t, "wrath")
        self.portal = os.path.join(self.wrath, "portal")
        lua = os.path.join(self.wrath, "custom", "lua_scripts")
        for d in (self.portal, lua, os.path.join(self.wrath, "profiles"), os.path.join(self.wrath, "logs", "world")):
            os.makedirs(d, exist_ok=True)
        open(os.path.join(lua, "ov_build.lua"), "w").write("OVBuild.DenLayout = X.DenLayout\n")
        open(os.path.join(self.wrath, "signup.env"), "w").write("SOAP_USER=WUSOAP\nSOAP_PASS=SOAPSECRET-sentinel\nSOAP_URL=x\n")
        self.mem = os.path.join(self.t, "meminfo")
        self.set_mem(4000)
        self.dir = os.path.join(self.wrath, "build-realm")
        self.env = dict(os.environ, PATH=self.bin + os.pathsep + os.environ["PATH"], STUB_STATE=self.st, STUB_SRC=self.src,
                        WU_TEST="1", WU_WRATH_DIR=self.wrath, WU_BUILD_DIR=self.dir, WU_RELAY_DIR=self.relay,
                        WU_PORTAL_DIR=self.portal, WU_MEMINFO=self.mem, WU_BUILD_MIN_DISK_MB="1", WU_HEALTH_TRIES="2",
                        WU_HEALTH_PAUSE="0", WU_BUILD_WORLD_WAIT="0", WU_BUILD_SETTLE="0", WU_OFF_PAUSE="0", WU_BUILD_WARN_SECS="300")

    def tearDown(self):
        shutil.rmtree(self.t, ignore_errors=True)

    def set_mem(self, mb):
        with open(self.mem, "w") as f:
            f.write(f"MemTotal: 64000000 kB\nMemAvailable: {mb * 1024} kB\n")

    def run_it(self, mode):
        p = subprocess.run(["bash", SCRIPT, self.src, mode], env=self.env, capture_output=True, text=True, timeout=120)
        return p.returncode, p.stdout + p.stderr

    def flag(self, name):
        open(os.path.join(self.st, name), "w").close()

    def read(self, name):
        p = os.path.join(self.st, name)
        return open(p).read() if os.path.exists(p) else ""

    def up(self, c):
        return os.path.exists(os.path.join(self.st, "up_" + c))

    def ids(self):
        return [open(p).read().strip() if os.path.exists(p) else None
                for p in (os.path.join(self.portal, "build-realm.id"), os.path.join(self.wrath, "build-realm.id"))]

    def test_dry_run_changes_nothing(self):
        rc, out = self.run_it("--check")
        self.assertEqual(rc, 0, out)
        self.assertTrue(out.rstrip().endswith("BUILD REALM DRY"), out[-300:])
        self.assertIn("ov_den.lua: missing or old; this step installs it", out)
        self.assertIn("memory available: 4000 MB", out)
        self.assertIn("ov-ac-worldserver: 2.1GiB", out)
        self.assertNotIn("WOULD STOP", out)
        log = self.read("docker.log")
        self.assertNotIn("docker run -d", log)
        self.assertNotRegex(self.read("sql.log"), r"(?m)^(CREATE|INSERT|UPDATE|DELETE|DROP|GRANT)")
        self.assertFalse(os.path.exists(self.dir))

    def test_low_memory_stops_before_anything(self):
        self.set_mem(900)
        rc, out = self.run_it("--go")
        self.assertEqual(rc, 1, out)
        self.assertIn("only 900 MB of memory available", out)
        self.assertNotIn("docker run -d", self.read("docker.log"))
        self.assertEqual(self.read("sql.log"), "")
        rc, out = self.run_it("--check")
        self.assertEqual(rc, 0, out)
        self.assertIn("WOULD STOP: only 900 MB", out)

    def test_old_scripts_stop_before_anything(self):
        open(os.path.join(self.wrath, "custom", "lua_scripts", "ov_build.lua"), "w").write("-- old\n")
        rc, out = self.run_it("--go")
        self.assertEqual(rc, 1, out)
        self.assertIn("ov_build.lua is older", out)
        self.assertIn("run the scripts step first", out)
        self.assertNotIn("docker run -d", self.read("docker.log"))

    def test_clean_install(self):
        rc, out = self.run_it("--go")
        self.assertEqual(rc, 0, out)
        self.assertTrue(out.rstrip().endswith("BUILD REALM ON"), out[-600:])
        sql = self.read("sql.log")
        # the login database: views of acore_auth's own tables, never its realm list or the main realm's script tables
        for t in ("account", "account_access", "account_banned", "uptime"):
            self.assertIn(f"CREATE OR REPLACE SQL SECURITY INVOKER VIEW `acore_auth_build`.`{t}` AS SELECT * FROM `acore_auth`.`{t}`;", sql)
        for t in ("realmlist", "ov_go_intents", "mod_x"):
            self.assertNotIn(f"VIEW `acore_auth_build`.`{t}`", sql)
        self.assertIn("CREATE TABLE IF NOT EXISTS `acore_auth_build`.realmlist LIKE `acore_auth`.realmlist;", sql)
        self.assertIn("DELETE FROM `acore_auth_build`.realmlist WHERE id <> 2;", sql)
        self.assertIn("UPDATE `acore_auth_build`.realmlist SET name = 'Builders'' Realm'", sql)
        self.assertIn("UPDATE `acore_auth_build`.realmlist SET address = '203.0.113.5', localAddress = '203.0.113.5', port = 40011 WHERE id = 2;", sql)
        self.assertNotRegex(sql, r"(?m)^(UPDATE|INSERT INTO|DELETE FROM|DROP|ALTER) `?acore_(auth|characters|world)`?\.",
                            "the main realm's databases are never written by the script")
        self.assertIn("INSERT IGNORE INTO `acore_characters_build`.`updates` SELECT * FROM `acore_characters`.`updates`;", sql)
        self.assertIn("GRANT ALL PRIVILEGES ON `acore_characters_build`.* TO 'acore'@'%';", sql)
        self.assertIn("--no-data acore_characters", self.read("docker.log"))
        # settings: this realm's databases and switches, 0600, nothing of the main realm's game switches, never printed
        w = open(os.path.join(self.dir, "world.env")).read()
        a = open(os.path.join(self.dir, "auth.env")).read()
        for f in ("world.env", "auth.env"):
            self.assertEqual(stat.S_IMODE(os.stat(os.path.join(self.dir, f)).st_mode), 0o600)
        for need in ("AC_CHARACTER_DATABASE_INFO=ac-database;3306;acore;DBSECRET-sentinel;acore_characters_build",
                     "AC_LOGIN_DATABASE_INFO=ac-database;3306;acore;DBSECRET-sentinel;acore_auth_build",
                     "AC_WORLD_DATABASE_INFO=ac-database;3306;acore;DBSECRET-sentinel;acore_world",
                     "AC_PLAYERBOTS_DATABASE_INFO=ac-database;3306;acore;DBSECRET-sentinel;acore_playerbots_build",
                     "AC_REALM_ID=2", "OV_BUILDREALM=1", "OV_DEN=1", "OV_EVENTS=0", "OV_DUELS=0", "AC_PLAYERBOTS_ENABLED=0",
                     "AC_AUTO_BALANCE_ENABLE_GLOBAL=0", "AC_PRELOAD_ALL_NON_INSTANCED_MAP_GRIDS=0", "AC_GRID_UNLOAD=1",
                     "AC_PLAYER_LIMIT=20", "AC_UPDATES_ENABLE_DATABASES=0", "AC_WARDEN_ENABLED=0", "AC_ALE_ENABLED=true"):
            self.assertIn(need + "\n", w)
        for gone in ("OV_EVENTS=1", "OV_MAIN=1", "AC_RATE_XP_KILL=3", "AC_PLAYER_LIMIT=1000"):
            self.assertNotIn(gone + "\n", w)
        self.assertIn("AC_LOGIN_DATABASE_INFO=ac-database;3306;acore;DBSECRET-sentinel;acore_auth_build\n", a)
        self.assertIn("AC_WRONG_PASS_MAX_COUNT=0\n", a)
        self.assertNotIn("AC_WRONG_PASS_MAX_COUNT=10", a)
        for secret in ("DBSECRET-sentinel", "SOAPSECRET-sentinel"):
            self.assertNotIn(secret, out)
        # containers: no published port, the main realm's images, caps, the save folder, the scripts read-only
        world = self.read("args_ov-ac-build-world")
        for need in ("--memory 1024m", "--env-file " + os.path.join(self.dir, "world.env"), "--label leyline.build=1",
                     "type=volume,src=ov-client-data,dst=/azerothcore/env/dist/data,readonly",
                     f"type=bind,src={self.wrath}/custom/lua_scripts,dst=/azerothcore/lua_scripts,readonly",
                     f"type=bind,src={self.dir}/dens,dst=/azerothcore/dens", "ov/ac-wotlk-worldserver:custom"):
            self.assertIn(need, world)
        auth = self.read("args_wu-build-auth")
        self.assertIn("ov/ac-wotlk-authserver:custom", auth)
        agent = self.read("args_wu-build-agent")
        self.assertIn("wu-build-tools:1 python -m wuhost.agent --state /state", agent)
        for args in (world, auth, agent):
            self.assertNotRegex(args, r"(^| )(-p|--publish) ")
        connects = self.read("connect.log")
        for c in ("ov-ac-build-world", "wu-build-auth", "wu-build-agent"):
            self.assertIn(f"network connect ov-wotlk_edge {c}", connects)
        log = self.read("docker.log")
        self.assertNotRegex(log, r"docker (stop|restart|rm -f|update)[^\n]* ov-ac-(worldserver|authserver)\b")
        self.assertEqual(self.read("restarts.log"), "")
        probes = self.read("probe.log")
        for need in ("probe logon wu-build-auth 3724", "probe world ov-ac-build-world 8085", "probe world ov-ac-worldserver 8085",
                     "probe logon ov-ac-authserver 3724"):
            self.assertIn(need, probes)
        # files on the box, the listing, the id files
        self.assertTrue(os.path.exists(os.path.join(self.wrath, "profiles", "build.env")))
        self.assertTrue(os.path.exists(os.path.join(self.wrath, "custom", "lua_scripts", "ov_buildrealm.lua")))
        self.assertEqual(open(os.path.join(self.wrath, "custom", "lua_scripts", "ov_den.lua")).read(),
                         open(os.path.join(ROOT, "realm", "custom", "lua_scripts", "ov_den.lua")).read(),
                         "ov_den.lua comes with this step (the scripts step doesn't carry it)")
        cfg = json.load(open(os.path.join(self.dir, "beacon", "state", "host.json")))
        self.assertTrue(cfg["official"])
        self.assertEqual((cfg["name"], cfg["auth_service"], cfg["world_service"]), ("Builders' Realm", "wu-build-auth:3724", "ov-ac-build-world:8085"))
        self.assertEqual((cfg["record"]["tags"], cfg["record"]["ruleset"], cfg["record"]["max_players"]), (["custom"], "custom", 20))
        self.assertNotRegex(json.dumps(cfg["record"]), r"(?i)world of warcraft|\bwow\b|warcraft|lich king|blizzard")
        self.assertNotRegex(json.dumps(cfg["record"]) + out, r"\b3\.3\.5|1\.12\b")
        rid = cfg["realm_id"]
        self.assertEqual(self.ids(), [rid, rid])
        backups = os.listdir(os.path.join(self.dir, "backups"))
        self.assertEqual(len(backups), 1)
        self.assertIn("acore_auth.sql.gz", os.listdir(os.path.join(self.dir, "backups", backups[0])))
        # again (new settings): nobody online, the same realm id
        rc, out = self.run_it("--go")
        self.assertEqual(rc, 0, out)
        self.assertEqual(json.load(open(os.path.join(self.dir, "beacon", "state", "host.json")))["realm_id"], rid)
        self.assertIn("nobody online", out)

    def test_world_that_never_greets_rolls_back(self):
        self.flag("fail_world_ov-ac-build-world")
        rc, out = self.run_it("--go")
        self.assertEqual(rc, 1, out)
        self.assertIn("did not greet", out)
        self.assertIn("rolled back", out)
        for c in ("ov-ac-build-world", "wu-build-auth", "wu-build-agent"):
            self.assertFalse(self.up(c), c)
        self.assertEqual(sorted(self.read("dropped.log").split()), ["acore_auth_build", "acore_characters_build", "acore_playerbots_build"])
        self.assertFalse(os.path.exists(os.path.join(self.wrath, "profiles", "build.env")))
        self.assertFalse(os.path.exists(os.path.join(self.wrath, "custom", "lua_scripts", "ov_buildrealm.lua")))
        self.assertFalse(os.path.exists(os.path.join(self.wrath, "custom", "lua_scripts", "ov_den.lua")))
        self.assertEqual(self.ids(), [None, None])

    def test_out_of_memory_rolls_back(self):
        self.flag("oom")
        rc, out = self.run_it("--go")
        self.assertEqual(rc, 1, out)
        self.assertIn("ran out of memory", out)
        self.assertFalse(self.up("ov-ac-build-world"))

    def test_main_realm_restarting_rolls_back(self):
        self.flag("main_restarts")
        rc, out = self.run_it("--go")
        self.assertEqual(rc, 1, out)
        self.assertIn("main realm's world server restarted", out)
        self.assertFalse(self.up("ov-ac-build-world"))

    def test_failed_connect_rolls_back(self):
        self.flag("fail_connect")
        rc, out = self.run_it("--go")
        self.assertEqual(rc, 1, out)
        self.assertIn("relay refused: test", out)
        self.assertFalse(self.up("wu-build-agent"))
        self.assertFalse(self.up("ov-ac-build-world"))
        self.assertEqual(self.ids(), [None, None])

    def test_off_with_builders_online_counts_down(self):
        rc, out = self.run_it("--go")
        self.assertEqual(rc, 0, out)
        with open(os.path.join(self.st, "players"), "w") as f:
            f.write("3")
        rc, out = self.run_it("--off")
        self.assertEqual(rc, 0, out)
        self.assertTrue(out.rstrip().endswith("BUILD REALM OFF"), out[-300:])
        self.assertIn("3 builder(s) online", out)
        self.assertEqual(self.read("soap.log").strip(), "server shutdown 300")
        for c in ("ov-ac-build-world", "wu-build-auth", "wu-build-agent"):
            self.assertFalse(self.up(c), c)
        self.assertEqual(self.ids(), [None, None])
        self.assertEqual(self.read("dropped.log"), "", "the databases stay")
        self.assertFalse(os.path.exists(os.path.join(self.dir, "soap.env")), "the SOAP login file is gone again")
        self.assertNotIn("SOAPSECRET-sentinel", out)
        self.assertNotRegex(self.read("docker.log"), r"ov-ac-worldserver\b[^\n]*(stop|restart)|(stop|restart)[^\n]*ov-ac-worldserver\b")

    def test_bats_and_steps(self):
        modes = {"check": "--check", "on": "--go", "off": "--off"}
        lasts = {"check": "BUILD REALM DRY", "on": "BUILD REALM ON", "off": "BUILD REALM OFF"}
        for m, bat in BATS.items():
            b = open(bat, encoding="utf-8").read()
            self.assertIn(f"bash $T/build-realm.sh $T {modes[m]}", b)
            self.assertIn(f"grep -q '^{lasts[m]}'", b)
            for f in ("hosting/build-realm", "hosting/kit/tools/wunet", "hosting/kit/tools/wuhost", "hosting/kit/tools/Dockerfile",
                      "realm/profiles/build.env", "realm/custom/lua_scripts/ov_buildrealm.lua",
                      "realm/custom/lua_scripts/ov_den.lua"):
                self.assertIn(f, b)
            self.assertNotRegex(b, r"(?i)world of warcraft|warcraft|blizzard")
        c = control_steps()
        self.assertEqual(c["STEPS"]["build-realm-check"][0], "hosting/build-realm/Builders realm - check.bat")
        self.assertEqual(c["STEPS"]["build-realm-on"][0], "hosting/build-realm/Builders realm - on.bat")
        self.assertEqual(c["STEPS"]["build-realm-off"][0], "hosting/build-realm/Builders realm - off.bat")
        self.assertIn("build-realm-check", c["READ_ONLY"])
        self.assertNotIn("build-realm-on", c["READ_ONLY"])

    def test_profile(self):
        keys = {}
        for ln in open(os.path.join(ROOT, "realm", "profiles", "build.env"), encoding="utf-8"):
            m = re.match(r"^([A-Z0-9_]+)=(.*)$", ln.strip())
            if m:
                self.assertNotIn(m.group(1), keys, "each key once")
                keys[m.group(1)] = m.group(2)
        for k, v in {"OV_BUILDREALM": "1", "OV_BUILDREALM_VISITORS": "0", "OV_DEN": "1", "OV_BUILD": "1", "OV_EVENTS": "0",
                     "OV_DUELS": "0", "OV_COMPANIONS": "0", "AC_PLAYERBOTS_ENABLED": "0", "AC_AUTO_BALANCE_ENABLE_GLOBAL": "0",
                     "AC_PRELOAD_ALL_NON_INSTANCED_MAP_GRIDS": "0", "AC_GRID_UNLOAD": "1", "AC_PLAYER_LIMIT": "20"}.items():
            self.assertEqual(keys.get(k), v, k)
        self.assertLessEqual(int(keys["OV_DEN_SIZE"]), 32)


if __name__ == "__main__":
    unittest.main()
