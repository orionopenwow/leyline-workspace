#!/usr/bin/env python3
"""Leyline Control's release audit: the step "audit" (read-only, so Autopilot may run it), the Release audit button
(one click starts it and the report opens), and the report served to this PC only, with the token.

    python3 -I tools/control/test_control_audit.py
"""
import http.client, importlib.util, json, os, sys, tempfile, threading, time, unittest
from http.server import ThreadingHTTPServer
from pathlib import Path

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
PORT = 18099
os.environ.update(WU_CONTROL_PORT=str(PORT), LOCALAPPDATA=tempfile.mkdtemp())
spec = importlib.util.spec_from_file_location("control", os.path.join(HERE, "control.py"))
ctl = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ctl)
T = Path(tempfile.mkdtemp())
ctl.AUDIT_HTML, ctl.AUDIT_TXT = T / "audit-latest.html", T / "audit-latest.txt"
srv = ThreadingHTTPServer(("127.0.0.1", PORT), ctl.H)
srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()


def req(method, path, headers=None):
    c = http.client.HTTPConnection("127.0.0.1", PORT, timeout=20)
    h = {"Host": f"127.0.0.1:{PORT}"}
    h.update(headers or {})
    c.request(method, path, headers=h)
    r = c.getresponse()
    return r.status, r.read()


class Audit(unittest.TestCase):
    def test_the_step(self):
        bat, title, interactive = ctl.STEPS["audit"]
        self.assertEqual(bat, "tools/audit/Release audit.bat")
        self.assertEqual(title, "Release audit (read-only, ~10 min)")
        self.assertFalse(interactive)
        self.assertIn("audit", ctl.READ_ONLY)
        self.assertTrue(os.path.isfile(os.path.join(ROOT, bat)))
        text = open(os.path.join(ROOT, bat), encoding="utf-8").read()
        self.assertIn(r"tools\audit\audit.py", text)
        self.assertIn("LEYLINE_AUDIT_NO_OPEN", text)
        self.assertIn("exit /b %RC%", text)          # a red audit is a failed step
        self.assertNotIn("pause", text.lower())       # never waits for a key under Control

    def test_one_click_button_starts_it(self):
        self.assertIn('id="audit"', ctl.PAGE)
        self.assertIn('"/api/audit"', ctl.PAGE)
        got = []
        ctl.start_run = lambda msg, items=None, record=True, title="": got.append((items, record, title)) or True
        st, _ = req("POST", "/api/audit")
        self.assertEqual(st, 403, "no token: refused")
        st, body = req("POST", "/api/audit", {"X-Token": ctl.TOKEN})
        self.assertEqual(st, 200, body)
        (items, record, title), = got
        self.assertEqual(items[0]["step"], "audit")
        self.assertTrue(items[0]["open"], "the button's run opens the report")
        self.assertFalse(record, "not written to UPDATES-DONE.txt")
        self.assertEqual(title, "Release audit")

    def test_the_report_needs_the_token(self):
        ctl.AUDIT_HTML.write_text("<!doctype html><title>Release audit</title>ok", encoding="utf-8")
        ctl.AUDIT_TXT.write_text("Leyline release audit, x\nResult: ALL GREEN  (70 passed, 0 failed, 3 skipped)\n", encoding="utf-8")
        st, _ = req("GET", "/audit")
        self.assertEqual(st, 403)
        st, body = req("GET", f"/audit?t={ctl.TOKEN}")
        self.assertEqual((st, b"Release audit" in body), (200, True))
        st, body = req("GET", "/api/state", {"X-Token": ctl.TOKEN})
        a = json.loads(body)["audit"]
        self.assertTrue(a["green"])
        self.assertIn("70 passed", a["result"])
        ctl.AUDIT_TXT.write_text("Leyline release audit, x\nResult: 2 FAILED  (68 passed, 2 failed, 3 skipped)\n", encoding="utf-8")
        self.assertFalse(ctl.audit_state()["green"])
        ctl.AUDIT_HTML.unlink()
        self.assertIsNone(ctl.audit_state())

    def test_autopilot_runs_never_pop_a_browser(self):
        src = open(os.path.join(HERE, "control.py"), encoding="utf-8").read()
        self.assertIn('if item["step"] == "audit" and not item.get("open"):', src)
        self.assertIn('env["LEYLINE_AUDIT_NO_OPEN"] = "1"', src)


if __name__ == "__main__":
    unittest.main()
