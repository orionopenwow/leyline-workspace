#!/usr/bin/env python3
"""Leyline Control's queue (review 4 Nov): how UPDATES-PENDING.txt is read, what run_all does with it, and what
Autopilot may start by itself. No step really runs: subprocess.Popen is a fake that prints and exits as told.

  * comments, blank lines, done ids and the CRLF / BOM of a Notepad save are handled; unknown steps are flagged;
  * a ship queues leyline, which queues leyline-launcher (ids <id>-leyline, <id>-leyline-launcher), each once;
  * a failed step stops the run, is not marked done, records last-run.json and stops Autopilot until PENDING changes;
  * an unknown step and an owner-only step (discord-apply) stop the run before anything runs;
  * secrets a step prints never reach its log;
  * "Ask before deploys": Autopilot takes only the read-only steps at the front;
  * the history names the chained steps and links their logs.

    python3 -I tools/control/test_control_queue.py
"""
import importlib.util, io, json, os, sys, tempfile, unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
os.environ.update(WU_CONTROL_PORT="18109", LOCALAPPDATA=tempfile.mkdtemp())
spec = importlib.util.spec_from_file_location("control_q", HERE / "control.py")
ctl = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ctl)


class FakeProc:
    """What a step's .bat would print, and its exit code (by step name in RESULTS)."""
    RESULTS = {}
    calls = []

    def __init__(self, argv, **kw):
        bat = argv[-1]
        step = next((k for k, v in ctl.STEPS.items() if bat.endswith(v[0].replace("/", os.sep)) or bat.endswith(v[0])),
                    "?")
        FakeProc.calls.append(step)
        out, self.rc = FakeProc.RESULTS.get(step, (f"ran {step}\r\n", 0))
        self.stdout = io.BytesIO(out.encode())
        self.stdin = io.BytesIO()

    def wait(self):
        return self.rc


class HangProc:
    """A step that prints one line and then waits for ever (an ssh asking about a host key), until it is killed."""
    killed = []

    def __init__(self, argv, **kw):
        r, self._w = os.pipe()
        os.write(self._w, b"waiting for an answer...\r\n")
        self.stdout = os.fdopen(r, "rb")
        self.stdin = io.BytesIO()
        self.pid = None
        self.rc = None

    def kill(self):
        HangProc.killed.append(True)
        if self.rc is None:
            self.rc = -9
            os.close(self._w)

    def wait(self):
        return self.rc


class Queue(unittest.TestCase):
    def setUp(self):
        self.t = Path(tempfile.mkdtemp(prefix="leyline-control-q-"))
        for name, val in (("PEND", "UPDATES-PENDING.txt"), ("DONE", "UPDATES-DONE.txt"), ("LOGS", "logs"),
                          ("LAST", "last-run.json"), ("AUTO", "autopilot.json"), ("LAST_CHECK", "last-check.json")):
            setattr(ctl, name, self.t / val)
        self._popen, ctl.subprocess.Popen = ctl.subprocess.Popen, FakeProc
        self._hq, ctl.hq_sync_quiet = ctl.hq_sync_quiet, lambda: None
        self._dr, ctl.discord_record = ctl.discord_record, lambda *a: None
        FakeProc.RESULTS, FakeProc.calls = {}, []
        ctl.state.pop("auto_failed", None)
        ctl.state["lines"] = []

    def tearDown(self):
        ctl.subprocess.Popen, ctl.hq_sync_quiet, ctl.discord_record = self._popen, self._hq, self._dr

    def pend(self, text):
        ctl.PEND.write_bytes(text.encode("utf-8-sig"))

    def done(self):
        return ctl.read_lines(ctl.DONE)

    def test_parsing(self):
        self.pend("# a comment\r\n\r\n1311a check  read the server\r\n1311b SIGNIN  deploy the site\r\n"
                  "1311c no-such-step\r\nbadline\r\n")
        ctl.DONE.write_text("1311a\n")
        got = ctl.pending()
        self.assertEqual([(i["id"], i["step"], i["known"]) for i in got],
                         [("1311b", "signin", True), ("1311c", "no-such-step", False)])
        self.assertEqual(got[0]["note"], "deploy the site")

    def test_ship_chains_leyline_then_launcher_once(self):
        self.pend("1311a ship  the day's work\n")
        ctl.run_all("")
        self.assertEqual(FakeProc.calls, ["ship", "leyline", "leyline-launcher"])
        self.assertEqual(self.done(), ["1311a", "1311a-leyline", "1311a-leyline-launcher"])
        self.assertTrue((ctl.LOGS / "1311a-leyline-leyline.log").exists())
        self.assertTrue((ctl.LOGS / "1311a-leyline-launcher-leyline-launcher.log").exists())
        hist = ctl.history()
        self.assertEqual([(h["id"], h["step"]) for h in hist][:3],
                         [("1311a-leyline-launcher", "leyline-launcher"), ("1311a-leyline", "leyline"), ("1311a", "ship")])
        self.assertTrue(all(h["log"] for h in hist[:3]), "every chained step links its log")
        # a leyline already queued after the ship is not doubled
        FakeProc.calls = []
        self.pend("1311a ship\n1311b ship\n1311c leyline\n")
        ctl.run_all("")
        self.assertEqual(FakeProc.calls, ["ship", "leyline", "leyline-launcher"])

    def test_failure_stops_and_blocks_autopilot(self):
        FakeProc.RESULTS["signin"] = ("== deploy\r\nERROR: Caddy refused\r\n", 1)
        self.pend("1311a check\n1311b signin\n1311c audit\n")
        ctl.run_all("")
        self.assertEqual(FakeProc.calls, ["check", "signin"])
        self.assertEqual(self.done(), ["1311a"])
        last = json.loads(ctl.LAST.read_text())
        self.assertEqual((last["ok"], last["failed_step"], last["ids"]), (False, "signin", ["1311b"]))
        self.assertEqual(ctl.state["auto_failed"], ctl.pending_stamp(), "Autopilot waits until PENDING changes")
        self.assertIn("ERROR: Caddy refused", (ctl.LOGS / "1311b-signin.log").read_text())
        self.assertEqual([i["id"] for i in ctl.pending()], ["1311b", "1311c"])

    def test_unknown_and_owner_only_stop_before_running(self):
        self.pend("1311a typo-step\n1311b check\n")
        ctl.run_all("")
        self.assertEqual(FakeProc.calls, [])
        self.assertEqual(self.done(), [])
        self.pend("1311c discord-apply\n1311d check\n")
        ctl.run_all("")
        self.assertEqual(FakeProc.calls, [], "discord-apply never runs from the queue")
        self.assertTrue(any("Discord panel" in ln for ln in ctl.state["lines"]))

    def test_secrets_never_in_the_log(self):
        FakeProc.RESULTS["check"] = ("password=hunter2hunter2\r\nghp_" + "A" * 36 + "\r\nall fine\r\n", 0)
        self.pend("1311a check\n")
        ctl.run_all("")
        log = (ctl.LOGS / "1311a-check.log").read_text()
        self.assertNotIn("hunter2hunter2", log)
        self.assertNotIn("ghp_" + "A" * 36, log)
        self.assertIn("all fine", log)

    def test_scrub_more_shapes(self):
        for line, gone in (("RELAY_BLOCK_FEED_KEY=abcdef0123456789abcdef", "abcdef0123456789abcdef"),
                           ('{"token": "abcd1234efgh5678"}', "abcd1234efgh5678"),
                           ('"client_secret":"GOCSPX-abcdefghijklmnop"', "GOCSPX-abcdefghijklmnop"),
                           ("hook https://discord.com/api/webhooks/123456/abcDEF_ghi-jkl", "abcDEF_ghi-jkl"),
                           ("id AKIAABCDEFGHIJKLMNOP", "AKIAABCDEFGHIJKLMNOP")):
            self.assertNotIn(gone, ctl.scrub(line), line)
        for plain in ("keys: 3 found", "== Release audit: done", "realm key: 4 ok", "FASTLOOP OK"):
            self.assertEqual(ctl.scrub(plain), plain)

    def _hang(self, stop_after=None):
        import threading
        ctl.subprocess.Popen = HangProc
        HangProc.killed = []
        old = ctl.step_timeout_s
        ctl.step_timeout_s = (lambda step: 2) if stop_after is None else (lambda step: 3600)
        if stop_after is not None:
            threading.Timer(stop_after, lambda: ctl.state.__setitem__("stop", True)).start()
        try:
            self.pend("1311a check  a step that hangs\n1311b check  the next one\n")
            ctl.run_all("")
        finally:
            ctl.step_timeout_s = old
        return (ctl.LOGS / "1311a-check.log").read_text()

    def test_a_hung_step_is_stopped_at_its_time_limit(self):
        log = self._hang()
        self.assertTrue(HangProc.killed)
        self.assertIn("ran past its", log)
        self.assertEqual(self.done(), [], "the hung step is not done and the run stops there")
        self.assertTrue(json.loads(ctl.LAST.read_text())["failed_step"] == "check")

    def test_stop_button_stops_the_running_step(self):
        log = self._hang(stop_after=0.5)
        self.assertTrue(HangProc.killed)
        self.assertIn("Stopped by you", log)
        self.assertEqual(self.done(), [])

    def test_timeouts(self):
        self.assertEqual(ctl.step_timeout_s("check"), 90 * 60)
        self.assertGreaterEqual(ctl.step_timeout_s("client-build"), 6 * 3600)
        self.assertTrue(all(k in ctl.STEPS for k in ctl.STEP_TIMEOUT_MIN), [k for k in ctl.STEP_TIMEOUT_MIN if k not in ctl.STEPS])

    def test_ask_before_deploys(self):
        mk = lambda sid, step: {"id": sid, "step": step, "known": True}   # noqa: E731
        q = [mk("a", "check"), mk("b", "audit"), mk("c", "signin"), mk("d", "check")]
        self.assertEqual([i["id"] for i in ctl.autopilot_safe(q)], ["a", "b"])
        self.assertEqual(ctl.autopilot_safe([mk("c", "ship")]), [])
        for s in ("ship", "signin", "leyline", "fresh-history", "fresh-leyline", "discord-apply", "server-fix"):
            self.assertNotIn(s, ctl.READ_ONLY, s)


if __name__ == "__main__":
    unittest.main()
