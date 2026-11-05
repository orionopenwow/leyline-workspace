#!/usr/bin/env python3
"""The reel kit's capture side (record.py) without a Windows PC: the ffmpeg command it records with, a take's life
(start, marks, stop, its .json; a failed take leaves nothing), which takes a run records, the Den flows sending only the
test actions the Leyline UI really has (Gizmo.lua TestAction), the hand takes' boxes, Edge on a fresh profile, and the
.bat files Control runs.

    python3 -I tools/reels/test_record.py
"""
import json
import os
import re
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)
import record as R  # noqa: E402

BRANDS = re.compile(r"\b(world of warcraft|wow|warcraft|lich king|blizzard)\b", re.I)


def read(rel, mode="r"):
    with open(os.path.join(ROOT, rel), mode) as f:
        return f.read()


class FakeProc:
    def __init__(self, cmd, **kw):
        self.cmd, self.kw, self.sent, self.killed = cmd, kw, b"", False
        out = cmd[-1]
        self.out = out
        proc = self

        class In:
            def write(self, b):
                proc.sent += b

            def flush(self):
                pass

            def close(self):
                # ffmpeg writes its file when it is told to stop
                with open(proc.out, "wb") as f:
                    f.write(b"mp4")
        self.stdin = In()

    def wait(self, timeout=None):
        return 0

    def kill(self):
        self.killed = True


class Clock:
    def __init__(self):
        self.t = 100.0

    def __call__(self):
        return self.t


class Grab(unittest.TestCase):
    def test_the_desktop_or_one_window(self):
        whole = R.gdigrab_cmd("ffmpeg", "o.mp4")
        self.assertEqual(whole[whole.index("-f") + 1], "gdigrab")
        self.assertEqual(whole[whole.index("-i") + 1], "desktop")
        self.assertNotIn("-offset_x", whole)
        self.assertEqual(whole[whole.index("-framerate") + 1], "30")
        win = R.gdigrab_cmd("ffmpeg", "o.mp4", (101, 52, 1281, 721))
        self.assertEqual(win[win.index("-offset_x") + 1], "101")
        self.assertEqual(win[win.index("-video_size") + 1], "1280x720", "even sizes for H.264")
        self.assertLess(win.index("-video_size"), win.index("-i"), "the grab's options come before its input")
        self.assertEqual(win[-1], "o.mp4")

    def test_a_window_is_kept_on_the_screen(self):
        self.assertEqual(R.clamp_rect((-8, -8, 1936, 1096), (1920, 1080)), (0, 0, 1920, 1080))
        self.assertEqual(R.clamp_rect((1800, 900, 400, 400), (1920, 1080)), (1800, 900, 120, 180))
        self.assertEqual(R.even_rect((0, 0, 1, 1)), (0, 0, 2, 2))


class Take(unittest.TestCase):
    def test_start_marks_stop(self):
        with tempfile.TemporaryDirectory() as d:
            clock = Clock()
            rec = R.Recorder("den-x", d, popen=FakeProc, clock=clock, ff="ffmpeg")
            rec.start((0, 0, 1280, 720), "game")
            self.assertTrue(rec.proc.out.endswith("den-x.part.mp4"), "recorded to a part file first")
            clock.t += 2.5
            rec.mark("carry")
            rec.guess("world", 30)
            clock.t += 4
            meta = rec.stop()
            self.assertEqual(rec.proc.sent, b"q", "ffmpeg is asked to stop, as a person would")
            self.assertEqual(meta["marks"], {"carry": 2.5, "world": 30.0})
            self.assertEqual(meta["guessed"], ["world"])
            self.assertAlmostEqual(meta["seconds"], 6.5)
            self.assertEqual(meta["rect"], [0, 0, 1280, 720])
            self.assertTrue(os.path.isfile(os.path.join(d, "den-x.mp4")))
            self.assertFalse(os.path.exists(os.path.join(d, "den-x.part.mp4")))
            with open(os.path.join(d, "den-x.json")) as f:
                self.assertEqual(json.load(f)["source"], "game")

    def test_a_failed_take_leaves_nothing(self):
        with tempfile.TemporaryDirectory() as d:
            rec = R.Recorder("den-y", d, popen=FakeProc, clock=Clock(), ff="ffmpeg")
            rec.start(None, "game")
            self.assertIsNone(rec.stop(ok=False))
            self.assertEqual([f for f in os.listdir(d)], [], "no half take for the edit to cut")

    def test_no_ffmpeg_says_so(self):
        rec = R.Recorder("x", tempfile.gettempdir(), popen=FakeProc, ff="")
        rec.ff = None
        with self.assertRaises(RuntimeError):
            rec.start()


class Plan(unittest.TestCase):
    def setUp(self):
        self.cat = R.catalog()

    def test_automatic_and_hand_runs(self):
        auto = R.chosen(self.cat, [], False)
        hand = R.chosen(self.cat, [], True)
        self.assertIn("game-den-carry", auto)
        self.assertIn("site-live", auto)
        self.assertNotIn("lf-login", auto, "launcher-fake takes are takes.cjs's")
        self.assertNotIn("site-queue", auto, "offline site takes are takes.cjs's")
        self.assertTrue(hand and all(self.cat[t]["source"] == "hand" for t in hand))
        self.assertFalse(set(auto) & set(hand))
        self.assertEqual(R.chosen(self.cat, ["game-den-fly"], False), ["game-den-fly"])
        self.assertEqual(R.chosen(self.cat, ["game-den-fly"], True), [])

    def test_unknown_take(self):
        self.assertEqual(R.main(["no-such-take"]), 2)
        self.assertEqual(R.main(["--list"]), 0)

    def test_hand_boxes_say_what_to_do(self):
        for tid, t in self.cat.items():
            if t["source"] != "hand":
                continue
            text = R.hand_text(tid, t)
            self.assertIn(t["prompt"], text)
            self.assertIn("%d seconds" % int(t["seconds"]), text)
            self.assertIn("Cancel skips", text)
            self.assertNotRegex(text, BRANDS)
            self.assertNotRegex(text.lower(), r"\bsign(ed)? ?in\b")

    def test_edge_opens_on_a_fresh_profile(self):
        cmd = R.edge_cmd("msedge.exe", "https://leylinelauncher.com/wrath/leyline", "C:\\tmp\\p", (1280, 760))
        self.assertIn("--app=https://leylinelauncher.com/wrath/leyline", cmd)
        self.assertIn("--user-data-dir=C:\\tmp\\p", cmd)
        self.assertIn("--window-size=1280,760", cmd)


class DenFlows(unittest.TestCase):
    """The Den takes send only `.den ui gizmo <action>` with actions Gizmo.lua's TestAction answers."""

    @classmethod
    def setUpClass(cls):
        lua = read("stream/fui-web/Interface/AddOns/WrathUnleashedUI/Gizmo.lua")
        body = lua[lua.index("function G.TestAction(w)"):]
        body = body[:body.index("\nend\n")]
        cls.actions = set(re.findall(r'a == "([a-z]+)"', body))

    def run_flow(self, flow):
        sent = []

        class Ch:
            def send(self, cmd, wait=10.0):
                sent.append(cmd)
                return {}
        rec = R.Recorder("t", tempfile.gettempdir(), popen=FakeProc, clock=Clock(), ff="ffmpeg")
        rec.t0 = 0.0
        sleep = R.time.sleep
        R.time.sleep = lambda s: None
        try:
            flow(rec, Ch())
        finally:
            R.time.sleep = sleep
        return sent, rec

    def check(self, flow, marks):
        sent, rec = self.run_flow(flow)
        for cmd in sent:
            m = re.fullmatch(r"\.den ui gizmo ([a-z]+)(?: .*)?", cmd)
            self.assertIsNotNone(m, cmd)
            self.assertIn(m.group(1), self.actions, cmd)
        self.assertEqual(list(rec.marks), marks)
        return sent

    def test_carry(self):
        sent = self.check(R.den_carry, ["build", "place", "carry", "drop", "turn"])
        self.assertIn(".den ui gizmo arm 2413", sent)
        self.assertIn(".den ui gizmo carry 0.25 -0.35", sent, "picked up where it was placed")
        self.assertEqual(sum(1 for c in sent if c == ".den ui gizmo commit"), 2)
        self.assertGreater(sum(1 for c in sent if c.startswith(".den ui gizmo hover")), 10, "it glides")

    def test_fly(self):
        sent = self.check(R.den_fly, ["build", "fly", "snap", "zoomout", "back"])
        self.assertLess(sent.index(".den ui gizmo fly W down"), sent.index(".den ui gizmo fly W up"))

    def test_the_marks_are_the_takes(self):
        cat = R.catalog()
        self.assertEqual(self.run_flow(R.den_carry)[1].marks.keys(), set(cat["game-den-carry"]["marks"]))
        self.assertEqual(self.run_flow(R.den_fly)[1].marks.keys(), set(cat["game-den-fly"]["marks"]))

    def test_glide(self):
        pts = R.path_points((0, 0), (1, -1), 10)
        self.assertEqual(len(pts), 10)
        self.assertEqual(pts[-1], (1, -1))
        self.assertLess(pts[0][0], 0.1, "eased: slow at the start")


class Bats(unittest.TestCase):
    def test_crlf_and_what_they_run(self):
        for name, script, extra in (("Reel record.bat", "record.py", ""), ("Reel record - hand takes.bat", "record.py", "--hand"),
                                    ("Reel make.bat", "edit.py", "")):
            raw = read("tools/reels/" + name, "rb")
            self.assertNotIn(b"\n", raw.replace(b"\r\n", b""), name + ": CRLF only")
            text = raw.decode("ascii")
            self.assertIn('%%PY%% -I "tools\\reels\\%s"' % script, text, name)
            if extra:
                self.assertIn(extra, text)
            self.assertIn('cd /d "%~dp0..\\.."', text)
            self.assertIn("Gyan.FFmpeg", text, name + ": ffmpeg is got when missing")
            self.assertNotRegex(text, BRANDS)
            for bad in ("password", "--key", "post", "upload"):
                self.assertNotIn(bad, text.lower().replace("nothing is posted", "").replace("is posted anywhere", ""), name)

    def test_the_record_bat_takes_the_newest_launcher(self):
        text = read("tools/reels/Reel record.bat")
        self.assertIn("releases/download/launcher-latest/Leyline.exe", text)


if __name__ == "__main__":
    unittest.main()
