#!/usr/bin/env python3
"""The stamp composer: writes realm/custom/lua_scripts/den-stamps/<name>.den.json from the recipes below.

    python3 tools/den-stamps/compose.py           writes every stamp (the same files every time: a fixed seed each)
    python3 tools/den-stamps/compose.py --check   only checks the files there are the ones it would write

A stamp is our own layout (x ahead of you, y to your left, z up from the ground, yaw in degrees) of gameobject entries
the Den's catalog already has (ov_build.lua; labs/den-lite/src/objects.rs). Nothing here comes from the game's maps.

The rules (docs/DEN-STAMPS.md has the why):
  - one focal point per stamp; seats, tents and stalls face it (or the path) with a few degrees of natural turn;
  - pieces go in large -> medium -> small; small things gather in clusters of 3 to 5 by a larger anchor;
  - no two pieces overlap (their boxes from footprints.json, turned by their yaw, with a hand's width between);
  - small things stand on a table's top (z = the table's height), inside its edge, or on the ground;
  - the way in (from you, along -x) and the paths stay clear;
  - every stamp has a seed of its own: the same file every time.
"""

import json
import math
import random
import sys
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
OUT = ROOT / "realm" / "custom" / "lua_scripts" / "den-stamps"
FOOT = json.loads((HERE / "footprints.json").read_text(encoding="utf-8"))["pieces"]

GAP = 0.1          # yards between two standing pieces
TABLE_GAP = 0.03   # between two things on one table
EDGE = 0.04        # a thing on a table keeps this far inside its edge
SPAN = 18.0        # every piece within this of the stamp's middle (Den Lite takes 40)


class Clash(Exception):
    pass


class Piece:
    def __init__(self, entry, x, y, z, yaw, role=None, on=None, faces=None, group=None, room=False):
        f = FOOT[str(entry)]
        self.entry, self.x, self.y, self.z, self.yaw = entry, x, y, z, yaw % 360.0
        self.w, self.d, self.h, self.kind, self.name = f["w"], f["d"], f["h"], f["kind"], f["name"]
        self.role, self.on, self.faces, self.group, self.room = role, on, faces, group, room

    def corners(self, grow=0.0):
        c, s = math.cos(math.radians(self.yaw)), math.sin(math.radians(self.yaw))
        hw, hd = self.w / 2 + grow, self.d / 2 + grow
        return [(self.x + a * c - b * s, self.y + a * s + b * c) for a, b in ((hw, hd), (-hw, hd), (-hw, -hd), (hw, -hd))]

    def top(self):
        return self.z + self.h


def boxes_meet(a, b, gap):
    """Two turned boxes (2D) closer than `gap`: the separating axis test on their four edges' normals."""
    pa, pb = a.corners(gap / 2), b.corners(gap / 2)
    for poly in (pa, pb):
        for i in range(4):
            (x1, y1), (x2, y2) = poly[i], poly[(i + 1) % 4]
            nx, ny = y2 - y1, x1 - x2
            ra = [nx * x + ny * y for x, y in pa]
            rb = [nx * x + ny * y for x, y in pb]
            if max(ra) <= min(rb) + 1e-9 or max(rb) <= min(ra) + 1e-9:
                return False
    return True


def clash(a, b):
    """Would `a` and `b` be in each other: the composer's one rule for overlaps (tests use it too)."""
    if "shell" in (a.kind, b.kind) or a.on is b or b.on is a:
        return False
    if (a.kind == "floor") != (b.kind == "floor"):
        return False
    if a.group and a.group == b.group:
        return False
    if a.z >= b.top() - 0.01 or b.z >= a.top() - 0.01:
        return False
    gap = TABLE_GAP if (a.on is not None and a.on is b.on) else GAP
    return boxes_meet(a, b, gap)


def point_in(p, x, y, inset=0.0):
    """(x, y) inside piece p's box, `inset` in from its edge."""
    c, s = math.cos(math.radians(p.yaw)), math.sin(math.radians(p.yaw))
    lx, ly = (x - p.x) * c + (y - p.y) * s, -(x - p.x) * s + (y - p.y) * c
    return abs(lx) <= p.w / 2 - inset + 1e-9 and abs(ly) <= p.d / 2 - inset + 1e-9


def facing(x, y, tx, ty):
    return math.degrees(math.atan2(ty - y, tx - x)) % 360.0


class Scene:
    def __init__(self, name, title, line, focus=(0.0, 0.0)):
        self.name, self.title, self.line, self.focus = name, title, line, focus
        self.rng = random.Random(zlib.crc32(name.encode("utf-8")))
        self.pieces, self.keepout, self.room = [], [], None

    # ---------------------------------------------------------------- the rules, checked on every piece
    def fits(self, p):
        if abs(p.x) > SPAN or abs(p.y) > SPAN:
            return False
        if any(clash(p, q) for q in self.pieces):
            return False
        standing = p.on is None or p.on.kind == "floor"
        if p.kind not in ("floor", "shell") and standing and p.z < 0.5:
            for k in self.keepout:
                if boxes_meet(p, k, 0.0):
                    return False
        if p.room and self.room:
            x0, x1, y0, y1 = self.room
            if any(not (x0 <= cx <= x1 and y0 <= cy <= y1) for cx, cy in p.corners()):
                return False
        return True

    def put(self, entry, x, y, yaw=0.0, **kw):
        """One piece exactly here (a layout's fixed pieces): an error when it breaks a rule."""
        p = Piece(entry, x, y, kw.pop("z", 0.0), yaw, **kw)
        if not self.fits(p):
            raise Clash("%s: %s at (%.2f, %.2f) doesn't fit" % (self.name, p.name, x, y))
        self.pieces.append(p)
        return p

    def natural(self, entry, x, y, yaw, move=0.15, turn=6.0, **kw):
        """A piece near here, turned and moved a little (people never set things down square): the nearest try
        that fits, an error when none does."""
        for k in range(40):
            m = move * (1 + k / 8)
            jx, jy = self.rng.uniform(-m, m), self.rng.uniform(-m, m)
            jt = self.rng.uniform(-turn, turn)
            p = Piece(entry, x + jx, y + jy, kw.get("z", 0.0), yaw + jt, **{a: b for a, b in kw.items() if a != "z"})
            if p.faces:
                p.yaw = (facing(p.x, p.y, *p.faces) + jt) % 360.0
            if self.fits(p):
                self.pieces.append(p)
                return p
        raise Clash("%s: no room for %s near (%.2f, %.2f)" % (self.name, FOOT[str(entry)]["name"], x, y))

    def facing_point(self, entry, x, y, target, turn=5.0, **kw):
        """A seat, tent or stall at (x, y) turned to face `target`."""
        return self.natural(entry, x, y, facing(x, y, *target), turn=turn, faces=target, **kw)

    def ring(self, entries, r, a0, a1, centre=None, **kw):
        """Pieces spread from angle a0 to a1 (degrees, 0 = ahead) at r yards round the centre, facing it."""
        cx, cy = centre or self.focus
        n = len(entries)
        out = []
        for i, e in enumerate(entries):
            a = math.radians(a0 + (a1 - a0) * (i / (n - 1) if n > 1 else 0.5))
            out.append(self.facing_point(e, cx + r * math.cos(a), cy + r * math.sin(a), (cx, cy), **kw))
        return out

    def cluster(self, anchor, entries, start=0.0, reach=2.6, **kw):
        """3 to 5 things gathered by an anchor point: each the nearest free spot to it, at a random angle."""
        ax, ay = anchor
        out = []
        for e in entries:
            f = FOOT[str(e)]
            r0 = max(f["w"], f["d"]) / 2
            placed = None
            for k in range(160):
                r = start + r0 * 0.5 + k * reach / 160
                a = self.rng.uniform(0, 2 * math.pi)
                p = Piece(e, ax + r * math.cos(a), ay + r * math.sin(a), 0.0, self.rng.uniform(0, 360), **kw)
                if self.fits(p):
                    placed = p
                    break
            if placed is None:
                raise Clash("%s: the cluster at (%.1f, %.1f) has no room for %s" % (self.name, ax, ay, f["name"]))
            self.pieces.append(placed)
            out.append(placed)
        return out

    def on(self, base, entries):
        """Small things on a table's (or a deck's) top: inside its edge, apart from each other, z its height."""
        out = []
        for e in entries:
            f = FOOT[str(e)]
            hw = (base.w / 2 - f["w"] / 2 - EDGE, base.d / 2 - f["d"] / 2 - EDGE)
            if min(hw) < 0:
                raise Clash("%s: %s is bigger than the %s's top" % (self.name, f["name"], base.name))
            c, s = math.cos(math.radians(base.yaw)), math.sin(math.radians(base.yaw))
            placed = None
            for _ in range(200):
                lx, ly = self.rng.uniform(-hw[0], hw[0]), self.rng.uniform(-hw[1], hw[1])
                # turned with the table, a little off square, so it stays inside the edge
                p = Piece(e, base.x + lx * c - ly * s, base.y + lx * s + ly * c, base.top(), base.yaw + self.rng.uniform(-8, 8), on=base)
                if all(point_in(base, cx, cy) for cx, cy in p.corners()) and self.fits(p):
                    placed = p
                    break
            if placed is None:
                raise Clash("%s: no room on the %s for %s" % (self.name, base.name, f["name"]))
            self.pieces.append(placed)
            out.append(placed)
        return out

    def dining(self, table, sides, entry=2413, **kw):
        """Chairs pulled in at a table's sides, each facing the table: sides is a list of (side, along), side one of
        +x, -x, +y, -y of the table, along how far along that side (yards from its middle)."""
        c, s = math.cos(math.radians(table.yaw)), math.sin(math.radians(table.yaw))
        seat = FOOT[str(entry)]
        out = []
        for side, along in sides:
            off = {"+x": (table.w / 2 + seat["w"] / 2 + 0.12, along), "-x": (-(table.w / 2 + seat["w"] / 2 + 0.12), along),
                   "+y": (along, table.d / 2 + seat["d"] / 2 + 0.12), "-y": (along, -(table.d / 2 + seat["d"] / 2 + 0.12))}[side]
            x, y = table.x + off[0] * c - off[1] * s, table.y + off[0] * s + off[1] * c
            # facing the table's middle line, not its centre point, as a chair pulled up to it
            tx, ty = (table.x + along * c * (side in "+y-y") - along * s * (side in "+x-x"),
                      table.y + along * s * (side in "+y-y") + along * c * (side in "+x-x"))
            out.append(self.facing_point(entry, x, y, (tx, ty), turn=4.0, move=0.06, **kw))
        return out

    def fence(self, a, b, entry=211063, gaps=(), group="fence"):
        """A run of fence segments from a to b (each its length long, joined end to end), leaving out the
        segments whose index is in gaps (a gate, a way in)."""
        L = FOOT[str(entry)]["w"]
        (x0, y0), (x1, y1) = a, b
        n = max(1, round(math.hypot(x1 - x0, y1 - y0) / L))
        yaw = facing(x0, y0, x1, y1)
        out = []
        for i in range(n):
            if i in gaps:
                continue
            t = (i + 0.5) / n
            out.append(self.put(entry, x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, yaw, group=group))
        return out

    def keep_clear(self, x, y, w, d, yaw=0.0):
        """A path: no standing piece in this box (rugs and boards may be)."""
        k = Piece(179969, x, y, 0.0, yaw)
        k.w, k.d, k.kind, k.name = w, d, "path", "path"
        self.keepout.append(k)

    # ---------------------------------------------------------------- the file
    def doc(self):
        props = []
        for p in self.pieces:
            e = {"entry": p.entry, "x": round(p.x, 2), "y": round(p.y, 2), "z": round(p.z, 2), "yaw": round(p.yaw % 360.0, 1)}
            if p.role:
                e["role"] = p.role
            props.append(e)
        return {"format": "leyline-den", "version": 1, "kind": "stamp", "name": self.name, "title": self.title, "line": self.line,
                "focus": [round(self.focus[0], 2), round(self.focus[1], 2)], "props": props}


# ==================================================================================================== the stamps
# x ahead of you (you come in from -x), y to your left. Each recipe: the anchors first (fire, tent, building, stalls),
# then the medium pieces round them, then the small things.

def camp():
    s = Scene("campfire", "Camp", "A camp round a fire: benches and stools facing it, bedrolls, a tent behind with its "
              "crates and barrels, a table of food, a weapon rack and torches at the edge.")
    s.keep_clear(-5.6, 0.0, 6.0, 2.0)                       # the way in, from you to the fire
    s.put(1953, 0.0, 0.0, 0.0)                              # the fire: the focal point
    s.facing_point(184592, 7.0, 0.2, (0.0, 0.0), turn=3.0)  # the tent behind it, its opening to the fire
    for e, a in ((24538, 0), (24538, 72), (24538, 288)):    # seats in a ring, facing the fire
        s.facing_point(e, 3.0 * math.cos(math.radians(a)), 3.0 * math.sin(math.radians(a)), (0.0, 0.0))
    for a in (132, 228):
        s.facing_point(182593, 2.8 * math.cos(math.radians(a)), 2.8 * math.sin(math.radians(a)), (0.0, 0.0), turn=12.0)
    for a in (36, 324):                                     # bedrolls, their foot to the fire
        s.facing_point(188346, 4.7 * math.cos(math.radians(a)), 4.7 * math.sin(math.radians(a)), (0.0, 0.0), turn=8.0)
    table = s.facing_point(181305, 6.4, -3.9, (0.0, 0.0), turn=4.0)
    s.cluster((6.9, 3.9), [179969, 179967, 181302, 92527, 183993])   # stores by the tent
    s.facing_point(19635, 3.0, -5.2, (0.0, 0.0), turn=4.0)           # a weapon rack at the edge
    s.facing_point(182593, 5.0, -5.0, table and (table.x, table.y), turn=10.0)
    s.on(table, [2562, 2687, 2148])
    s.natural(180043, -3.4, 2.4, 0.0, turn=20.0)            # torches either side of the way in
    s.natural(180043, -3.4, -2.4, 0.0, turn=20.0)
    s.natural(179977, 4.6, 2.6, 0.0, turn=20.0)             # a lantern by the tent
    return s


def inn():
    s = Scene("inn", "Inn", "A small inn you walk into: a bar with kegs behind it, tables with chairs pulled in and food on "
              "them, a rug, a hearth with a stool, lanterns at the door, a mailbox and barrels outside.", focus=(12.6, 1.6))
    s.put(19007, 11.0, 0.0, 180.0, role="house")
    s.room = (7.8, 14.1, -3.65, 3.65)                      # inside its walls (kept where the furniture always stood)
    s.put(179977, 4.5, 1.2, 0.0, role="door")
    s.put(179977, 4.5, -1.2, 0.0)
    s.put(19626, 4.0, -3.5, 0.0, role="mailbox")
    s.keep_clear(6.4, 0.0, 4.0, 1.6)                        # the door to the bar
    bar = s.put(19624, 12.4, 1.6, 180.0, room=True)         # the bar, its front to the room
    for y in (2.75, 1.75):                                  # kegs behind it
        s.natural(179976, 13.52, y, 180.0, move=0.03, turn=5.0, room=True)
    s.natural(179973, 13.52, 0.65, 0.0, move=0.03, turn=5.0, room=True)
    s.on(bar, [2687, 2687, 2562])
    for y in (2.4, 0.9):                                    # stools at the bar, facing it
        s.facing_point(182593, 11.35, y, (12.4, y), turn=8.0, move=0.05, room=True)
    a = s.put(181075, 9.4, -2.3, 0.0, room=True)            # a long table, chairs at its sides
    s.dining(a, [("-x", -0.45), ("-x", 0.5), ("+x", -0.45), ("+x", 0.5)], room=True)
    s.on(a, [2562, 2148, 2687])
    s.put(181396, 9.35, 1.7, 90.0, room=True)                           # a rug under the round table
    b = s.natural(186422, 9.15, 1.7, 0.0, move=0.03, turn=4.0, room=True)
    for ang in (0, 90, 270):
        r = 1.3
        s.facing_point(2413, b.x + r * math.cos(math.radians(ang)), b.y + r * math.sin(math.radians(ang)), (b.x, b.y),
                       turn=6.0, move=0.04, room=True)
    s.on(b, [1558, 2687])
    s.natural(180473, 13.35, -2.95, 0.0, move=0.05, turn=10.0, room=True)  # the hearth in the far corner
    s.facing_point(182593, 12.0, -2.6, (13.35, -2.95), turn=10.0, move=0.05, room=True)
    s.put(180765, 9.4, -2.3, 0.0, z=2.6)                    # a lantern hanging over the long table
    s.cluster((5.4, 3.6), [179967, 179969, 180575])         # barrels by the wall outside
    return s


def market():
    s = Scene("market", "Market", "A market street: four stalls facing a clear path, goods in front of each, a counter "
              "with food, a bench, lamp posts at the ends and a banner.", focus=(0.0, 0.0))
    s.keep_clear(0.0, 0.0, 18.0, 2.2)                       # the street
    stalls = [(19637, -3.8, 4.45), (180031, 3.8, 4.45), (180032, -3.8, -4.45), (179966, 3.8, -4.3)]
    for e, x, y in stalls:                                  # stalls facing the street
        s.facing_point(e, x, y, (x, 0.0), turn=2.0, move=0.05)
    counter = s.facing_point(19624, 0.0, 3.5, (0.0, 0.0), turn=2.0, move=0.05)
    s.facing_point(24538, 0.0, -3.4, (0.0, 0.0), turn=4.0, move=0.05)
    goods = [[190559, 195192, 179969], [179967, 92527, 183993], [179969, 179970, 184858], [190559, 195192, 181302]]
    for (e, x, y), g in zip(stalls, goods):                 # goods set out in front of each stall
        side = 1.0 if y > 0 else -1.0
        for dx, item in zip((-1.15, 0.05, 1.2), g):
            s.natural(item, x + dx, side * 2.05, 90.0 * side, move=0.1, turn=12.0)
    s.on(counter, [2562, 2148, 2687])
    s.natural(19457, -8.0, 2.1, 0.0, move=0.05, turn=0.0)
    s.natural(19457, 8.0, -2.1, 0.0, move=0.05, turn=0.0)
    s.facing_point(180773, 8.0, 2.1, (0.0, 0.0), turn=2.0, move=0.05)
    return s


def garden():
    s = Scene("garden", "Garden", "A fenced vegetable plot: a gap in the fence for the way in, four rows of plants (one "
              "kind to a row) either side of a path, a hay stack, a basket and a water barrel, a bench and lanterns.",
              focus=(4.5, 0.0))
    x0, x1, y0, y1 = -1.0, 8.0, -4.5, 4.5
    s.fence((x0, y1), (x1, y1))                             # three sides closed
    s.fence((x1, y1), (x1, y0))
    s.fence((x1, y0), (x0, y0))
    s.fence((x0, y0), (x0, y1), gaps=(1,))                  # the near side open in the middle
    s.keep_clear(2.5, 0.0, 9.0, 1.4)                        # the path down the middle
    s.natural(19429, 6.6, 3.0, 0.0, move=0.05, turn=25.0)   # hay in the far corner
    rows = [(3.25, 181087), (1.85, 2109), (-1.85, 20939), (-3.25, 181103)]
    for y, e in rows:                                       # a row each, a little uneven
        for x in (0.4, 1.6, 2.8, 4.0):
            s.natural(e, x, y, 0.0, move=0.08, turn=40.0)
    s.cluster((6.5, -3.0), [190559, 195192, 179967], reach=1.4)
    s.facing_point(24538, -2.4, 3.2, (4.5, 0.0), turn=4.0)  # a bench outside, looking at the plot
    s.natural(179977, -1.9, 1.9, 0.0, turn=20.0)            # lanterns at the gap
    s.natural(179977, -1.9, -1.9, 0.0, turn=20.0)
    return s


def dock():
    s = Scene("dock", "Dock", "Boards out over the water: barrels and crates at the land end, sacks halfway, lanterns at "
              "the corners and a fishing chair at the end looking out.", focus=(10.0, 0.0))
    decks = [s.put(190615, x, 0.0, 0.0, group="deck") for x in (2.0, 6.0, 10.0)]
    s.keep_clear(5.0, 0.0, 10.0, 1.0)                       # the walk out to the end
    for d, (ax, ay), items in ((decks[0], (2.0, 1.3), [179967, 179969, 92527]), (decks[1], (6.0, -1.3), [179970, 183993, 184858])):
        for e, slot in zip(items, (-1.3, 0.0, 1.3)):        # a row along the boards' edge, a little uneven
            placed = None
            for _ in range(200):
                x, y = ax + slot + s.rng.uniform(-0.12, 0.12), ay + s.rng.uniform(-0.15, 0.15)
                p = Piece(e, x, y, d.top(), s.rng.uniform(-10, 10) + s.rng.choice((0, 90)), on=d)
                if all(point_in(d, cx, cy) for cx, cy in p.corners()) and s.fits(p):
                    placed = p
                    break
            if placed is None:
                raise Clash("dock: no room for %s" % e)
            s.pieces.append(placed)
    end = decks[2]
    for y in (1.65, -1.65):
        s.put(179977, 11.65, y, 0.0, z=end.top(), on=end)
    s.put(186475, 11.1, -0.55, 0.0, z=end.top(), on=end, faces=(20.0, -0.55))   # looking out over the water
    s.put(195192, 11.3, 0.6, 30.0, z=end.top(), on=end)
    s.natural(19457, -0.9, 2.6, 0.0, turn=0.0)              # a lamp post where the boards start
    s.cluster((-1.2, -2.8), [179969, 179967], reach=1.2)
    return s


def house():
    s = Scene("house", "Small house", "A cottage you walk into: a bed against the back wall with a candle by it, a table "
              "with two chairs and a book, a rug, a barrel in the corner; a lantern at the door, a mailbox and flowers.",
              focus=(7.0, 0.0))
    s.put(19262, 7.0, 0.0, 180.0, role="house")
    s.room = (4.2, 9.65, -2.65, 2.65)
    s.put(179977, 2.0, 1.5, 180.0, role="door")
    s.put(19626, 2.0, -2.5, 180.0, role="mailbox")
    s.keep_clear(3.6, 0.0, 3.2, 1.4)
    s.put(19504, 8.4, 1.55, 180.0, room=True)               # the bed along the back wall
    s.put(181396, 6.6, 0.0, 90.0, room=True)
    t = s.put(182093, 6.0, -1.25, 0.0, room=True)           # a small table, a chair either side
    s.dining(t, [("-x", 0.0), ("+x", 0.0)], room=True)
    s.on(t, [2695, 1558, 2562])
    n = s.natural(182093, 9.1, -0.2, 0.0, move=0.04, turn=8.0, room=True)  # by the bed
    s.on(n, [180765])
    s.natural(179967, 9.1, -2.05, 0.0, move=0.04, turn=30.0, room=True)
    for y in (2.6, -1.4):
        s.natural(2109, 1.0, y, 0.0, move=0.1, turn=40.0)
    return s


def smithy():
    s = Scene("smithy", "Smithy", "An open-air forge: the melting pot and the fire behind the work bench, the quench "
              "barrel a step from the fire, racks along the back, crates of finished weapons, coal sacks, an armour "
              "stand out front for buyers and a torch.", focus=(5.0, 1.4))
    s.keep_clear(-3.5, 0.0, 5.0, 1.6)
    s.put(19412, 5.2, 1.6, 180.0)                           # the forge: the focal point
    s.natural(180473, 5.2, -0.8, 0.0, move=0.05, turn=10.0)  # its fire
    bench = s.facing_point(176561, 2.4, 0.4, (-2.0, 0.4), turn=2.0, move=0.05)   # the bench between the smith and you
    s.natural(201774, 3.7, 3.1, 0.0, move=0.05, turn=30.0)  # the quench barrel, a step from the forge
    s.facing_point(19635, 7.2, -1.9, (5.0, 0.0), turn=3.0, move=0.05)   # racks along the back
    s.facing_point(19635, 7.2, 3.9, (5.0, 1.4), turn=3.0, move=0.05)
    s.facing_point(180774, 7.9, 0.4, (0.0, 0.4), turn=2.0, move=0.04)   # the smith's banner
    s.cluster((4.6, -3.4), [202892, 202891, 1670], reach=1.6)          # finished work
    s.cluster((6.9, 1.0), [92527, 184858], reach=1.2)                 # coal
    s.facing_point(26480, 0.6, -2.4, (-4.0, -2.4), turn=4.0, move=0.05)  # on show to buyers
    s.on(bench, [180765])
    s.natural(180043, 0.6, 2.6, 0.0, turn=20.0)
    return s


def nook():
    s = Scene("nook", "Reading nook", "A quiet corner on a rug: two high-backed chairs turned to a little table with a "
              "book and a candle, a bench, a second table of books, a lantern and potted plants.", focus=(0.8, 0.0))
    s.put(181396, 0.8, 0.0, 90.0)
    t = s.put(182093, 0.8, 0.0, 15.0)
    s.ring([2489, 2489], 1.4, 140, 220, centre=(0.8, 0.0), move=0.04)
    s.facing_point(24538, 2.9, 0.0, (0.8, 0.0), turn=3.0, move=0.05)
    side = s.put(190689, 2.6, 2.4, 0.0)
    s.on(t, [2695, 1558])
    s.on(side, [2695, 2695, 1558])
    s.natural(179977, 2.6, -2.2, 0.0, turn=20.0)
    s.natural(181087, -1.0, 1.9, 0.0, move=0.1, turn=40.0)
    s.natural(181087, 4.0, -1.3, 0.0, move=0.1, turn=40.0)
    s.natural(182593, 0.7, -1.75, 0.0, move=0.05, turn=30.0)
    return s


def shrine():
    s = Scene("shrine", "Shrine", "A small shrine: a statue looking down a clear path, braziers either side, candles in "
              "an arc at its feet, prayer mats and stone benches facing it, banners and bushes behind, flowers.",
              focus=(5.0, 0.0))
    s.keep_clear(-1.8, 0.0, 8.4, 1.2)
    s.facing_point(19521, 5.0, 0.0, (-6.0, 0.0), turn=0.0, move=0.0)   # the focal point, looking at you
    for y in (2.4, -2.4):
        s.natural(180473, 4.7, y, 0.0, move=0.05, turn=10.0)
    for a, e in ((138, 188436), (164, 188438), (196, 188438), (222, 188436)):
        s.natural(e, 5.0 + 2.2 * math.cos(math.radians(a)), 2.2 * math.sin(math.radians(a)), 0.0, move=0.05, turn=30.0)
    for y in (0.95, -0.95):                                 # prayer mats facing it
        s.facing_point(188347, 1.7, y, (5.0, 0.0), turn=3.0, move=0.04)
    for y in (1.8, -1.8):
        s.facing_point(24388, -1.6, y, (5.0, 0.0), turn=3.0, move=0.04)
    s.facing_point(180773, 6.9, 1.4, (-6.0, 0.0), turn=2.0, move=0.04)
    s.facing_point(180774, 6.9, -1.4, (-6.0, 0.0), turn=2.0, move=0.04)
    for y in (3.7, -3.7):
        s.natural(181824, 6.6, y, 0.0, move=0.1, turn=40.0)
    for x, y, e in ((6.2, 0.9, 181103), (6.2, -0.9, 181103), (3.6, 3.4, 2109), (3.6, -3.4, 2109)):
        s.natural(e, x, y, 0.0, move=0.08, turn=40.0)
    return s


RECIPES = [camp, inn, market, garden, dock, house, smithy, nook, shrine]


def scenes():
    return [r() for r in RECIPES]


def text(scene):
    return json.dumps(scene.doc(), indent=1) + "\n"


def main(argv):
    check = "--check" in argv
    bad = 0
    for sc in scenes():
        path = OUT / ("%s.den.json" % sc.name)
        t = text(sc)
        if check:
            if not path.is_file() or path.read_text(encoding="utf-8") != t:
                print("differs: %s (run tools/den-stamps/compose.py)" % path.name)
                bad += 1
        else:
            path.write_text(t, encoding="utf-8", newline="\n")
            print("%-10s %2d pieces  %s" % (sc.name, len(sc.pieces), sc.title))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
