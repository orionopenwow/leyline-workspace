"""tools/den-looks: the patches go into the game's Data only for a Den session and come out exactly; nothing that
isn't ours is touched. Run: python3 tools/den-looks/test_den_looks.py"""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import den_looks  # noqa: E402


class DenLooks(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.data = self.tmp / "game" / "Data"
        self.data.mkdir(parents=True)
        (self.data / "patch-T.MPQ").write_bytes(b"den ground")
        den_looks.STORE = self.tmp / "looks"
        den_looks.STORE.mkdir()
        for _, name, _, _ in den_looks.MODULES:
            (den_looks.STORE / name).write_bytes(b"pr " + name.encode())

    def test_the_default_set_keeps_their_rules(self):
        letters = [m[0] for m in den_looks.chosen([])]
        self.assertTrue({"B", "D", "E"} <= set(letters) and "A" in letters and "M" not in letters)
        self.assertIn("A", [m[0] for m in den_looks.chosen(["U"])])

    def test_in_for_the_den_then_out_exactly(self):
        done = den_looks.put_in(self.data, den_looks.chosen([]))
        self.assertEqual(sorted(done), sorted(m[1] for m in den_looks.chosen([])))
        self.assertEqual((self.data / "patch-A.mpq").read_bytes(), b"pr patch-A.mpq")
        self.assertEqual(json.loads((self.data / den_looks.MARK).read_text())["files"], done)
        self.assertEqual(den_looks.clean(self.data), 0)
        self.assertEqual(sorted(p.name for p in self.data.iterdir()), ["patch-T.MPQ"], "only the Den's own ground is left")
        self.assertTrue((den_looks.STORE / "patch-A.mpq").is_file(), "the downloads stay for next time")

    def test_a_slot_used_by_something_else_stops_it(self):
        (self.data / "Patch-C.MPQ").write_bytes(b"someone else's mod")
        self.assertIsNone(den_looks.put_in(self.data, den_looks.chosen([])))
        self.assertFalse((self.data / "patch-A.mpq").exists(), "nothing changed")
        self.assertEqual((self.data / "Patch-C.MPQ").read_bytes(), b"someone else's mod")

    def test_leftovers_from_a_crash_come_out_first(self):
        den_looks.put_in(self.data, den_looks.chosen([]))
        self.assertIsNotNone(den_looks.put_in(self.data, den_looks.chosen([])) if den_looks.clean(self.data, quiet=True) == 0 else None)

    def test_nothing_downloaded_changes_nothing(self):
        for p in den_looks.STORE.iterdir():
            p.unlink()
        self.assertIsNone(den_looks.put_in(self.data, den_looks.chosen([])))
        self.assertEqual([p.name for p in self.data.iterdir()], ["patch-T.MPQ"])


if __name__ == "__main__":
    unittest.main()
