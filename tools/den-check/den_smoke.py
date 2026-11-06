#!/usr/bin/env python3
"""Den smoke, no game window (Control: den-smoke; docs/DEN-NEXT.md). The real Den Lite program on this PC on spare
ports with a scratch Dens folder, and the headless client (wowclient.py) speaking the game's own protocol:

    py -3 -I tools/den-check/den_smoke.py [<den-lite program>]

1. the one click's way in: sign in, realm list, world, the ready-made character, in the world (timed);
2. Sky Island: `.den island` floats the Den, the landing by the tree, its welcome tale under way;
3. a fall off the island is put back by the tree;
4. a prop placed there;
5. the mouse: the editor's own verbs (Gizmo.lua's `preview`, `hover`, `where`) give the copy where the expected
   model says (den_check.project: the same camera numbers the hover calibration fits);
6. the island as a code (`.den pack`): it decodes (raw deflate, the crc) to the island's own look;
7. home again (`.den island off`).

Nothing here needs the game, Windows, or a window with the keyboard: it is the protocol-level check the den-move test
(which drives the real game) stands on. Logs: tools/den-check/logs/den-smoke-latest.txt.
"""
import json
import math
import re
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import den_check  # noqa: E402
import wowclient  # noqa: E402

SMSG_MESSAGECHAT = 0x096
CMSG_MESSAGECHAT = 0x095
MSG_MOVE_TELEPORT_ACK = 0x0C7
MSG_MOVE_HEARTBEAT = 0x0EE
FALLING = 0x1000
# labs/den-lite/src/skyisland.rs
SKY = {"x": 15700.0, "y": 15700.0, "z": 180.0, "landing": (0.0, -4.0), "reach": 78.0}
CAM = {"fov": math.pi / 2, "pivot": 2.0, "dist": 12.0, "pitch": 0.5}


def system_line(op, body):
    """A system chat line's text (SMSG_MESSAGECHAT type 0: u8 type, u32 lang, u64 sender, u32 flags, u64 target, u32
    length, the text), else None."""
    if op != SMSG_MESSAGECHAT or len(body) < 30 or body[0] != 0 or body[1:5] == b"\xff\xff\xff\xff":
        return None
    n = struct.unpack_from("<I", body, 25)[0]
    return body[29:29 + max(0, n - 1)].decode("utf-8", "replace")


def addon_line(op, body):
    """An addon line's text ("OVMOD\\t<mod>\\t<name>\\t<i>/<n>\\t<data>"), else None."""
    if op != SMSG_MESSAGECHAT or len(body) < 30 or body[1:5] != b"\xff\xff\xff\xff":
        return None
    n = struct.unpack_from("<I", body, 25)[0]
    return body[29:29 + max(0, n - 1)].decode("utf-8", "replace")


def say_body(text):
    """CMSG_MESSAGECHAT, say (type 1, Common 7)."""
    return struct.pack("<II", 1, 7) + text.encode() + b"\0"


def teleport_ack(tp):
    """The game's answer to a near teleport (MSG_MOVE_TELEPORT_ACK: packed guid 1, its counter, a time)."""
    return b"\x01\x01" + tp[2:6] + struct.pack("<I", 1000)


def teleport_spot(tp):
    """x, y, z of a near teleport (packed guid, counter, then the movement info: flags, flags2, time, x y z o)."""
    mask = tp[0]
    at = 1 + bin(mask).count("1") + 4 + 4 + 2 + 4
    return struct.unpack_from("<fff", tp, at)


def heartbeat(x, y, z, flags=0):
    b = b"\x01\x01" + struct.pack("<IHI", flags, 0, 2000) + struct.pack("<ffff", x, y, z, 0.0) + struct.pack("<I", 500)
    if flags & FALLING:
        b += struct.pack("<ffff", -30.0, 0.0, 1.0, 0.0)
    return b


def decode_code(code):
    """A Den code (labs/den-lite denpack.rs) to its text; raises ValueError."""
    p = code.strip().split(".")
    if len(p) != 5 or p[0] != "ley1":
        raise ValueError("not a code")
    raw = __import__("base64").urlsafe_b64decode(p[3] + "=" * (-len(p[3]) % 4))
    text = zlib.decompress(raw, -15) if p[2] == "z" else raw
    if "%08x" % (zlib.crc32(text) & 0xFFFFFFFF) != p[4]:
        raise ValueError("its check doesn't match")
    return p[1], text.decode("utf-8")


class Den:
    """Den Lite started for the smoke: spare ports, a scratch Dens folder."""

    def __init__(self, program):
        self.dir = Path(tempfile.mkdtemp(prefix="den-smoke-"))
        self.lp, self.wp = den_check.free_port(), den_check.free_port()
        t0 = time.time()
        self.p = subprocess.Popen([str(program), "--dens", str(self.dir), "--bind", "127.0.0.1", "--login-port", str(self.lp), "--world-port", str(self.wp),
                                   "--password", "den", "--no-friends", "--no-companion"],
                                  stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                  creationflags=0x08000000 if os.name == "nt" else 0)
        for _ in range(100):
            if den_check.port_open(self.lp):
                break
            time.sleep(0.05)
        self.start_ms = (time.time() - t0) * 1000

    def close(self):
        self.p.terminate()
        try:
            self.p.wait(5)
        except subprocess.TimeoutExpired:
            self.p.kill()
        shutil.rmtree(self.dir, ignore_errors=True)


class Client:
    def __init__(self, lp):
        t0 = time.time()
        k, realms = wowclient.login("127.0.0.1", lp, "DEN", "den")
        self.w = wowclient.World(realms[0][1], "DEN", k)
        self.w.until(wowclient.SMSG_AUTH_RESPONSE)
        self.w.send(wowclient.CMSG_CHAR_ENUM)
        b = self.w.until(wowclient.SMSG_CHAR_ENUM)
        guid = struct.unpack_from("<Q", b, 1)[0]
        e = b.index(b"\0", 9)
        self.name = b[9:e].decode("utf-8", "replace")
        self.w.send(wowclient.CMSG_PLAYER_LOGIN, struct.pack("<Q", guid))
        self.w.until(wowclient.SMSG_LOGIN_VERIFY_WORLD)
        self.enter_ms = (time.time() - t0) * 1000
        self.teleports = []

    def say(self, text):
        self.w.send(CMSG_MESSAGECHAT, say_body(text))

    def until_line(self, prefix, seconds=10):
        """System lines until one starts with `prefix` (teleports on the way answered as the game does)."""
        end = time.time() + seconds
        while time.time() < end:
            op, b = self.w.frame()
            if op == MSG_MOVE_TELEPORT_ACK:
                self.teleports.append(teleport_spot(b))
                self.w.send(MSG_MOVE_TELEPORT_ACK, teleport_ack(b))
                continue
            t = system_line(op, b)
            if t is not None and t.startswith(prefix):
                return t
        raise TimeoutError("no line starting %r" % prefix)

    def addon(self, text):
        self.w.addon(self.name, "OVMOD", text)

    def until_addon(self, want, seconds=10):
        end = time.time() + seconds
        while time.time() < end:
            op, b = self.w.frame()
            t = addon_line(op, b)
            if t is not None and t.startswith(want):
                return t[len(want):]
        raise TimeoutError("no addon line %r" % want)


def memory_mb(pid, run=subprocess.run):
    """A process's memory in use now (MB): Windows' tasklist, Linux's /proc; None when it can't be read."""
    try:
        if os.name == "nt":
            out = run(["tasklist", "/fi", "PID eq %d" % pid, "/fo", "csv", "/nh"], capture_output=True, text=True, timeout=15).stdout
            cells = [c.strip('"') for c in out.strip().split('","')]
            return round(int(re.sub(r"[^0-9]", "", cells[-1])) / 1024, 1) if cells and re.search(r"[0-9]", cells[-1]) else None
        for line in Path("/proc/%d/status" % pid).read_text().splitlines():
            if line.startswith("VmRSS:"):
                return round(int(line.split()[1]) / 1024, 1)
    except Exception:  # noqa: BLE001 - not known
        return None
    return None


def smoke(rep, program):
    den = Den(program)
    try:
        rep.add("INFO", "Den Lite started", "%.0f ms, scratch Dens folder" % den.start_ms)
        try:
            c = Client(den.lp)
        except Exception as e:  # noqa: BLE001 - said in plain words
            rep.add("FAIL", "into the world", str(e)[:200], "Send logs\\den-smoke-latest.txt.")
            return
        rep.ok(c.enter_ms < 5000, "the one click's way in: sign in, realm, the ready character, the world", "%.0f ms" % c.enter_ms)
        # 2. Sky Island
        c.say(".den island")
        c.until_line("Off to Sky Island")
        c.until_line("You are at Sky Island")
        lx, ly = SKY["x"] + SKY["landing"][0], SKY["y"] + SKY["landing"][1]
        t = c.teleports[-1] if c.teleports else (0, 0, 0)
        rep.ok(abs(t[0] - lx) < 0.05 and abs(t[1] - ly) < 0.05, "Sky Island: landed by the tree", "%.1f %.1f %.1f" % t)
        c.w.send(MSG_MOVE_HEARTBEAT, heartbeat(lx, ly, SKY["z"] + 0.1))
        c.say(".den tale status")
        line = c.until_line("Welcome to Sky Island")
        rep.ok("step 1 of 4" in line, "Sky Island: its welcome tale under way", line[:120])
        # 3. off the edge
        c.w.send(MSG_MOVE_HEARTBEAT, heartbeat(SKY["x"] + 70, SKY["y"], SKY["z"] - 30, FALLING))
        c.until_line("You fell off the island")
        t = c.teleports[-1]
        rep.ok(abs(t[0] - lx) < 0.05 and abs(t[1] - ly) < 0.05, "a fall off the island: back by the tree", "%.1f %.1f %.1f" % t)
        c.w.send(MSG_MOVE_HEARTBEAT, heartbeat(lx, ly, SKY["z"] + 0.1))
        # 4. a prop placed there
        c.say(".den place chair")
        c.say(".den where")
        line = c.until_line("You stand at")
        rep.ok(True, "a prop placed on the island", line[:120])
        # 5. the mouse against the model (the camera's first guesses: Den Lite's scratch folder has no calibration)
        c.addon("wubuild\tcmd\t1/1\tmode build")
        aspect = 16 / 9
        c.addon("wubuild\tcmd\t1/1\tpreview 2413 0 -0.3 %s" % aspect)
        worst = 0.0
        for u, v in ((0.05, -0.28), (0.2, -0.2), (-0.25, -0.3), (0.1, -0.12)):
            c.addon("wubuild\tcmd\t1/1\thover %s %s %s" % (u, v, aspect))
            c.addon("wubuild\tcmd\t1/1\twhere")
            g = c.until_addon("OVMOD\twubuild\tghost\t1/1\t")
            gx, gy, gz = [float(x) for x in g.split(",")[:3]]
            want = den_check.project(CAM, (lx, ly, SKY["z"] + 0.1, 0.0), u, v, aspect, SKY["z"])
            if want is None:
                continue
            worst = max(worst, math.hypot(gx - want[0], gy - want[1]))
        rep.ok(worst < 0.15, "the mouse: the copy where the model says", "at most %.3f yd apart over 4 hovers" % worst,
               "Den Lite's ray and the model disagree: run den-lite-test (replay::) and send both logs.")
        c.addon("wubuild\tcmd\t1/1\tpreview off")
        # 6. the island as a code
        c.say(".den pack smoke")
        c.until_line("Your Den as a code")
        try:
            kind, text = decode_code((den.dir / "smoke.denpack").read_text())
            look = json.loads(text).get("room", {}).get("look")
            rep.ok(kind == "den" and look == "sky", "the island as a code (no server: the code is the Den)", "%s, look %s" % (kind, look))
        except Exception as e:  # noqa: BLE001
            rep.add("FAIL", "the island as a code", str(e)[:200])
        # 7. home
        c.say(".den island off")
        c.until_line("Off to your Den")
        rep.ok(True, "home again (.den island off)")
        mb = memory_mb(den.p.pid)
        rep.add("INFO", "Den Lite's memory with the island and one player", "%s MB" % mb if mb is not None else "not readable here")
        c.w.close()
    except TimeoutError as e:
        rep.add("FAIL", "the smoke stopped", str(e), "Den Lite didn't answer: build it again (den-lite) and send logs\\den-smoke-latest.txt.")
    finally:
        den.close()


def program_of(argv):
    if len(argv) > 1:
        return Path(argv[1])
    if os.environ.get("DEN_LITE_BIN"):
        return Path(os.environ["DEN_LITE_BIN"])
    return den_check.app_dir() / "den-lite" / "bin" / den_check.exe_name("den-lite")


def main(argv=None):
    argv = sys.argv if argv is None else argv
    rep = den_check.Report()
    title = "DEN SMOKE (no game window)"
    print("%s (%s)" % (title, time.strftime("%Y-%m-%d %H:%M")))
    prog = program_of(argv)
    if rep.ok(prog.is_file(), "the Den Lite program", str(prog), "Press den-lite in Control first (it builds Den Lite on this PC)."):
        smoke(rep, prog)
    code = rep.end(title)
    logs = HERE / "logs"
    logs.mkdir(exist_ok=True)
    (logs / "den-smoke-latest.txt").write_text("%s (%s)\n%s\n%s" % (title, time.strftime("%Y-%m-%d %H:%M"), rep.text(), "PASS" if code == 0 else "FAIL"), encoding="utf-8")
    return code


if __name__ == "__main__":
    sys.exit(main())
