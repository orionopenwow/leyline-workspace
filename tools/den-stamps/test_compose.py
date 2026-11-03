#!/usr/bin/env python3
"""python3 tools/den-stamps/test_compose.py: the stamp composer's rules hold for every stamp, and the files shipped
in realm/custom/lua_scripts/den-stamps are the ones it writes."""

import json
import math
import re
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import compose  # noqa: E402

ROOT = HERE.parent.parent
OBJECTS = ROOT / "labs" / "den-lite" / "src" / "objects.rs"
BRAND = ("world of warcraft", "warcraft", "lich king", "blizzard")


def known_entries():
    return {int(m.group(1)) for m in re.finditer(r"^\s+\((\d+), \d+, \d+, \"", OBJECTS.read_text(encoding="utf-8"), re.M)}


def angle_off(a, b):
    return abs((a - b + 180.0) % 360.0 - 180.0)


class Composer(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.scenes = compose.scenes()

    def test_eight_to_ten_stamps_of_ten_to_forty_pieces(self):
        names = [s.name for s in self.scenes]
        self.assertTrue(8 <= len(names) <= 10, names)
        self.assertEqual(len(set(names)), len(names))
        for s in self.scenes:
            self.assertTrue(10 <= len(s.pieces) <= 40, "%s: %d pieces" % (s.name, len(s.pieces)))

    def test_no_two_pieces_overlap(self):
        for s in self.scenes:
            ps = s.pieces
            for i in range(len(ps)):
                for j in range(i + 1, len(ps)):
                    self.assertFalse(compose.clash(ps[i], ps[j]), "%s: %s and %s overlap" % (s.name, ps[i].name, ps[j].name))

    def test_the_overlap_test_itself(self):
        a = compose.Piece(179969, 0, 0, 0, 0)
        self.assertTrue(compose.clash(a, compose.Piece(179969, 0.95, 0, 0, 0)), "crates 0.95 apart touch")
        self.assertFalse(compose.clash(a, compose.Piece(179969, 1.2, 0, 0, 0)))
        self.assertTrue(compose.clash(a, compose.Piece(179969, 1.2, 0, 0, 45)), "turned 45 degrees its corner reaches")
        self.assertFalse(compose.clash(a, compose.Piece(181396, 0, 0, 0, 0)), "a crate may stand on a rug")
        t = compose.Piece(181075, 0, 0, 0, 0)
        self.assertFalse(compose.clash(t, compose.Piece(2562, 0, 0, t.top(), 0, on=t)), "bread on the table")
        self.assertTrue(compose.clash(t, compose.Piece(2562, 0, 0, 0.2, 0)), "bread inside the table")

    def test_every_entry_is_in_the_catalog_and_has_a_size(self):
        known = known_entries()
        self.assertGreater(len(known), 300)
        for s in self.scenes:
            for p in s.pieces:
                self.assertIn(p.entry, known, "%s: %s" % (s.name, p.entry))
                self.assertIn(str(p.entry), compose.FOOT)

    def test_small_things_stand_on_tables_inside_their_edge(self):
        on_tables = 0
        for s in self.scenes:
            for p in s.pieces:
                if p.on is None:
                    self.assertTrue(p.z == 0.0 or p.entry == 180765, "%s: %s floats at %.2f" % (s.name, p.name, p.z))
                    continue
                on_tables += 1
                self.assertAlmostEqual(p.z, p.on.top(), places=6)
                for cx, cy in p.corners():
                    self.assertTrue(compose.point_in(p.on, cx, cy), "%s: %s hangs over the %s's edge" % (s.name, p.name, p.on.name))
                if p.on.kind == "table":
                    self.assertIn(p.kind, ("item",), "%s: only small things on tables" % s.name)
        self.assertGreater(on_tables, 15)

    def test_seats_and_tents_face_what_they_should(self):
        checked = 0
        for s in self.scenes:
            for p in s.pieces:
                if p.faces:
                    want = compose.facing(p.x, p.y, *p.faces)
                    self.assertLess(angle_off(p.yaw, want), 15.0, "%s: %s faces %.0f, wants %.0f" % (s.name, p.name, p.yaw, want))
                    checked += 1
        self.assertGreater(checked, 40)
        camp = next(s for s in self.scenes if s.name == "campfire")
        seats = [p for p in camp.pieces if p.kind == "seat" and math.hypot(p.x, p.y) < 3.5]
        self.assertGreaterEqual(len(seats), 5)
        for p in seats:
            self.assertLess(angle_off(p.yaw, compose.facing(p.x, p.y, 0, 0)), 15.0, "a camp seat faces the fire")

    def test_paths_stay_clear(self):
        for s in self.scenes:
            for p in s.pieces:
                if p.kind in ("floor", "shell") or (p.on is not None and p.on.kind == "table") or p.z >= 0.5:
                    continue
                for k in s.keepout:
                    self.assertFalse(compose.boxes_meet(p, k, 0.0), "%s: %s on the path" % (s.name, p.name))

    def test_the_way_in_faces_you(self):
        """You stand at -x: nothing tall stands between you and each stamp's middle (a path, or open ground)."""
        for s in self.scenes:
            if not s.keepout:
                continue
            self.assertTrue(any(k.x - k.w / 2 < min(p.x for p in s.pieces if p.kind != "shell") + 1.5 for k in s.keepout), s.name)

    def test_buildings_furniture_inside_the_walls(self):
        for name in ("inn", "house"):
            s = next(x for x in self.scenes if x.name == name)
            roles = {p.role for p in s.pieces if p.role}
            self.assertEqual(roles, {"house", "door", "mailbox"})
            inside = [p for p in s.pieces if p.room]
            self.assertGreater(len(inside), 5)
            x0, x1, y0, y1 = s.room
            for p in inside:
                for cx, cy in p.corners():
                    self.assertTrue(x0 <= cx <= x1 and y0 <= cy <= y1, "%s: %s through the wall" % (name, p.name))

    def test_the_same_every_time(self):
        again = compose.scenes()
        self.assertEqual([compose.text(a) for a in self.scenes], [compose.text(b) for b in again])

    def test_large_pieces_come_first(self):
        """The recipes lay the anchors first: the first piece of each stamp is one of its biggest."""
        for s in self.scenes:
            sizes = sorted((p.w * p.d for p in s.pieces if p.kind != "floor"), reverse=True)
            first = next(p for p in s.pieces if p.kind != "floor")
            self.assertGreaterEqual(first.w * first.d, sizes[min(4, len(sizes) - 1)], s.name)


class Files(unittest.TestCase):
    def test_the_shipped_files_are_the_composers(self):
        for s in compose.scenes():
            path = compose.OUT / ("%s.den.json" % s.name)
            self.assertTrue(path.is_file(), path)
            self.assertEqual(path.read_text(encoding="utf-8"), compose.text(s), "%s: run tools/den-stamps/compose.py" % path.name)

    def test_every_file_is_a_stamp_den_lite_and_the_den_read(self):
        files = sorted(compose.OUT.glob("*.den.json"))
        self.assertEqual({f.name.split(".")[0] for f in files}, {s.name for s in compose.scenes()})
        for f in files:
            doc = json.loads(f.read_text(encoding="utf-8"))
            self.assertEqual((doc["format"], doc["version"], doc["kind"]), ("leyline-den", 1, "stamp"))
            self.assertTrue(re.fullmatch(r"[a-z0-9_-]{1,24}", doc["name"]))
            self.assertTrue(1 <= len(doc["props"]) <= 60)
            text = (doc["title"] + " " + doc["line"]).lower()
            self.assertFalse(any(b in text for b in BRAND) or re.search(r"\bwow\b", text), f.name)
            for p in doc["props"]:
                self.assertTrue(set(p) <= {"entry", "x", "y", "z", "yaw", "role"}, p)
                self.assertTrue(all(abs(p[k]) <= 40 for k in ("x", "y", "z")))
                self.assertTrue(0 <= p["yaw"] < 360)

    def test_footprints_are_sane(self):
        for k, f in compose.FOOT.items():
            self.assertTrue(k.isdigit())
            self.assertIn(f["kind"], ("seat", "table", "item", "floor", "fence", "shell", "ground"))
            self.assertTrue(0 < f["w"] <= 20 and 0 < f["d"] <= 20 and 0 < f["h"] <= 20, k)


if __name__ == "__main__":
    unittest.main(verbosity=1)
