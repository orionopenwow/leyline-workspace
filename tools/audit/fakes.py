"""The audit's fake world, for CI and for a run anywhere but the owner's PC (audit.py --fake):

  * the website: the site's own code (stream/wrath_portal.py) on 127.0.0.1 with fake data (signed out, no realm);
  * the relay: a listing with Wrath Unleashed on it and a join page;
  * GitHub: the launcher, Host pack and Den Lite release files with matching checksums (Den Lite: the real program
    when DEN_LITE_BIN is set, so the Den section runs it);
  * the realm: FakeWrath, the Host kit's fake 3.3.5a realm (hosting/tests/fake_realm.py) with a world that has a
    player create, creatures, spells, a mount, the first-login menu and the boost, melee and .bug;
  * SSH: the server script's output as the real server prints it (all green, and the audit account's secret).
"""
from __future__ import annotations

import asyncio
import hashlib
import io
import json
import os
import struct
import sys
import tempfile
import threading
import zipfile
import zlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from common import ROOT
import play335 as p

sys.path.insert(0, str(ROOT / "hosting" / "tests"))
from wunet import wire335 as w  # noqa: E402

FAKE_PASSWORD = "FAKEAUDITPASS123"
SSH_CHECKS = """== the release audit, server side
AUDIT world ok running for 30 h 2 min; restarts since it was made: 3
AUDIT world_restart ok no restart in the last hour
AUDIT oom ok no out-of-memory kills in the last 24 h
AUDIT soap ok AzerothCore rev. 1234 Connected players: 3. Characters in world: 40. Server uptime: 1 Day(s)
AUDIT lua ok 31 scripts, no Lua errors in the last 24 h
AUDIT lua_loaded skip no 'loaded' line in the last 72 h of the log (normal when it has run longer)
AUDIT slice ok the served slice is whole
AUDIT relay ok running, healthz ok, 2 realms listed
AUDIT gmconsole ok answers on 127.0.0.1:8099
AUDIT bugs ok the bug manifest answers: 4 open, 19 in all
AUDIT memory ok 2100 MB memory available
AUDIT disk ok 41 GB disk free
AUDIT DONE
"""
SSH_ACCOUNT = f"AUDIT account ok AUDIT password changed (a fresh password, kept in /root/.leyline-audit)\nAUDIT_SECRET {FAKE_PASSWORD}\nAUDIT DONE\n"
WU_ID = "abcdefghijklmnop"


# ---------------------------------------------------------------------- tiny HTTP servers
def serve(routes: dict, fallback=None):
    """routes: path -> (status, content type, bytes). Returns (server, base url)."""
    class H(BaseHTTPRequestHandler):
        def log_message(self, *a):
            pass

        def do_GET(self):
            path = self.path.split("?")[0]
            hit = routes.get(path) or (fallback(path) if fallback else None)
            st, ct, body = hit or (404, "text/plain", b"not found")
            self.send_response(st)
            self.send_header("Content-Type", ct)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            try:
                self.wfile.write(body)
            except (BrokenPipeError, ConnectionResetError):
                pass     # the audit reads only what it needs

    class S(ThreadingHTTPServer):
        def handle_error(self, request, client_address):
            pass
    srv = S(("127.0.0.1", 0), H)
    srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, f"http://127.0.0.1:{srv.server_address[1]}"


def host_pack_zip() -> bytes:
    b = io.BytesIO()
    with zipfile.ZipFile(b, "w") as z:
        z.writestr("Host a Leyline realm.bat", "@echo off\r\necho fake\r\n")
        z.writestr("tools/wuhost/selfcheck.py", "def main():\n    return 0\n")
        z.writestr("tools/wunet/__init__.py", "")
    return b.getvalue()


def github(tmp: Path):
    exe = b"MZ" + os.urandom(2 << 20)
    pack = host_pack_zip()
    den_real = os.environ.get("DEN_LITE_BIN")
    den_linux = Path(den_real).read_bytes() if den_real and os.path.isfile(den_real) else b"\x7fELF fake"
    den_win = b"MZ fake den lite"
    sha = lambda b: hashlib.sha256(b).hexdigest()  # noqa: E731
    rel = "/gh/orionopenwow/Leyline/releases/download"
    ws = "/gh/orionopenwow/leyline-workspace/releases/download/den-lite-latest"
    bin_ = "application/octet-stream"
    routes = {
        "/api/repos/orionopenwow/Leyline/releases/tags/launcher-latest": (200, "application/json", json.dumps(
            {"name": "Leyline launcher (latest, 0.3.14)",
             "assets": [{"name": "Leyline.exe", "size": len(exe), "digest": "sha256:" + sha(exe)}]}).encode()),
        f"{rel}/launcher-latest/Leyline.exe": (200, bin_, exe),
        f"{rel}/host-pack/leyline-host-pack-2026.1101.1.zip": (200, "application/zip", pack),
        f"{rel}/host-pack/leyline-host-pack-2026.1101.1.zip.sha256": (200, "text/plain", f"{sha(pack)}  leyline-host-pack-2026.1101.1.zip\n".encode()),
        f"{ws}/den-lite-windows-x64.exe": (200, bin_, den_win),
        f"{ws}/den-lite-windows-x64.exe.sha256": (200, "text/plain", f"{sha(den_win)}  den-lite-windows-x64.exe\n".encode()),
        f"{ws}/den-lite-linux-x64": (200, bin_, den_linux),
        f"{ws}/den-lite-linux-x64.sha256": (200, "text/plain", f"{sha(den_linux)}  den-lite-linux-x64\n".encode()),
    }
    # every other release download the site links to: there (the real ones are checked on the owner's PC)
    srv, base = serve(routes, lambda path: (200, bin_, b"PK fake") if "/releases/download/" in path else None)
    hostpack = {"format": 1, "name": "Host pack", "version": "2026.1101.1",
                "download": "https://github.com/orionopenwow/Leyline/releases/download/host-pack/leyline-host-pack-2026.1101.1.zip",
                "sha256": sha(pack), "size": len(pack)}
    return srv, base, hostpack


def relay():
    listing = {"format": 1, "realms": [
        {"id": WU_ID, "name": "Wrath Unleashed", "live": True, "players": 3, "bots": 40, "ruleset": "main",
         "type": "PvE", "tagline": "", "claimed": True, "rules": []}]}
    return serve({"/api/listing/realms.json": (200, "application/json", json.dumps(listing).encode()),
                  f"/join/{WU_ID}": (200, "text/html; charset=utf-8", b"<!doctype html><html><body>Join</body></html>")})


def portal(tmp: Path, relay_base: str, hostpack: dict):
    """The site's own code, as test_site.py runs it, with fake data."""
    mods, arm, data, ley = (tmp / d for d in ("mods", "armory", "data", "leyline"))
    for d in (mods / "files", arm / "c", data, ley):
        d.mkdir(parents=True, exist_ok=True)
    (ley / "hostpack.json").write_text(json.dumps(hostpack))
    (mods / "catalog.json").write_text(json.dumps({"format": 1, "mods": [
        {"id": "audit-mod", "name": "Audit Mod", "version": "1.0.0", "kind": "ui", "zip": "audit-mod-1.0.0.zip",
         "description": "A test entry."}]}))
    (mods / "files" / "audit-mod-1.0.0.zip").write_bytes(b"PK")
    (arm / "index.json").write_text(json.dumps({"updated": 0, "chars": []}))
    os.environ.update(SOAP_USER="WUSOAP", SOAP_PASS="x", AC_SOAP_URL="http://127.0.0.1:9/", PORTAL_URL="http://127.0.0.1:9",
                      WU_STREAM="0", STREAM_HOST="play.example", PUBLIC_HOST="site.example", WU_MODS_DIR=str(mods),
                      WU_ARMORY_DIR=str(arm), WU_ART_DIR=str(ROOT / "stream" / "www-files" / "art"),
                      WU_PORTAL_DATA=str(data), WU_LEYLINE_DIR=str(ley),
                      WU_RELAY_LISTING=relay_base + "/api/listing/realms.json", WU_JOIN_BASE=relay_base,
                      WU_FORM_SECRET="audit-fake-form-secret")
    import importlib.util
    spec = importlib.util.spec_from_file_location("wrath_portal_audit", ROOT / "stream" / "wrath_portal.py")
    wp = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(wp)
    wp.realm_status = lambda: {"online": True, "players": 3, "bots": 40, "updated_at": "2026-11-01T12:00:00Z"}
    for m in ("social", "profiles", "herokeys", "forum"):   # what the portal's main() starts
        mod = getattr(wp, m, None)
        if mod is not None and hasattr(mod, "setup"):
            try:
                mod.setup()
            except Exception:  # noqa: BLE001 - a fake store that can't start just leaves that part empty
                pass
    srv = ThreadingHTTPServer(("127.0.0.1", 0), wp.PageHandler)
    srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, f"http://127.0.0.1:{srv.server_address[1]}"


# ---------------------------------------------------------------------- the realm
def values(fields: dict) -> bytes:
    n = max(fields) // 32 + 1 if fields else 0
    masks = [0] * n
    for i in fields:
        masks[i // 32] |= 1 << (i % 32)
    return bytes([n]) + struct.pack(f"<{n}I", *masks) + b"".join(struct.pack("<I", fields[i]) for i in sorted(fields))


def living(x, y, z, o, spline=False, self_=False) -> bytes:
    flags = p.UF_LIVING | (0x01 if self_ else 0) | (p.UF_LOWGUID if not self_ else 0)
    mflags = p.MF_SPLINE_ENABLED | p.MOVE_FORWARD if spline else 0
    out = struct.pack("<H", flags) + struct.pack("<IHIffffI", mflags, 0, 0, x, y, z, o, 0) + struct.pack("<9f", *[7.0] * 9)
    if spline:
        out += struct.pack("<I", p.SPLINE_FINAL_POINT) + struct.pack("<fff", x + 5, y, z)
        out += struct.pack("<III", 100, 2000, 7) + struct.pack("<fff", 1.0, 1.0, 0.0) + struct.pack("<I", 0)
        out += struct.pack("<I", 2) + struct.pack("<6f", x, y, z, x + 5, y, z) + b"\x00" + struct.pack("<fff", x + 5, y, z)
    if not self_:
        out += struct.pack("<I", 123)
    return out


def create(guid, typ, mv: bytes, fields: dict) -> bytes:
    return b"\x02" + p.pack_guid(guid) + bytes([typ]) + mv + values(fields)


def update(*blocks) -> bytes:
    return struct.pack("<I", len(blocks)) + b"".join(blocks)


def chat(ctype, sender, text, name="") -> bytes:
    out = struct.pack("<BiQI", ctype, 0, sender, 0)
    if ctype == p.CHAT_MONSTER_SAY:
        out += struct.pack("<I", len(name) + 1) + name.encode() + b"\x00" + struct.pack("<Q", 0)
    else:
        out += struct.pack("<Q", 0)
    return out + struct.pack("<I", len(text) + 1) + text.encode() + b"\x00\x00"


def gossip(guid, menu, items) -> bytes:
    out = struct.pack("<QIII", guid, menu, 1, len(items))
    for i, t in enumerate(items):
        out += struct.pack("<IBBI", i, 0, 0, 0) + t.encode() + b"\x00" + b"\x00"
    return out + struct.pack("<I", 0)


MOB = 0xF130000000000000 | (2031 << 24) | 77
VENDOR = 0xF130000000000000 | (3587 << 24) | 78


class FakeWrath:
    """The Host kit's fake realm's logon, and a world with what the audit's player does."""

    def __init__(self, user="AUDIT", password=FAKE_PASSWORD):
        from fake_realm import FakeRealm
        self.realm = FakeRealm(realm_name="Wrath Unleashed")
        self.realm.add_account(user, password)
        self.realm.on_world = self.on_world
        self.events = []
        self.chars = []
        self.next_guid = 1

    async def start(self):
        await self.realm.start()
        self.realm.world_address = f"127.0.0.1:{self.realm.world_port}"
        return self

    async def stop(self):
        await self.realm.stop()

    async def on_world(self, reader, writer):
        try:
            await self._world(reader, writer)
        except (asyncio.IncompleteReadError, ConnectionError, ValueError) as e:
            self.events.append(("world_error", str(e)))
        finally:
            writer.close()

    async def _world(self, reader, writer):
        import os as _os
        seed = _os.urandom(4)
        body = struct.pack("<I", 1) + seed + _os.urandom(32)
        writer.write(struct.pack(">H", len(body) + 2) + struct.pack("<H", w.SMSG_AUTH_CHALLENGE) + body)
        await writer.drain()
        enc = dec = None

        async def recv():
            h = await reader.readexactly(6)
            if dec:
                h = dec.apply(h)
            size = struct.unpack_from(">H", h)[0]
            return struct.unpack_from("<I", h, 2)[0], await reader.readexactly(size - 4)

        def send(op, b=b""):
            if len(b) + 2 > 0x7FFF:
                hh = struct.pack(">I", (len(b) + 2) | 0x800000)[1:] + struct.pack("<H", op)
            else:
                hh = struct.pack(">H", len(b) + 2) + struct.pack("<H", op)
            writer.write((enc.apply(hh) if enc else hh) + b)

        op, b = await recv()
        end = b.index(b"\x00", 8)
        user = b[8:end].decode()
        p_ = end + 1
        client_seed = b[p_ + 4:p_ + 8]
        digest = b[p_ + 28:p_ + 48]
        K = self.realm.sessions.get(user)
        if not K or hashlib.sha1(user.encode() + bytes(4) + client_seed + seed + K).digest() != digest:
            raise ValueError("session digest mismatch")
        enc, dec = w.header_crypt(K, server=True)
        send(w.SMSG_AUTH_RESPONSE, bytes([w.AUTH_OK]) + struct.pack("<IBIB", 0, 0, 0, 2))
        await writer.drain()
        me, lvl, menu_step = None, 1, None
        while True:
            op, b = await recv()
            if op == w.CMSG_CHAR_ENUM:
                out = bytes([len(self.chars)])
                for c in self.chars:
                    out += struct.pack("<Q", c["guid"]) + c["name"].encode() + b"\x00"
                    out += bytes([c["race"], c["class"], 0, 0, 0, 0, 0, 0, c["level"]])
                    out += struct.pack("<II", 141, 1) + struct.pack("<fff", 10311.3, 832.5, 1326.4)
                    out += struct.pack("<IIIB", 0, 0, 0, 1) + struct.pack("<III", 0, 0, 0) + bytes(23 * 9)
                send(w.SMSG_CHAR_ENUM, out)
            elif op == w.CMSG_CHAR_CREATE:
                name = b[:b.index(b"\x00")].decode()
                self.chars.append({"guid": self.next_guid, "name": name, "race": b[-9], "class": b[-8], "level": 1})
                self.next_guid += 1
                self.events.append(("create", name))
                send(w.SMSG_CHAR_CREATE, bytes([w.CHAR_CREATE_SUCCESS]))
            elif op == p.CMSG_CHAR_DELETE:
                g = struct.unpack_from("<Q", b)[0]
                n = len(self.chars)
                self.chars = [c for c in self.chars if c["guid"] != g]
                self.events.append(("delete", g))
                send(p.SMSG_CHAR_DELETE, bytes([p.CHAR_DELETE_SUCCESS if len(self.chars) < n else 0x48]))
            elif op == w.CMSG_PLAYER_LOGIN:
                me = struct.unpack_from("<Q", b)[0]
                lvl = 1
                send(w.SMSG_LOGIN_VERIFY_WORLD, struct.pack("<Iffff", 1, 10311.3, 832.5, 1326.4, 0.0))
                send(p.SMSG_INITIAL_SPELLS, b"\x00" + struct.pack("<H", 3) + b"".join(
                    struct.pack("<IH", s, 0) for s in (75, 2973, 58984)) + struct.pack("<H", 0))
                up = update(
                    create(me, p.TYPE_PLAYER, living(10311.3, 832.5, 1326.4, 0.0, self_=True),
                           {2: 0x19, p.U_HEALTH: 50, p.U_LEVEL: 1, p.U_FACTION: 4,
                            p.P_VISIBLE_ITEM_1 + 2 * p.SLOT_MAINHAND: 12282, p.P_VISIBLE_ITEM_1 + 2 * p.SLOT_RANGED: 2504}),
                    create(MOB, p.TYPE_UNIT, living(10320.0, 835.0, 1326.0, 1.0, spline=True),
                           {p.F_ENTRY: 2031, p.U_HEALTH: 42, p.U_LEVEL: 2, p.U_FACTION: 7}),
                    create(VENDOR, p.TYPE_UNIT, living(10305.0, 830.0, 1326.0, 2.0),
                           {p.F_ENTRY: 3587, p.U_HEALTH: 100, p.U_LEVEL: 10, p.U_FACTION: 80, p.U_NPCFLAGS: 0x80}))
                raw = zlib.compress(up)
                send(p.SMSG_COMPRESSED_UPDATE_OBJECT, struct.pack("<I", len(up)) + raw)
                send(p.SMSG_LEARNED_SPELL, struct.pack("<IH", 10793, 0))
                send(p.SMSG_GOSSIP_MESSAGE, gossip(me, 60001, ["Begin your adventure at level 1",
                                                               "Boost to level 80 (spec gear, professions, Dalaran)"]))
                menu_step = "choice"
                self.events.append(("enter", me))
            elif op == p.CMSG_GOSSIP_SELECT_OPTION:
                _, _, idx = struct.unpack_from("<QII", b)
                if menu_step == "choice" and idx == 0:
                    send(p.SMSG_GOSSIP_COMPLETE)
                    menu_step = None
                elif menu_step == "choice":
                    send(p.SMSG_GOSSIP_MESSAGE, gossip(me, 60001, ["Marksmanship", "Back"]))
                    menu_step = "spec"
                elif menu_step == "spec":
                    send(p.SMSG_GOSSIP_MESSAGE, gossip(me, 60001, ["Alchemy (Grand Master, 450)", "No professions: boost now as Marksmanship"]))
                    menu_step = "prof"
                elif menu_step == "prof" and idx == 1:
                    send(p.SMSG_GOSSIP_COMPLETE)
                    lvl = 80
                    send(p.SMSG_UPDATE_OBJECT, update(b"\x00" + p.pack_guid(me) + values({p.U_LEVEL: 80})))
                    self.events.append(("boost", me))
            elif op == p.CMSG_CAST_SPELL:
                spell = struct.unpack_from("<I", b, 1)[0]
                if spell == 10793 or spell == 58984:
                    send(p.SMSG_SPELL_START, p.pack_guid(me) + p.pack_guid(me) + struct.pack("<BI", 1, spell))
                    send(p.SMSG_SPELL_GO, p.pack_guid(me) + p.pack_guid(me) + struct.pack("<BI", 1, spell))
                    if spell == 10793:
                        send(p.SMSG_UPDATE_OBJECT, update(b"\x00" + p.pack_guid(me) + values({p.U_MOUNT: 6080})))
                else:
                    send(p.SMSG_CAST_FAILED, struct.pack("<BIB", 1, spell, 2))
                self.events.append(("cast", spell))
            elif op == p.CMSG_CANCEL_MOUNT_AURA:
                send(p.SMSG_UPDATE_OBJECT, update(b"\x00" + p.pack_guid(me) + values({p.U_MOUNT: 0})))
            elif op == p.CMSG_ATTACKSWING:
                g = struct.unpack_from("<Q", b)[0]
                if g == MOB:
                    send(p.SMSG_ATTACKSTART, struct.pack("<QQ", me, g))
                else:
                    send(p.SMSG_ATTACKSTOP, p.pack_guid(me) + p.pack_guid(g) + struct.pack("<I", 0))
                self.events.append(("attack", g))
            elif op == p.CMSG_MESSAGECHAT:
                text = b[8:].split(b"\x00")[0].decode()
                if text == ".bug":
                    send(p.SMSG_MESSAGECHAT, chat(p.CHAT_SYSTEM, 0, "|cff66ccff[Wrath Unleashed]|r Type .bug and what happened, e.g. .bug the quest giver"))
                elif text == ".boost" and lvl < 10:
                    send(p.SMSG_GOSSIP_MESSAGE, gossip(me, 60001, ["Begin your adventure at level 1", "Boost to level 80"]))
                    menu_step = "choice"
                self.events.append(("say", text))
            elif op in (p.MSG_MOVE_START_FORWARD, p.MSG_MOVE_HEARTBEAT, p.MSG_MOVE_STOP, p.MSG_MOVE_SET_FACING):
                self.events.append(("move", op))
            elif op == w.CMSG_PING:
                send(w.SMSG_PONG, b[:4])
            elif op == w.CMSG_LOGOUT_REQUEST:
                send(w.SMSG_LOGOUT_RESPONSE, struct.pack("<IB", 0, 0))
                send(w.SMSG_LOGOUT_COMPLETE)
                self.events.append(("logout", me))
            await writer.drain()


class Fakes:
    """Everything the audit talks to, on 127.0.0.1. .ctx_args() gives Ctx's fake and rewrite."""

    def __init__(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="leyline-audit-fake-"))
        self.gh_srv, self.gh, hostpack = github(self.tmp)
        self.relay_srv, self.relay = relay()
        self.site_srv, self.site = portal(self.tmp, self.relay, hostpack)
        self.loop = asyncio.new_event_loop()
        threading.Thread(target=self.loop.run_forever, daemon=True).start()
        self.wrath = asyncio.run_coroutine_threadsafe(FakeWrath().start(), self.loop).result(10)

    def ctx_args(self):
        fake = {"site": self.site, "relay": self.relay, "auth": f"127.0.0.1:{self.wrath.realm.auth_port}",
                "ssh_checks": SSH_CHECKS, "ssh_account": SSH_ACCOUNT}
        rewrite = [("https://github.com/", self.gh + "/gh/"), ("https://api.github.com/", self.gh + "/api/")]
        return fake, rewrite

    def close(self):
        for s in (self.gh_srv, self.relay_srv, self.site_srv):
            s.shutdown()
            s.server_close()
        self.loop.call_soon_threadsafe(self.loop.stop)
        import shutil
        shutil.rmtree(self.tmp, ignore_errors=True)
