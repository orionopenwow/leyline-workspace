#!/usr/bin/env python3
"""Leyline Control's Discord & GM card: read-only (GET /api/gm only reads this PC's files: the bugs step's backlog,
the last server check's Discord bridge lines, the addons and looks waiting for review, the last quick edits), the
token needed, no secrets in what it shows, and its buttons only existing steps on a click (never discord-apply).

    python3 -I tools/control/test_control_gm.py
"""
import http.client, importlib.util, json, os, tempfile, threading, time, unittest
from http.server import ThreadingHTTPServer
from pathlib import Path

HERE = os.path.dirname(os.path.abspath(__file__))
PORT = 18102
os.environ.update(WU_CONTROL_PORT=str(PORT), LOCALAPPDATA=tempfile.mkdtemp())
spec = importlib.util.spec_from_file_location("control", os.path.join(HERE, "control.py"))
ctl = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ctl)
T = Path(tempfile.mkdtemp())
for k, rel in {"BUGS_BACKLOG": "bugs/backlog.json", "SERVER_CHECK": "logs/server-check-latest.txt", "UPSTREAM": "mods/upstream.json",
               "LOOKS": "stream/looks.json", "QUICK_LOG": "control/quick/applied.jsonl", "LAST_CHECK": "last-check.json",
               "DISCORD_DOC": "discord.json", "DISCORD_TOKEN": "token.txt", "AUTO": "autopilot.json"}.items():
    setattr(ctl, k, T / rel)
srv = ThreadingHTTPServer(("127.0.0.1", PORT), ctl.H)
srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()


def req(method, path, headers=None, body=None):
    c = http.client.HTTPConnection("127.0.0.1", PORT, timeout=20)
    h = {"Host": f"127.0.0.1:{PORT}"}
    h.update(headers or {})
    c.request(method, path, body=body, headers=h)
    r = c.getresponse()
    return r.status, r.read()


def write(rel, text):
    p = T / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")
    return p


def files():
    return {p: p.stat().st_mtime_ns for p in T.rglob("*") if p.is_file()}


class Empty(unittest.TestCase):
    def test_nothing_on_this_pc_yet(self):
        g = ctl.gm_overview()
        self.assertFalse(g["bugs"]["have"])
        self.assertFalse(g["bridge"]["seen"])
        self.assertEqual((g["mods"]["addons"], g["mods"]["looks"]), (0, 0))
        self.assertEqual(g["quick"], [])
        self.assertFalse(g["discord"]["token"])


class Filled(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        now = time.time()
        items = [{"id": f"B-{i:08x}", "status": st, "title": f"Report {i}", "count": i + 1, "sources": ["game", "discord"],
                  "last": now - i * 60} for i, st in enumerate(["new", "fixed", "triaged", "planned", "wontfix", "new"])]
        items.append({"id": "B-merged00", "status": "new", "title": "Merged", "merged_into": "B-00000000", "last": now})
        items.append({"id": "B-secret00", "status": "new", "title": "My PASSWORD=hunter2 leaked", "last": now - 9999})
        write("bugs/backlog.json", json.dumps({"format": 1, "items": items, "seen": {}}))
        write("logs/server-check-latest.txt", "\n".join([
            "== host", "up 3 days", "== Discord bridge (wu-discord)", "  state: active; enabled: enabled",
            "  Bridge up: World chat <-> #world", "  DISCORD_TOKEN=abc.def.ghi", "", "DIAG OK"]))
        write("mods/upstream.json", json.dumps({"entries": [
            {"id": "up:a", "display_name": "Alpha", "review": {"by": "", "at": ""}},
            {"id": "up:b", "name": "Beta", "review": {"by": "Orion", "at": "x"}},
            {"id": "up:c", "name": "Gamma", "review": {"by": ""}}, {"id": "up:d", "name": "NoReview"}]}))
        write("stream/looks.json", json.dumps({"looks": [{"id": "look:h", "name": "High", "review": {"by": ""}},
                                                         {"id": "look:m", "name": "Max", "review": {"by": "Claude"}}]}))
        write("control/quick/applied.jsonl", "\n".join([
            json.dumps({"at": "2026-11-03T10:00:00Z", "id": "1311a", "deploy": "signin", "files": [["stream/x.py", "abc"]], "ok": True}),
            "not json",
            json.dumps({"at": "2026-11-03T11:00:00Z", "id": "1311c", "deploy": "none", "files": [], "ok": False, "why": "stream/.env: secret"})]))
        write("last-check.json", json.dumps({"at": "2026-11-03T09:00:00Z", "id": "check-1", "ok": True}))

    def test_open_bugs_newest_first(self):
        b = ctl.bugs_overview()
        self.assertTrue(b["have"])
        self.assertEqual(b["open"], 5, "new, triaged and planned; not fixed, wontfix or merged")
        self.assertEqual([i["id"] for i in b["items"]][:4], ["B-00000000", "B-00000002", "B-00000003", "B-00000005"])
        self.assertEqual(b["items"][0]["sources"], ["game", "discord"])
        self.assertNotIn("hunter2", json.dumps(b), "scrubbed")

    def test_bridge_from_the_last_server_check(self):
        br = ctl.bridge_overview()
        self.assertTrue(br["seen"])
        self.assertEqual((br["state"], br["enabled"]), ("active", "enabled"))
        self.assertIn("Bridge up: World chat <-> #world", br["lines"][0])
        self.assertNotIn("abc.def.ghi", json.dumps(br))

    def test_pending_review_only(self):
        m = ctl.mods_overview()
        self.assertEqual((m["addons"], m["addon_names"]), (2, ["Alpha", "Gamma"]))
        self.assertEqual((m["looks"], m["look_names"]), (1, ["High"]))

    def test_quick_edits_newest_first(self):
        q = ctl.quick_overview()
        self.assertEqual([x["id"] for x in q], ["1311c", "1311a"])
        self.assertFalse(q[0]["ok"])
        self.assertEqual(q[1]["files"], 1)

    def test_endpoint_needs_the_token_and_changes_nothing(self):
        before = files()
        self.assertEqual(req("GET", "/api/gm")[0], 403)
        st, body = req("GET", "/api/gm", {"X-Token": ctl.TOKEN})
        self.assertEqual(st, 200)
        g = json.loads(body)
        self.assertEqual(g["bugs"]["open"], 5)
        self.assertEqual(g["server"]["last_check"]["ok"], True)
        self.assertEqual(files(), before, "read-only: no file written or touched")
        self.assertNotIn(b"hunter2", body)
        self.assertNotIn(b"abc.def.ghi", body)

    def test_actions_are_existing_steps_never_owner_only(self):
        acts = [a["step"] for a in ctl.gm_overview()["actions"]]
        self.assertEqual(acts, ["check", "bugs", "discord-check", "discord-preview", "worldlog", "mods-daily"])
        self.assertTrue(all(a in ctl.STEPS for a in acts))
        self.assertNotIn("discord-apply", acts)
        # the page's buttons go through /api/do, which still refuses the owner-only step
        st, body = req("POST", "/api/do", {"X-Token": ctl.TOKEN, "Content-Type": "application/json"}, json.dumps({"run": "discord-apply"}))
        self.assertEqual(st, 400)


class Page(unittest.TestCase):
    def test_the_card_is_on_the_page(self):
        st, body = req("GET", f"/?t={ctl.TOKEN}")
        self.assertEqual(st, 200)
        page = body.decode()
        self.assertIn('<section class="card" id="gmcard"><h2>Discord &amp; GM</h2>', page)
        self.assertIn('fetch("/api/gm",{headers:H})', page)
        self.assertIn('post("/api/do",{run:t.dataset.gmrun})', page, "a click runs the step, through the same checks")
        self.assertIn("Read-only", page)

    def test_diag_reports_the_bridge(self):
        diag = (Path(HERE).parents[1] / "realm" / "diag.sh").read_text(encoding="utf-8")
        self.assertIn('echo; echo "== Discord bridge (wu-discord)"', diag)
        self.assertIn("systemctl is-active wu-discord", diag)
        self.assertLess(diag.index("== Discord bridge"), diag.index('echo "DIAG OK"'), "before DIAG OK")
        self.assertNotIn("discord.env", diag.split("== Discord bridge", 1)[1], "never prints the settings file")


if __name__ == "__main__":
    unittest.main(verbosity=2)
