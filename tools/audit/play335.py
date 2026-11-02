"""More of the 3.3.5a (build 12340) game protocol than wunet.wire335 has, for the release audit's headless player:
a background reader, the objects around you (SMSG_UPDATE_OBJECT and its compressed form), chat, gossip menus,
movement, melee, spells, mounts and character delete. Packet layouts follow AzerothCore 3.3.5a (Object.cpp
BuildMovementUpdate, Unit::BuildMovementPacket, MoveSplineFlag, Chat.cpp BuildChatPacket, GossipDef.cpp,
UpdateFields.h). Standard library only.

Checked two ways: the audit's fake realm (tests/fake_wrath.py) and Den Lite, whose packets are written by an
independent library (gtker's wow_world_messages): the player's own create, the companion's (entry, faction, level)
and the other player's create all have to read back right (tests/test_audit.py, den section).
"""
from __future__ import annotations

import asyncio
import math
import struct
import zlib

from wunet import wire335 as w

SMSG_UPDATE_OBJECT = 0x0A9
SMSG_DESTROY_OBJECT = 0x0AA
SMSG_COMPRESSED_UPDATE_OBJECT = 0x1F6
CMSG_MESSAGECHAT = 0x095
SMSG_MESSAGECHAT = 0x096
SMSG_GM_MESSAGECHAT = 0x3B3
SMSG_NOTIFICATION = 0x1CB
SMSG_INITIAL_SPELLS = 0x12A
SMSG_LEARNED_SPELL = 0x12B
CMSG_CAST_SPELL = 0x12E
SMSG_CAST_FAILED = 0x130
SMSG_SPELL_START = 0x131
SMSG_SPELL_GO = 0x132
SMSG_SPELL_FAILURE = 0x133
CMSG_SET_SELECTION = 0x13D
CMSG_ATTACKSWING = 0x141
CMSG_ATTACKSTOP = 0x142
SMSG_ATTACKSTART = 0x143
SMSG_ATTACKSTOP = 0x144
SMSG_ATTACKSWING_NOTINRANGE = 0x145
SMSG_ATTACKSWING_BADFACING = 0x146
SMSG_ATTACKSWING_DEADTARGET = 0x148
SMSG_ATTACKSWING_CANT_ATTACK = 0x149
CMSG_GOSSIP_SELECT_OPTION = 0x17C
SMSG_GOSSIP_MESSAGE = 0x17D
SMSG_GOSSIP_COMPLETE = 0x17E
SMSG_LEVELUP_INFO = 0x1D4
CMSG_CHAR_DELETE = 0x038
SMSG_CHAR_DELETE = 0x03C
MSG_MOVE_START_FORWARD = 0x0B5
MSG_MOVE_STOP = 0x0B7
MSG_MOVE_SET_FACING = 0x0DA
MSG_MOVE_HEARTBEAT = 0x0EE
CMSG_SET_ACTIVE_MOVER = 0x26A
SMSG_NEW_WORLD = 0x03E
MSG_MOVE_WORLDPORT_ACK = 0x0DC
MSG_MOVE_TELEPORT_ACK = 0x0C7
CMSG_CANCEL_MOUNT_AURA = 0x375
SMSG_LOGOUT_RESPONSE = w.SMSG_LOGOUT_RESPONSE
SMSG_LOGOUT_COMPLETE = w.SMSG_LOGOUT_COMPLETE

CHAR_DELETE_SUCCESS = 0x47
CHAT_SYSTEM, CHAT_SAY, CHAT_MONSTER_SAY = 0x00, 0x01, 0x0C
LANG_UNIVERSAL = 0
MOVE_FORWARD = 0x1

# UpdateFields.h, 3.3.5a
TYPE_UNIT, TYPE_PLAYER, TYPE_GAMEOBJECT = 3, 4, 5
F_ENTRY = 0x03
U_HEALTH, U_LEVEL, U_FACTION, U_FLAGS, U_DISPLAY, U_MOUNT, U_NPCFLAGS = 0x18, 0x36, 0x37, 0x3B, 0x43, 0x45, 0x52
P_VISIBLE_ITEM_1 = 0x11B               # PLAYER_VISIBLE_ITEM_1_ENTRYID; slot n: + 2 * n
SLOT_SHIRT, SLOT_MAINHAND, SLOT_RANGED = 3, 15, 17
UNIT_FLAG_NOT_ATTACKABLE = 0x2 | 0x80 | 0x100 | 0x2000000   # non attackable, not attackable 1, immune to players, not selectable

# BuildMovementUpdate flags
UF_TRANSPORT, UF_HAS_TARGET, UF_UNKNOWN, UF_LOWGUID, UF_LIVING = 0x02, 0x04, 0x08, 0x10, 0x20
UF_STATIONARY, UF_VEHICLE, UF_POSITION, UF_ROTATION = 0x40, 0x80, 0x100, 0x200
MF_ONTRANSPORT, MF_FALLING, MF_SWIMMING, MF_FLYING = 0x200, 0x1000, 0x200000, 0x2000000
MF_SPLINE_ELEVATION, MF_SPLINE_ENABLED = 0x4000000, 0x8000000
MF2_ALWAYS_PITCH, MF2_INTERPOLATED = 0x20, 0x400
SPLINE_FINAL_POINT, SPLINE_FINAL_TARGET, SPLINE_FINAL_ANGLE = 0x8000, 0x10000, 0x20000


class ParseError(Exception):
    pass


# ---------------------------------------------------------------------- reading and writing
class Buf:
    def __init__(self, b: bytes, p: int = 0):
        self.b, self.p = b, p

    def take(self, fmt: str):
        try:
            v = struct.unpack_from("<" + fmt, self.b, self.p)
        except struct.error as e:
            raise ParseError(f"short packet at {self.p}: {e}") from None
        self.p += struct.calcsize("<" + fmt)
        return v if len(v) > 1 else v[0]

    def u8(self):
        return self.take("B")

    def u32(self):
        return self.take("I")

    def pguid(self) -> int:
        mask, g = self.u8(), 0
        for i in range(8):
            if mask & (1 << i):
                g |= self.u8() << (8 * i)
        return g

    def cstr(self) -> str:
        end = self.b.find(b"\x00", self.p)
        if end < 0:
            raise ParseError("unterminated string")
        s = self.b[self.p:end].decode("utf-8", "replace")
        self.p = end + 1
        return s

    def left(self) -> int:
        return len(self.b) - self.p


def pack_guid(g: int) -> bytes:
    mask, out = 0, bytearray()
    for i in range(8):
        byte = (g >> (8 * i)) & 0xFF
        if byte:
            mask |= 1 << i
            out.append(byte)
    return bytes([mask]) + bytes(out)


def movement_info(flags: int, time_ms: int, x: float, y: float, z: float, o: float) -> bytes:
    """The client's MovementInfo (no transport, no pitch, not falling)."""
    return struct.pack("<IHIffffI", flags, 0, time_ms & 0xFFFFFFFF, x, y, z, o, 0)


def read_movement(r: Buf) -> dict:
    """BuildMovementUpdate: returns {"pos": (x, y, z, o) or None}."""
    flags = r.take("H")
    pos = None
    if flags & UF_LIVING:
        mflags, mflags2, _t = r.take("IHI")
        pos = r.take("ffff")
        if mflags & MF_ONTRANSPORT:
            r.pguid()
            r.take("ffff")
            r.take("IB")
            if mflags2 & MF2_INTERPOLATED:
                r.u32()
        if mflags & (MF_SWIMMING | MF_FLYING) or mflags2 & MF2_ALWAYS_PITCH:
            r.take("f")
        r.u32()                                  # fall time
        if mflags & MF_FALLING:
            r.take("ffff")
        if mflags & MF_SPLINE_ELEVATION:
            r.take("f")
        r.take("9f")                             # speeds
        if mflags & MF_SPLINE_ENABLED:
            sflags = r.u32()
            if sflags & SPLINE_FINAL_ANGLE:
                r.take("f")
            elif sflags & SPLINE_FINAL_TARGET:
                r.take("Q")
            elif sflags & SPLINE_FINAL_POINT:
                r.take("fff")
            r.take("III")                        # time passed, duration, id
            r.take("fff")                        # duration mod, next, vertical acceleration
            r.u32()                              # effect start time
            n = r.u32()
            if n > 1000:
                raise ParseError(f"spline with {n} points")
            r.take(f"{3 * n}f")
            r.u8()
            r.take("fff")
    elif flags & UF_POSITION:
        r.pguid()
        x, y, z = r.take("fff")
        r.take("fff")
        o = r.take("f")
        r.take("f")
        pos = (x, y, z, o)
    elif flags & UF_STATIONARY:
        pos = r.take("ffff")
    if flags & UF_UNKNOWN:
        r.u32()
    if flags & UF_LOWGUID:
        r.u32()
    if flags & UF_HAS_TARGET:
        r.pguid()
    if flags & UF_TRANSPORT:
        r.u32()
    if flags & UF_VEHICLE:
        r.take("If")
    if flags & UF_ROTATION:
        r.take("q")
    return {"pos": pos, "flags": flags}


def read_values(r: Buf) -> dict:
    n = r.u8()
    if n > 64:
        raise ParseError(f"{n} mask blocks")
    masks = [r.u32() for _ in range(n)]
    out = {}
    for bi, m in enumerate(masks):
        bit = 0
        while m:
            if m & 1:
                out[bi * 32 + bit] = r.u32()
            m >>= 1
            bit += 1
    return out


def parse_update(body: bytes) -> list[dict]:
    """SMSG_UPDATE_OBJECT's blocks: {"kind": values|movement|create|out|near, "guid", ...}."""
    r = Buf(body)
    count, out = r.u32(), []
    if count > 5000:
        raise ParseError(f"{count} blocks")
    for _ in range(count):
        kind = r.u8()
        if kind == 0:
            g = r.pguid()
            out.append({"kind": "values", "guid": g, "fields": read_values(r)})
        elif kind == 1:
            g = r.pguid()
            out.append({"kind": "movement", "guid": g, **read_movement(r)})
        elif kind in (2, 3):
            g = r.pguid()
            t = r.u8()
            mv = read_movement(r)
            out.append({"kind": "create", "guid": g, "type": t, "pos": mv["pos"], "fields": read_values(r)})
        elif kind in (4, 5):
            n = r.u32()
            out.append({"kind": "out" if kind == 4 else "near", "guids": [r.pguid() for _ in range(n)]})
        else:
            raise ParseError(f"update block type {kind}")
    return out


def uncompress_update(body: bytes) -> bytes:
    size = struct.unpack_from("<I", body)[0]
    data = zlib.decompress(body[4:])
    if len(data) != size:
        raise ParseError("compressed update: wrong size")
    return data


def parse_chat(body: bytes, gm: bool = False) -> dict:
    r = Buf(body)
    ctype, _lang, sender, _flags = r.take("BiQI")
    name, target = "", 0
    if ctype in (0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x29, 0x2A, 0x35):     # monster lines, raid boss lines, battlenet
        r.u32()
        name = r.cstr()
        target = r.take("Q")
        if target and (target >> 48) not in (0x0000, 0xF140):           # a creature target: its name too
            r.u32()
            r.cstr()
    elif ctype == 0x08:                                                 # whisper foreign
        r.u32()
        name = r.cstr()
        target = r.take("Q")
    elif ctype in (0x24, 0x25, 0x26):                                   # bg system lines
        target = r.take("Q")
        if target and (target >> 48) != 0:
            r.u32()
            r.cstr()
    elif ctype in (0x30, 0x31):                                         # achievements
        target = r.take("Q")
    else:
        if gm:
            r.u32()
            name = r.cstr()
        if ctype == 0x11:                                               # channel
            r.cstr()
        target = r.take("Q")
    r.u32()
    text = r.cstr()
    return {"type": ctype, "sender": sender, "name": name, "text": text}


def parse_gossip(body: bytes) -> dict:
    r = Buf(body)
    guid, menu, _text, n = r.take("QIII")
    items = []
    for _ in range(min(n, 64)):
        idx, _icon, coded, _money = r.take("IBBI")
        items.append({"index": idx, "coded": bool(coded), "text": r.cstr(), "box": r.cstr()})
    return {"guid": guid, "menu": menu, "items": items}


def strip_colours(s: str) -> str:
    import re
    return re.sub(r"\|c[0-9a-fA-F]{8}|\|r", "", s)


# ---------------------------------------------------------------------- the session
class Session:
    """A logged-in world session with a reader task: every packet lands in .events and updates the state."""

    def __init__(self, world: w.World):
        self.w = world
        self.w.timeout = 3600
        self.events: list[tuple[int, bytes]] = []
        self.cond = asyncio.Condition()
        self.closed: BaseException | None = None
        self.objects: dict[int, dict] = {}
        self.me = 0
        self.pos, self.map = [0.0, 0.0, 0.0, 0.0], 0
        self.spells: set[int] = set()
        self.chat: list[dict] = []
        self.parse_errors: list[str] = []
        self.task = asyncio.get_running_loop().create_task(self._read())
        self.t0 = asyncio.get_running_loop().time()

    # -------------------------------------------------- reading
    async def _read(self):
        try:
            while True:
                op, body = await self.w.recv()
                if op == w.SMSG_TIME_SYNC_REQ and len(body) >= 4:
                    self.send(w.CMSG_TIME_SYNC_RESP, body[:4] + struct.pack("<I", self.ms()))
                elif op == SMSG_NEW_WORLD and len(body) >= 20:      # a far teleport (the boost's trip to Dalaran)
                    self.map, *pos = struct.unpack_from("<Iffff", body)
                    self.pos = list(pos)
                    self.send(MSG_MOVE_WORLDPORT_ACK)
                elif op == MSG_MOVE_TELEPORT_ACK:                    # a near teleport: say we are there
                    r = Buf(body)
                    if r.pguid() == self.me:
                        counter = r.u32()
                        self.send(MSG_MOVE_TELEPORT_ACK, pack_guid(self.me) + struct.pack("<II", counter, self.ms()))
                try:
                    self._note(op, body)
                except (ParseError, struct.error, ValueError, zlib.error, IndexError) as e:
                    self.parse_errors.append(f"{op:#x}: {e}")
                async with self.cond:
                    self.events.append((op, body))
                    self.cond.notify_all()
        except (asyncio.IncompleteReadError, ConnectionError, OSError, asyncio.TimeoutError, w.WowError) as e:
            self.closed = e
            async with self.cond:
                self.cond.notify_all()
        except asyncio.CancelledError:
            raise

    def _note(self, op, body):
        if op == SMSG_COMPRESSED_UPDATE_OBJECT:
            op, body = SMSG_UPDATE_OBJECT, uncompress_update(body)
        if op == SMSG_UPDATE_OBJECT:
            for blk in parse_update(body):
                if blk["kind"] == "create":
                    self.objects[blk["guid"]] = {"guid": blk["guid"], "type": blk["type"], "pos": blk["pos"],
                                                 "fields": dict(blk["fields"])}
                elif blk["kind"] == "values" and blk["guid"] in self.objects:
                    self.objects[blk["guid"]]["fields"].update(blk["fields"])
                elif blk["kind"] == "movement" and blk["guid"] in self.objects and blk["pos"]:
                    self.objects[blk["guid"]]["pos"] = blk["pos"]
                elif blk["kind"] == "out":
                    for g in blk["guids"]:
                        self.objects.pop(g, None)
        elif op == SMSG_DESTROY_OBJECT and len(body) >= 8:
            self.objects.pop(struct.unpack_from("<Q", body)[0], None)
        elif op == SMSG_INITIAL_SPELLS:
            r = Buf(body)
            r.u8()
            for _ in range(r.take("H")):
                self.spells.add(r.u32())
                r.take("H")
        elif op == SMSG_LEARNED_SPELL and len(body) >= 4:
            self.spells.add(struct.unpack_from("<I", body)[0])
        elif op in (SMSG_MESSAGECHAT, SMSG_GM_MESSAGECHAT):
            self.chat.append(parse_chat(body, op == SMSG_GM_MESSAGECHAT))
        elif op == SMSG_NOTIFICATION:
            self.chat.append({"type": -1, "sender": 0, "name": "", "text": Buf(body).cstr()})

    async def wait(self, pred, timeout: float, start: int | None = None):
        """The first event from index `start` (default: now) for which pred(op, body) is true, else None."""
        i = len(self.events) if start is None else start
        loop = asyncio.get_running_loop()
        end = loop.time() + timeout
        async with self.cond:
            while True:
                while i < len(self.events):
                    op, body = self.events[i]
                    i += 1
                    try:
                        if pred(op, body):
                            return op, body
                    except (struct.error, ParseError, ValueError, IndexError):
                        pass
                if self.closed:
                    raise w.WowError(f"the server closed the connection ({type(self.closed).__name__})")
                left = end - loop.time()
                if left <= 0:
                    return None
                try:
                    await asyncio.wait_for(self.cond.wait(), left)
                except asyncio.TimeoutError:
                    return None

    def mark(self) -> int:
        return len(self.events)

    @staticmethod
    def chat_of(op, body):
        if op in (SMSG_MESSAGECHAT, SMSG_GM_MESSAGECHAT):
            c = parse_chat(body, op == SMSG_GM_MESSAGECHAT)
        elif op == SMSG_NOTIFICATION:
            c = {"type": -1, "sender": 0, "name": "", "text": Buf(body).cstr()}
        else:
            return None
        c["text"] = strip_colours(c["text"])
        return c

    def lines_since(self, start: int) -> list[dict]:
        out = []
        for op, body in self.events[start:]:
            try:
                c = self.chat_of(op, body)
            except (ParseError, struct.error):
                c = None
            if c:
                out.append(c)
        return out

    async def wait_line(self, match, timeout: float, start: int | None = None):
        """The first chat line (system, say, monster say, notification) from `start` that match(line) accepts."""
        ev = await self.wait(lambda op, b: (lambda c: bool(c) and match(c))(self.chat_of(op, b)), timeout, start)
        return self.chat_of(*ev) if ev else None

    # -------------------------------------------------- writing
    def ms(self) -> int:
        return int((asyncio.get_running_loop().time() - self.t0) * 1000) & 0xFFFFFFFF

    def send(self, op: int, body: bytes = b""):
        if not self.closed:
            self.w.send(op, body)

    async def flush(self):
        try:
            await self.w.writer.drain()
        except (ConnectionError, OSError) as e:
            self.closed = e

    async def say(self, text: str):
        self.send(CMSG_MESSAGECHAT, struct.pack("<II", CHAT_SAY, LANG_UNIVERSAL) + text.encode() + b"\x00")
        await self.flush()

    async def char_enum(self, timeout=20):
        at = self.mark()
        self.send(w.CMSG_CHAR_ENUM)
        await self.flush()
        ev = await self.wait(lambda op, b: op == w.SMSG_CHAR_ENUM, timeout, at)
        if not ev:
            raise w.WowError("no character list")
        return w.parse_char_enum(ev[1])

    async def char_create(self, name, race, cls, gender=0, timeout=20) -> int:
        at = self.mark()
        self.send(w.CMSG_CHAR_CREATE, name.encode() + b"\x00" + bytes([race, cls, gender, 0, 0, 0, 0, 0, 0]))
        await self.flush()
        ev = await self.wait(lambda op, b: op == w.SMSG_CHAR_CREATE, timeout, at)
        if not ev:
            raise w.WowError("no answer to character create")
        return ev[1][0]

    async def char_delete(self, guid, timeout=20) -> int:
        at = self.mark()
        self.send(CMSG_CHAR_DELETE, struct.pack("<Q", guid))
        await self.flush()
        ev = await self.wait(lambda op, b: op == SMSG_CHAR_DELETE, timeout, at)
        if not ev:
            raise w.WowError("no answer to character delete")
        return ev[1][0]

    async def enter(self, guid, timeout=60) -> dict:
        at = self.mark()
        self.me = guid
        self.send(w.CMSG_PLAYER_LOGIN, struct.pack("<Q", guid))
        await self.flush()
        ev = await self.wait(lambda op, b: op in (w.SMSG_LOGIN_VERIFY_WORLD, w.SMSG_CHARACTER_LOGIN_FAILED), timeout, at)
        if not ev:
            raise w.WowError(f"not in the world within {timeout}s")
        if ev[0] == w.SMSG_CHARACTER_LOGIN_FAILED:
            raise w.WowError(f"character login failed (code {ev[1][0] if ev[1] else None})")
        mapid, x, y, z, o = struct.unpack_from("<Iffff", ev[1])
        self.pos = [x, y, z, o]
        self.map = mapid
        self.send(CMSG_SET_ACTIVE_MOVER, struct.pack("<Q", guid))
        await self.flush()
        return {"map": mapid, "x": x, "y": y, "z": z, "o": o}

    async def ping(self, n=7, timeout=15) -> bool:
        at = self.mark()
        self.send(w.CMSG_PING, struct.pack("<II", n, 50))
        await self.flush()
        return bool(await self.wait(lambda op, b: op == w.SMSG_PONG, timeout, at))

    async def move(self, op: int, flags: int, x, y, z, o):
        self.send(op, pack_guid(self.me) + movement_info(flags, self.ms(), x, y, z, o))
        await self.flush()
        self.pos = [x, y, z, o]

    async def walk(self, dx: float, dy: float, speed: float = 7.0):
        """Run from here by (dx, dy) yards as a client does: start, a heartbeat every half second, stop."""
        x, y, z, _ = self.pos
        dist = math.hypot(dx, dy)
        o = math.atan2(dy, dx) % (2 * math.pi)
        await self.move(MSG_MOVE_START_FORWARD, MOVE_FORWARD, x, y, z, o)
        steps = max(1, int(dist / (speed * 0.5)))
        for i in range(1, steps + 1):
            await asyncio.sleep(0.5)
            f = i / steps
            await self.move(MSG_MOVE_HEARTBEAT, MOVE_FORWARD, x + dx * f, y + dy * f, z, o)
        await self.move(MSG_MOVE_STOP, 0, x + dx, y + dy, z, o)

    async def face(self, x, y):
        px, py, pz, _ = self.pos
        await self.move(MSG_MOVE_SET_FACING, 0, px, py, pz, math.atan2(y - py, x - px) % (2 * math.pi))

    async def cast(self, spell: int, target: int = 0, timeout: float = 6.0):
        """Cast; returns ("go"|"start"|"failed", detail) or None when nothing answered."""
        at = self.mark()
        targets = struct.pack("<I", 0x2) + pack_guid(target) if target else struct.pack("<I", 0)
        self.send(CMSG_CAST_SPELL, struct.pack("<BIB", 1, spell, 0) + targets)
        await self.flush()

        def mine(op, b):
            if op == SMSG_CAST_FAILED:
                return struct.unpack_from("<I", b, 1)[0] == spell
            if op in (SMSG_SPELL_GO, SMSG_SPELL_FAILURE):
                r = Buf(b)
                r.pguid()
                caster = r.pguid()
                r.u8()
                return caster == self.me and r.u32() == spell
            return False
        ev = await self.wait(mine, timeout, at)
        if not ev:
            started = any(op == SMSG_SPELL_START for op, _ in self.events[at:])
            return ("start", "started, no result") if started else None
        if ev[0] == SMSG_CAST_FAILED:
            return "failed", f"cast failed, reason {ev[1][5]}"
        if ev[0] == SMSG_SPELL_FAILURE:
            return "failed", "interrupted"
        return "go", ""

    async def logout(self, within: float = 30) -> str:
        """"" when logged out, else why not."""
        at = self.mark()
        self.send(w.CMSG_LOGOUT_REQUEST)
        await self.flush()
        ev = await self.wait(lambda op, b: op == SMSG_LOGOUT_RESPONSE, 15, at)
        if not ev:
            return "no answer to the logout request"
        reason = struct.unpack_from("<I", ev[1])[0] if len(ev[1]) >= 4 else 0
        if reason:
            return f"logout refused (reason {reason}: in combat or falling)"
        done = await self.wait(lambda op, b: op == SMSG_LOGOUT_COMPLETE, within, at)
        return "" if done else "logout never completed"

    def units(self):
        return [o for o in self.objects.values() if o["type"] == TYPE_UNIT]

    def players(self):
        return [o for o in self.objects.values() if o["type"] == TYPE_PLAYER]

    def my(self, field: int, default=0) -> int:
        o = self.objects.get(self.me)
        return o["fields"].get(field, default) if o else default

    def close(self):
        self.task.cancel()
        try:
            self.w.writer.close()
        except Exception:  # noqa: BLE001
            pass


async def connect(auth_host, auth_port, user, password, realm_pick=None, world_override=None, timeout=30):
    """Logon, realm list, world auth. Returns (Session, realm dict, realms)."""
    K, realms = await w.logon(auth_host, auth_port, user, password, timeout=timeout)
    if not realms:
        raise w.WowError("the realm list is empty")
    realm = next((r for r in realms if realm_pick and realm_pick(r)), realms[0])
    host, port = w.split_host_port(world_override or realm["address"], 8085)
    world = await w.world_session(host, port, user, K, realm_id=realm["id"], timeout=timeout)
    return Session(world), realm, realms
