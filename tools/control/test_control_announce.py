#!/usr/bin/env python3
"""Leyline Control's Pending announcements (the Discord card): drafts listed exactly as they would post, Check and
Reject without the network, Post only after a typed YES for the very draft the owner read (its sha256), the owner-only
discord-announce step never from the queue, Autopilot or a Waiting on you button, the read-only announce-check step,
and no bot token in any log, file or page. The steps really run announce.py (each .bat's python line, run here)
against a small fake Discord.

    python3 -I tools/control/test_control_announce.py
"""
import http.client, importlib.util, json, os, shlex, subprocess, sys, tempfile, threading, time, unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
DL = ROOT / "tools" / "discord-leyline"
PORT = 18103
os.environ.update(WU_CONTROL_PORT=str(PORT), LOCALAPPDATA=tempfile.mkdtemp())
spec = importlib.util.spec_from_file_location("control", HERE / "control.py")
ctl = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ctl)

TOKEN = "MTAxNTk5ODg3.Gx1abc.secret-token-part-zzzzzzzzzzzzzzzz"
T = Path(tempfile.mkdtemp(prefix="leyline-control-announce-"))
ctl.DISCORD_TOKEN = T / "token.txt"            # never the real tools/discord-leyline/token.txt
ctl.DISCORD_DOC = T / "discord.json"
ctl.ANNOUNCE_DIR = T / "announce"              # never the real drafts
ctl.LOGS = T / "logs"
ctl.AUTO = T / "autopilot.json"
ctl.LAST = T / "last-run.json"
ctl.DONE = T / "UPDATES-DONE.txt"
ctl.PEND = T / "UPDATES-PENDING.txt"
ctl.hq_sync_quiet = lambda: None
srv = ThreadingHTTPServer(("127.0.0.1", PORT), ctl.H)
srv.daemon_threads = True
threading.Thread(target=srv.serve_forever, daemon=True).start()


class FakeDiscord:
    """One server with a news #announcements; records every request and the Authorization header it got."""
    def __init__(self):
        self.log, self.auth = [], set()

    def handle(self, method, path, body, headers):
        self.log.append((method, path, body))
        self.auth.add(headers.get("Authorization", ""))
        if path == "/users/@me/guilds":
            return 200, [{"id": "1", "name": "Leyline"}]
        if path == "/guilds/1/channels":
            return 200, [{"id": "8", "type": 0, "name": "general"}, {"id": "9", "type": 5, "name": "📣︱announcements"}]
        if method == "POST" and path == "/channels/9/messages":
            return 200, {"id": "777", "content": body["content"]}
        if method == "POST" and path == "/channels/9/messages/777/crosspost":
            return 200, {"id": "777"}
        return 404, {"message": "Unknown", "code": 0}

    def writes(self):
        return [w for w in self.log if w[0] != "GET"]


def serve(fake):
    class Hd(BaseHTTPRequestHandler):
        def log_message(self, *a):
            pass

        def go(self):
            n = int(self.headers.get("Content-Length") or 0)
            body = json.loads(self.rfile.read(n)) if n else None
            st, out = fake.handle(self.command, self.path.split("?")[0][len("/api"):], body, self.headers)
            raw = json.dumps(out).encode()
            self.send_response(st)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(raw)))
            self.end_headers()
            self.wfile.write(raw)
        do_GET = do_POST = go
    s = ThreadingHTTPServer(("127.0.0.1", 0), Hd)
    threading.Thread(target=s.serve_forever, daemon=True).start()
    return s


REAL_POPEN = subprocess.Popen
FAKE = {}


def shim_popen(args, **kw):
    """cmd /c call <the step's .bat>: run its python line the way the .bat would, against the fake Discord."""
    if not (isinstance(args, list) and args[:3] == ["cmd", "/c", "call"]):
        return REAL_POPEN(args, **kw)
    bat = Path(args[3])
    line = next(l for l in bat.read_text(encoding="utf-8").splitlines() if l.startswith("%PY% announce.py"))
    argv = shlex.split(line.split("announce.py", 1)[1])
    env = {k: v for k, v in kw.pop("env").items() if "proxy" not in k.lower()}
    env.update(LEYLINE_DISCORD_API=f"http://127.0.0.1:{FAKE['srv'].server_address[1]}/api",
               LEYLINE_DISCORD_BACKOFF="0.01", NO_PROXY="*")
    kw.pop("creationflags", None)
    kw["cwd"] = str(bat.parent)
    return REAL_POPEN([sys.executable, str(DL / "announce.py"), *argv, "--dir", str(ctl.ANNOUNCE_DIR),
                       "--token-file", str(ctl.DISCORD_TOKEN)], env=env, **kw)


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


def wait_idle(timeout=60):
    end = time.time() + timeout
    time.sleep(0.2)
    while ctl.state["running"] and time.time() < end:
        time.sleep(0.1)
    assert not ctl.state["running"], "step still running"


def draft(kind="milestone", ping="everyone", body="Boss fights are here. https://leylinelauncher.com"):
    return f"---\ntitle: Big news\nkind: {kind}\nping: {ping}\ncreated: 2026-11-05\n---\n{body}\n"


class Base(unittest.TestCase):
    def setUp(self):
        ctl.remove_discord_token()
        for f in (ctl.DISCORD_DOC,):
            try:
                f.unlink()
            except FileNotFoundError:
                pass
        import shutil
        shutil.rmtree(ctl.ANNOUNCE_DIR, ignore_errors=True)
        (ctl.ANNOUNCE_DIR / "pending").mkdir(parents=True)
        self.fake = FakeDiscord()
        FAKE["srv"] = serve(self.fake)

    def tearDown(self):
        FAKE["srv"].shutdown()
        FAKE["srv"].server_close()

    def write(self, i, text):
        (ctl.ANNOUNCE_DIR / "pending" / f"{i}.md").write_text(text, encoding="utf-8")

    def drafts(self):
        return {d["id"]: d for d in state()[1]["discord"]["announce"]["drafts"]}

    def all_text(self):
        out = [state()[0], "\n".join(ctl.state["lines"]), req("GET", f"/?t={ctl.TOKEN}")[1]]
        for f in list(ctl.LOGS.glob("*")) + [ctl.DISCORD_DOC, ctl.LAST] + list(ctl.ANNOUNCE_DIR.rglob("*")):
            if f.is_file():
                out.append(f.read_text(encoding="utf-8", errors="replace"))
        return out


class Steps(Base):
    def test_step_table(self):
        for s, arg in (("discord-announce", "post --yes-from-control"), ("announce-check", "check --all")):
            bat, title, interactive = ctl.STEPS[s]
            raw = (ROOT / bat).read_bytes()
            self.assertFalse(interactive)
            self.assertIn(b"\r\n", raw, "a .bat keeps CRLF")
            text = raw.decode("utf-8")
            self.assertIn(f"%PY% announce.py {arg}", text)
            self.assertNotIn("pause", text.lower())
            self.assertNotIn("set /p", text.lower())
        self.assertIn("announce-check", ctl.READ_ONLY)
        self.assertNotIn("announce-check", ctl.OWNER_ONLY)
        self.assertIn("discord-announce", ctl.OWNER_ONLY)
        self.assertNotIn("discord-announce", ctl.READ_ONLY)
        self.assertIn("discord-apply", ctl.OWNER_ONLY)
        items = [{"id": "1", "step": "announce-check", "known": True}, {"id": "2", "step": "discord-announce", "known": True}]
        self.assertEqual([i["step"] for i in ctl.autopilot_safe(items)], ["announce-check"])

    def test_post_never_from_a_queue_or_button(self):
        ctl.save_discord_token(TOKEN)
        self.write("a", draft())
        st, b = post("/api/do", {"run": "discord-announce"})
        self.assertEqual(st, 400, b)
        item = {"id": "9002", "step": "discord-announce", "title": "x", "note": "", "known": True,
                "announce_id": "a", "announce_sha": self.drafts()["a"]["sha"]}
        ctl.run_all("", [item], record=False)          # what Autopilot or Launch all would do with a queued line
        self.assertTrue(any("runs only from Leyline Control's Discord panel" in l and "the draft" in l
                            for l in ctl.state["lines"]))
        self.assertEqual(self.fake.log, [])
        self.assertFalse((ctl.LOGS / "9002-discord-announce.log").exists())
        self.assertTrue((ctl.ANNOUNCE_DIR / "pending" / "a.md").exists())

    def test_announce_check_step_is_read_only(self):
        self.write("a", draft())
        self.write("b", draft(kind="minor"))
        st, b = post("/api/do", {"run": "announce-check"})
        self.assertEqual(st, 200, b)
        wait_idle()
        logs = list(ctl.LOGS.glob("*-announce-check.log"))
        self.assertEqual(len(logs), 1)
        t = logs[0].read_text(encoding="utf-8")
        self.assertIn("ANNOUNCE CHECK OK: 1 of 2", t)
        self.assertIn("Minor fixes don't get an announcement", t)
        self.assertEqual(self.fake.log, [])
        self.assertEqual(len(list((ctl.ANNOUNCE_DIR / "pending").glob("*.md"))), 2)


class Card(Base):
    def test_drafts_listed_as_they_would_post(self):
        self.write("2026-11-05-x", draft(body="TODO(owner): confirm\nHi"))
        self.write("2026-11-05-y", draft(ping="none"))
        (ctl.ANNOUNCE_DIR / "pending" / "Not An Id.md").write_text(draft())
        d = self.drafts()
        self.assertEqual(sorted(d), ["2026-11-05-x", "2026-11-05-y"])
        x, y = d["2026-11-05-x"], d["2026-11-05-y"]
        self.assertTrue(x["message"].startswith("@everyone\n**Big news**"))
        self.assertTrue(any("TODO(owner)" in p for p in x["problems"]))
        self.assertEqual(y["problems"], [])
        self.assertNotIn("@everyone", y["message"])
        self.assertEqual(len(y["sha"]), 64)
        page = req("GET", f"/?t={ctl.TOKEN}")[1]
        panel = page[page.index('id="discord"'):page.index('id="gmcard"')]
        for s in ("Pending announcements", 'id="anlist"', "type YES", "TODO(owner)"):
            self.assertIn(s, panel)
        self.assertIn("Post to #announcements", page)
        for bad in ("World of Warcraft", "WoW", "Warcraft", "Lich King", "Blizzard"):
            self.assertNotIn(bad, panel + ctl.STEPS["discord-announce"][1] + ctl.STEPS["announce-check"][1])

    def test_check_and_reject_without_the_network(self):
        self.write("a", draft(kind="fix"))
        st, b = post("/api/discord/announce-check", {"id": "a"})
        self.assertEqual(st, 200, b)
        j = json.loads(b)
        self.assertFalse(j["ok"])
        self.assertTrue(any("Minor fixes" in p for p in j["draft"]["problems"]))
        for bad in ("../a", "missing", ""):
            self.assertEqual(post("/api/discord/announce-check", {"id": bad})[0], 409, bad)
        st, b = post("/api/discord/announce-reject", {"id": "a", "why": "minor"})
        self.assertEqual(st, 200, b)
        self.assertFalse((ctl.ANNOUNCE_DIR / "pending" / "a.md").exists())
        self.assertIn("why: minor", (ctl.ANNOUNCE_DIR / "rejected" / "a.md").read_text())
        self.assertEqual(self.drafts(), {})
        self.assertEqual(self.fake.log, [])

    def test_the_stricter_post_check(self):
        self.write("a", draft())
        # the cookie or the link alone is not enough for the Discord card; JSON only; same-origin only
        st, _ = req("POST", f"/api/discord/announce-reject?t={ctl.TOKEN}", {"Content-Type": "application/json",
                    "Cookie": f"lc_token={ctl.TOKEN}"}, json.dumps({"id": "a"}))
        self.assertEqual(st, 403)
        self.assertEqual(post("/api/discord/announce-reject", {"id": "a"}, {"Content-Type": "text/plain"})[0], 415)
        self.assertEqual(post("/api/discord/announce-reject", {"id": "a"}, {"Sec-Fetch-Site": "cross-site"})[0], 403)
        self.assertEqual(post("/api/discord/announce-reject", {"id": "a"}, {"Origin": "http://evil.example"})[0], 403)
        self.assertTrue((ctl.ANNOUNCE_DIR / "pending" / "a.md").exists())

    def test_post_needs_token_clean_draft_yes_and_the_same_draft(self):
        self.write("a", draft())
        self.write("bad", draft(body="TODO(owner): not yet"))
        sha = self.drafts()["a"]["sha"]
        self.assertEqual(post("/api/discord/announce-post", {"id": "a", "sha": sha, "confirm": "YES"})[0], 409)  # no token
        ctl.save_discord_token(TOKEN)
        st, b = post("/api/discord/announce-post", {"id": "bad", "sha": self.drafts()["bad"]["sha"], "confirm": "YES"})
        self.assertEqual(st, 409)
        self.assertIn("TODO(owner)", b)
        self.assertEqual(post("/api/discord/announce-post", {"id": "a", "sha": sha, "confirm": "yes"})[0], 400)
        self.assertEqual(post("/api/discord/announce-post", {"id": "a", "sha": "0" * 64, "confirm": "YES"})[0], 409)
        self.write("a", draft(body="Edited after the owner read it."))
        st, b = post("/api/discord/announce-post", {"id": "a", "sha": sha, "confirm": "YES"})
        self.assertEqual(st, 409)
        self.assertIn("changed since you read it", b)
        time.sleep(0.3)
        self.assertEqual(self.fake.log, [])
        self.assertIsNone(ctl.state["running"])

    def test_post_after_yes_pings_publishes_moves_and_hides_the_token(self):
        ctl.save_discord_token(TOKEN)
        self.write("2026-11-05-boss", draft())
        sha = self.drafts()["2026-11-05-boss"]["sha"]
        st, b = post("/api/discord/announce-post", {"id": "2026-11-05-boss", "sha": sha, "confirm": "YES"})
        self.assertEqual(st, 200, b)
        wait_idle()
        w = self.fake.writes()
        self.assertEqual([x[1] for x in w], ["/channels/9/messages", "/channels/9/messages/777/crosspost"],
                         "\n".join(ctl.state["lines"][-20:]))
        self.assertEqual(w[0][2]["allowed_mentions"], {"parse": ["everyone"]})
        self.assertTrue(w[0][2]["content"].startswith("@everyone\n**Big news**"))
        self.assertEqual(self.fake.auth, {"Bot " + TOKEN})                     # it did use the saved token
        self.assertFalse((ctl.ANNOUNCE_DIR / "pending" / "2026-11-05-boss.md").exists())
        posted = (ctl.ANNOUNCE_DIR / "posted" / "2026-11-05-boss.md").read_text()
        self.assertIn("message_id: 777", posted)
        self.assertIn("published: yes", posted)
        a = state()[1]["discord"]["announce"]
        self.assertEqual(a["drafts"], [])
        self.assertTrue(a["last"]["ok"])
        self.assertEqual(a["last"]["draft"], "2026-11-05-boss")
        self.assertTrue(a["last"]["log"].endswith("-discord-announce.log"))
        log = (ctl.LOGS / a["last"]["log"]).read_text(encoding="utf-8")
        self.assertIn("ANNOUNCE POST OK", log)
        # posting it again: it isn't pending any more
        self.assertEqual(post("/api/discord/announce-post", {"id": "2026-11-05-boss", "sha": sha, "confirm": "YES"})[0], 409)
        for text in self.all_text():
            self.assertNotIn(TOKEN, text)
            for part in TOKEN.split("."):
                self.assertNotIn(part, text)


if __name__ == "__main__":
    unittest.main(verbosity=1)
