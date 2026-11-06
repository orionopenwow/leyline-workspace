"""tools/solo/solo_wasm.py: the checksum and the magic are checked, the file lands whole.

    python3 tools/solo/test_solo_wasm.py
"""
import hashlib
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import solo_wasm  # noqa: E402

WASM = b"\0asm\x01\0\0\0" + b"\x00" * 64


class SoloWasm(unittest.TestCase):
    def test_check(self):
        good = hashlib.sha256(WASM).hexdigest()
        self.assertEqual(solo_wasm.check(WASM, f"{good}  den-lite-web.wasm\n"), good)
        with self.assertRaises(ValueError):
            solo_wasm.check(WASM + b"x", f"{good}  den-lite-web.wasm\n")
        with self.assertRaises(ValueError):
            solo_wasm.check(WASM, f"{good}  other.wasm\n")
        bad = b"MZ" + WASM[2:]
        with self.assertRaises(ValueError):
            solo_wasm.check(bad, f"{hashlib.sha256(bad).hexdigest()}  den-lite-web.wasm\n")

    def test_install(self):
        with tempfile.TemporaryDirectory() as d:
            out = solo_wasm.install(WASM, Path(d) / "solo")
            self.assertEqual(out.read_bytes(), WASM)
            self.assertEqual([p.name for p in (Path(d) / "solo").iterdir()], ["den-lite-web.wasm"])

    def test_the_release_and_the_page_agree(self):
        self.assertTrue(solo_wasm.BASE.endswith("/releases/download/den-lite-latest/"))
        self.assertIn('"den-lite-web.wasm"', (solo_wasm.REPO / "stream" / "server" / "www-patch.py").read_text(encoding="utf-8"))
        self.assertIn("den-lite-web.wasm", (solo_wasm.REPO / "stream" / "www-files" / "solo" / "solo.js").read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
