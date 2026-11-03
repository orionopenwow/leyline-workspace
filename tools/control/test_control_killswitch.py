#!/usr/bin/env python3
"""Leyline Control's kill switch card (docs/KILL-SWITCH.md): owner only and never a step (not in STEPS, so never the
queue, Autopilot, Launch all or /api/do), the stricter POST checks, the message and choices saved, each stage only
after its own fresh preview with the same message and choices and its typed phrase, Pause, Undo, Delete after Pause
and the wait with the backup first, the dry run, and no bot token in any log, file or page. Every service is a fake
(tools/killswitch/fakes.py).

    python3 -I tools/control/test_control_killswitch.py
"""
import http.client, importlib.util, json, os, sys, tempfile, threading, time, unittest
from http.server import ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
PORT = 18102
os.environ.update(WU_CONTROL_PORT=str(PORT), LOCALAPPDATA=tempfile.mkdtemp())
spec = importlib.util.spec_from_file_location("control", HERE / "control.py")
ctl = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ctl)
sys.path.insert(0, str(ROOT / "tools" / "killswitch"))
import killswitch as ks  # noqa: E402
import fakes  # noqa: E402

TOKEN = "MTk4NjIyNDgzNDcxOTI1MjQ4Mz.GaBcDe." + "q" * 38          # a bot token's shape; never real
GOOD = "Thank you for every adventure.\nThe realms close tonight."
T = Path(tempfile.mkdtemp(prefix="leyline-control-ks-"))
ctl.DISCORD_TOKEN = T / "token.txt"
ctl.DISCORD_DOC = T / "discord.json"
ctl.KS_DOC = T / "killswitch.json"
ctl.LOGS = T / "logs"
ctl.AUTO = T / "autopilot.json"
ctl.LAST = T / "last-run.json"
ctl.DONE = T / "UPDATES-DONE.txt"
ctl.PEND = T / "UPDATES-PENDING.txt"
srv = ThreadingHTTPServer(("127.0.0.1", PORT), ctl.H)
srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()
W = {}


def req(method, path, headers=None, body=None):
    c = http.client.HTTPConnection("127.0.0.1", PORT, timeout=30)
    h = {"Host": f"127.0.0.1:{PORT}"}
    h.update(headers or {})
    c.request(method, path, body=body, headers=h)
    r = c.getresponse()
    return r.status, r.read().decode("utf-8", "replace")


def post(path, obj=None, extra=None):
    h = {"X-Token": ctl.TOKEN, "Content-Type": "application/json", "Origin": f"http://127.0.0.1:{PORT}"}
    h.update(extra or {})
    return req("POST", path, h, json.dumps(obj or {}))


def kstate():
    st, b = req("GET", "/api/state", {"X-Token": ctl.TOKEN})
    assert st == 200, b
    return json.loads(b)["killswitch"]


def wait_idle(timeout=120):
    end = time.time() + timeout
    time.sleep(0.2)
    while ctl.state["running"] and time.time() < end:
        time.sleep(0.1)
    assert not ctl.state["running"], "still running"


class Base(unittest.TestCase):
    def setUp(self):
        for f in (ctl.KS_DOC, ctl.DISCORD_TOKEN):
            try:
                f.unlink()
            except FileNotFoundError:
                pass
        root, gh, dc, ssrv, steps, desk = fakes.world()
        W.update(root=root, gh=gh, dc=dc, srv=ssrv, steps=steps, desk=desk)
        ctl.KS_FACTORY = lambda: {"github": W["gh"], "discord": W["dc"] if ctl.discord_secret() else None,
                                  "server": W["srv"], "run_step": W["steps"]}
        ctl.KS_BACKUP_ROOT = desk
        ctl.save_discord_token(TOKEN)

    def save(self, message=GOOD, parts=ks.PARTS, dry=False, private=False, title="Leyline is closing"):
        st, b = post("/api/killswitch/save", {"title": title, "message": message, "parts": list(parts),
                                              "options": {"private": private, "dry": dry}})
        self.assertEqual(st, 200, b)

    def preview(self, stage):
        st, b = post("/api/killswitch/preview", {"stage": stage})
        self.assertEqual(st, 200, b)
        wait_idle()
        return kstate()

    def go(self, stage, phrase=None, pid=None):
        k = kstate()
        st, b = post("/api/killswitch/go", {"stage": stage, "preview": pid or (k.get("preview") or {}).get("id"),
                                            "phrase": ks.PHRASES[stage] if phrase is None else phrase})
        if st == 200:
            wait_idle()
        return st, b

    def nothing_changed(self, snap):
        self.assertEqual(W["srv"].snapshot(), snap)
        self.assertEqual(W["gh"].writes(), [])
        self.assertEqual(W["dc"].ops, [])
        self.assertEqual(W["steps"].ran, [])


class NotAStep(Base):
    def test_never_a_step(self):
        self.assertFalse([s for s in ctl.STEPS if "kill" in s])
        snap = W["srv"].snapshot()
        for step in ("killswitch", "killswitch-pause", "killswitch-delete"):
            st, b = post("/api/do", {"run": step})
            self.assertEqual(st, 400, b)
        ctl.PEND.write_text("9001 killswitch-pause shut it all\n9002 killswitch-delete\n", encoding="utf-8")
        ctl.run_all("", None, record=False)            # what Launch all or Autopilot would do with queued lines
        self.assertTrue(any("Unknown step 'killswitch-pause'" in l for l in ctl.state["lines"]))
        self.nothing_changed(snap)
        ctl.PEND.unlink()

    def test_post_protection(self):
        body = json.dumps({"stage": "pause"})
        js = {"Content-Type": "application/json"}
        for h in ({**js}, {**js, "Cookie": f"lc_token={ctl.TOKEN}"}, {**js, "X-Token": ctl.TOKEN, "Origin": "http://evil.example"},
                  {**js, "X-Token": ctl.TOKEN, "Sec-Fetch-Site": "cross-site"}, {"X-Token": ctl.TOKEN, "Content-Type": "text/plain"},
                  {**js, "X-Token": ctl.TOKEN, "Host": "evil.example"}):
            for path in ("/api/killswitch/preview", "/api/killswitch/go", "/api/killswitch/save"):
                st, _ = req("POST", path, h, body)
                self.assertIn(st, (403, 415), (path, h))
        st, _ = req("POST", f"/api/killswitch/go?t={ctl.TOKEN}", js, body)
        self.assertEqual(st, 403)
        self.assertEqual(post("/api/killswitch/save", {"message": "x" * 20000})[0], 413)
        self.assertFalse(ctl.KS_DOC.exists())

    def test_save_checks(self):
        for bad in ({"title": "t", "message": "m", "parts": ["nuke"], "options": {}},
                    {"title": "t" * 81, "message": "m", "parts": [], "options": {}},
                    {"title": "t", "message": "m" * 1801, "parts": [], "options": {}},
                    {"title": "t", "message": "m", "parts": [], "options": {"dry": "yes"}},
                    {"title": "t", "message": 5, "parts": [], "options": {}}):
            self.assertEqual(post("/api/killswitch/save", bad)[0], 400, bad)
        self.assertEqual(post("/api/killswitch/preview", {"stage": "nuke"})[0], 400)
        k = kstate()
        self.assertIn("TODO(owner)", k["config"]["message"])                  # the default text
        self.assertEqual(k["config"]["parts"], list(ks.PARTS))


class Flow(Base):
    def test_todo_message_blocks_pause(self):
        snap = W["srv"].snapshot()
        k = self.preview("pause")
        self.assertTrue(any("TODO(owner)" in x for x in k["preview"]["problems"]))
        self.assertFalse(k["ready"]["pause"])
        self.assertEqual(self.go("pause")[0], 409)
        self.nothing_changed(snap)

    def test_pause_then_undo(self):
        self.save(private=True)
        snap = W["srv"].snapshot()
        self.assertEqual(self.go("pause")[0], 409)                              # no preview yet
        k = self.preview("pause")
        self.assertEqual(k["preview"]["problems"], [], k["preview"])
        self.assertTrue(k["ready"]["pause"])
        self.nothing_changed(snap)                                              # the preview only read
        self.assertEqual(self.go("pause", phrase="shut down leyline")[0], 400)
        self.assertEqual(self.go("pause", phrase="YES")[0], 400)
        self.assertEqual(self.go("pause", pid="old")[0], 409)
        self.nothing_changed(snap)
        # a changed message needs a new preview
        self.save(message=GOOD + " Bye.", private=True)
        self.assertFalse(kstate()["ready"]["pause"])
        self.assertIn("changed", kstate()["why"]["pause"])
        self.save(private=True)
        st, b = self.go("pause")
        self.assertEqual(st, 200, b)
        k = kstate()
        self.assertTrue(k["last"]["ok"], k["last"])
        self.assertEqual(k["paused"]["parts"], list(ks.PARTS))
        self.assertEqual(json.loads((W["srv"].data / "leyline/shutdown.json").read_text())["mode"], "closed")
        self.assertEqual(W["steps"].ran, list(ks.REALM_STEPS))
        self.assertTrue(all(r["archived"] and r["private"] for r in W["gh"].repos.values()))
        self.assertEqual(self.go("pause")[0], 409)                              # one preview, one run
        self.assertTrue(k["last"]["log"].startswith("killswitch-"))
        self.assertTrue((ctl.LOGS / k["last"]["log"]).exists())
        # Undo
        self.assertTrue(kstate()["ready"]["delete"] is False)
        k = self.preview("resume")
        self.assertTrue(k["ready"]["resume"], k)
        self.assertEqual(self.go("resume", phrase=ks.PAUSE_PHRASE)[0], 400)
        self.assertEqual(self.go("resume")[0], 200)
        k = kstate()
        self.assertTrue(k["last"]["ok"], k["last"])
        self.assertNotIn("paused", k)
        self.assertNotIn("leyline/shutdown.json", W["srv"].snapshot())
        self.assertFalse(any(r["archived"] or r["private"] for r in W["gh"].repos.values()))
        self.assertEqual(post("/api/killswitch/preview", {"stage": "resume"})[0], 409)

    def test_delete_after_pause_and_the_wait(self):
        self.save()
        k = self.preview("delete")
        self.assertFalse(k["ready"]["delete"])
        self.preview("pause")
        self.assertEqual(self.go("pause")[0], 200)
        k = self.preview("delete")
        self.assertTrue(any("hours after the Pause" in x for x in k["preview"]["problems"]))
        self.assertTrue(any("delete_repo" in x for x in k["preview"]["problems"]))
        d = json.loads(ctl.KS_DOC.read_text())
        d["paused"]["at"] -= ks.DELETE_WAIT + 60
        ctl.KS_DOC.write_text(json.dumps(d))
        W["gh"]._scopes.add("delete_repo")
        k = self.preview("delete")
        self.assertEqual(k["preview"]["problems"], [])
        self.assertTrue(k["ready"]["delete"])
        self.assertEqual(self.go("delete", phrase=ks.PAUSE_PHRASE)[0], 400)
        self.assertEqual(W["gh"].writes(), [e for e in W["gh"].writes() if e[0] == "edit"])   # nothing deleted yet
        # stale preview refused
        d = json.loads(ctl.KS_DOC.read_text())
        d["preview"]["at"] -= ks.PREVIEW_FRESH + 5
        ctl.KS_DOC.write_text(json.dumps(d))
        st, b = self.go("delete")
        self.assertEqual(st, 409)
        self.assertIn("over an hour old", b)
        self.preview("delete")
        self.assertEqual(self.go("delete")[0], 200)
        k = kstate()
        self.assertTrue(k["last"]["ok"], k["last"])
        self.assertTrue(Path(k["last"]["backup"], "MANIFEST.json").exists())
        self.assertEqual(W["gh"].repos, {})
        self.assertTrue(all(o[2] for o in W["gh"].ops if o[0] == "delete"))
        self.assertTrue(any("Delete Server" in m for m in k["last"]["manual"]))
        self.assertTrue(any("DNS" in m for m in k["last"]["manual"]))
        self.assertTrue(k["paused"]["deleted"])
        self.assertFalse(k["ready"]["resume"])
        self.assertEqual(post("/api/killswitch/go", {"stage": "resume", "preview": "x", "phrase": ks.RESUME_PHRASE})[0], 409)

    def test_wrong_gh_account(self):
        W["gh"]._login = "Chumpton"
        self.save(parts=["github"])
        snap = W["srv"].snapshot()
        k = self.preview("pause")
        self.assertTrue(any("Chumpton" in x for x in k["preview"]["problems"]))
        self.assertEqual(self.go("pause")[0], 409)
        self.nothing_changed(snap)

    def test_dry_run(self):
        self.save(dry=True)
        snap = W["srv"].snapshot()
        self.preview("pause")
        self.assertEqual(self.go("pause")[0], 200)
        k = kstate()
        self.assertTrue(k["last"]["dry"])
        self.assertNotIn("paused", k)
        self.nothing_changed(snap)
        self.assertIn("(dry run) would", (ctl.LOGS / k["last"]["log"]).read_text())

    def test_token_never_anywhere(self):
        self.save(parts=["message", "discord"])
        W["dc"].guild_doc["name"] = "Leyline " + TOKEN          # even if Discord echoed it back
        self.preview("pause")
        self.go("pause")
        texts = [json.dumps(kstate()), "\n".join(ctl.state["lines"]), ctl.KS_DOC.read_text(),
                 req("GET", f"/?t={ctl.TOKEN}")[1]]
        texts += [f.read_text(encoding="utf-8", errors="replace") for f in ctl.LOGS.glob("*") if f.is_file()]
        self.assertTrue(any("[hidden]" in t for t in texts))
        for t in texts:
            self.assertNotIn(TOKEN, t)
            for part in TOKEN.split("."):
                self.assertNotIn(part, t)


class Card(Base):
    def test_the_card(self):
        page = req("GET", f"/?t={ctl.TOKEN}")[1]
        card = page[page.index('id="killswitch"'):page.index("</details></section>", page.index('id="killswitch"'))]
        for s in ('id="ksmsg"', 'id="kstitle"', 'id="ksparts"', 'id="ksprivate"', 'id="ksdry"', 'id="ksprev-pause"',
                  'id="ksprev-delete"', 'id="ksprev-resume"', 'id="ksphrase"', "never the queue or Autopilot", "Desktop"):
            self.assertIn(s, card)
        for bad in ("World of Warcraft", "WoW", "Warcraft", "Lich King", "Blizzard"):
            self.assertNotIn(bad, card)
        k = kstate()
        self.assertEqual(k["phrases"], ks.PHRASES)
        self.assertEqual([p["key"] for p in k["parts"]], list(ks.PARTS))


if __name__ == "__main__":
    unittest.main(verbosity=1)
