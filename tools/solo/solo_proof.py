#!/usr/bin/env python3
"""Leyline Solo, Phase 0 (docs/LEYLINE-SOLO-PHASE0.md): a first-era world with companions on this PC, from the
player's OWN game folder, offline, signing in by itself. Run by the Control steps solo-check, solo-setup, solo-start,
solo-stop, solo-signin and solo-signin-off. Standard library only (Windows Python 3; the tests run anywhere).

    solo_proof.py check   [game folder]    read-only: a first-era client (build 5875)? disk, ports, the slot
    solo_proof.py setup   [game folder]    the engine from solo-engine-latest (checked), the maps from YOUR folder,
                                           the database, the local account SOLO; --mmaps adds the monster paths
    solo_proof.py start | stop | status    the local world (database, login server, world server) on 127.0.0.1
    solo_proof.py signin  [game folder]    the game signs in by itself: realmlist.wtf, Config.wtf's account name
                                           and our sign-in add-on in a patch archive of ours; --off puts it all back
    solo_proof.py ci-smoke --pkg DIR       (CI) a fresh database from the package, SOLO signs in on the login server

The game folder: the argument, else tools/solo/folder.txt (one line, the owner's; git-ignored), else the first
first-era folder in the Leyline launcher's game folders (%APPDATA%\\Leyline\\settings.json).

Everything lives in %LOCALAPPDATA%\\Leyline\\solo-proof (LEYLINE_SOLO_DIR changes it). The generated passwords (the
database's, SOLO's, the stop account's) are only in that folder's state.json and the server settings files beside
it: never printed, never logged (every line this tool prints from a server log goes through `scrub`).
"""
import argparse
import hashlib
import json
import os
import re
import secrets
import shutil
import socket
import struct
import subprocess
import sys
import time
import urllib.request
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
import mpq112  # noqa: E402

BUILD = 5875
RELEASE = "https://github.com/orionopenwow/leyline-workspace/releases/download/solo-engine-latest/"
ZIP = "leyline-solo-engine-win64.zip"
ACCOUNT = "SOLO"
ADMIN = "SOLOSTOP"            # the stop account (game master level 3, SOAP on 127.0.0.1 only): never plays
REALM = "Leyline Solo"
PORTS = {"login": 3724, "world": 8085, "database": 13336, "soap": 7878}
DBS = ("classicmangos", "classiccharacters", "classicrealmd", "classiclogs")
READY = "World initialized"
SLOT = "patch-Z.MPQ"
OUR_LUA = "Interface\\GlueXML\\LeylineSolo.lua"
OUR_XML = "Interface\\GlueXML\\LeylineSolo.xml"
SESSION = "Interface\\GlueXML\\LeylineSoloSession.lua"
TOC = "Interface\\GlueXML\\GlueXML.toc"
OURS_MAX = 4 * 1024 * 1024
NO_SESSION = "-- Leyline Solo: no sign-in written (solo_proof.py signin writes one).\r\nLEYLINE_SOLO_SESSION = nil\r\n"
NEED_GB = 8
EXE_NAMES = ("WoW.exe", "Wow.exe")
FIRST_ERA_DATA = ("dbc.MPQ", "terrain.MPQ", "interface.MPQ")
LATER_ERA_DATA = ("common.MPQ", "expansion.MPQ", "lichking.MPQ", "common-2.MPQ", "expansion1.MPQ")
SRP_N = int("894B645E89E1535BBDAD5B8B290650530801B18EBFBF5E8FAB3C82872A3E9BB7", 16)
SRP_G = 7

_SECRETS = []


# ------------------------------------------------------------------------------------------------ output
def scrub(line):
    for s in _SECRETS:
        if s:
            line = line.replace(s, "[hidden]")
    return line


def out(line=""):
    print(scrub(str(line)), flush=True)


class Report:
    def __init__(self):
        self.rows = []

    def add(self, kind, what, detail="", fix=""):
        self.rows.append((kind, what, detail, fix))
        out("%-4s %s%s" % (kind, what, (": " + detail) if detail else ""))
        if fix and kind in ("FAIL", "WARN"):
            out("     fix: " + fix)

    @property
    def ok(self):
        return not any(r[0] == "FAIL" for r in self.rows)


# ------------------------------------------------------------------------------------------------ where things are
def home():
    if os.environ.get("LEYLINE_SOLO_DIR"):
        return Path(os.environ["LEYLINE_SOLO_DIR"])
    base = os.environ.get("LOCALAPPDATA") or str(Path.home() / ".local" / "share")
    return Path(base) / "Leyline" / "solo-proof"


def app_dir():
    if os.environ.get("LEYLINE_APP_DIR"):
        return Path(os.environ["LEYLINE_APP_DIR"])
    if os.name == "nt":
        return Path(os.environ.get("APPDATA", str(Path.home()))) / "Leyline"
    return Path(os.environ.get("XDG_DATA_HOME", str(Path.home() / ".local" / "share"))) / "Leyline"


def find_ci(d, name):
    try:
        for e in os.listdir(d):
            if e.lower() == name.lower():
                return Path(d) / e
    except OSError:
        pass
    return None


def game_folder(arg=None):
    """(folder, where it came from): the argument, else tools/solo/folder.txt, else the launcher's first first-era
    game folder (the selected one first)."""
    if arg:
        return Path(arg), "the command line"
    ft = HERE / "folder.txt"
    try:
        for line in ft.read_text(encoding="utf-8-sig").splitlines():
            line = line.strip().strip('"')
            if line and not line.startswith("#"):
                return Path(line), "tools\\solo\\folder.txt"
    except OSError:
        pass
    try:
        s = json.loads((app_dir() / "settings.json").read_text(encoding="utf-8"))
    except (OSError, ValueError):
        s = {}
    clients = [c for c in (s.get("clients") or []) if isinstance(c, dict) and c.get("build") == "1.12.1" and c.get("path")]
    clients.sort(key=lambda c: c.get("id") != s.get("selected_client"))
    if clients:
        return Path(clients[0]["path"]), "the launcher's game folders (%s)" % (clients[0].get("name") or "first era")
    return None, ""


def pe_version(path):
    """The file version of a Windows program from its version resource (VS_FIXEDFILEINFO), or None."""
    try:
        b = Path(path).read_bytes()
    except OSError:
        return None
    if not b.startswith(b"MZ"):
        return None
    sig = struct.pack("<I", 0xFEEF04BD)
    at = b.find(sig)
    while at >= 0:
        if b[at + 4:at + 8] == struct.pack("<I", 0x00010000) and len(b) >= at + 16:
            ms, ls = struct.unpack_from("<II", b, at + 8)
            return (ms >> 16, ms & 0xFFFF, ls >> 16, ls & 0xFFFF)
        at = b.find(sig, at + 1)
    return None


def data_dir(game):
    return find_ci(game, "Data")


def mpq_names(game):
    d = data_dir(game)
    try:
        return sorted(e for e in os.listdir(d) if e.lower().endswith(".mpq")) if d else []
    except OSError:
        return []


def first_era(game):
    """(is it, how we know, the program's version or None)."""
    for n in EXE_NAMES:
        p = find_ci(game, n)
        if p:
            v = pe_version(p)
            if v:
                return v[3] == BUILD and v[0] == 1, "%s says build %d" % (p.name, v[3]), v
    names = {n.lower() for n in mpq_names(game)}
    if any(n.lower() in names for n in LATER_ERA_DATA):
        return False, "its archives are a later era's", None
    if all(n.lower() in names for n in FIRST_ERA_DATA):
        return True, "its archives are the first era's (no version in the program)", None
    return False, "no game program and not the first era's archives", None


# ------------------------------------------------------------------------------------------------ state
def state_path():
    return home() / "state.json"


def load_state():
    try:
        s = json.loads(state_path().read_text(encoding="utf-8"))
    except (OSError, ValueError):
        s = {}
    for k in ("root_pw", "db_pw", "solo_pw", "admin_pw"):
        if s.get(k):
            _SECRETS.append(s[k])
    return s


def save_state(s):
    p = state_path()
    p.parent.mkdir(parents=True, exist_ok=True)
    tmp = p.with_suffix(".tmp")
    tmp.write_text(json.dumps(s, indent=2), encoding="utf-8")
    if os.name != "nt":
        os.chmod(tmp, 0o600)
    os.replace(tmp, p)


def new_password(n=16):
    # upper case letters and digits: the game sends account passwords upper-cased, at most 16 characters
    return "".join(secrets.choice("ABCDEFGHJKLMNPQRSTUVWXYZ23456789") for _ in range(n))


# ------------------------------------------------------------------------------------------------ SRP6 (accounts)
def srp6_row(user, password, salt=None):
    """(s hex, v hex) as CMaNGOS stores them (AccountMgr::CreateAccount: SRP6::CalculateVerifier). The salt is 32
    random bytes with a non-zero top byte, so its byte form is the same whichever width a program uses."""
    if salt is None:
        while True:
            salt = int.from_bytes(secrets.token_bytes(32), "big")
            if salt >> 248:
                break
    h = hashlib.sha1((user.upper() + ":" + password.upper()).encode("utf-8")).digest()
    sb = salt.to_bytes((salt.bit_length() + 7) // 8, "little")
    x = int.from_bytes(hashlib.sha1(sb + h).digest(), "little")
    v = pow(SRP_G, x, SRP_N)

    def hx(n):
        t = "%X" % n
        return t if len(t) % 2 == 0 else "0" + t
    return hx(salt), hx(v)


# ------------------------------------------------------------------------------------------------ ports, disk
def port_free(port, host="127.0.0.1"):
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        s.bind((host, port))
        return True
    except OSError:
        return False
    finally:
        s.close()


def port_open(port, host="127.0.0.1", timeout=1.0):
    try:
        with socket.create_connection((host, port), timeout=timeout):
            return True
    except OSError:
        return False


def free_gb(p):
    p = Path(p)
    while not p.exists() and p.parent != p:
        p = p.parent
    try:
        return shutil.disk_usage(str(p)).free / 1e9
    except OSError:
        return 0.0


# ------------------------------------------------------------------------------------------------ check
def cmd_check(a):
    r = Report()
    game, where = game_folder(a.game)
    if not game:
        r.add("FAIL", "game folder", "none given",
              "put your first-era game folder's path in tools\\solo\\folder.txt, or add it in the launcher's Game folders")
        return 1
    r.add("INFO", "game folder", "%s (from %s)" % (game, where))
    if not game.is_dir():
        r.add("FAIL", "game folder", "not a folder on this PC", "check the path")
        return 1
    ok, how, _v = first_era(game)
    r.add("PASS" if ok else "FAIL", "first-era client (build %d)" % BUILD, how,
          "" if ok else "pick the folder of your first-era game (it has Data\\dbc.MPQ and Data\\terrain.MPQ)")
    names = mpq_names(game)
    r.add("INFO", "archives in Data", ", ".join(names) or "none")
    slot = find_ci(data_dir(game) or game, a.slot) if data_dir(game) else None
    if slot:
        mine = is_ours(slot)
        r.add("PASS" if mine else "WARN", "the sign-in slot " + a.slot,
              "ours (from an earlier solo-signin)" if mine else "taken by an archive that isn't ours (never touched)",
              "" if mine else "solo-signin can use another free slot: --slot patch-Y.MPQ")
    else:
        r.add("PASS", "the sign-in slot " + a.slot, "free")
    toc, src = player_toc(game, a.slot)
    r.add("PASS" if toc else "WARN", "the login screen's file list", ("read from your " + src) if toc else src,
          "" if toc else "solo-signin can still prefill the account name (the fallback); the add-on needs this list")
    for n in ("realmlist.wtf",):
        p = find_ci(game, n)
        r.add("INFO", n, scrub_realmlist(p) if p else "not there (solo-signin writes one, and takes it away again)")
    cfg = find_ci(find_ci(game, "WTF") or game, "Config.wtf") if find_ci(game, "WTF") else None
    r.add("INFO", "WTF\\Config.wtf", "there" if cfg else "not there yet (the game makes it on its first start)")
    gb = free_gb(home())
    r.add("PASS" if gb >= NEED_GB else "FAIL", "free disk where the world goes", "%.1f GB at %s" % (gb, home()),
          "" if gb >= NEED_GB else "free up %d GB (the engine, your maps, the database)" % NEED_GB)
    st = load_state()
    running = is_running(st)
    for what, port in PORTS.items():
        if running.get(what):
            r.add("PASS", "port %d (%s)" % (port, what), "in use by the Solo world (running)")
        else:
            free = port_free(port)
            r.add("PASS" if free else "FAIL", "port %d (%s)" % (port, what), "free" if free else "in use by another program",
                  "" if free else "close the program that listens on %d (another server?)" % port)
    r.add("INFO", "set up", "yes (%s)" % st.get("engine_version", "?") if st.get("setup_done") else "not yet (press solo-setup)")
    r.add("INFO", "python", sys.version.split()[0])
    out("CHECK %s" % ("OK" if r.ok else "FAILED"))
    return 0 if r.ok else 1


def scrub_realmlist(p):
    try:
        t = p.read_text(encoding="latin-1").strip().splitlines()
        return "; ".join(x.strip() for x in t[:2]) or "empty"
    except OSError:
        return "unreadable"


# ------------------------------------------------------------------------------------------------ the engine
def download(url, dest):
    req = urllib.request.Request(url, headers={"User-Agent": "leyline-solo-proof"})
    with urllib.request.urlopen(req, timeout=60) as resp, open(dest, "wb") as f:
        total = int(resp.headers.get("Content-Length") or 0)
        got, last = 0, 0.0
        while True:
            b = resp.read(1 << 20)
            if not b:
                break
            f.write(b)
            got += len(b)
            if time.monotonic() - last > 10:
                last = time.monotonic()
                out("  downloading: %d MB%s" % (got >> 20, (" of %d MB" % (total >> 20)) if total else ""))


def sha256_file(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def get_engine(st, zip_path=None):
    """The engine zip (downloaded and checked, or the one given), unpacked into <home>/engine. Returns its json."""
    h = home()
    h.mkdir(parents=True, exist_ok=True)
    if zip_path:
        z = Path(zip_path)
        want = sha256_file(z)
        sidecar = Path(str(z) + ".sha256")
        if sidecar.is_file():
            listed = sidecar.read_text().split()[0].lower()
            if listed != want:
                raise SystemExit("FAIL the engine zip does not match its .sha256")
    else:
        z = h / ZIP
        sumf = h / (ZIP + ".sha256")
        out("== the engine: %s" % RELEASE)
        download(RELEASE + ZIP + ".sha256", sumf)
        want = sumf.read_text().split()[0].lower()
        if not re.fullmatch(r"[0-9a-f]{64}", want):
            raise SystemExit("FAIL the release's .sha256 doesn't look right")
        if not z.is_file() or sha256_file(z) != want:
            download(RELEASE + ZIP, z)
        if sha256_file(z) != want:
            raise SystemExit("FAIL the download does not match its .sha256 (try solo-setup again)")
    eng = h / "engine"
    if st.get("engine_sha256") == want and (eng / "solo-engine.json").is_file():
        out("  the engine is already unpacked (%s)" % want[:12])
    else:
        if eng.exists():
            shutil.rmtree(eng)
        tmp = h / "engine.tmp"
        if tmp.exists():
            shutil.rmtree(tmp)
        with zipfile.ZipFile(z) as zf:
            for m in zf.infolist():
                target = (tmp / m.filename).resolve()
                if not str(target).startswith(str(tmp.resolve())):
                    raise SystemExit("FAIL the zip has a path outside its folder: %s" % m.filename)
            zf.extractall(tmp)
        os.replace(tmp, eng)
        out("  unpacked into %s" % eng)
    st["engine_sha256"] = want
    info = json.loads((eng / "solo-engine.json").read_text(encoding="utf-8"))
    st["engine_version"] = info.get("version", "?")
    return info


def exe(p):
    return str(p) + (".exe" if os.name == "nt" and not str(p).lower().endswith(".exe") else "")


def run_logged(args, log, cwd=None, what=""):
    """Run a program to its end with its output in `log`; a line every 30 s says it is still going."""
    t0 = time.monotonic()
    with open(log, "ab") as lf:
        p = subprocess.Popen(args, cwd=cwd, stdout=lf, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
        last = t0
        while p.poll() is None:
            time.sleep(1)
            if time.monotonic() - last > 30:
                last = time.monotonic()
                out("  %s: still going (%d min)" % (what, (last - t0) // 60))
    mins = (time.monotonic() - t0) / 60
    return p.returncode, mins


def extract(st, game, mmaps):
    eng, data, logs = home() / "engine", home() / "data", home() / "logs"
    data.mkdir(parents=True, exist_ok=True)
    logs.mkdir(parents=True, exist_ok=True)
    tools = eng / "tools"
    if not (data / ".maps-done").is_file():
        out("== dbc and maps from your own game folder (a few minutes)")
        rc, m = run_logged([exe(tools / "ad"), "-i", str(game), "-o", str(data)], logs / "extract-maps.log", cwd=str(data),
                           what="maps")
        if rc != 0:
            raise SystemExit("FAIL the map extractor stopped (exit %d): logs\\extract-maps.log" % rc)
        (data / ".maps-done").write_text("ok")
        out("  maps done in %.0f min" % m)
    if not (data / "vmaps" / ".done").is_file():
        out("== vmaps (line of sight; 10-30 minutes)")
        shutil.rmtree(data / "Buildings", ignore_errors=True)
        (data / "vmaps").mkdir(parents=True, exist_ok=True)
        gd = data_dir(game) or (game / "Data")
        rc, m1 = run_logged([exe(tools / "vmap_extractor"), "-d", str(gd), "-o", str(data)], logs / "extract-vmaps.log",
                            cwd=str(data), what="vmaps")
        if rc != 0:
            raise SystemExit("FAIL the vmap extractor stopped (exit %d): logs\\extract-vmaps.log" % rc)
        rc, m2 = run_logged([exe(tools / "vmap_assembler"), str(data / "Buildings"), str(data / "vmaps")],
                            logs / "extract-vmaps.log", cwd=str(data), what="vmaps")
        if rc != 0:
            raise SystemExit("FAIL the vmap assembler stopped (exit %d): logs\\extract-vmaps.log" % rc)
        shutil.rmtree(data / "Buildings", ignore_errors=True)
        (data / "vmaps" / ".done").write_text("ok")
        out("  vmaps done in %.0f min" % (m1 + m2))
    if mmaps and not (data / "mmaps" / ".done").is_file():
        out("== monster paths (the slow part: 1-3 hours on all CPU threads)")
        (data / "mmaps").mkdir(parents=True, exist_ok=True)
        rc, m = run_logged([exe(tools / "MoveMapGen"), "--silent", "--configInputPath", str(tools / "config.json"),
                            "--offMeshInput", str(tools / "offmesh.txt"), "--workdir", str(data) + os.sep,
                            "--buildGameObjects", "--threads", str(os.cpu_count() or 2)],
                           logs / "extract-mmaps.log", cwd=str(tools), what="monster paths")
        if rc != 0:
            raise SystemExit("FAIL building the monster paths stopped (exit %d); run solo-setup --mmaps again to finish"
                             % rc)
        (data / "mmaps" / ".done").write_text("ok")
        out("  monster paths done in %.0f min" % m)
    elif not mmaps:
        out("== monster paths: skipped (creatures walk straight at you; solo-setup --mmaps adds them, 1-3 hours)")
    st["mmaps"] = (data / "mmaps" / ".done").is_file()


# ------------------------------------------------------------------------------------------------ the database
def fwd(p):
    return str(p).replace("\\", "/")


def db_files(st, eng=None):
    h = home()
    eng = Path(eng) if eng else h / "engine"
    run = h / "run"
    run.mkdir(parents=True, exist_ok=True)
    (h / "logs").mkdir(parents=True, exist_ok=True)
    ini = run / "my.ini"
    ini.write_text("\n".join([
        "[mysqld]", "basedir=" + fwd(eng / "mysql"), "datadir=" + fwd(h / "db"), "port=%d" % PORTS["database"],
        "bind-address=127.0.0.1", "mysqlx=OFF", "skip-name-resolve", "skip-log-bin",
        "sql_mode=NO_ENGINE_SUBSTITUTION", "max_allowed_packet=256M", "log-error=" + fwd(h / "logs" / "mysqld.err"), ""]),
        encoding="ascii")
    cnf = run / "client.cnf"
    cnf.write_text("[client]\nuser=root\npassword=%s\nhost=127.0.0.1\nport=%d\n" % (st["root_pw"], PORTS["database"]),
                   encoding="ascii")
    if os.name != "nt":
        os.chmod(cnf, 0o600)
    return eng, ini, cnf


def mysql(st, sql=None, db=None, stdin_path=None, eng=None, check=True):
    eng, _ini, cnf = db_files(st, eng)
    args = [exe(eng / "mysql" / "bin" / "mysql"), "--defaults-extra-file=" + str(cnf), "-N", "-B",
            "--default-character-set=utf8mb4"]
    if db:
        args.append(db)
    if stdin_path:
        with open(stdin_path, "rb") as f:
            p = subprocess.run(args, stdin=f, capture_output=True)
    else:
        # the statements go in on stdin, never on a command line (some hold a password)
        p = subprocess.run(args, input=(sql or "").encode("utf-8"), capture_output=True)
    text = scrub(p.stdout.decode("utf-8", "replace"))
    if check and p.returncode != 0:
        raise SystemExit("FAIL the database said: %s" % scrub(p.stderr.decode("utf-8", "replace")).strip()[:400])
    return text


def db_start(st, eng=None):
    if port_open(PORTS["database"]):
        return None
    eng, ini, _ = db_files(st, eng)
    p = spawn([exe(eng / "mysql" / "bin" / "mysqld"), "--defaults-file=" + str(ini)], home() / "logs" / "mysqld.out",
              cwd=str(home()))
    for _ in range(90):
        if port_open(PORTS["database"]):
            return p
        if p.poll() is not None:
            break
        time.sleep(1)
    raise SystemExit("FAIL the database did not start: %s" % (home() / "logs" / "mysqld.err"))


def db_stop(st, eng=None):
    if not port_open(PORTS["database"]):
        return
    eng, _ini, cnf = db_files(st, eng)
    subprocess.run([exe(eng / "mysql" / "bin" / "mysqladmin"), "--defaults-extra-file=" + str(cnf), "shutdown"],
                   capture_output=True, stdin=subprocess.DEVNULL)
    for _ in range(60):
        if not port_open(PORTS["database"]):
            return
        time.sleep(1)


def db_init(st, eng=None):
    """A database server of its own in <home>/db, its root password generated; the package's dump imported; the
    core's own login (db_pw) for the server programs; the accounts SOLO (to play) and SOLOSTOP (to stop it cleanly)."""
    h = home()
    for k in ("root_pw", "db_pw", "solo_pw", "admin_pw"):
        if not st.get(k):
            st[k] = new_password(20 if k in ("root_pw", "db_pw") else 16)
        _SECRETS.append(st[k])
    save_state(st)
    eng, ini, _cnf = db_files(st, eng)
    dbdir = h / "db"
    if not (dbdir / "mysql").is_dir():
        if dbdir.exists():
            shutil.rmtree(dbdir)
        out("== a database server of its own (%s)" % dbdir)
        init = h / "run" / "init.sql"
        init.write_text(("CREATE USER IF NOT EXISTS 'root'@'127.0.0.1' IDENTIFIED BY '{p}'; GRANT ALL PRIVILEGES ON *.* "
                         "TO 'root'@'127.0.0.1' WITH GRANT OPTION; ALTER USER 'root'@'localhost' IDENTIFIED BY '{p}';\n")
                        .format(p=st["root_pw"]), encoding="ascii")
        try:
            p = subprocess.run([exe(eng / "mysql" / "bin" / "mysqld"), "--defaults-file=" + str(ini), "--initialize-insecure",
                                "--init-file=" + fwd(init), "--console"], capture_output=True, stdin=subprocess.DEVNULL)
        finally:
            init.unlink(missing_ok=True)
        if p.returncode != 0:
            raise SystemExit("FAIL the database could not be made (%s)" % (h / "logs" / "mysqld.err"))
        st["db_imported"] = False
        save_state(st)
    started = db_start(st, eng)
    try:
        if not st.get("db_imported"):
            dump = eng / "sql" / "solo-databases.sql"
            out("== the world, characters and login databases (one import: 5-15 minutes)")
            t0 = time.monotonic()
            mysql(st, stdin_path=dump, eng=eng)
            out("  imported in %.0f min" % ((time.monotonic() - t0) / 60))
            st["db_imported"] = True
            save_state(st)
        grants = "".join("GRANT ALL PRIVILEGES ON `%s`.* TO 'solo'@'127.0.0.1'; " % d for d in DBS)
        mysql(st, "CREATE USER IF NOT EXISTS 'solo'@'127.0.0.1' IDENTIFIED BY '%s'; ALTER USER 'solo'@'127.0.0.1' "
                  "IDENTIFIED BY '%s'; %s FLUSH PRIVILEGES;" % (st["db_pw"], st["db_pw"], grants), eng=eng)
        make_accounts(st, eng)
        mysql(st, "DELETE FROM realmlist WHERE id <> 1; INSERT INTO realmlist (id, name, address, port, icon, realmflags, "
                  "timezone, allowedSecurityLevel, population, realmbuilds) VALUES (1, '%s', '127.0.0.1', %d, 0, 0, 1, 0, 0, "
                  "'%d') ON DUPLICATE KEY UPDATE name = VALUES(name), address = '127.0.0.1', port = VALUES(port), "
                  "realmbuilds = VALUES(realmbuilds)" % (REALM, PORTS["world"], BUILD), db="classicrealmd", eng=eng)
        n = mysql(st, "SELECT COUNT(*) FROM creature_template", db="classicmangos", eng=eng).strip()
        out("  world database: %s creature templates" % n)
        if not n.isdigit() or int(n) < 1000:
            raise SystemExit("FAIL the world database looks empty")
    finally:
        if started is not None:
            db_stop(st, eng)


def make_accounts(st, eng=None):
    """SOLO (to play, game master level 0) and SOLOSTOP (level 3, only for the clean stop over SOAP on 127.0.0.1),
    their verifiers made here exactly as the core makes them; set again on every setup."""
    for user, pw, gm in ((ACCOUNT, st["solo_pw"], 0), (ADMIN, st["admin_pw"], 3)):
        s, v = srp6_row(user, pw)
        mysql(st, "INSERT INTO account (username, gmlevel, v, s, joindate) VALUES ('%s', %d, '%s', '%s', NOW()) "
                  "ON DUPLICATE KEY UPDATE gmlevel = VALUES(gmlevel), v = VALUES(v), s = VALUES(s), locked = 0, "
                  "failed_logins = 0" % (user, gm, v, s), db="classicrealmd", eng=eng)
    mysql(st, "INSERT INTO realmcharacters (realmid, acctid, numchars) SELECT realmlist.id, account.id, 0 FROM realmlist, "
              "account LEFT JOIN realmcharacters ON acctid = account.id WHERE acctid IS NULL", db="classicrealmd", eng=eng)


# ------------------------------------------------------------------------------------------------ server settings
KEY_RE = re.compile(r"^([A-Za-z][A-Za-z0-9_.]*)\s*=")


def overrides(text):
    o = {}
    for line in text.splitlines():
        m = KEY_RE.match(line)
        if m:
            o[m.group(1)] = line.split("=", 1)[1].strip()
    return o


def apply_conf(dist_text, *layers):
    """The core's .conf.dist with every "Key = Value" of each layer put in its place (or added at the end), as the
    host kit's run-server.sh does."""
    o = {}
    for layer in layers:
        o.update(layer)
    seen, lines = set(), []
    for line in dist_text.splitlines():
        m = KEY_RE.match(line)
        if m and m.group(1) in o:
            lines.append("%s = %s" % (m.group(1), o[m.group(1)]))
            seen.add(m.group(1))
        else:
            lines.append(line)
    for k, v in o.items():
        if k not in seen:
            lines.append("%s = %s" % (k, v))
    return "\n".join(lines) + "\n"


def write_confs(st, eng=None):
    h = home()
    eng = Path(eng) if eng else h / "engine"
    run, dist, conf = h / "run", eng / "bin" / "conf-dist", eng / "conf"
    run.mkdir(parents=True, exist_ok=True)

    def info(db):
        return '"127.0.0.1;%d;solo;%s;%s"' % (PORTS["database"], st["db_pw"], db)
    rd = lambda p: p.read_text(encoding="utf-8", errors="replace")  # noqa: E731
    mangosd = {"DataDir": '"%s"' % fwd(h / "data"), "LogsDir": '"%s"' % fwd(h / "logs"), "PidFile": '""',
               "LoginDatabaseInfo": info("classicrealmd"), "WorldDatabaseInfo": info("classicmangos"),
               "CharacterDatabaseInfo": info("classiccharacters"), "LogsDatabaseInfo": info("classiclogs"),
               "mmap.enabled": "1" if st.get("mmaps") else "0",
               "SOAP.Enabled": "1", "SOAP.IP": '"127.0.0.1"', "SOAP.Port": str(PORTS["soap"]),
               "WorldServerPort": str(PORTS["world"]), "BindIP": '"127.0.0.1"'}
    realmd = {"LogsDir": '"%s"' % fwd(h / "logs"), "PidFile": '""', "LoginDatabaseInfo": info("classicrealmd"),
              "RealmServerPort": str(PORTS["login"]), "BindIP": '"127.0.0.1"'}
    files = {
        "mangosd.conf": apply_conf(rd(dist / "mangosd.conf.dist"), overrides(rd(conf / "solo.conf")), mangosd),
        "realmd.conf": apply_conf(rd(dist / "realmd.conf.dist"), overrides(rd(conf / "solo-realmd.conf")), realmd),
        "aiplayerbot.conf": apply_conf(rd(dist / "aiplayerbot.conf.dist"), overrides(rd(conf / "solo-aiplayerbot.conf"))),
        "anticheat.conf": apply_conf(rd(dist / "anticheat.conf.dist"), overrides(rd(conf / "solo-anticheat.conf"))),
    }
    for name, text in files.items():
        p = run / name
        p.write_text(text, encoding="utf-8")
        if os.name != "nt":
            os.chmod(p, 0o600)


# ------------------------------------------------------------------------------------------------ setup
def cmd_setup(a):
    st = load_state()
    game, where = game_folder(a.game)
    if not game or not game.is_dir():
        out("FAIL no first-era game folder (tools\\solo\\folder.txt, or the launcher's Game folders)")
        return 1
    ok, how, _ = first_era(game)
    if not ok:
        out("FAIL %s is not a first-era client: %s" % (game, how))
        return 1
    if is_running(st).get("world") or is_running(st).get("login"):
        out("FAIL the Solo world is running: press solo-stop first")
        return 1
    out("game folder: %s (from %s; read only, nothing in it is changed by setup)" % (game, where))
    t0 = time.monotonic()
    get_engine(st, a.zip)
    save_state(st)
    if not a.no_extract:
        extract(st, game, a.mmaps)
    save_state(st)
    db_init(st)
    write_confs(st)
    st.update({"setup_done": True, "game": str(game), "realm": REALM, "account": ACCOUNT})
    save_state(st)
    out("SETUP OK in %.0f min: the local account %s is made (its password is kept in %s, never shown)."
        % ((time.monotonic() - t0) / 60, ACCOUNT, state_path()))
    out("next: solo-start, then solo-signin")
    return 0


# ------------------------------------------------------------------------------------------------ start / stop
def spawn(args, log, cwd=None):
    """A program that keeps running after this tool ends: its own process group, no console window, output to `log`,
    nothing inherited (Control waits for this tool's output to end, not for the servers)."""
    flags = 0
    if os.name == "nt":
        flags = 0x00000008 | 0x00000200 | 0x08000000      # DETACHED_PROCESS, CREATE_NEW_PROCESS_GROUP, NO_WINDOW
        try:
            return subprocess.Popen(args, cwd=cwd, stdout=open(log, "ab"), stderr=subprocess.STDOUT,
                                    stdin=subprocess.DEVNULL, close_fds=True, creationflags=flags | 0x01000000)  # BREAKAWAY
        except OSError:
            pass
        return subprocess.Popen(args, cwd=cwd, stdout=open(log, "ab"), stderr=subprocess.STDOUT,
                                stdin=subprocess.DEVNULL, close_fds=True, creationflags=flags)
    return subprocess.Popen(args, cwd=cwd, stdout=open(log, "ab"), stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
                            close_fds=True, start_new_session=True)


def pid_alive(pid):
    if not pid:
        return False
    if os.name == "nt":
        import ctypes
        k = ctypes.windll.kernel32
        h = k.OpenProcess(0x1000, False, int(pid))         # PROCESS_QUERY_LIMITED_INFORMATION
        if not h:
            return False
        try:
            code = ctypes.c_ulong()
            return bool(k.GetExitCodeProcess(h, ctypes.byref(code))) and code.value == 259    # STILL_ACTIVE
        finally:
            k.CloseHandle(h)
    try:
        os.kill(int(pid), 0)
        return True
    except OSError:
        return False


def kill_pid(pid):
    if not pid_alive(pid):
        return
    if os.name == "nt":
        subprocess.run(["taskkill", "/F", "/PID", str(pid)], capture_output=True)
    else:
        try:
            os.kill(int(pid), 15)
        except OSError:
            pass


def is_running(st):
    pids = st.get("pids") or {}
    r = {k: pid_alive(pids.get(k)) for k in ("database", "login", "world")}
    r["soap"] = r["world"]
    return r


def world_log():
    return home() / "logs" / "Server.log"


def ready_seen():
    for p in (world_log(), home() / "logs" / "mangosd.out"):
        try:
            if READY in p.read_text(encoding="utf-8", errors="replace"):
                return True
        except OSError:
            pass
    return False


def tail(p, n=12):
    try:
        lines = p.read_text(encoding="utf-8", errors="replace").splitlines()[-n:]
    except OSError:
        return
    for line in lines:
        out("  | " + line)


def cmd_start(a):
    st = load_state()
    if not st.get("setup_done"):
        out("FAIL not set up yet: press solo-setup first")
        return 1
    h = home()
    eng = h / "engine"
    write_confs(st)
    run = is_running(st)
    pids = st.setdefault("pids", {})
    for what in ("login", "world"):
        if not run.get(what) and not port_free(PORTS[what]):
            out("FAIL port %d (%s) is in use by another program" % (PORTS[what], what))
            return 1
    if not run.get("database"):
        p = db_start(st)
        if p is not None:
            pids["database"] = p.pid
    save_state(st)
    if not run.get("login"):
        pids["login"] = spawn([exe(eng / "bin" / "realmd"), "-c", str(h / "run" / "realmd.conf")], h / "logs" / "realmd.out",
                              cwd=str(h / "run")).pid
    if not run.get("world"):
        for p in (world_log(), h / "logs" / "mangosd.out"):
            try:
                p.unlink()
            except OSError:
                pass
        pids["world"] = spawn([exe(eng / "bin" / "mangosd"), "-c", str(h / "run" / "mangosd.conf"),
                               "-p", str(h / "run" / "aiplayerbot.conf")], h / "logs" / "mangosd.out", cwd=str(h / "run")).pid
    save_state(st)
    out("== starting (the world server takes a minute or two the first time)")
    deadline = time.monotonic() + (a.wait or 600)
    while time.monotonic() < deadline:
        if ready_seen() and port_open(PORTS["world"]) and port_open(PORTS["login"]):
            out("START OK: the Solo world is up on 127.0.0.1 (login %d, world %d). Next: solo-signin, then open your game."
                % (PORTS["login"], PORTS["world"]))
            return 0
        if not pid_alive(pids.get("world")):
            out("FAIL the world server stopped. The end of its log:")
            tail(world_log())
            tail(h / "logs" / "mangosd.out")
            return 1
        time.sleep(3)
    out("FAIL the world server did not say \"%s\" in time. The end of its log:" % READY)
    tail(world_log())
    return 1


def soap(user, password, command, timeout=15):
    """One console command through the world server's SOAP port on 127.0.0.1 (urn:MaNGOS). Returns (ok, text)."""
    import base64
    from xml.sax.saxutils import escape
    body = ('<?xml version="1.0" encoding="utf-8"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/'
            'envelope/" xmlns:ns1="urn:MaNGOS"><SOAP-ENV:Body><ns1:executeCommand><command>' + escape(command)
            + '</command></ns1:executeCommand></SOAP-ENV:Body></SOAP-ENV:Envelope>').encode()
    auth = base64.b64encode(("%s:%s" % (user, password)).encode()).decode()
    req = urllib.request.Request("http://127.0.0.1:%d/" % PORTS["soap"], data=body, method="POST",
                                 headers={"Content-Type": "text/xml", "Authorization": "Basic " + auth})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            text = resp.read().decode("utf-8", "replace")
    except OSError as e:
        return False, scrub(str(e))
    return "<faultstring>" not in text, scrub(re.sub(r"<[^>]+>", " ", text)).strip()[:200]


def cmd_stop(a):
    st = load_state()
    pids = st.get("pids") or {}
    if pid_alive(pids.get("world")):
        ok, why = soap(ADMIN, st.get("admin_pw", ""), "server shutdown 1")
        out("== the world server saves and stops%s" % ("" if ok else " (the clean stop didn't answer: %s)" % why))
        for _ in range(90 if ok else 0):
            if not pid_alive(pids.get("world")):
                break
            time.sleep(1)
        if pid_alive(pids.get("world")):
            out("  it didn't stop by itself: ended")
            kill_pid(pids.get("world"))
    if pid_alive(pids.get("login")):
        kill_pid(pids.get("login"))
    if st.get("setup_done"):
        db_stop(st)
    if pid_alive(pids.get("database")):
        kill_pid(pids.get("database"))
    st["pids"] = {}
    save_state(st)
    scrub_session(st)
    out("STOP OK: the Solo world is stopped.")
    return 0


def logon(user, password, port=None, timeout=15):
    """A full first-era (build 5875) logon (SRP6) and realm list request on 127.0.0.1. Returns the number of realms."""
    sys.path.insert(0, str(ROOT / "hosting" / "kit" / "tools"))
    from wunet import srp6, wire335
    s = socket.create_connection(("127.0.0.1", port or PORTS["login"]), timeout=timeout)

    def rd(n):
        b = b""
        while len(b) < n:
            c = s.recv(n - len(b))
            if not c:
                raise OSError("the login server closed the connection")
            b += c
        return b
    try:
        s.sendall(wire335.logon_challenge(user, BUILD))
        head = rd(3)
        if head[0] != 0x00 or head[2] != 0x00:
            raise OSError("logon challenge refused (code %#04x)" % head[2])
        B = rd(32)
        g = int.from_bytes(rd(rd(1)[0]), "little")
        N = int.from_bytes(rd(rd(1)[0]), "little")
        salt = rd(32)
        rd(16)
        if rd(1)[0]:
            raise OSError("the account wants extra security")
        c = srp6.Client(user, password)
        A, M1 = c.process(B, salt, g, N)
        s.sendall(bytes([0x01]) + A + M1 + bytes(20) + bytes([0, 0]))
        h = rd(2)
        if h[0] != 0x01 or h[1] != 0x00:
            raise OSError("logon proof refused (code %#04x): wrong password" % h[1])
        rest = rd(24)
        if not c.check_server(rest[:20]):
            raise OSError("the server's proof did not match")
        s.sendall(bytes([0x10, 0, 0, 0, 0]))
        h = rd(3)
        if h[0] != 0x10:
            raise OSError("unexpected reply %#x to the realm list request" % h[0])
        body = rd(struct.unpack_from("<H", h, 1)[0])
        return body[4] if len(body) > 4 else 0
    finally:
        s.close()


def cmd_status(a):
    st = load_state()
    run = is_running(st)
    out("set up: %s" % ("yes, engine %s, monster paths %s" % (st.get("engine_version"), "yes" if st.get("mmaps") else "no")
                        if st.get("setup_done") else "no (press solo-setup)"))
    for what in ("database", "login", "world"):
        out("%-9s %s, port %d %s" % (what, "running" if run.get(what) else "stopped", PORTS[what],
                                    "open" if port_open(PORTS[what]) else "closed"))
    ready = ready_seen()
    out("world server says \"%s\": %s" % (READY, "yes" if ready else "no"))
    if run.get("login") and st.get("solo_pw"):
        try:
            n = logon(ACCOUNT, st["solo_pw"])
            out("sign-in check: %s signs in on the login server, %d realm(s) listed" % (ACCOUNT, n))
        except (OSError, ImportError) as e:
            out("sign-in check: FAILED (%s)" % scrub(str(e)))
    si = st.get("signin") or {}
    out("signin: %s" % ("on (%s)" % si.get("game") if si.get("on") else "off"))
    if a.log:
        tail(world_log(), 30)
    out("STATUS %s" % ("UP" if run.get("world") and ready else "DOWN"))
    return 0


# ------------------------------------------------------------------------------------------------ sign-in
def lua_bytes(s):
    return '"' + "".join("\\%03d" % b for b in s.encode("utf-8")) + '"'


def char_name_ok(n):
    return bool(re.fullmatch(r"[A-Z][a-z]{1,11}", n or ""))


def session_lua(user, password, realm, char=None):
    """LeylineSoloSession.lua: the sign-in as escaped bytes (nothing of it readable as plain text)."""
    if not re.fullmatch(r"[A-Z0-9]{1,16}", user or "") or not re.fullmatch(r"[A-Z0-9]{1,16}", password or ""):
        raise ValueError("that sign-in can't be handed to the game")
    extra = ""
    if char:
        if not char_name_ok(char):
            raise ValueError("a character's name is 2 to 12 letters, the first upper case")
        extra = ", char = " + lua_bytes(char)
    return ("-- Leyline Solo: the sign-in to this PC's own world, written by solo_proof.py signin, taken out by "
            "solo-stop and solo-signin --off.\r\nLEYLINE_SOLO_SESSION = { user = %s, pass = %s, realm = %s%s }\r\n"
            % (lua_bytes(user), lua_bytes(password), lua_bytes(realm), extra))


def is_ours(path):
    try:
        if Path(path).stat().st_size > OURS_MAX:
            return False
        return mpq112.Archive(str(path)).has(OUR_LUA)
    except (OSError, mpq112.MpqError):
        return False


def slot_order(names, ours):
    """The player's archives that can hold the login screen's file list, the one the game reads last first."""
    def prio(n):
        m = re.fullmatch(r"patch(?:-(.))?\.mpq", n.lower())
        if m:
            c = m.group(1)
            return (2, 0 if c is None else (1 if c.isdigit() else 2), c or "")
        return (1 if n.lower() == "interface.mpq" else 0, 0, n.lower())
    cand = [n for n in names if n.lower() != ours.lower() and (n.lower() == "interface.mpq" or n.lower().startswith("patch"))]
    return sorted(cand, key=prio, reverse=True)


def player_toc(game, ours=SLOT):
    """(text, where) of Interface\\GlueXML\\GlueXML.toc from the player's own archives (the last one read wins), or
    (None, why not)."""
    d = data_dir(game)
    if not d:
        return None, "no Data folder"
    why = "no archive of yours holds it"
    for n in slot_order(mpq_names(game), ours):
        try:
            a = mpq112.Archive(str(d / n))
            if a.has(TOC):
                return a.read(TOC).decode("latin-1"), n
        except (OSError, mpq112.MpqError) as e:
            why = "%s could not be read here (%s)" % (n, e)
    return None, why


def toc_with_ours(toc):
    lines = toc.replace("\r\n", "\n").split("\n")
    while lines and lines[-1] == "":
        lines.pop()
    if not any(x.strip().lower() == "leylinesolo.xml" for x in lines):
        lines.append("LeylineSolo.xml")
    return ("\r\n".join(lines) + "\r\n").encode("latin-1")


def solo_archive(toc_bytes, session):
    files = [(TOC, toc_bytes),
             (OUR_XML, (HERE / "glue" / "LeylineSolo.xml").read_bytes()),
             (OUR_LUA, (HERE / "glue" / "LeylineSolo.lua").read_bytes()),
             (SESSION, session.encode("ascii"))]
    return mpq112.build(files)


def set_wtf_line(text, key, value):
    """Config.wtf with `SET key "value"` (replaced where it is, else added), keeping its line ends."""
    nl = "\r\n" if "\r\n" in text or not text else "\n"
    lines = text.splitlines()
    pat = re.compile(r"^\s*SET\s+%s\s" % re.escape(key), re.I)
    new = 'SET %s "%s"' % (key, value)
    done = False
    for i, line in enumerate(lines):
        if pat.match(line):
            lines[i] = new
            done = True
    if not done:
        lines.append(new)
    return nl.join(lines) + nl


def backup_dir(st):
    return home() / "signin-backup"


def write_bytes(p, b):
    p.parent.mkdir(parents=True, exist_ok=True)
    tmp = p.with_name(p.name + ".leyline-tmp")
    tmp.write_bytes(b)
    os.replace(tmp, p)


def cmd_signin(a):
    st = load_state()
    if a.off:
        return signin_off(st)
    game, where = game_folder(a.game)
    if not game or not game.is_dir():
        out("FAIL no first-era game folder (tools\\solo\\folder.txt, or the launcher's Game folders)")
        return 1
    ok, how, _ = first_era(game)
    if not ok:
        out("FAIL %s is not a first-era client: %s" % (game, how))
        return 1
    if not st.get("setup_done") or not st.get("solo_pw"):
        out("FAIL not set up yet: press solo-setup first")
        return 1
    si = st.get("signin") or {}
    if si.get("on") and Path(si.get("game", "")) != game:
        out("FAIL sign-in is on for another folder (%s): press solo-signin-off first" % si.get("game"))
        return 1
    if a.char and not char_name_ok(a.char):
        out("FAIL a character's name is 2 to 12 letters, the first upper case")
        return 1
    slot_name = si.get("slot") or a.slot
    if not re.fullmatch(r"patch-[0-9A-Za-z]\.MPQ", slot_name, re.I):
        out("FAIL a slot is patch-<one letter or digit>.MPQ")
        return 1
    bdir = backup_dir(st)
    bdir.mkdir(parents=True, exist_ok=True)
    changed = si.get("changed") or {}        # relative path -> backup file name, or "" when it wasn't there
    added = si.get("added") or {}            # relative path -> sha256 of what we wrote

    def keep_original(rel):
        if rel in changed or rel in added:
            return
        p = game / rel
        if p.is_file():
            name = "%02d-%s" % (len(changed) + 1, re.sub(r"[^A-Za-z0-9.]", "_", rel))
            shutil.copy2(p, bdir / name)
            changed[rel] = name
        else:
            changed[rel] = ""

    # 1. realmlist.wtf (the game's root) -> this PC
    rl = find_ci(game, "realmlist.wtf")
    rel_rl = rl.name if rl else "realmlist.wtf"
    keep_original(rel_rl)
    write_bytes(game / rel_rl, b"set realmlist 127.0.0.1\r\n")
    out("realmlist.wtf: set realmlist 127.0.0.1 (the old one is kept and comes back with solo-signin-off)")
    # 2. WTF/Config.wtf: the account name and the realm, prefilled (the fallback when the add-on is ignored)
    wtf = find_ci(game, "WTF")
    cfg = find_ci(wtf, "Config.wtf") if wtf else None
    rel_cfg = "%s/%s" % (wtf.name if wtf else "WTF", cfg.name if cfg else "Config.wtf")
    keep_original(rel_cfg)
    text = (game / rel_cfg).read_text(encoding="latin-1") if (game / rel_cfg).is_file() else ""
    text = set_wtf_line(text, "accountName", ACCOUNT)
    text = set_wtf_line(text, "realmName", REALM)
    if re.search(r"^\s*SET\s+realmList\s", text, re.I | re.M):
        text = set_wtf_line(text, "realmList", "127.0.0.1")
    write_bytes(game / rel_cfg, text.encode("latin-1"))
    out("WTF\\Config.wtf: account name %s and realm \"%s\" prefilled" % (ACCOUNT, REALM))
    # 3. our sign-in add-on in a patch archive of ours
    pwf = password_file()
    write_bytes(pwf, ("Leyline Solo: the password of the local account %s on this PC's own world (127.0.0.1 only).\r\n"
                      "Type it once on the login screen if the game does not sign in by itself.\r\n\r\n%s\r\n"
                      % (ACCOUNT, st["solo_pw"])).encode("ascii"))
    out("the password, for typing it once if needed: open %s yourself (it is never shown here)" % pwf)
    if a.no_addon:
        out("sign-in add-on: not installed (--no-addon): the account name is prefilled, type the password once")
    else:
        d = data_dir(game)
        if not d:
            out("FAIL no Data folder in %s" % game)
            return save_signin(st, game, slot_name, changed, added, 1)
        slot = find_ci(d, slot_name)
        rel_slot = "%s/%s" % (d.name, slot.name if slot else slot_name)
        if slot and rel_slot not in added and not is_ours(slot):
            out("FAIL %s is taken by an archive that isn't ours: it is left as it is. Try --slot patch-Y.MPQ" % slot_name)
            return save_signin(st, game, slot_name, changed, added, 1)
        toc, src = player_toc(game, slot_name)
        if not toc:
            out("WARN the login screen's file list could not be read (%s): no add-on; the prefilled account name is the "
                "fallback (type the password once)" % src)
        else:
            try:
                session = session_lua(ACCOUNT, st["solo_pw"], REALM, a.char)
                b = solo_archive(toc_with_ours(toc), session)
                write_bytes(game / rel_slot, b)
            except OSError as e:
                out("FAIL could not write %s (is the game open? close it first): %s" % (rel_slot, e))
                return save_signin(st, game, slot_name, changed, added, 1)
            added[rel_slot] = hashlib.sha256(b).hexdigest()
            out("sign-in add-on: %s (our own login-screen add-on, after the file list from your %s; the sign-in for "
                "this PC's own world%s)" % (rel_slot.replace("/", "\\"), src, (", entering the world as " + a.char) if a.char else ""))
    rc = save_signin(st, game, slot_name, changed, added, 0)
    out("SIGNIN ON: open your game from %s. It should sign in by itself and land on character select." % game)
    return rc


def password_file():
    return home() / "SOLO password.txt"


def save_signin(st, game, slot, changed, added, rc):
    st["signin"] = {"on": True, "game": str(game), "slot": slot, "changed": changed, "added": added}
    save_state(st)
    return rc


def scrub_session(st):
    """solo-stop: the sign-in archive keeps the add-on but no password (written again by the next solo-signin)."""
    si = st.get("signin") or {}
    if not si.get("on"):
        return
    game = Path(si["game"])
    for rel, sha in list((si.get("added") or {}).items()):
        p = game / rel
        try:
            if p.is_file() and hashlib.sha256(p.read_bytes()).hexdigest() == sha:
                a = mpq112.Archive(str(p))
                b = solo_archive(a.read(TOC), NO_SESSION)
                write_bytes(p, b)
                si["added"][rel] = hashlib.sha256(b).hexdigest()
                out("signin: the password is taken out of %s (solo-signin writes it again)" % rel.replace("/", "\\"))
        except (OSError, mpq112.MpqError, KeyError) as e:
            out("WARN could not take the password out of %s (is the game open?): %s" % (rel, e))
    st["signin"] = si
    save_state(st)


def signin_off(st):
    si = st.get("signin") or {}
    if not si.get("on"):
        out("SIGNIN OFF: nothing of ours is there.")
        return 0
    game, bdir = Path(si["game"]), backup_dir(st)
    rc = 0
    for rel, sha in (si.get("added") or {}).items():
        p = game / rel
        try:
            if not p.exists():
                continue
            if hashlib.sha256(p.read_bytes()).hexdigest() == sha or is_ours(p):
                p.unlink()
                out("removed %s (ours)" % rel.replace("/", "\\"))
            else:
                out("WARN %s changed since we wrote it: left as it is" % rel.replace("/", "\\"))
        except OSError as e:
            out("FAIL could not remove %s (is the game open? close it first): %s" % (rel, e))
            rc = 1
    for rel, name in (si.get("changed") or {}).items():
        p = game / rel
        try:
            if name:
                write_bytes(p, (bdir / name).read_bytes())
                shutil.copystat(bdir / name, p)
                out("put back %s" % rel.replace("/", "\\"))
            elif p.exists():
                p.unlink()
                out("removed %s (it wasn't there before)" % rel.replace("/", "\\"))
        except OSError as e:
            out("FAIL could not put back %s: %s" % (rel, e))
            rc = 1
    if rc == 0:
        for name in (si.get("changed") or {}).values():
            if name:
                (bdir / name).unlink(missing_ok=True)
        st["signin"] = {"on": False}
        save_state(st)
        password_file().unlink(missing_ok=True)
        out("SIGNIN OFF: your game folder is as it was.")
    return rc


# ------------------------------------------------------------------------------------------------ CI smoke
def cmd_ci_smoke(a):
    """CI: the package's database set up from nothing exactly as on a player's PC, the login server started, and SOLO
    signed in with a full logon; then the passwords are looked for in every log (never there)."""
    os.environ["LEYLINE_SOLO_DIR"] = str(Path(a.work).resolve())
    h = home()
    if h.exists():
        shutil.rmtree(h)
    h.mkdir(parents=True)
    st = {}
    pkg = Path(a.pkg).resolve()
    db_init(st, eng=pkg)
    write_confs(st, eng=pkg)
    st["setup_done"] = True
    save_state(st)
    db = db_start(st, eng=pkg)
    login = spawn([exe(pkg / "bin" / "realmd"), "-c", str(h / "run" / "realmd.conf")], h / "logs" / "realmd.out",
                  cwd=str(h / "run"))
    try:
        for _ in range(60):
            if port_open(PORTS["login"]):
                break
            if login.poll() is not None:
                tail(h / "logs" / "realmd.out", 30)
                raise SystemExit("FAIL realmd stopped")
            time.sleep(1)
        n = logon(ACCOUNT, st["solo_pw"])
        out("realmd: %s signed in, %d realm(s)" % (ACCOUNT, n))
        if n < 1:
            raise SystemExit("FAIL no realm listed")
        try:
            logon(ACCOUNT, "WRONGPASSWORD1")
            raise SystemExit("FAIL a wrong password was taken")
        except OSError:
            out("realmd: a wrong password is refused")
        rows = mysql(st, "SELECT name, address, port, realmbuilds FROM realmlist", db="classicrealmd", eng=pkg).strip()
        out("realmlist: %s" % rows)
        pb = mysql(st, "SELECT COUNT(*) FROM ai_playerbot_texts", db="classicmangos", eng=pkg).strip()
        out("playerbots texts: %s" % pb)
    finally:
        login.kill()
        db_stop(st, eng=pkg)
        if db is not None:
            db.kill()
    for p in (h / "logs").rglob("*"):
        if p.is_file():
            t = p.read_bytes()
            for s in (st["root_pw"], st["db_pw"], st["solo_pw"], st["admin_pw"]):
                if s.encode() in t:
                    raise SystemExit("FAIL a password is in %s" % p.name)
    out("CI-SMOKE OK")
    return 0


# ------------------------------------------------------------------------------------------------ main
def main(argv=None):
    ap = argparse.ArgumentParser(prog="solo_proof.py", description="Leyline Solo, Phase 0 (docs/LEYLINE-SOLO-PHASE0.md)")
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("check")
    c.add_argument("game", nargs="?")
    c.add_argument("--slot", default=SLOT)
    c = sub.add_parser("setup")
    c.add_argument("game", nargs="?")
    c.add_argument("--mmaps", action="store_true", help="also build the monster paths (1-3 hours)")
    c.add_argument("--zip", help="a local engine zip instead of the download (its .sha256 beside it is checked)")
    c.add_argument("--no-extract", action="store_true", help=argparse.SUPPRESS)
    c = sub.add_parser("start")
    c.add_argument("--wait", type=int, default=600)
    sub.add_parser("stop")
    c = sub.add_parser("status")
    c.add_argument("--log", action="store_true")
    c = sub.add_parser("signin")
    c.add_argument("game", nargs="?")
    c.add_argument("--off", action="store_true")
    c.add_argument("--slot", default=SLOT)
    c.add_argument("--char", help="enter the world with this character once character select is up")
    c.add_argument("--no-addon", action="store_true", help="only realmlist.wtf and the prefilled account name")
    c = sub.add_parser("ci-smoke")
    c.add_argument("--pkg", required=True)
    c.add_argument("--work", required=True)
    a = ap.parse_args(argv)
    return {"check": cmd_check, "setup": cmd_setup, "start": cmd_start, "stop": cmd_stop, "status": cmd_status,
            "signin": cmd_signin, "ci-smoke": cmd_ci_smoke}[a.cmd](a)


if __name__ == "__main__":
    sys.exit(main())
