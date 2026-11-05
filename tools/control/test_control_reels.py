#!/usr/bin/env python3
"""Leyline Control's reel steps (tools/reels; owner 4 Nov: reels, flow videos, a launch video kept up to date):
reel-record and reel-record-hand start the game or wait for the owner, so Autopilot never runs them by itself with
"Ask before deploys" on; reel-make only cuts videos on this PC, so it may. Each step's .bat is there, CRLF, and runs the
kit's own script; no step posts anything.

    python3 -I tools/control/test_control_reels.py
"""
import importlib.util
import os
import re
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
os.environ.update(WU_CONTROL_PORT="18097", LOCALAPPDATA=tempfile.mkdtemp())
spec = importlib.util.spec_from_file_location("control", os.path.join(HERE, "control.py"))
ctl = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ctl)

BRANDS = re.compile(r"\b(world of warcraft|wow|warcraft|lich king|blizzard)\b", re.I)


class ReelSteps(unittest.TestCase):
    def test_the_steps(self):
        want = {"reel-record": ("tools/reels/Reel record.bat", "record.py"),
                "reel-record-hand": ("tools/reels/Reel record - hand takes.bat", "record.py\" --hand"),
                "reel-make": ("tools/reels/Reel make.bat", "edit.py")}
        for step, (bat, runs) in want.items():
            self.assertIn(step, ctl.STEPS)
            got_bat, title, interactive = ctl.STEPS[step]
            self.assertEqual(got_bat, bat)
            self.assertFalse(interactive)
            self.assertTrue(title.startswith("Reels: "), title)
            self.assertNotRegex(title, BRANDS)
            raw = open(os.path.join(ROOT, bat), "rb").read()
            self.assertNotIn(b"\n", raw.replace(b"\r\n", b""), bat + " is CRLF")
            self.assertIn(("tools\\reels\\" + runs).encode(), raw)

    def test_only_reel_make_runs_by_itself(self):
        self.assertIn("reel-make", ctl.READ_ONLY)
        for step in ("reel-record", "reel-record-hand"):
            self.assertNotIn(step, ctl.READ_ONLY, step + " starts the game or waits for you")
            self.assertNotIn(step, ctl.OWNER_ONLY, step + " is queued like any step")

    def test_nothing_is_posted(self):
        for step in ("reel-record", "reel-record-hand", "reel-make"):
            title = ctl.STEPS[step][1].lower()
            self.assertTrue("posted" not in title or "nothing is posted" in title, title)
            self.assertNotIn("upload", title)


if __name__ == "__main__":
    unittest.main()
