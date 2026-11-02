"""Wrath Unleashed, played as a new player by the headless 3.3.5a client, on the live realm, with the audit's own
account only (AUDIT: made or refreshed by the server for each run, its password never shown).

    log in, list characters, make a level-1 night elf hunter, enter the world, the first-login choice, spawn,
    the starting weapon and bow, the realm's level-1 mount, a basic spell, running a few yards, targeting a
    creature and starting melee (SMSG_ATTACKSTART), log out; then a second character takes the realm's boost to
    80 and is deleted again, and so is the first. Nothing but the audit account's own characters is ever deleted.
"""
from __future__ import annotations

import asyncio
import math
import random
import struct
import time

from common import FAIL, PASS, SKIP, short
import play335 as p
from wunet import wire335 as w

F = "Wrath Unleashed"
RACE, CLASS = 4, 3                 # night elf hunter: starts with a bow
START_MOUNT = 10793                # Striped Nightsaber (ov_main.lua START_MOUNT[4])
SELF_SPELLS = [58984, 20594, 59752, 20572, 26297, 28880, 2457]   # racials first, then warrior's stance
NAME_IN_USE = (0x31, 0x32)
ACT = "Tell Claude this line (the realm's log helps: run the worldlog step)."


def make_name() -> str:
    syl = ["ra", "ne", "lo", "vi", "ma", "ri", "sa", "to", "li", "na", "fe", "do"]
    return ("Aud" + "".join(random.sample(syl, 3)))[:12].capitalize()


def dist(a, b):
    return math.hypot(a[0] - b[0], a[1] - b[1])


def attackable(u, me_level=1):
    f = u["fields"]
    high = (u["guid"] >> 48) & 0xFFFF
    return (high == 0xF130 and f.get(p.U_HEALTH, 1) > 0 and not f.get(p.U_NPCFLAGS, 0)
            and not f.get(p.U_FLAGS, 0) & p.UNIT_FLAG_NOT_ATTACKABLE and f.get(p.U_LEVEL, 1) <= me_level + 9)


async def pick_option(s: p.Session, menu: dict, words, start: int):
    """Choose the gossip item whose text has one of `words`; returns the next menu (or None)."""
    item = next((i for i in menu["items"] if any(x.lower() in i["text"].lower() for x in words)), None)
    if not item:
        return None, None
    at = s.mark()
    s.send(p.CMSG_GOSSIP_SELECT_OPTION, struct.pack("<QII", menu["guid"], menu["menu"], item["index"]))
    await s.flush()
    ev = await s.wait(lambda op, b: op == p.SMSG_GOSSIP_MESSAGE, 6, at)
    return item, (p.parse_gossip(ev[1]) if ev else None)


async def logout_hard(s: p.Session, secs=35) -> str:
    end = time.time() + secs
    why = "never asked"
    while time.time() < end:
        why = await s.logout(20)
        if not why or "refused" not in why:
            return why
        await asyncio.sleep(5)
    return why


async def new_char(s: p.Session):
    for _ in range(4):
        name = make_name()
        code = await s.char_create(name, RACE, CLASS)
        if code == w.CHAR_CREATE_SUCCESS:
            chars = await s.char_enum()
            c = next((c for c in chars if c["name"].lower() == name.lower()), None)
            if c:
                return c, code
        elif code not in NAME_IN_USE:
            return None, code
    return None, code


async def level_one(ctx, rec, s: p.Session, c):
    t = time.time()
    start = s.mark()
    pos = await s.enter(c["guid"])
    rec.add(F, "Enter the world", PASS, f"map {pos['map']}, zone {c['zone']}", secs=time.time() - t)

    ev = await s.wait(lambda op, b: op == p.SMSG_GOSSIP_MESSAGE, 10, start)
    if ev:
        menu = p.parse_gossip(ev[1])
        item, _ = await pick_option(s, menu, ["at level 1", "adventure"], 0)
        rec.add(F, "The first-login choice (level 1 or boost) is offered", PASS if item else FAIL,
                " / ".join(i["text"][:40] for i in menu["items"]), ACT)
    else:
        rec.add(F, "The first-login choice (level 1 or boost) is offered", SKIP,
                "no menu within 10 s (the realm's main profile may be off)")
    await asyncio.sleep(3)                      # the starting kit and the mount come with the first login

    me = s.objects.get(c["guid"])
    if s.parse_errors and not me:
        rec.add(F, "Spawn: your character arrives", SKIP, "the client could not read the update: " + s.parse_errors[0])
        return
    if not me:
        rec.add(F, "Spawn: your character arrives", FAIL, "no create for the player", ACT)
        return
    x, y = (me["pos"] or (pos["x"], pos["y"]))[:2]
    rec.add(F, "Spawn: your character arrives", PASS,
            f"level {me['fields'].get(p.U_LEVEL)}, at {x:.0f}, {y:.0f}; {len(s.units())} creatures and "
            f"{len(s.players()) - 1} other players near")

    mh = s.my(p.P_VISIBLE_ITEM_1 + 2 * p.SLOT_MAINHAND)
    rg = s.my(p.P_VISIBLE_ITEM_1 + 2 * p.SLOT_RANGED)
    rec.add(F, "Starting weapon and the hunter's bow", PASS if mh and rg else FAIL,
            f"main hand item {mh or 'none'}, ranged item {rg or 'none'}",
            "Run the scripts step (the starting kit is in ov_main.lua), then tell Claude.")

    # the realm's level-1 mount
    if START_MOUNT in s.spells:
        res = await s.cast(START_MOUNT, timeout=6)
        mounted = False
        for _ in range(10):
            if s.my(p.U_MOUNT):
                mounted = True
                break
            await asyncio.sleep(0.3)
        rec.add(F, "Mount up (the realm's level-1 mount)", PASS if mounted else FAIL,
                f"mount model {s.my(p.U_MOUNT)}" if mounted else (res[1] if res else "no answer to the cast"), ACT)
        s.send(p.CMSG_CANCEL_MOUNT_AURA)
        await s.flush()
        await asyncio.sleep(1)
    else:
        rec.add(F, "Mount up (the realm's level-1 mount)", SKIP,
                f"the character doesn't know the mount spell {START_MOUNT} (this realm gives none at level 1)")

    spell = next((x for x in SELF_SPELLS if x in s.spells), None)
    if spell:
        res = await s.cast(spell, timeout=6)
        rec.add(F, "Cast a basic spell", PASS if res and res[0] in ("go", "start") else FAIL,
                f"spell {spell}: " + (res[0] + (" " + res[1] if res[1] else "") if res else "no answer"), ACT)
    else:
        rec.add(F, "Cast a basic spell", SKIP, f"none of the self spells the client knows how to cast are known "
                f"({len(s.spells)} spells known)")

    t = time.time()
    await s.walk(4.0, 3.0)
    alive = await s.ping(9)
    rec.add(F, "Movement accepted (ran 5 yards)", PASS if alive else FAIL,
            "the server kept the session and answered a ping" if alive else "no ping answer after moving", ACT,
            secs=time.time() - t)

    here = s.pos
    # out of melee reach first: SMSG_ATTACKSTART comes at once, the swing misses for range, and no fight starts
    cands = sorted([u for u in s.units() if attackable(u) and u["pos"]],
                   key=lambda u: (dist(u["pos"], here) < 6, dist(u["pos"], here)))[:6]
    if not cands:
        rec.add(F, "Target a creature and start melee", SKIP, f"no attackable creature near the start ({len(s.units())} units seen)")
    else:
        got, tried = None, 0
        for u in cands:
            tried += 1
            await s.face(u["pos"][0], u["pos"][1])
            at = s.mark()
            s.send(p.CMSG_SET_SELECTION, struct.pack("<Q", u["guid"]))
            s.send(p.CMSG_ATTACKSWING, struct.pack("<Q", u["guid"]))
            await s.flush()
            ev = await s.wait(lambda op, b: (op == p.SMSG_ATTACKSTART and struct.unpack_from("<QQ", b)[1] == u["guid"])
                              or op in (p.SMSG_ATTACKSWING_CANT_ATTACK, p.SMSG_ATTACKSTOP), 3, at)
            s.send(p.CMSG_ATTACKSTOP)
            await s.flush()
            if ev and ev[0] == p.SMSG_ATTACKSTART:
                got = u
                break
        if got:
            f = got["fields"]
            rec.add(F, "Target a creature and start melee", PASS,
                    f"creature {f.get(p.F_ENTRY)} (level {f.get(p.U_LEVEL)}) {dist(got['pos'], here):.0f} yards away: SMSG_ATTACKSTART")
        else:
            rec.add(F, "Target a creature and start melee", FAIL, f"no attack started on {tried} nearby creatures", ACT)

    # Report a bug (the Esc menu's button says ".bug <text>"): an empty .bug only gets the realm's how-to line, so
    # nothing is filed
    at = s.mark()
    await s.say(".bug")
    line = await s.wait_line(lambda c: ".bug" in c["text"] and c["type"] in (p.CHAT_SYSTEM, -1), 6, at)
    rec.add("Bug reports", "In game: the realm's .bug command answers (nothing filed)", PASS if line else FAIL,
            (line or {}).get("text", "no answer within 6 s")[:90], "Run the scripts step (ov_reports.lua), then tell Claude.")

    why = await logout_hard(s)
    rec.add(F, "Log out", PASS if not why else FAIL, why or "back at character select", ACT)
    return not why


async def boost(ctx, rec, s: p.Session):
    c, code = await new_char(s)
    if not c:
        rec.add(F, "Boost to level 80 (a second test character)", FAIL, f"character create code {code:#x}", ACT)
        return
    start = s.mark()
    await s.enter(c["guid"])
    ev = await s.wait(lambda op, b: op == p.SMSG_GOSSIP_MESSAGE, 10, start)
    if not ev:
        at = s.mark()
        await s.say(".boost")
        ev = await s.wait(lambda op, b: op == p.SMSG_GOSSIP_MESSAGE, 6, at)
    if not ev:
        rec.add(F, "Boost to level 80 (a second test character)", SKIP,
                "the realm offered no boost to a fresh character (no menu, .boost gave none)")
    else:
        t = time.time()
        menu = p.parse_gossip(ev[1])
        item, specs = await pick_option(s, menu, ["Boost to level 80", "boost"], 0)
        spec = None
        if specs:
            spec = next((i for i in specs["items"] if i["text"].strip().lower() != "back"), None)
            nxt = None
            if spec:
                _, nxt = await pick_option(s, specs, [spec["text"]], 0)
            if nxt:
                await pick_option(s, nxt, ["boost now", "No professions", "No more professions"], 0)
        end = time.time() + 45
        while time.time() < end and s.my(p.U_LEVEL) != 80:
            await asyncio.sleep(0.5)
        lvl = s.my(p.U_LEVEL)
        rec.add(F, "Boost to level 80 (a second test character)", PASS if lvl == 80 else FAIL,
                f"level {lvl} after {time.time() - t:.0f} s" + (f", spec {spec['text'][:30]}" if spec else ""), ACT,
                secs=time.time() - t)
    why = await logout_hard(s)
    if why:
        rec.add(F, "The boosted test character is deleted again", FAIL, f"could not log out: {why}",
                "Nothing to do: the next audit removes it.")
        return
    code = await s.char_delete(c["guid"])
    rec.add(F, "The boosted test character is deleted again", PASS if code == p.CHAR_DELETE_SUCCESS else FAIL,
            f"delete code {code:#x}", "Nothing to do now: the next audit removes it first.")


async def _run(ctx, rec, user, pw):
    if "auth" in ctx.fake:
        host, port = w.split_host_port(ctx.fake["auth"], 3724)
    else:
        host, port = ctx.host("realmlist"), 3724
    want = ctx.cfg["realm_name"].lower()
    t = time.time()
    try:
        s, realm, realms = await p.connect(host, port, user, pw, realm_pick=lambda r: want in r["name"].lower())
    except (w.WowError, OSError, asyncio.TimeoutError, asyncio.IncompleteReadError) as e:
        rec.add(F, "Log in (the audit account) and pick the realm", FAIL, f"{host}:{port}: {short(e)}",
                "Run the check step: the login server may be down.")
        return
    rec.add(F, "Log in (the audit account) and pick the realm", PASS,
            f"{host}: \"{realm['name']}\" of {len(realms)} realms", secs=time.time() - t)
    try:
        chars = await s.char_enum()
        rec.add(F, "List characters", PASS, f"{len(chars)} on the audit account")
        gone = 0
        for c in chars:                         # the audit account's own leftovers from an earlier run
            if await s.char_delete(c["guid"]) == p.CHAR_DELETE_SUCCESS:
                gone += 1
        if chars:
            rec.add(F, "Earlier audit characters removed", PASS if gone == len(chars) else FAIL,
                    f"{gone} of {len(chars)}", "Nothing to do now: the next audit tries again.")
        t = time.time()
        c, code = await new_char(s)
        if not c:
            rec.add(F, "Create a level-1 character", FAIL, f"character create answered {code:#x}", ACT)
            return
        rec.add(F, "Create a level-1 character", PASS, f"{c['name']}, night elf hunter, level {c['level']}",
                secs=time.time() - t)
        ok = await level_one(ctx, rec, s, c)
        if ok and not ctx.cfg.get("no_boost"):
            await boost(ctx, rec, s)
        elif not ok:
            rec.add(F, "Boost to level 80 (a second test character)", SKIP, "the first character did not log out")
        if ok:
            code = await s.char_delete(c["guid"])
            rec.add(F, "The level-1 test character is deleted again", PASS if code == p.CHAR_DELETE_SUCCESS else FAIL,
                    f"delete code {code:#x}", "Nothing to do now: the next audit removes it first.")
    except (w.WowError, OSError, asyncio.TimeoutError, asyncio.IncompleteReadError) as e:
        rec.add(F, "Gameplay checks ran to the end", FAIL, short(e), ACT)
    finally:
        s.close()


def run(ctx, rec, user, pw):
    if not pw:
        rec.add(F, "Gameplay as a new player", SKIP, "no audit account on this PC (needs the server's SSH key)")
        return
    try:
        asyncio.run(asyncio.wait_for(_run(ctx, rec, user, pw), 300))
    except asyncio.TimeoutError:
        rec.add(F, "Gameplay checks finished in time", FAIL, "over 5 minutes", ACT)
    except Exception as e:  # noqa: BLE001
        rec.add(F, "Gameplay checks ran to the end", FAIL, short(e), ACT)
