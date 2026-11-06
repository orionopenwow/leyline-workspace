#!/usr/bin/env python3
"""tools/phonelab/phonelab.py, the Control step "phonelab": a Phone Lab session made by the portal's own module
(stream/phonelab.py) is found (the newest, or a named one), printed one tap per block (where, on what, the events in
order with the preventDefault marks, the full screen ask), copied for the PC, and its sketches written as JPEG files.
The .bat: CRLF, read only, registered as a READ_ONLY Control step.

    python3 -I tools/phonelab/test_phonelab.py
"""
import contextlib
import importlib.util
import io
import json
import os
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


TMP = tempfile.mkdtemp(prefix="wu-pl-tool-")
os.environ["WU_PHONELAB_DIR"] = os.path.join(TMP, "lab")
LAB = load("lab", os.path.join(ROOT, "stream", "phonelab.py"))
TOOL = load("tool", os.path.join(HERE, "phonelab.py"))
JPEG = "data:image/jpeg;base64,/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAP////8="


def session():
    m = LAB.start(by="owner")
    batch = {"code": m["code"], "seq": 1, "page": "/wrath/leyline", "recs": [
        {"kind": "page", "t": 5, "p": "/wrath/leyline", "ua": "Mozilla/5.0 (Linux; Android 14; Pixel 7)", "dpr": 2.625, "screen": [412, 915],
         "w": 412, "h": 839, "o": "portrait-primary", "touch": 5},
        {"kind": "tap", "t": 2300, "p": "/wrath/leyline", "x": 380, "y": 30, "on": ".hb-top > label.hb-menu-b", "text": "Menu",
         "seq": [["touchstart", 0, 0], ["pointerdown", 0, 1], ["touchend", 92, 0], ["pointerup", 92, 0], ["click", 95, 0]],
         "click": ".hb-top > label.hb-menu-b", "moved": 3, "fingers": 1, "resp": 12, "shot": 0,
         "before": {"w": 412, "h": 839, "o": "portrait-primary", "fs": False}, "after": {"w": 412, "h": 839, "o": "portrait-primary", "fs": False}},
        {"kind": "fs-ask", "t": 9000, "p": "/h.html", "from": "pointerup", "gesture": True, "ok": True},
        {"kind": "error", "t": 9100, "p": "/h.html", "msg": "boom", "at": "touch.js:12"}],
        "shots": [{"i": 0, "data": JPEG}]}
    assert LAB.record(json.dumps(batch).encode(), "198.51.100.9")[0] == 204
    return m["code"]


class Tool(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.code = session()

    def run_main(self, *args):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            rc = TOOL.main(list(args))
        return rc, out.getvalue()

    def test_server_side_newest_printed_and_copied(self):
        copy = os.path.join(TMP, "copy")
        rc, text = self.run_main("--dir", LAB.data_dir(), "--latest", "--code", "%CODE%", "--copy", copy)
        self.assertEqual(rc, 0)
        self.assertIn(f"PHONE LAB {self.code}", text)
        self.assertIn('TAP   (380,30) on .hb-top > label.hb-menu-b "Menu"', text)
        self.assertIn("touchstart@0 pointerdown@0! touchend@92 pointerup@92 click@95", text)
        self.assertIn("page answered 12 ms, sketch #0", text)
        self.assertIn("FULL SCREEN ASK from pointerup gesture=True ok", text)
        self.assertIn("ERROR boom touch.js:12", text)
        self.assertIn("--- /h.html", text)
        self.assertTrue(text.rstrip().endswith(f"PHONELAB OK: {self.code} 1 taps"))
        self.assertTrue(os.path.isfile(copy + ".json") and os.path.isfile(copy + ".jsonl"))

    def test_pc_side_from_the_copy_with_sketches(self):
        copy = os.path.join(TMP, "copy2")
        self.run_main("--dir", LAB.data_dir(), "--latest", "--copy", copy)
        shots = os.path.join(TMP, "shots")
        rc, text = self.run_main("--file", copy + ".jsonl", "--shots", shots)
        self.assertEqual(rc, 0)
        self.assertIn("1 sketches written", text)
        with open(os.path.join(shots, "sketch-000.jpg"), "rb") as f:
            self.assertEqual(f.read(3), b"\xff\xd8\xff")

    def test_named_and_missing(self):
        rc, text = self.run_main("--dir", LAB.data_dir(), "--latest", "--code", self.code)
        self.assertEqual(rc, 0)
        rc, text = self.run_main("--dir", os.path.join(TMP, "empty"), "--latest")
        self.assertEqual(rc, 1)
        self.assertIn("PHONELAB NONE", text)

    def test_bat_and_step(self):
        with open(os.path.join(HERE, "Phone Lab.bat"), "rb") as f:
            bat = f.read()
        self.assertNotIn(b"\n", bat.replace(b"\r\n", b""), "CRLF only")
        self.assertIn(b"/srv/wrath/portal-data/phonelab", bat)
        self.assertNotIn(b"rm -rf", bat)
        with open(os.path.join(ROOT, "tools", "control", "control.py"), encoding="utf-8") as f:
            ctl = f.read()
        self.assertIn('"phonelab": ("tools/phonelab/Phone Lab.bat"', ctl)
        self.assertRegex(ctl, r'READ_ONLY = \{[^}]*"phonelab"')
        with open(os.path.join(ROOT, ".gitignore"), encoding="utf-8") as f:
            self.assertIn("/tools/phonelab/live/", f.read())


if __name__ == "__main__":
    unittest.main()
