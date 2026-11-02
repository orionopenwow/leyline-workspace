"""Your Den / Play solo and Invite friends, as a new player: the real Den Lite program on this PC (loopback only,
its own free ports and a temporary Dens folder, removed after), driven by the headless 3.3.5a client.

    login DEN / den, the quick start, .den place, .den save, Jaina answers "Jaina, tips", the Den kept by itself
    over a restart, a guest login for a friend (den-lite guest add), the friend's client in through the friends'
    listener, both see each other, the login taken back (revoke), Den Lite stopped.
"""
from __future__ import annotations

import asyncio
import hashlib
import json
import os
import shutil
import socket
import subprocess
import tempfile
import time
from pathlib import Path

from common import FAIL, PASS, SKIP, appdata, is_windows, short
import play335 as p
from wunet import wire335 as w

F = "Your Den"
HOST_GUID = 1    # labs/den-lite world.rs PLAYER_GUID


def free_port() -> int:
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def find_den_lite(ctx) -> tuple[str | None, str]:
    """The program: $DEN_LITE_BIN, the launcher's copy (%APPDATA%\\Leyline\\den-lite\\bin), or den-lite-latest
    downloaded and checked against its .sha256."""
    env = os.environ.get("DEN_LITE_BIN")
    if env and Path(env).is_file():
        return env, "DEN_LITE_BIN"
    ad = appdata()
    if ad:
        exe = ad / "Leyline" / "den-lite" / "bin" / ("den-lite.exe" if is_windows() else "den-lite")
        if exe.is_file():
            return str(exe), "the launcher's copy"
    got = ctx.cache.get("den_lite_download")
    if got:
        return got, "den-lite-latest (downloaded, checksum matched)"
    return None, "no den-lite on this PC and the download didn't check out"


def download_den_lite(ctx) -> tuple[str | None, str]:
    """den-lite-latest for this PC with its .sha256; the file goes to the audit's temp folder."""
    rel = ctx.cfg["releases"]
    name = rel["den_lite_windows"] if is_windows() else rel["den_lite_linux"]
    base = rel["den_lite"]
    sha = ctx.get(base + name + ".sha256", timeout=30)
    if sha.status != 200:
        return None, f"{name}.sha256: HTTP {sha.status or sha.error}"
    want = (sha.text.split() or [""])[0].lower()
    dest = Path(ctx.temp) / name
    h = hashlib.sha256()
    with open(dest, "wb") as f:
        def sink(chunk):
            h.update(chunk)
            f.write(chunk)
        r = ctx.get(base + name, timeout=120, sink=sink)
    if r.status != 200:
        return None, f"{name}: HTTP {r.status or r.error}"
    if h.hexdigest() != want:
        return None, f"{name}: sha256 {h.hexdigest()[:12]} is not the {want[:12]} its .sha256 names"
    if not is_windows():
        os.chmod(dest, 0o755)
    ctx.cache["den_lite_download"] = str(dest)
    return str(dest), f"{dest.stat().st_size // 1024} KB, sha256 matches"


class Program:
    def __init__(self, exe, dens, lp, wp):
        self.exe, self.dens, self.lp, self.wp = exe, dens, lp, wp
        flags = getattr(subprocess, "CREATE_NO_WINDOW", 0)
        self.proc = subprocess.Popen([exe, "--dens", str(dens), "--bind", "127.0.0.1", "--password", "den",
                                      "--login-port", str(lp), "--world-port", str(wp)],
                                     stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                     creationflags=flags)

    def up(self, secs=10) -> bool:
        end = time.time() + secs
        while time.time() < end:
            if self.proc.poll() is not None:
                return False
            try:
                with socket.create_connection(("127.0.0.1", self.lp), timeout=0.5):
                    return True
            except OSError:
                time.sleep(0.1)
        return False

    def stop(self, secs=10) -> bool:
        """The launcher's way: the stop file (den-lite.stop). True when it exited by itself."""
        try:
            (self.dens / "den-lite.stop").write_text("audit\n")
        except OSError:
            pass
        try:
            self.proc.wait(secs)
            ok = True
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait(5)
            ok = False
        try:
            (self.dens / "den-lite.stop").unlink()
        except OSError:
            pass
        return ok


def guest(exe, dens, *args) -> dict:
    out = subprocess.run([exe, "guest", args[0], "--dens", str(dens), *args[1:]], capture_output=True, text=True, timeout=20,
                         creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    if out.returncode != 0:
        raise RuntimeError((out.stderr or out.stdout).strip()[:200])
    return json.loads(out.stdout.strip().splitlines()[-1])


async def enter_den(lp, user, password, timeout=20):
    s, realm, _ = await p.connect("127.0.0.1", lp, user, password, timeout=timeout)
    chars = await s.char_enum()
    if not chars:
        raise w.WowError("no character in the list")
    start = s.mark()
    await s.enter(chars[0]["guid"])
    return s, realm, chars[0], start


def props_in(line: str) -> int | None:
    import re
    m = re.search(r"(\d+) props?", line)
    return int(m.group(1)) if m else None


async def _run(ctx, rec, exe):
    dens = Path(tempfile.mkdtemp(prefix="leyline-audit-den-", dir=ctx.temp))
    lp, wp = free_port(), free_port()
    prog = Program(exe, dens, lp, wp)
    sessions = []
    try:
        t = time.time()
        if not prog.up():
            rec.add(F, "Den Lite starts on this PC", FAIL, f"it did not answer on 127.0.0.1:{lp}",
                    "Run the den-lite step (Build Den Lite) again, or tell Claude this line.")
            return
        rec.add(F, "Den Lite starts on this PC", PASS, f"answered in {time.time() - t:.1f}s", secs=time.time() - t)

        t = time.time()
        try:
            s, realm, char, start = await enter_den(lp, "DEN", "den")
        except (w.WowError, OSError, asyncio.TimeoutError, asyncio.IncompleteReadError) as e:
            rec.add(F, "Play solo: log in as DEN / den and enter the Den", FAIL, short(e), "Tell Claude this line.")
            return
        sessions.append(s)
        rec.add(F, "Play solo: log in as DEN / den and enter the Den", PASS,
                f"realm \"{realm['name']}\", character {char['name']}", secs=time.time() - t)

        welcome = await s.wait_line(lambda c: "Welcome" in c["text"], 8, start)
        await asyncio.sleep(1.0)
        lines = [c["text"] for c in s.lines_since(start) if c["type"] == p.CHAT_SYSTEM]
        if welcome and len(lines) >= 4:
            rec.add(F, "Quick start lines at login", PASS, f"{len(lines)} lines, first: {welcome['text'][:60]}")
        else:
            rec.add(F, "Quick start lines at login", FAIL, f"{len(lines)} system lines; welcome: {bool(welcome)}",
                    "Tell Claude: Den Lite's quick start did not show.")

        me = s.objects.get(HOST_GUID)
        jaina = next((u for u in s.units() if u["fields"].get(p.F_ENTRY) == 32364), None)
        if s.parse_errors:
            rec.add(F, "The world packets read cleanly", FAIL, "; ".join(s.parse_errors[:3]), "Tell Claude this line.")
        elif me and me["fields"].get(p.U_LEVEL) == 1 and me["fields"].get(p.P_VISIBLE_ITEM_1 + 2 * p.SLOT_SHIRT):
            rec.add(F, "Your character and the room arrive", PASS,
                    f"level 1, dressed, {sum(1 for o in s.objects.values() if o['type'] == p.TYPE_GAMEOBJECT)} room pieces")
        else:
            rec.add(F, "Your character and the room arrive", FAIL, "the player's create is missing or undressed",
                    "Tell Claude this line.")

        at = s.mark()
        await s.say(".den place chair")
        c = await s.wait_line(lambda c: "is down" in c["text"] or "props" in c["text"], 6, at)
        rec.add(F, ".den place chair", PASS if c and "is down" in c["text"] else FAIL,
                (c or {}).get("text", "no answer")[:80], "Tell Claude: .den place did not work in Den Lite.")

        at = s.mark()
        await s.say(".den save audit")
        c = await s.wait_line(lambda c: "Saved" in c["text"] or "save" in c["text"].lower(), 6, at)
        saved = (dens / "audit.den.json").exists()
        rec.add(F, ".den save audit", PASS if c and saved else FAIL,
                (c or {}).get("text", "no answer").split(":")[0][:80] + ("" if saved else "; no audit.den.json"),
                "Tell Claude: .den save did not write a file.")

        if jaina:
            rec.add(F, "Jaina is there at login", PASS, f"level {jaina['fields'].get(p.U_LEVEL)}, "
                    f"faction {jaina['fields'].get(p.U_FACTION)}")
        else:
            rec.add(F, "Jaina is there at login", FAIL, "no companion in the world", "Tell Claude this line.")
        at = s.mark()
        await s.say("Jaina, tips")
        c = await s.wait_line(lambda c: c["type"] == p.CHAT_MONSTER_SAY, 8, at)
        rec.add(F, "Jaina answers \"Jaina, tips\"", PASS if c else FAIL,
                f"{c['name']}: {c['text'][:70]}" if c else "no answer within 8 s", "Tell Claude: Jaina said nothing.")

        await asyncio.sleep(2.5)            # the autosave: within 2 seconds of a change
        why = await s.logout(10)
        s.close()
        sessions.remove(s)
        clean = prog.stop()
        rec.add(F, "Stop Den Lite (the launcher's stop file)", PASS if clean else FAIL,
                "exited by itself" if clean else "had to be killed after 10 s" + (f"; logout: {why}" if why else ""),
                "Tell Claude: Den Lite ignores its stop file.")

        t = time.time()
        prog = Program(exe, dens, lp, wp)
        if not prog.up():
            rec.add(F, "The Den is kept over a restart (autosave)", FAIL, "Den Lite did not start again", "Tell Claude this line.")
            return
        s, _, _, start = await enter_den(lp, "DEN", "den")
        sessions.append(s)
        welcome = await s.wait_line(lambda c: "Welcome" in c["text"], 8, start)
        n = props_in(welcome["text"]) if welcome else None
        rec.add(F, "The Den is kept over a restart (autosave)", PASS if n and n >= 1 else FAIL,
                (welcome or {}).get("text", "no welcome line")[:80], "Tell Claude: the Den came back empty after a restart.",
                secs=time.time() - t)

        # ---------------------------------------------------------------- Invite friends
        try:
            g = guest(exe, dens, "add", "--name", "Rowan")
            rec.hide(g.get("pass"))
            rec.add(F, "Invite friends: a guest login is made", PASS, f"account {g['user']}, for 24 hours")
        except (RuntimeError, ValueError, KeyError, OSError, subprocess.TimeoutExpired) as e:
            rec.add(F, "Invite friends: a guest login is made", FAIL, short(e), "Tell Claude this line.")
            return
        try:
            rl, _rw = (int(x) for x in (dens / "den-lite.remote").read_text().split())
        except (OSError, ValueError):
            rec.add(F, "Invite friends: the friend's client joins (loopback)", FAIL, "no den-lite.remote file",
                    "Tell Claude this line.")
            return
        t = time.time()
        try:
            f, _, fchar, fstart = await enter_den(rl, g["user"], g["pass"])
            sessions.append(f)
        except (w.WowError, OSError, asyncio.TimeoutError, asyncio.IncompleteReadError) as e:
            rec.add(F, "Invite friends: the friend's client joins (loopback)", FAIL, short(e), "Tell Claude this line.")
            return
        rec.add(F, "Invite friends: the friend's client joins (loopback)", PASS, f"as {fchar['name']}", secs=time.time() - t)
        await asyncio.sleep(1.5)
        sees_host = HOST_GUID in f.objects and f.objects[HOST_GUID]["type"] == p.TYPE_PLAYER
        host_sees = any(o["guid"] != HOST_GUID for o in s.players())
        rec.add(F, "Host and friend see each other", PASS if sees_host and host_sees else FAIL,
                f"friend sees host: {sees_host}, host sees friend: {host_sees}", "Tell Claude this line.")
        why = await f.logout(10)
        f.close()
        sessions.remove(f)
        try:
            guest(exe, dens, "revoke", g["user"])
            try:
                await w.logon("127.0.0.1", rl, g["user"], g["pass"], timeout=6)
                rec.add(F, "Stop sharing: the guest login is taken back", FAIL, "it still logs in", "Tell Claude this line.")
            except (w.WowError, OSError, asyncio.TimeoutError, asyncio.IncompleteReadError):
                rec.add(F, "Stop sharing: the guest login is taken back", PASS, "refused after revoke")
        except (RuntimeError, ValueError, OSError, subprocess.TimeoutExpired) as e:
            rec.add(F, "Stop sharing: the guest login is taken back", FAIL, short(e), "Tell Claude this line.")
        tun = os.environ.get("DEN_TUNNEL_BIN")
        rec.add(F, "Invite friends through the relay", SKIP,
                "needs the launcher's tunnel program (DEN_TUNNEL_BIN); the launcher does this itself: manual item 4"
                if not tun else "DEN_TUNNEL_BIN is set, but this audit only runs it in the hosting tests")
    finally:
        for x in sessions:
            x.close()
        prog.stop(5)
        shutil.rmtree(dens, ignore_errors=True)


def run(ctx, rec):
    exe, how = find_den_lite(ctx)
    if not exe:
        got, why = download_den_lite(ctx)
        if got:
            exe, how = got, "den-lite-latest, " + why
        else:
            rec.add(F, "Den Lite on this PC", SKIP, f"{how}: {why}")
            return
    rec.add(F, "Den Lite on this PC", PASS, how)
    try:
        asyncio.run(asyncio.wait_for(_run(ctx, rec, exe), 150))
    except asyncio.TimeoutError:
        rec.add(F, "Den checks finished in time", FAIL, "over 150 s", "Tell Claude this line.")
    except Exception as e:  # noqa: BLE001 - one section never stops the audit
        rec.add(F, "Den checks ran to the end", FAIL, short(e), "Tell Claude this line.")
