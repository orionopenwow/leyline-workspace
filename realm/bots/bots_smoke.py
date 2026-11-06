#!/usr/bin/env python3
r"""The bot smoke test's player (docs/BOTS-UPDATES.md). No game client: a headless 3.3.5a session (the kit's
wunet/wire335) logs in to a throwaway realm, and the realm's own console (SOAP) does what a GM would.

What it proves, in order (each a named check in the report):
  console        SOAP answers; the test account is made with the realm's own `account create` and made GM with
                 `account set gmlevel` (a random password, never printed, never written down)
  character      a character is made through the headless session
  world          it enters the world and the probe (ov_bots_probe.lua) answers
  compat_static  (bots-smoke.sh runs compat.py before this; listed in the report when given --static)
  addclass       `.playerbots bot addclass rogue`: one bot logs in and joins our group
  remove         `.playerbots bot remove <name>`: it leaves
  dungeon        `.go xyz` into a dungeon (The Deadmines, from ov_companions_dungeons.lua): we're on its map
  recruit        ov_companions.lua's `.recruit party`: 3+ bots join, on our map and instance, alive, we're their master
  compat_live    every mod-ale method ov_companions.lua needs is on a live bot (.ovsmoke compat)
  follow         we move 15 yards (a near teleport): the bots come after us
  fight          one practice mob attacks us: the bots fight it and kill it
  dismiss        ov_companions.lua's `.dismiss all`: the group has no bots left

    python3 -I bots_smoke.py smoke [--image ghcr.io/orionopenwow/wu-ac-wotlk] [--tag canary] [--playerbots-src DIR]
                                   [--ale-src DIR] [--data-volume ov-client-data] [--out r.json] [--keep]
        all of it: the throwaway realm (docker-compose.smoke.yml) from the images under test, the console's login,
        the player, then the realm removed. Needs Docker and the dbc + maps volume extracted from YOUR OWN game
        folder (realm\1-extract.bat makes ov-client-data); it is only read. Exit 3 without it. bots-smoke.sh and
        realm\Bots smoke test.bat (Control step bots-smoke) run this.
    python3 -I bots_smoke.py run --auth 127.0.0.1:13724 --world 127.0.0.1:13785 --soap 127.0.0.1:17878 [--out r.json]
        only the player, against a realm that is already up
        the SOAP login is SMOKE_SOAP_USER / SMOKE_SOAP_PASS (env; bots-smoke.sh makes it, see admin-sql)
    python3 -I bots_smoke.py admin-sql --creds FILE
        SQL for the console's own login (an administrator account with a random password, the way the core's
        `account create` stores it); the login goes to FILE (mode 600) for `run`. The first account has to be
        made this way: the console only answers to an account.

Exit 0 when every check passed. Standard library + wunet.
"""
import argparse
import asyncio
import base64
import json
import math
import os
import re
import secrets
import struct
import sys
import time
import urllib.error
import urllib.request
from xml.sax.saxutils import escape, unescape

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
for p in (os.path.join(REPO, "hosting", "kit", "tools"), HERE):
    if p not in sys.path:
        sys.path.insert(0, p)
from wunet import srp6, wire335  # noqa: E402
import compat  # noqa: E402

CMSG_MESSAGECHAT = 0x095
SMSG_MESSAGECHAT = 0x096
SMSG_NEW_WORLD = 0x03E
MSG_MOVE_WORLDPORT_ACK = 0x0DC
MSG_MOVE_TELEPORT_ACK = 0x0C7
SMSG_NOTIFICATION = 0x1CB
CHAT_MSG_SYSTEM = 0x00
CHAT_MSG_SAY = 0x01
LANG_COMMON = 7            # a human knows it; the core refuses LANG_UNIVERSAL from players
RACE_HUMAN, CLASS_WARRIOR = 1, 1
LEVEL = 20
PRACTICE_MOB = 38          # ov_bots_probe.lua's default practice mob


# ---------------------------------------------------------------------------- packets (pure, unit tested)
def say_packet(text: str) -> bytes:
    """CMSG_MESSAGECHAT body: a /say line (a dot command typed in chat)."""
    return struct.pack("<II", CHAT_MSG_SAY, LANG_COMMON) + text.encode("utf-8") + b"\x00"


def parse_chat(body: bytes):
    """(type, text) of an SMSG_MESSAGECHAT system line (ChatHandler::BuildChatPacket), else None."""
    if len(body) < 1 + 4 + 8 + 4 + 8 + 4 or body[0] != CHAT_MSG_SYSTEM:
        return None
    p = 1 + 4 + 8 + 4 + 8
    (n,) = struct.unpack_from("<I", body, p)
    p += 4
    if n == 0 or p + n > len(body):
        return None
    return body[0], body[p:p + n].rstrip(b"\x00").decode("utf-8", "replace")


def read_packed_guid(b: bytes, p: int = 0):
    mask = b[p]
    p += 1
    guid = 0
    for i in range(8):
        if mask & (1 << i):
            guid |= b[p] << (8 * i)
            p += 1
    return guid, p


def packed_guid(guid: int) -> bytes:
    mask, out = 0, b""
    for i in range(8):
        byte = (guid >> (8 * i)) & 0xFF
        if byte:
            mask |= 1 << i
            out += bytes([byte])
    return bytes([mask]) + out


def teleport_ack(body: bytes, now_ms: int) -> bytes:
    """The client's MSG_MOVE_TELEPORT_ACK for the server's one: packed guid, the counter, our time."""
    guid, p = read_packed_guid(body)
    (counter,) = struct.unpack_from("<I", body, p)
    return packed_guid(guid) + struct.pack("<II", counter, now_ms & 0xFFFFFFFF)


def smoke_lines(text: str):
    """The probe's answers in a system line: OVSMOKE {json}."""
    out = []
    for m in re.finditer(r"OVSMOKE (\{.*\})", text or ""):
        try:
            out.append(json.loads(m.group(1)))
        except ValueError:
            pass
    return out


def dungeon_door(repo=REPO, dungeon="deadmines"):
    """(map, x, y, z, o) just inside a dungeon, from ov_companions_dungeons.lua (one source for the doors)."""
    t = open(os.path.join(repo, "realm", "custom", "lua_scripts", "ov_companions_dungeons.lua"), encoding="utf-8").read()
    m = re.search(r'id = "' + re.escape(dungeon) + r'".*?pos = \{([^}]*)\}', t)
    if not m:
        raise ValueError(f"no dungeon {dungeon!r} in ov_companions_dungeons.lua")
    v = [float(x) for x in m.group(1).split(",")]
    return int(v[0]), v[1], v[2], v[3], v[4]


# ---------------------------------------------------------------------------- what the probe saw (pure)
def bots_of(rep):
    return [m for m in (rep or {}).get("members", []) if m.get("bot")]


def judge_recruit(rep, want=3):
    """ok, detail: enough bots, all on our map and instance, alive, ours."""
    me = (rep or {}).get("me") or {}
    bots = bots_of(rep)
    bad = [b.get("name") for b in bots if b.get("map") != me.get("map") or b.get("instance") != me.get("instance")
           or not b.get("alive") or (b.get("master") not in (None, me.get("name")))]
    ok = len(bots) >= want and not bad
    return ok, f"{len(bots)} bots (want {want}+)" + (f"; not with us: {', '.join(map(str, bad))}" if bad else "")


def judge_follow(rep, yards):
    bots = bots_of(rep)
    far = [f"{b.get('name')} {b.get('dist')}" for b in bots if b.get("dist") is None or b.get("dist") > yards]
    return bool(bots) and not far, (f"all {len(bots)} within {yards} yards" if not far else "still far: " + ", ".join(far))


def moved_to(rep, to, within):
    me = (rep or {}).get("me") or {}
    if me.get("x") is None or me.get("y") is None:
        return False
    return ((me["x"] - to[0]) ** 2 + (me["y"] - to[1]) ** 2) ** 0.5 <= within


def judge_fight(samples):
    """ok when a bot was in combat at some point and the mob ended dead (or gone)."""
    if not samples:
        return False, "no answer from the probe"
    fought = sorted({n for s in samples for n in (s.get("bots_in_combat") or [])})
    last = samples[-1]
    dead = last.get("alive") is False
    ok = bool(fought) and dead
    return ok, f"bots fighting: {', '.join(fought) or 'none'}; mob {'dead' if dead else 'alive at %s%%' % last.get('hp')}"


# ---------------------------------------------------------------------------- the console (SOAP)
def soap(host, port, user, password, command, timeout=20):
    body = ('<?xml version="1.0" encoding="utf-8"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/" '
            'xmlns:ns1="urn:AC"><SOAP-ENV:Body><ns1:executeCommand><command>' + escape(command)
            + '</command></ns1:executeCommand></SOAP-ENV:Body></SOAP-ENV:Envelope>').encode()
    auth = base64.b64encode(f"{user}:{password}".encode()).decode()
    req = urllib.request.Request(f"http://{host}:{port}/", data=body, method="POST",
                                 headers={"Content-Type": "text/xml", "SOAPAction": '"urn:AC#executeCommand"',
                                          "Authorization": "Basic " + auth})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            text = resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        text = e.read().decode("utf-8", "replace")
    m = re.search(r"<result>(.*?)</result>", text, re.S) or re.search(r"<faultstring>(.*?)</faultstring>", text, re.S)
    res = unescape(m.group(1) if m else text[:200]).strip()
    return ("<result>" in text and "<faultstring>" not in text), res


def gen(n=12):
    return "".join(secrets.choice("ABCDEFGHJKLMNPQRSTUVWXYZ23456789") for _ in range(n))


def admin_sql(creds_path):
    user, pw = "SMOKEADMIN", gen(16)
    salt, ver = srp6.make_verifier(user, pw)
    fd = os.open(creds_path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as f:
        f.write(f"SMOKE_SOAP_USER={user}\nSMOKE_SOAP_PASS={pw}\n")
    # account_access changed its column names upstream (id/gmlevel -> AccountID/SecurityLevel): one of the two
    # inserts works on any core; run with mysql --force
    return "\n".join([
        f"INSERT INTO account (username, salt, verifier, expansion) VALUES ('{user}', 0x{salt.hex()}, 0x{ver.hex()}, 2);",
        f"INSERT INTO account_access (AccountID, SecurityLevel, RealmID) SELECT id, 3, -1 FROM account WHERE username = '{user}';",
        f"INSERT INTO account_access (id, gmlevel, RealmID) SELECT id, 3, -1 FROM account WHERE username = '{user}';",
    ]) + "\n"


# ---------------------------------------------------------------------------- the player
class Player:
    def __init__(self, w, guid, name, log, poll=3.0):
        self.w, self.guid, self.name, self.log, self.poll = w, guid, name, log, poll
        self.answers = asyncio.Queue()
        self.system = []
        self.map = None
        self.task = None

    def start(self):
        self.task = asyncio.ensure_future(self.pump())

    async def pump(self):
        loop = asyncio.get_running_loop()
        try:
            while True:
                op, body = await self.w.recv()
                if op == wire335.SMSG_TIME_SYNC_REQ and len(body) >= 4:
                    self.w.send(wire335.CMSG_TIME_SYNC_RESP, body[:4] + struct.pack("<I", int(loop.time() * 1000) & 0xFFFFFFFF))
                elif op == SMSG_NEW_WORLD and len(body) >= 4:
                    self.map = struct.unpack_from("<I", body)[0]
                    self.w.send(MSG_MOVE_WORLDPORT_ACK)
                elif op == MSG_MOVE_TELEPORT_ACK and body:
                    self.w.send(MSG_MOVE_TELEPORT_ACK, teleport_ack(body, int(loop.time() * 1000)))
                elif op == SMSG_MESSAGECHAT:
                    c = parse_chat(body)
                    if c:
                        self.system.append(c[1])
                        for a in smoke_lines(c[1]):
                            await self.answers.put(a)
                elif op == SMSG_NOTIFICATION:
                    self.system.append(body.rstrip(b"\x00").decode("utf-8", "replace"))
                else:
                    continue
                await self.w.writer.drain()
        except (asyncio.IncompleteReadError, ConnectionError, OSError):
            self.log("world connection closed")

    async def say(self, text):
        self.w.send(CMSG_MESSAGECHAT, say_packet(text))
        await self.w.writer.drain()

    async def ask(self, what, kind, within=15.0):
        """`.ovsmoke <what>` and its answer of type `kind`."""
        while not self.answers.empty():
            self.answers.get_nowait()
        await self.say(".ovsmoke " + what)
        end = time.monotonic() + within
        while True:
            left = end - time.monotonic()
            if left <= 0:
                return None
            try:
                a = await asyncio.wait_for(self.answers.get(), left)
            except asyncio.TimeoutError:
                return None
            if a.get("t") in (kind, "error"):
                return a

    async def until(self, what, kind, test, within, every=None):
        """Ask every few seconds until test(answer) is true; the last answer."""
        every = self.poll if every is None else every
        end = time.monotonic() + within
        last = None
        while time.monotonic() < end:
            last = await self.ask(what, kind)
            if last is not None and test(last):
                return last, True
            await asyncio.sleep(every)
        return last, False


class Report:
    def __init__(self, quiet=False):
        self.checks, self.quiet, self.t0 = [], quiet, time.monotonic()

    def log(self, msg):
        if not self.quiet:
            print(f"[{time.monotonic() - self.t0:6.1f}s] {msg}", flush=True)

    def add(self, name, ok, detail=""):
        self.checks.append({"name": name, "ok": bool(ok), "detail": detail})
        self.log(f"{'PASS' if ok else 'FAIL'} {name}: {detail}")
        return ok

    @property
    def ok(self):
        return bool(self.checks) and all(c["ok"] for c in self.checks)


async def wait_soap(a, rep, within):
    end = time.monotonic() + within
    last = ""
    while time.monotonic() < end:
        try:
            ok, res = soap(a.soap_host, a.soap_port, a.soap_user, a.soap_pass, "server info", timeout=10)
            if ok:
                return True, res
            last = res
        except OSError as e:
            last = str(e)
        await asyncio.sleep(5)
    return False, last


async def run(a, rep):
    ok, info = await wait_soap(a, rep, a.wait)
    if not rep.add("console", ok, (info.splitlines() or [""])[0][:120] if ok else f"no answer in {a.wait}s: {info[:120]}"):
        return
    user, pw = "SMOKE" + gen(6), gen(12)
    ok, res = soap(a.soap_host, a.soap_port, a.soap_user, a.soap_pass, f"account create {user} {pw}")
    res = res.replace(pw, "***")
    ok2, res2 = soap(a.soap_host, a.soap_port, a.soap_user, a.soap_pass, f"account set gmlevel {user} 3 -1")
    if not rep.add("account", ok and ok2, f"{user} made GM through the console" if ok and ok2 else (res or res2)[:160]):
        return

    ah, ap = wire335.split_host_port(a.auth, 3724)
    wh, wp = wire335.split_host_port(a.world, 8085)
    K, realms = await wire335.logon(ah, ap, user, pw, timeout=30)
    w = await wire335.world_session(wh, wp, user, K, realm_id=(realms[0]["id"] if realms else 1), timeout=60)
    name = wire335.random_name("Smk")
    code = await wire335.char_create(w, name, race=RACE_HUMAN, cls=CLASS_WARRIOR)
    chars = [c for c in await wire335.char_enum(w) if c["name"].lower() == name.lower()]
    if not rep.add("character", code == wire335.CHAR_CREATE_SUCCESS and chars, f"{name} made (code {code:#x})"):
        return
    pos = await wire335.enter_world(w, chars[0]["guid"])
    me = Player(w, chars[0]["guid"], chars[0]["name"], rep.log, a.poll)
    me.start()
    try:
        await asyncio.sleep(a.poll)
        soap(a.soap_host, a.soap_port, a.soap_user, a.soap_pass, f"character level {me.name} {LEVEL}")
        await me.say(".gm off")
        first, _ = await me.until("report", "report", lambda r: r.get("t") == "report", within=60)
        if not rep.add("world", first is not None and first.get("t") == "report",
                       f"in the world on map {pos['map']}; the probe answers" if first else "the probe (ov_bots_probe.lua) never answered"):
            return
        await steps(a, rep, me)
    finally:
        try:
            await me.say(".dismiss all")
            await asyncio.sleep(min(1.0, a.poll))
        except Exception:  # noqa: BLE001 - best effort on the way out
            pass
        if me.task:
            me.task.cancel()
        w.writer.close()


async def steps(a, rep, me):
    # 1. mod-playerbots itself: addclass, then remove
    await me.say(".playerbots bot addclass rogue")
    r, ok = await me.until("report", "report", lambda r: len(bots_of(r)) >= 1, within=a.bot_wait)
    names = [b["name"] for b in bots_of(r)]
    rep.add("addclass", ok, f"joined: {', '.join(map(str, names))}" if ok else "no bot joined in %ds (system: %s)" % (a.bot_wait, " | ".join(me.system[-3:])[:200]))
    for n in names:
        await me.say(f".playerbots bot remove {n}")
    if names:
        r, ok = await me.until("report", "report", lambda r: len(bots_of(r)) == 0, within=60)
        rep.add("remove", ok, "the group has no bots" if ok else f"{len(bots_of(r))} still in the group")

    # 2. into a dungeon
    if a.dungeon != "none":
        mp, x, y, z, o = dungeon_door(a.repo, a.dungeon)
        await me.say(f".go xyz {x} {y} {z} {mp} {o}")
        r, ok = await me.until("report", "report", lambda r: (r.get("me") or {}).get("map") == mp, within=60)
        if not rep.add("dungeon", ok, f"on map {mp} ({a.dungeon})" if ok else f"still on map {(r or {}).get('me', {}).get('map')}"):
            return

    # 3. ov_companions.lua's own command
    await me.say(".recruit party")
    r, ok = await me.until("report", "report", lambda r: judge_recruit(r, a.want)[0], within=a.bot_wait + 60)
    rep.add("recruit", ok, judge_recruit(r, a.want)[1] + ("" if ok else " (system: %s)" % " | ".join(me.system[-4:])[:240]))
    if not bots_of(r):
        return

    # 4. the methods the script relies on, asked of a live bot
    req = compat.needs(a.repo)["required_methods"]
    c = await me.ask("compat " + ",".join(req), "compat")
    rep.add("compat_live", c is not None and not c.get("missing") and not c.get("error"),
            (f"missing on a live bot: {', '.join(c['missing'])}" if c and c.get("missing") else
             (c or {}).get("error") or f"all {len(req)} present"))

    # 5. follow: a short hop (a near teleport) and the bots come after us
    if a.follow:
        rr = await me.ask("report", "report")
        mine = (rr or {}).get("me") or {}
        to = hop(rr, a.follow)
        await me.say(".go xyz {:.2f} {:.2f} {:.2f}".format(*to))
        # the hop counts once the realm has us there (after our teleport ack); then the bots must come after us
        there = lambda r: moved_to(r, to, a.follow / 3)  # noqa: E731
        r, ok = await me.until("report", "report", lambda r: there(r) and judge_follow(r, a.follow_yards)[0], within=60)
        rep.add("follow", ok, judge_follow(r, a.follow_yards)[1] if r and there(r) else
                "we never got there (map %s)" % mine.get("map") if r else "no report")

    # 6. one pull
    p = await me.ask(f"pull {a.mob}", "pull")
    if not rep.add("pull", p is not None and not p.get("error"), (p or {}).get("error") or f"{(p or {}).get('name')} level {(p or {}).get('level')}"):
        return
    samples = []
    end = time.monotonic() + a.fight_wait
    while time.monotonic() < end:
        s = await me.ask("fight", "fight")
        if s:
            samples.append(s)
            if s.get("alive") is False and any(x.get("bots_in_combat") for x in samples):
                break
        await asyncio.sleep(a.poll)
    rep.add("fight", *judge_fight(samples))

    # 7. dismiss
    await me.say(".dismiss all")
    r, ok = await me.until("report", "report", lambda r: len(bots_of(r)) == 0, within=90)
    rep.add("dismiss", ok, "the group has no bots" if ok else f"{len(bots_of(r))} still in the group")


def hop(rep, yards):
    """A point `yards` ahead of us (the probe gives no position: from the dungeon door's own facing)."""
    me = (rep or {}).get("me") or {}
    x, y, z, o = me.get("x"), me.get("y"), me.get("z"), me.get("o")
    if None in (x, y, z, o):
        raise ValueError("the probe's report has no position")
    return x + yards * math.cos(o), y + yards * math.sin(o), z


# ---------------------------------------------------------------------------- the throwaway realm
COMPOSE_FILE = os.path.join(HERE, "docker-compose.smoke.yml")
PROJECT = "wu-bots-smoke"
LUA_FILES = ("ov_companions.lua", "ov_companions_dungeons.lua", "ov_modapi.lua")
PORTS = {"auth": 13724, "world": 13785, "soap": 17878}


def docker(args, stdin=None, timeout=1800, quiet=False):
    import subprocess
    try:
        p = subprocess.run(["docker", *args], input=stdin, capture_output=True, text=True, timeout=timeout)
    except FileNotFoundError:
        return 127, "Docker is not installed (or not on PATH)."
    except subprocess.TimeoutExpired:
        return -2, "Docker took too long."
    return p.returncode, (p.stdout or "") + (p.stderr or "")


def compose(work, *args, **kw):
    return docker(["compose", "-p", PROJECT, "-f", COMPOSE_FILE, "--env-file", os.path.join(work, ".env"), *args], **kw)


def prepare(work, repo, image, tag, data):
    """The temp folder: the realm scripts the bots need plus the probe, and the compose .env (a random database
    password; mode 600)."""
    import shutil
    lua = os.path.join(work, "lua")
    os.makedirs(lua, exist_ok=True)
    for f in LUA_FILES:
        shutil.copy(os.path.join(repo, "realm", "custom", "lua_scripts", f), lua)
    shutil.copy(os.path.join(HERE, "smoke", "ov_bots_probe.lua"), lua)
    for f in os.listdir(lua):
        os.chmod(os.path.join(lua, f), 0o644)
    os.chmod(lua, 0o755)
    env = {"SMOKE_DB_PASSWORD": secrets.token_hex(16), "SMOKE_IMAGE": image, "SMOKE_TAG": tag, "SMOKE_DATA_VOLUME": data,
           "SMOKE_LUA_DIR": lua}
    fd = os.open(os.path.join(work, ".env"), os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as f:
        f.write("".join(f"{k}={v}\n" for k, v in env.items()))
    return env


def static_check(pb_src, ale_src, repo):
    """compat.py's result as JSON-able dict (the canary ran it already; here it goes in the report)."""
    try:
        pb = compat.playerbots_has(pb_src)
        ale = compat.ale_has(ale_src) if ale_src else None
    except ValueError as e:
        return {"ok": False, "missing": [{"kind": "tree", "name": str(e), "fatal": True}]}
    miss = compat.check(compat.needs(repo), pb, ale)
    print(compat.report(miss))
    return {"ok": not any(m[2] for m in miss), "missing": [{"kind": k, "name": x, "fatal": f} for k, x, f in miss]}


def smoke(a):
    """All of it. Returns the exit code."""
    import shutil
    import tempfile
    rc, out = docker(["volume", "inspect", a.data_volume], timeout=60)
    if rc != 0:
        print(f"bots smoke: no '{a.data_volume}' volume here. It holds the dbc + maps extracted from your own game folder")
        print("(realm\\1-extract.bat). The smoke test runs only where that volume is; nothing is downloaded.")
        return 3
    work = tempfile.mkdtemp(prefix="wu-bots-smoke-")
    os.chmod(work, 0o700)
    ok = False
    try:
        prepare(work, a.repo, a.image, a.tag, a.data_volume)
        if a.playerbots_src:
            print("== static guard (compat.py)", flush=True)
            with open(os.path.join(work, "static.json"), "w") as f:
                json.dump(static_check(a.playerbots_src, a.ale_src, a.repo), f)
            a.static = os.path.join(work, "static.json")
        print(f"== realm from {a.image}-*:{a.tag}", flush=True)
        compose(work, "pull", "-q", "ac-db-import", "ac-authserver", "ac-worldserver")
        for step in (("up", "-d", "--wait", "ac-database"), ("up", "-d", "ac-db-import", "ac-authserver", "ac-worldserver")):
            rc, out = compose(work, *step)
            if rc != 0:
                print("the realm didn't start: " + (out.strip().splitlines() or ["?"])[-1][:300])
                return 1
        print("== the console's own login (random, never printed)", flush=True)
        mysql = "mysql -uroot -p\"$MYSQL_ROOT_PASSWORD\""
        end = time.monotonic() + a.wait
        while time.monotonic() < end:
            rc, out = compose(work, "exec", "-T", "ac-database", "sh", "-c",
                              mysql + " -N -e 'SELECT 1 FROM acore_auth.realmlist LIMIT 1' 2>/dev/null", timeout=60)
            if rc == 0 and out.strip().endswith("1"):
                break
            time.sleep(10)
        creds = os.path.join(work, "creds")
        compose(work, "exec", "-T", "ac-database", "sh", "-c", mysql + " --force acore_auth 2>/dev/null", stdin=admin_sql(creds), timeout=120)
        got = dict(l.split("=", 1) for l in open(creds).read().split())
        a.soap_user, a.soap_pass = got["SMOKE_SOAP_USER"], got["SMOKE_SOAP_PASS"]
        a.auth, a.world = f"127.0.0.1:{PORTS['auth']}", f"127.0.0.1:{PORTS['world']}"
        a.soap_host, a.soap_port = "127.0.0.1", PORTS["soap"]
        print("== the player", flush=True)
        code = play(a)
        ok = code == 0
        return code
    finally:
        if not ok or a.logs:
            rc, out = compose(work, "logs", "--no-color", "--tail", "80", "ac-worldserver", timeout=60)
            print("== worldserver, last lines")
            print("\n".join(l for l in out.splitlines() if "password" not in l.lower()))
        if a.keep:
            print(f"kept: {work} (docker compose -p {PROJECT})")
        else:
            compose(work, "down", "-v", "--remove-orphans", timeout=300)
            shutil.rmtree(work, ignore_errors=True)


def play(a):
    rep = Report(a.quiet)
    if a.static:
        try:
            st = json.load(open(a.static))
            rep.add("compat_static", st.get("ok"), "; ".join(f"{m['kind']} {m['name']}" for m in st.get("missing", []) if m.get("fatal")) or "every command, setting and method is there")
        except (OSError, ValueError) as e:
            rep.add("compat_static", False, f"no compat.py result: {e}")
    if not a.soap_user or not a.soap_pass:
        rep.add("console", False, "no SMOKE_SOAP_USER / SMOKE_SOAP_PASS (smoke makes them with admin-sql)")
    else:
        try:
            asyncio.run(run(a, rep))
        except (wire335.WowError, OSError, asyncio.TimeoutError, asyncio.IncompleteReadError, ValueError) as e:
            rep.add("session", False, f"{type(e).__name__}: {e}")
    out = {"ok": rep.ok, "checks": rep.checks, "when": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}
    if a.out:
        with open(a.out, "w") as f:
            json.dump(out, f, indent=1)
    print(("BOTS SMOKE OK" if rep.ok else "BOTS SMOKE FAILED") + f" ({sum(c['ok'] for c in rep.checks)}/{len(rep.checks)} checks)")
    return 0 if rep.ok else 1


def player_args(r):
    r.add_argument("--repo", default=REPO)
    r.add_argument("--dungeon", default="deadmines", help="an id in ov_companions_dungeons.lua, or none")
    r.add_argument("--want", type=int, default=3, help="bots .recruit party must bring")
    r.add_argument("--follow", type=float, default=15.0, help="yards to hop for the follow check (0: skip)")
    r.add_argument("--follow-yards", type=float, default=12.0)
    r.add_argument("--mob", type=int, default=PRACTICE_MOB)
    r.add_argument("--wait", type=int, default=1800, help="seconds to wait for the realm's console")
    r.add_argument("--bot-wait", type=int, default=90)
    r.add_argument("--fight-wait", type=int, default=120)
    r.add_argument("--static", help="compat.py --json output to add to the report")
    r.add_argument("--poll", type=float, default=3.0, help="seconds between two looks")
    r.add_argument("--out")
    r.add_argument("--quiet", action="store_true")


def main(argv=None):
    ap = argparse.ArgumentParser(description="The bot smoke test.")
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("admin-sql")
    s.add_argument("--creds", required=True)
    r = sub.add_parser("run")
    r.add_argument("--auth", required=True)
    r.add_argument("--world", required=True)
    r.add_argument("--soap", required=True)
    player_args(r)
    m = sub.add_parser("smoke")
    m.add_argument("--image", default=os.environ.get("SMOKE_IMAGE", "ghcr.io/orionopenwow/wu-ac-wotlk"))
    m.add_argument("--tag", default=os.environ.get("SMOKE_TAG", "canary"))
    m.add_argument("--data-volume", default=os.environ.get("SMOKE_DATA_VOLUME", "ov-client-data"))
    m.add_argument("--playerbots-src")
    m.add_argument("--ale-src")
    m.add_argument("--keep", action="store_true")
    m.add_argument("--logs", action="store_true", help="the worldserver's last lines even when it passed")
    player_args(m)
    a = ap.parse_args(argv)
    if a.cmd == "admin-sql":
        sys.stdout.write(admin_sql(a.creds))
        return 0
    if a.cmd == "smoke":
        if not re.fullmatch(r"[A-Za-z0-9._-]{1,64}", a.tag) or not re.fullmatch(r"[a-z0-9./_-]{1,128}", a.image):
            print("bots smoke: that image or tag doesn't look right")
            return 2
        return smoke(a)
    a.soap_host, a.soap_port = wire335.split_host_port(a.soap, 7878)
    a.soap_user, a.soap_pass = os.environ.get("SMOKE_SOAP_USER", ""), os.environ.get("SMOKE_SOAP_PASS", "")
    return play(a)


if __name__ == "__main__":
    sys.exit(main())
