"""python3 tools/den-ui/icons/test_icons.py: the Den editor's own icons (make_icons.py) are what the 3.3.5a client reads
from an addon folder (uncompressed 32-bit TGA, 64 x 64, bottom-up rows), every name Style.lua S.ICONS lists has its
file, the Den UI's files use no game icon (Interface\\Icons), and both ways the addon reaches players carry them: the
addon pack (addons/pack/make-addonpack.py) and the browser stream's overlay (stream/server/fui-overlay.py)."""
import importlib.util
import os
import re
import struct
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
ADDON = os.path.join(ROOT, "stream", "fui-web", "Interface", "AddOns", "WrathUnleashedUI")
ICONS = os.path.join(ADDON, "Media", "Icons")
DEN_UI = ("Builder.lua", "DenBar.lua", "DenHome.lua", "Style.lua")


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    m = importlib.util.module_from_spec(spec)
    sys.path.insert(0, os.path.dirname(path))
    try:
        spec.loader.exec_module(m)
    finally:
        sys.path.pop(0)
    return m


class Icons(unittest.TestCase):
    def names(self):
        with open(os.path.join(ADDON, "Style.lua"), encoding="utf-8") as fh:
            s = fh.read()
        block = re.search(r"S\.ICONS = \{(.*?)\}", s, re.S).group(1)
        return re.findall(r'"(\w+)"', block)

    def test_every_listed_icon_is_a_tga_the_client_reads(self):
        names = self.names()
        self.assertGreaterEqual(len(names), 40)
        for n in names:
            p = os.path.join(ICONS, n + ".tga")
            self.assertTrue(os.path.isfile(p), n)
            b = open(p, "rb").read()
            idlen, cmap, kind, _, _, _, _, _, w, h, bpp, desc = struct.unpack("<BBBHHBHHHHBB", b[:18])
            self.assertEqual((idlen, cmap, kind, bpp, desc & 0x0F), (0, 0, 2, 32, 8), n)   # uncompressed true colour, 8 alpha bits
            self.assertEqual(desc & 0x20, 0, n)                                          # bottom-left origin
            self.assertEqual((w, h), (64, 64), n)
            self.assertEqual(len(b), 18 + w * h * 4, n)
            alpha = b[18 + 3::4]
            self.assertTrue(any(a > 200 for a in alpha) and any(a == 0 for a in alpha), n + ": white lines on transparent")

    def test_the_generator_makes_exactly_these(self):
        if importlib.util.find_spec("PIL") is None:
            self.skipTest("Pillow isn't installed here")
        mk = load(os.path.join(os.path.dirname(__file__), "make_icons.py"), "make_icons")
        self.assertEqual(sorted(mk.ICONS), sorted(self.names()))
        files = sorted(f[:-4] for f in os.listdir(ICONS) if f.endswith(".tga"))
        self.assertEqual(files, sorted(self.names()))

    def test_the_den_ui_uses_no_game_icon(self):
        for f in DEN_UI:
            with open(os.path.join(ADDON, f), encoding="utf-8") as fh:
                s = fh.read()
            code = "\n".join(l.split("--", 1)[0] for l in s.splitlines())
            self.assertNotIn("Interface\\\\Icons", code, f)

    def test_the_pack_and_the_stream_carry_them(self):
        pack = load(os.path.join(ROOT, "addons", "pack", "make-addonpack.py"), "make_addonpack")
        self.assertIn(".tga", pack.ALLOWED)
        self.assertNotIn("Media", pack.SKIP_DIRS)
        self.assertNotIn("Icons", pack.SKIP_DIRS)
        ov = load(os.path.join(ROOT, "stream", "server", "fui-overlay.py"), "fui_overlay")
        self.assertTrue(ov.taken(("Interface", "AddOns", "WrathUnleashedUI", "Media", "Icons", "rotate.tga")))


if __name__ == "__main__":
    unittest.main()
