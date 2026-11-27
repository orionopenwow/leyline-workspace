#!/usr/bin/env python3
"""Leyline Control without the server (docs/SERVERLESS.md; the owner shut it down on 26 Nov 2026).

  * RETIRED names only real steps, each with a one-line reason; the GitHub chain (ship -> leyline -> leyline-launcher),
    the local tests and the Discord card's steps (Discord's own API, no server) are not retired;
  * a retired line in UPDATES-PENDING.txt never runs: a clear log line and log file, it leaves the queue, and the run
    goes on (not a failure, so Autopilot doesn't stop); "Ask before deploys" lets it through to be skipped;
  * no button runs one (/api/do, /api/check, /api/audit, HQ, GM console), the page hides the server's buttons, the GM
    card's actions leave them out, the kill switch's step runner refuses them, and the HQ sync after a run is skipped;
  * the header polls GitHub's public API, never the old server's names;
  * the switch: control/server.json {"on": true} brings all back, {"steps": [...]} some, LEYLINE_SERVER=1 all.

    python3 -I tools/control/test_control_retired.py
"""
import http.client, importlib.util, io, json, os, tempfile, threading, unittest
from http.server import ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parent
PORT = 18113
os.environ.update(WU_CONTROL_PORT=str(PORT), LOCALAPPDATA=tempfile.mkdtemp())
os.environ.pop("LEYLINE_SERVER", None)
spec = importlib.util.spec_from_file_location("control_r", HERE / "control.py")
ctl = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ctl)
srv = ThreadingHTTPServer(("127.0.0.1", PORT), ctl.H)
srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()


def req(method, path, body=None):
    c = http.client.HTTPConnection("127.0.0.1", PORT, timeout=20)
    h = {"Host": f"127.0.0.1:{PORT}", "X-Token": ctl.TOKEN, "Content-Type": "application/json"}
    c.request(method, path, body=json.dumps(body) if body is not None else None, headers=h)
    r = c.getresponse()
    return r.status, r.read()


class FakeProc:
    calls = []

    def __init__(self, argv, **kw):
        bat = argv[-1].replace(os.sep, "/")
        step = next((k for k, v in ctl.STEPS.items() if bat.endswith(v[0])), "?")
        FakeProc.calls.append(step)
        self.stdout = io.BytesIO(f"ran {step}\r\n".encode())
        self.stdin = io.BytesIO()
        self.rc = 0

    def wait(self):
        return self.rc


class Rig(unittest.TestCase):
    def setUp(self):
        self.t = Path(tempfile.mkdtemp(prefix="leyline-control-r-"))
        for name, val in (("PEND", "UPDATES-PENDING.txt"), ("DONE", "UPDATES-DONE.txt"), ("LOGS", "logs"),
                          ("LAST", "last-run.json"), ("AUTO", "autopilot.json"), ("LAST_CHECK", "last-check.json"),
                          ("SERVER_DOC", "control/server.json")):
            setattr(ctl, name, self.t / val)
        self._popen, ctl.subprocess.Popen = ctl.subprocess.Popen, FakeProc
        self._hq, ctl.hq_sync_quiet = ctl.hq_sync_quiet, lambda: None
        self._dr, ctl.discord_record = ctl.discord_record, lambda *a: None
        FakeProc.calls = []
        ctl.state.pop("auto_failed", None)
        ctl.state["lines"] = []
        os.environ.pop("LEYLINE_SERVER", None)

    def tearDown(self):
        ctl.subprocess.Popen, ctl.hq_sync_quiet, ctl.discord_record = self._popen, self._hq, self._dr
        os.environ.pop("LEYLINE_SERVER", None)

    def switch(self, doc):
        ctl.SERVER_DOC.parent.mkdir(parents=True, exist_ok=True)
        ctl.SERVER_DOC.write_text(json.dumps(doc), encoding="utf-8")


class Table(Rig):
    def test_retired_names_real_steps_with_one_line_reasons(self):
        self.assertTrue(set(ctl.RETIRED) <= set(ctl.STEPS), set(ctl.RETIRED) - set(ctl.STEPS))
        for k, why in ctl.RETIRED.items():
            self.assertTrue(why and "\n" not in why and len(why) < 200, k)
            self.assertEqual(ctl.retired(k), why)
        for bad in ("World of Warcraft", "WoW", "Warcraft", "Lich King", "Blizzard"):
            self.assertNotIn(bad, " ".join(ctl.RETIRED.values()))

    def test_the_server_steps_are_retired(self):
        for s in ("signin", "page", "client", "scripts", "scripts-reload", "realm", "realm-image", "foreverui", "check",
                  "audit", "server-fix", "worldlog", "gm", "relay", "hangout", "mods", "bugs", "feedback", "phonelab",
                  "discord-bridge", "hq-sync", "hqnote", "hq-inbox", "sv-site", "sv-site-check", "sv-site-off",
                  "release-mirror", "tidy", "caddy-check", "slice", "combos", "domain-on"):
            self.assertTrue(ctl.retired(s), s)

    def test_github_local_and_discord_steps_are_kept(self):
        for s in ("ship", "ship-again", "leyline", "leyline-launcher", "leyline-mobile", "leyline-solo", "hostkit",
                  "addons", "sp-playtest", "sp-playtest-boss", "den-test", "den-check", "den-lite", "den-lite-test",
                  "bots-smoke", "boss-rush", "mods-daily", "addons-curate", "thumbs", "fresh-check", "fresh-history",
                  "fresh-leyline", "fresh-leyline-check", "builds-why", "apply-check", "apply", "commons-check",
                  "discord-preview", "discord-apply", "discord-check", "discord-read", "discord-announce",
                  "announce-check", "solo-check", "solo-setup"):
            self.assertEqual(ctl.retired(s), "", s)

    def test_ship_says_the_website_builds_from_the_public_repo(self):
        self.assertIn("GitHub Pages", ctl.STEPS["ship"][1])
        self.assertIn("GitHub Pages", ctl.STEPS["leyline"][1])


class Queue(Rig):
    def test_a_retired_line_is_skipped_logged_and_the_run_goes_on(self):
        ctl.PEND.write_text("1311a signin  the site\n1311b ship  the day's work\n1311c check\n", encoding="utf-8")
        p = {i["id"]: i for i in ctl.pending()}
        self.assertTrue(p["1311a"]["retired"] and p["1311c"]["retired"])
        self.assertEqual(p["1311b"]["retired"], "")
        ctl.run_all("")
        self.assertEqual(FakeProc.calls, ["ship", "leyline", "leyline-launcher"], "signin and check never ran")
        done = ctl.read_lines(ctl.DONE)
        for sid in ("1311a", "1311b", "1311b-leyline", "1311b-leyline-launcher", "1311c"):
            self.assertIn(sid, done)
        self.assertEqual(ctl.pending(), [], "the queue moved on")
        log = (ctl.LOGS / "1311a-signin.log").read_text(encoding="utf-8")
        self.assertIn("Skipped 'signin' (1311a): retired", log)
        self.assertIn("docs/SERVERLESS.md", log)
        self.assertTrue(any("Skipped 'check'" in ln for ln in ctl.state["lines"]))
        self.assertIsNone(ctl.state.get("auto_failed"), "a skip is not a failure: Autopilot keeps going")
        self.assertTrue(json.loads(ctl.LAST.read_text())["ok"])

    def test_ask_before_deploys_lets_retired_lines_through_to_be_skipped(self):
        mk = lambda sid, step: {"id": sid, "step": step, "known": True}   # noqa: E731
        q = [mk("a", "signin"), mk("b", "discord-check"), mk("c", "ship"), mk("d", "check")]
        self.assertEqual([i["id"] for i in ctl.autopilot_safe(q)], ["a", "b"])

    def test_the_switch_brings_them_back(self):
        self.switch({"steps": ["relay"]})
        self.assertEqual(ctl.retired("relay"), "")
        self.assertTrue(ctl.retired("signin"))
        self.switch({"on": True})
        self.assertEqual([s for s in ctl.RETIRED if ctl.retired(s)], [])
        ctl.PEND.write_text("1311a signin\n", encoding="utf-8")
        ctl.run_all("")
        self.assertEqual(FakeProc.calls, ["signin"])
        self.switch({"on": False})
        self.assertTrue(ctl.retired("signin"))
        os.environ["LEYLINE_SERVER"] = "1"
        self.assertEqual(ctl.retired("signin"), "")


class Buttons(Rig):
    def test_no_button_runs_a_retired_step(self):
        started = []
        old, ctl.start_run = ctl.start_run, lambda *a, **k: started.append(a) or True
        try:
            for path, body in (("/api/do", {"run": "signin"}), ("/api/do", {"run": "server-fix"}), ("/api/check", {}),
                               ("/api/audit", {}), ("/api/open-hq", {}), ("/api/gm-console", {})):
                st, out = req("POST", path, body)
                self.assertEqual(st, 409, path)
                self.assertIn(b"retired", out, path)
            self.assertEqual(started, [])
            st, _ = req("POST", "/api/do", {"run": "den-check"})
            self.assertEqual(st, 200, "a kept step still runs from a button")
        finally:
            ctl.start_run = old

    def test_state_page_and_gm_card(self):
        st, out = req("GET", "/api/state")
        self.assertEqual(st, 200)
        self.assertTrue(json.loads(out)["serverless"])
        for bid in ("audit", "check", "hq", "gmc"):
            self.assertRegex(ctl.PAGE, rf'<button id="{bid}" data-server="1"', bid)
        self.assertIn("[data-server]", ctl.PAGE)
        acts = [a["step"] for a in ctl.gm_overview()["actions"]]
        self.assertFalse([a for a in acts if ctl.retired(a)], acts)

    def test_kill_switch_runner_and_hq_sync_refuse(self):
        ran = []
        old_popen, ctl.subprocess.Popen = ctl.subprocess.Popen, lambda *a, **k: ran.append(a)
        old_run, ctl.subprocess.run = ctl.subprocess.run, lambda *a, **k: ran.append(a)
        try:
            self.assertEqual(ctl.ks_run_step("hangout-off"), 1)
            self._hq()          # the real hq_sync_quiet (setUp swapped in a stub)
        finally:
            ctl.subprocess.Popen, ctl.subprocess.run = old_popen, old_run
        self.assertEqual(ran, [], "nothing started")


class Health(Rig):
    def test_header_polls_github_never_the_server(self):
        urls = []

        class R(io.BytesIO):
            status = 200

            def __enter__(self):
                return self

            def __exit__(self, *a):
                return False

        def fake(req, timeout=0):
            urls.append(req.full_url)
            if "releases" in req.full_url:
                return R(json.dumps({"assets": [{"name": "Leyline.exe"}]}).encode())
            return R(json.dumps({"workflow_runs": [{"conclusion": "success"}]}).encode())
        old, ctl.urllib.request.urlopen = ctl.urllib.request.urlopen, fake
        try:
            res = ctl.github_health()
        finally:
            ctl.urllib.request.urlopen = old
        self.assertTrue(urls and all(u.startswith("https://api.github.com/repos/orionopenwow/Leyline/") for u in urls), urls)
        self.assertEqual({k: v["ok"] for k, v in res.items()}, {"Launcher release": True, "GitHub builds": True})
        self.assertGreaterEqual(ctl.HEALTH_EVERY["github"], 300, "GitHub's unauthenticated limit is 60 an hour")


if __name__ == "__main__":
    unittest.main()
