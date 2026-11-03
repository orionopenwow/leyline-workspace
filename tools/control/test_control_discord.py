#!/usr/bin/env python3
"""Leyline Control's Discord panel: the bot token box (saved to token.txt only, never shown back, the stricter POST
check), the steps discord-preview / discord-apply / discord-check (read-only ones, the owner-only one, never from
Autopilot, Launch all or a Waiting on you button), the preview's summary, Do it only after a fresh preview and a typed
YES, and no token in any log or on the page. The steps really run discord_leyline.py against the fake Discord of
tools/discord-leyline/test_discord_leyline.py (each step's .bat line is read and run with Python here).

    python3 -I tools/control/test_control_discord.py
"""
import http.client, importlib.util, json, os, re, shlex, subprocess, sys, tempfile, threading, time, unittest
from http.server import ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
DL = ROOT / "tools" / "discord-leyline"
PORT = 18100
os.environ.update(WU_CONTROL_PORT=str(PORT), LOCALAPPDATA=tempfile.mkdtemp())
spec = importlib.util.spec_from_file_location("control", HERE / "control.py")
ctl = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ctl)
sys.path.insert(0, str(DL))
import test_discord_leyline as fake_dl  # noqa: E402  (the fake Discord)

TOKEN = fake_dl.TOKEN
T = Path(tempfile.mkdtemp(prefix="leyline-control-discord-"))
ctl.DISCORD_TOKEN = T / "token.txt"            # never the real tools/discord-leyline/token.txt
ctl.DISCORD_DOC = T / "discord.json"
ctl.LOGS = T / "logs"
ctl.AUTO = T / "autopilot.json"
ctl.LAST = T / "last-run.json"
ctl.DONE = T / "UPDATES-DONE.txt"
ctl.PEND = T / "UPDATES-PENDING.txt"
srv = ThreadingHTTPServer(("127.0.0.1", PORT), ctl.H)
srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()

REAL_POPEN = subprocess.Popen
FAKE = {}


def shim_popen(args, **kw):
    """cmd /c call <the step's .bat>: run its python line the way the .bat would, against the fake Discord."""
    if not (isinstance(args, list) and args[:3] == ["cmd", "/c", "call"]):
        return REAL_POPEN(args, **kw)
    bat = Path(args[3])
    line = next(l for l in bat.read_text(encoding="utf-8").splitlines() if l.startswith("%PY% discord_leyline.py"))
    argv = shlex.split(line.split("discord_leyline.py", 1)[1])
    env = {k: v for k, v in kw.pop("env").items() if "proxy" not in k.lower()}
    env.update(LEYLINE_DISCORD_API=f"http://127.0.0.1:{FAKE['srv'].server_address[1]}/api",
               LEYLINE_BACKUP_ROOT=str(FAKE["backups"]), LEYLINE_DISCORD_BACKOFF="0.01", NO_PROXY="*")
    kw.pop("creationflags", None)
    kw["cwd"] = str(bat.parent)
    return REAL_POPEN([sys.executable, str(DL / "discord_leyline.py"), *argv, "--token-file", str(ctl.DISCORD_TOKEN)],
                      env=env, **kw)


ctl.subprocess.Popen = shim_popen


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


def state():
    st, b = req("GET", "/api/state", {"X-Token": ctl.TOKEN})
    assert st == 200, b
    return b, json.loads(b)


def wait_idle(timeout=90):
    end = time.time() + timeout
    time.sleep(0.2)
    while ctl.state["running"] and time.time() < end:
        time.sleep(0.1)
    assert not ctl.state["running"], "step still running"


class Base(unittest.TestCase):
    def setUp(self):
        ctl.remove_discord_token()
        try:
            ctl.DISCORD_DOC.unlink()
        except FileNotFoundError:
            pass
        self.tmp = Path(tempfile.mkdtemp(prefix="leyline-cd-"))
        FAKE["backups"] = self.tmp / "Desktop"
        FAKE["backups"].mkdir()
        self.fake = fake_dl.Fake(fake_dl.base_world(), FAKE["backups"])
        FAKE["srv"] = fake_dl.serve(self.fake)

    def tearDown(self):
        FAKE["srv"].shutdown()
        FAKE["srv"].server_close()

    def run_step(self, step):
        st, b = post("/api/do", {"run": step})
        self.assertEqual(st, 200, b)
        wait_idle()

    def all_text(self):
        """Every log, Control's own files and the page's state: where a token must never be."""
        out = [state()[0], "\n".join(ctl.state["lines"]), req("GET", f"/?t={ctl.TOKEN}")[1]]
        for f in list(ctl.LOGS.glob("*")) + [ctl.DISCORD_DOC, ctl.LAST] + list(FAKE["backups"].rglob("*")):
            if f.is_file():
                out.append(f.read_text(encoding="utf-8", errors="replace"))
        return out


class TokenBox(Base):
    def test_save_status_remove(self):
        _, s = state()
        self.assertEqual(s["discord"]["token"], {"saved": False, "last4": ""})
        st, b = post("/api/discord/token", {"token": f'  "Bot {TOKEN}"\n'})
        self.assertEqual(st, 200, b)
        self.assertNotIn(TOKEN, b)
        self.assertEqual(ctl.DISCORD_TOKEN.read_text().strip(), TOKEN)      # tidied: no quotes, no "Bot "
        raw, s = state()
        self.assertEqual(s["discord"]["token"], {"saved": True, "last4": TOKEN[-4:]})
        self.assertNotIn(TOKEN, raw)
        self.assertNotIn(TOKEN[:-4], raw)
        page = req("GET", f"/?t={ctl.TOKEN}")[1]
        self.assertIn('id="dtokin"', page)
        self.assertIn('type="password"', page)
        self.assertNotIn(TOKEN, page)
        self.assertEqual(post("/api/discord/token-remove")[0], 200)
        self.assertFalse(ctl.DISCORD_TOKEN.exists())
        self.assertFalse(state()[1]["discord"]["token"]["saved"])

    def test_bad_tokens_refused(self):
        for bad in ("", "short", "has spaces in it but is quite long enough", "x" * 300, "<script>alert(1)</script>xxxxxxxxxxxxxxxxx"):
            st, b = post("/api/discord/token", {"token": bad})
            self.assertEqual(st, 400, bad)
        self.assertFalse(ctl.DISCORD_TOKEN.exists())

    def test_post_protection(self):
        body = json.dumps({"token": TOKEN})
        js = {"Content-Type": "application/json"}
        cases = [
            {**js},                                                              # no session token
            {**js, "X-Token": "x" * 30},
            {**js, "Cookie": f"lc_token={ctl.TOKEN}"},                           # the cookie alone isn't enough here
            {**js, "X-Token": ctl.TOKEN, "Host": "evil.example"},
            {**js, "X-Token": ctl.TOKEN, "Origin": "http://evil.example"},
            {**js, "X-Token": ctl.TOKEN, "Sec-Fetch-Site": "cross-site"},
            {"X-Token": ctl.TOKEN, "Content-Type": "text/plain"},              # a form can't send JSON
        ]
        for h in cases:
            st, _ = req("POST", "/api/discord/token", h, body)
            self.assertIn(st, (403, 415), h)
        st, _ = req("POST", f"/api/discord/token?t={ctl.TOKEN}", js, body)    # nor the link's token
        self.assertEqual(st, 403)
        self.assertEqual(req("POST", "/api/discord/token", {**js, "X-Token": ctl.TOKEN}, json.dumps({"token": "a" * 5000}))[0], 413)
        self.assertFalse(ctl.DISCORD_TOKEN.exists())
        self.assertEqual(post("/api/discord/token", {"token": TOKEN})[0], 200)
        self.assertEqual(req("POST", "/api/discord/token-remove", {"Cookie": f"lc_token={ctl.TOKEN}", **js}, "{}")[0], 403)
        self.assertTrue(ctl.DISCORD_TOKEN.exists())

    def test_scrub_hides_the_saved_token(self):
        post("/api/discord/token", {"token": TOKEN})
        self.assertEqual(ctl.scrub(f"x {TOKEN} y"), "x [hidden] y")
        real_shape = "MTk4NjIyNDgzNDcxOTI1MjQ4Mz." + "GaBcDe." + "a" * 38
        self.assertNotIn(real_shape, ctl.scrub("token " + real_shape))


class Steps(Base):
    def test_step_table(self):
        for s in ("discord-preview", "discord-apply", "discord-check"):
            bat, _title, interactive = ctl.STEPS[s]
            self.assertTrue((ROOT / bat).is_file(), bat)
            self.assertFalse(interactive)
            text = (ROOT / bat).read_text(encoding="utf-8")
            self.assertNotIn("pause", text.lower())
            self.assertNotIn("set /p", text.lower())
        self.assertIn("discord-preview", ctl.READ_ONLY)
        self.assertIn("discord-check", ctl.READ_ONLY)
        self.assertNotIn("discord-apply", ctl.READ_ONLY)
        self.assertIn("discord-apply", ctl.OWNER_ONLY)
        self.assertIn("--yes-from-control", (ROOT / ctl.STEPS["discord-apply"][0]).read_text())
        self.assertIn("--from-control", (ROOT / ctl.STEPS["discord-preview"][0]).read_text())

    def test_apply_never_from_a_queue_or_button(self):
        st, b = post("/api/do", {"run": "discord-apply"})
        self.assertEqual(st, 400, b)
        post("/api/discord/token", {"token": TOKEN})
        item = {"id": "9001", "step": "discord-apply", "title": "x", "note": "", "known": True}
        ctl.run_all("", [item], record=False)           # what Autopilot or Launch all would do with a queued one
        self.assertTrue(any("runs only from Leyline Control's Discord panel" in l for l in ctl.state["lines"]))
        self.assertEqual(self.fake.writes(), [])
        self.assertFalse((ctl.LOGS / "9001-discord-apply.log").exists())

    def test_preview_then_yes_then_apply_and_no_token_anywhere(self):
        # no token yet: Do it refused; preview stops by itself without asking
        self.assertEqual(post("/api/discord/apply", {"confirm": "YES"})[0], 409)
        self.run_step("discord-preview")
        _, s = state()
        self.assertFalse(s["discord"]["preview"]["ok"])
        self.assertIn("Control > Discord", "\n".join(ctl.state["lines"]))
        self.assertEqual(self.fake.log, [])

        post("/api/discord/token", {"token": TOKEN})
        self.run_step("discord-preview")
        _, s = state()
        p = s["discord"]["preview"]
        self.assertTrue(p["ok"], p)
        self.assertEqual(p["server"], "OpenVanilla Wrath")
        self.assertGreater(p["changes"], 0)
        self.assertEqual(sum(1 for i in p["items"] if i["change"]), p["changes"])
        self.assertTrue(p["log"].endswith("-discord-preview.log"))
        self.assertTrue(s["discord"]["ready"])
        self.assertEqual(self.fake.writes(), [])                                # the preview wrote nothing
        st, log = req("GET", f"/api/log?t={ctl.TOKEN}&name={p['log']}")
        self.assertEqual(st, 200)
        self.assertIn("PREVIEW (nothing will change)", log)

        # Do it: needs the typed YES and this very preview
        self.assertEqual(post("/api/discord/apply", {"preview": p["id"], "confirm": "yes please"})[0], 400)
        self.assertEqual(post("/api/discord/apply", {"preview": "old", "confirm": "YES"})[0], 409)
        self.assertEqual(self.fake.writes(), [])
        st, b = post("/api/discord/apply", {"preview": p["id"], "confirm": "YES"})
        self.assertEqual(st, 200, b)
        wait_idle()
        _, s = state()
        a = s["discord"]["apply"]
        self.assertTrue(a["ok"], "\n".join(ctl.state["lines"][-30:]))
        self.assertEqual(a["changes"], p["changes"])
        self.assertTrue(self.fake.writes())
        self.assertTrue(all(w[4] for w in self.fake.writes()), "a write before the backup")
        self.assertTrue(s["discord"]["preview"]["used"])
        self.assertFalse(s["discord"]["ready"])                                 # the next one needs a new preview
        self.assertEqual(post("/api/discord/apply", {"preview": p["id"], "confirm": "YES"})[0], 409)

        self.run_step("discord-check")
        _, s = state()
        self.assertTrue(s["discord"]["check"]["ok"])
        self.assertTrue(any("WU_DISCORD_GUILD=1" in l for l in s["discord"]["check"]["lines"]))
        self.run_step("discord-preview")
        self.assertEqual(state()[1]["discord"]["preview"]["changes"], 0)
        self.assertFalse(state()[1]["discord"]["ready"])

        logs = list(ctl.LOGS.glob("*-discord-*.log"))
        self.assertGreaterEqual(len(logs), 4)
        for text in self.all_text():
            self.assertNotIn(TOKEN, text)
            for part in TOKEN.split("."):
                self.assertNotIn(part, text)

    def test_stale_preview_refused(self):
        post("/api/discord/token", {"token": TOKEN})
        self.run_step("discord-preview")
        d = json.loads(ctl.DISCORD_DOC.read_text())
        d["preview"]["at"] -= ctl.DISCORD_FRESH + 5
        ctl.DISCORD_DOC.write_text(json.dumps(d))
        st, b = post("/api/discord/apply", {"preview": d["preview"]["id"], "confirm": "YES"})
        self.assertEqual(st, 409)
        self.assertIn("over an hour old", b)
        self.assertEqual(self.fake.writes(), [])

    def test_server_changed_since_the_preview(self):
        post("/api/discord/token", {"token": TOKEN})
        self.run_step("discord-preview")
        p = state()[1]["discord"]["preview"]
        self.fake.w["channels"].append({"id": "77", "name": "brand-new", "type": 0, "parent_id": None, "position": 9,
                                        "guild_id": "1", "permission_overwrites": [], "topic": None,
                                        "rate_limit_per_user": 0})
        self.assertEqual(post("/api/discord/apply", {"preview": p["id"], "confirm": "YES"})[0], 200)
        wait_idle()
        self.assertFalse(state()[1]["discord"]["apply"]["ok"])
        self.assertTrue(any("Run the preview again" in l for l in ctl.state["lines"]))
        self.assertEqual(self.fake.writes(), [])

    def test_the_cards_choices(self):
        """Old channels and Post and pin: chosen in the card, used by the preview, and Do it does that preview's."""
        post("/api/discord/token", {"token": TOKEN})
        self.assertEqual(state()[1]["discord"]["options"], {"old": "archive", "pins": False})
        for bad in ({"old": "nuke", "pins": False}, {"old": "all", "pins": "yes"}, {}):
            self.assertEqual(post("/api/discord/options", bad)[0], 400, bad)
        self.assertEqual(post("/api/discord/options", {"old": "empty", "pins": True}, {"X-Token": "nope"})[0], 403)
        st, b = post("/api/discord/options", {"old": "empty", "pins": True})
        self.assertEqual(st, 200, b)
        self.run_step("discord-preview")
        s = state()[1]["discord"]
        p = s["preview"]
        self.assertEqual(p["options"], {"old": "empty", "pins": True})
        self.assertTrue(s["ready"], s)
        self.assertTrue(any(i["section"] == "Pinned posts" and i["change"] for i in p["items"]), p["items"])
        self.assertTrue(any(i["section"] == "DELETED" and "no messages" in i["text"] for i in p["items"]), p["items"])
        log = (ctl.LOGS / p["log"]).read_text(encoding="utf-8")
        self.assertIn("Choices: old channels: delete the empty ones, archive the rest", log)
        # changed since the preview: Do it waits for a new one
        post("/api/discord/options", {"old": "all", "pins": True})
        s = state()[1]["discord"]
        self.assertFalse(s["ready"])
        self.assertIn("changed the choices", s["why"])
        self.assertEqual(post("/api/discord/apply", {"preview": p["id"], "confirm": "YES"})[0], 409)
        post("/api/discord/options", {"old": "empty", "pins": True})
        self.assertTrue(state()[1]["discord"]["ready"])
        self.assertEqual(post("/api/discord/apply", {"preview": p["id"], "confirm": "YES"})[0], 200)
        wait_idle()
        a = state()[1]["discord"]["apply"]
        self.assertTrue(a["ok"], "\n".join(ctl.state["lines"][-30:]))
        self.assertIsNone(self.fake.chan("23"), "the empty old channel was deleted")
        self.assertIsNotNone(self.fake.chan("22"), "the one with messages was not")
        self.assertTrue(self.fake.w.get("pins"), "the guide posts were pinned")
        self.run_step("discord-preview")
        self.assertEqual(state()[1]["discord"]["preview"]["changes"], 0)

    def test_the_card_has_the_choices(self):
        page = req("GET", f"/?t={ctl.TOKEN}")[1]
        panel = page[page.index('id="discord"'):page.index('id="gmcard"')]
        for s in ('id="dold"', 'value="archive"', 'value="empty"', 'value="all"', 'id="dpins"', "Post and pin",
                  "even with messages", "13 channels"):
            self.assertIn(s, panel)

    def test_summary_parser(self):
        log = T / "x.log"
        log.write_text("Signed in as the bot b.\nPREVIEW (nothing will change) for the server \"Orion's\":\n\n"
                       "Roles:\n  * make role Guide\n    keep Admin\nArchive:\n  * archive #old\n\n2 change(s).\n",
                       encoding="utf-8")
        s = ctl.discord_summary(log)
        self.assertEqual((s["server"], s["changes"]), ("Orion's", 2))
        self.assertEqual([i["section"] for i in s["items"]], ["Roles", "Roles", "Archive"])
        self.assertEqual([i["change"] for i in s["items"]], [True, False, True])

    def test_no_banned_words_on_the_page(self):
        page = req("GET", f"/?t={ctl.TOKEN}")[1]
        panel = page[page.index('id="discord"'):page.index("Claude's notes")]
        titles = " ".join(ctl.STEPS[s][1] for s in ("discord-preview", "discord-apply", "discord-check"))
        for bad in ("World of Warcraft", "WoW", "Warcraft", "Lich King", "Blizzard"):
            self.assertNotIn(bad, panel + titles)


if __name__ == "__main__":
    unittest.main(verbosity=2)
