"""Report a bug (stream/bugreport.py): the page signed out and in, the form (token, lengths, limits, sanctions), the
launcher's JSON endpoint (Bearer only), and what lands in the intake file: display name, redacted text, the place;
never an account id, email or IP address. A fake handler and fake forum sign-in: no portal, no network.

    python3 -I stream/tests/test_bugreport.py
"""
import importlib.util
import io
import json
import os
import shutil
import sys
import tempfile
import unittest
import re
import urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
sys.dont_write_bytecode = True
TMP = tempfile.mkdtemp()
os.environ["WU_PORTAL_DATA"] = TMP
spec = importlib.util.spec_from_file_location("wu_bugreport", os.path.join(HERE, "..", "bugreport.py"))
BR = importlib.util.module_from_spec(spec)
spec.loader.exec_module(BR)

ALICE = {"acct": "ALICE1", "name": "Alice", "tag": "1234", "staff": False, "session": "sess-a", "display": "Alice"}


class ForumError(Exception):
    def __init__(self, status, text):
        super().__init__(text)
        self.status, self.text = status, text


class FakeForum:
    def __init__(self):
        self.who = None
        self.tokens = {"good-token": dict(ALICE, session="", via="launcher")}

    def viewer(self, h):
        return self.who

    def token_viewer(self, h):
        a = h.headers.get("Authorization", "")
        if not a.startswith("Bearer "):
            return None
        if a[7:] not in self.tokens:
            raise ForumError(401, "Signed out.")
        return self.tokens[a[7:]]

    def _same_site(self, h):
        return h.headers.get("Sec-Fetch-Site", "") != "cross-site"


class FakeAdmin:
    banned = set()

    def blocked(self, what, acct=""):
        return "Your account can't post right now." if acct in self.banned else ""


FORUM, ADMIN = FakeForum(), FakeAdmin()
NS = {"forum": FORUM, "admin": ADMIN, "render": lambda body, title="": f"<title>{title}</title>{body}".encode(),
      "DISCORD_URL": "https://discord.gg/leyline", "rl_bucket": lambda ip: ip}
BR.bind(lambda n: NS[n])


class H:
    def __init__(self, body=b"", headers=None, ip="198.51.100.7"):
        self.headers = dict(headers or {})
        self.headers.setdefault("Content-Length", str(len(body)))
        self.rfile = io.BytesIO(body)
        self._ip = ip
        self.sent = None

    def ip(self):
        return self._ip

    def send(self, status, body, ctype="text/html; charset=utf-8", headers=None):
        self.sent = (status, body.decode("utf-8"), ctype, headers or {})


def intake():
    try:
        with open(BR.INTAKE) as f:
            return [json.loads(x) for x in f.read().splitlines()]
    except OSError:
        return []


def form(**kw):
    return urllib.parse.urlencode(kw).encode()


class Pages(unittest.TestCase):
    def setUp(self):
        BR._acct_times.clear(); BR._net_times.clear()
        FORUM.who = None
        ADMIN.banned = set()
        shutil.rmtree(os.path.join(TMP, "bugs"), ignore_errors=True)

    def test_routes(self):
        self.assertTrue(BR.match("/wrath/bugs") and BR.match("/wrath/bugs/api/report"))
        self.assertFalse(BR.match("/wrath/bugs/x"))

    def test_signed_out_page_says_how_to_sign_in_and_other_ways(self):
        h = H(); BR.get(h, "/wrath/bugs", {})
        st, body = h.sent[:2]
        self.assertEqual(st, 200)
        self.assertIn("/login?next=%2Fwrath%2Fbugs", body)
        self.assertIn(".bug", body); self.assertIn("#bug-reports", body)
        self.assertIn('href="https://discord.gg/leyline"', body)
        self.assertNotIn("<form", body)

    def test_form_round_trip(self):
        FORUM.who = ALICE
        h = H(); BR.get(h, "/wrath/bugs", {"where": ["den"], "page": ["Your Den <b>"]})
        body = h.sent[1]
        self.assertIn('value="den" selected', body)
        self.assertIn("Your Den &lt;b&gt;", body)
        tok = BR.form_token(ALICE)
        h = H(form(t=tok, where="den", page="Your Den", text="My props vanish after I save, mail me alice@example.com"))
        BR.post(h, "/wrath/bugs")
        self.assertEqual(h.sent[0], 200, h.sent[1])
        self.assertIn("Thanks, we have it", h.sent[1])
        [rec] = intake()
        self.assertEqual((rec["source"], rec["reporter"], rec["where"]["page"]), ("site", "Alice", "Your Den: Your Den"))
        self.assertIn("[email]", rec["text"])
        raw = open(BR.INTAKE).read()
        for leak in ("alice@example.com", "ALICE1", "198.51.100.7", "sess-a"):
            self.assertNotIn(leak, raw)
        self.assertEqual(os.stat(BR.INTAKE).st_mode & 0o777, 0o600)

    def test_bad_token_short_text_cross_site(self):
        FORUM.who = ALICE
        h = H(form(t="nope", where="site", text="the realm list is empty for me")); BR.post(h, "/wrath/bugs")
        self.assertEqual(h.sent[0], 400); self.assertIn("timed out", h.sent[1])
        self.assertIn("the realm list is empty for me", h.sent[1], "the text is kept for a second try")
        h = H(form(t=BR.form_token(ALICE), text="broken")); BR.post(h, "/wrath/bugs")
        self.assertEqual(h.sent[0], 400); self.assertIn("at least", h.sent[1])
        h = H(form(t=BR.form_token(ALICE), text="the realm list is empty"), {"Sec-Fetch-Site": "cross-site"})
        BR.post(h, "/wrath/bugs")
        self.assertEqual(h.sent[0], 403)
        self.assertEqual(intake(), [])

    def test_signed_out_post_is_refused(self):
        h = H(form(t="x", text="the realm list is empty for me")); BR.post(h, "/wrath/bugs")
        self.assertEqual(h.sent[0], 401)
        self.assertEqual(intake(), [])

    def test_limits_per_account_and_network(self):
        FORUM.who = ALICE
        for i in range(BR.REPORTS_HOUR):
            h = H(form(t=BR.form_token(ALICE), text=f"report number {i} about the realm list"))
            BR.post(h, "/wrath/bugs")
            self.assertEqual(h.sent[0], 200)
        h = H(form(t=BR.form_token(ALICE), text="one report too many about it")); BR.post(h, "/wrath/bugs")
        self.assertEqual(h.sent[0], 429)
        self.assertEqual(BR.allowed("OTHER", "203.0.113.1", now=0), "")
        for i in range(BR.NET_HOUR - 1):
            BR.allowed(f"A{i}", "203.0.113.1", now=1)
        self.assertIn("network", BR.allowed("B", "203.0.113.1", now=2))

    def test_sanctioned_accounts_cannot_report(self):
        FORUM.who = ALICE; ADMIN.banned = {"ALICE1"}
        h = H(form(t=BR.form_token(ALICE), text="the realm list is empty for me")); BR.post(h, "/wrath/bugs")
        self.assertEqual(h.sent[0], 403)

    def test_launcher_api(self):
        doc = json.dumps({"text": "Host a server button does nothing", "page": "Host a server", "app": "0.9.1 <x>"}).encode()
        h = H(doc); BR.post(h, "/wrath/bugs/api/report")
        self.assertEqual(h.sent[0], 401)
        h = H(doc, {"Authorization": "Bearer bad"}); BR.post(h, "/wrath/bugs/api/report")
        self.assertEqual(h.sent[0], 401)
        h = H(doc, {"Authorization": "Bearer good-token", "Cookie": "wr_session=x"}); BR.post(h, "/wrath/bugs/api/report")
        self.assertEqual(h.sent[0], 200, h.sent[1])
        out = json.loads(h.sent[1])
        self.assertTrue(out["ok"])
        self.assertNotIn("Access-Control-Allow-Origin", h.sent[3])
        [rec] = intake()
        self.assertEqual(rec["id"], "launcher:" + out["id"])
        self.assertEqual((rec["source"], rec["where"]["page"], rec["where"]["app"]), ("launcher", "The launcher: Host a server", "0.9.1 x"))
        h = H(b"[1]", {"Authorization": "Bearer good-token"}); BR.post(h, "/wrath/bugs/api/report")
        self.assertEqual(h.sent[0], 400)

    def test_rotation(self):
        os.makedirs(os.path.dirname(BR.INTAKE), exist_ok=True)
        with open(BR.INTAKE, "w") as f:
            f.write("x" * (BR.ROTATE_BYTES + 1))
        rec, _ = BR.make_record("site", "Bo", "the realm list is empty for me")
        BR.append(rec)
        self.assertTrue(os.path.exists(BR.INTAKE + ".1"))
        self.assertEqual(len(intake()), 1)

    def test_no_brand_names_on_the_page(self):
        import re
        FORUM.who = ALICE
        h = H(); BR.get(h, "/wrath/bugs", {})
        self.assertNotRegex(h.sent[1], r"(?i)world\s*of\s*warcraft|\bwow\b|warcraft|lich\s*king|blizzard")


class DiscordPlan(unittest.TestCase):
    """docs/DISCORD-PLAN.md's template (stream/discord_template_v2.json) and the first posts (docs/discord-initial-posts.md)."""
    ROOT = os.path.join(HERE, "..", "..")

    def setUp(self):
        s = importlib.util.spec_from_file_location("dg_check", os.path.join(HERE, "..", "discord_growth.py"))
        self.dg = importlib.util.module_from_spec(s); s.loader.exec_module(self.dg)
        self.v1 = json.load(open(os.path.join(HERE, "..", "discord_template.json"), encoding="utf-8"))
        self.v2 = self.dg.load_template(os.path.join(HERE, "..", "discord_template_v2.json"))
        with open(os.path.join(self.ROOT, "docs", "discord-initial-posts.md"), encoding="utf-8") as f:
            self.posts = f.read()

    def test_template_keeps_the_bot_working(self):
        self.assertEqual(self.v2["posts"], self.v1["posts"])
        self.assertEqual([(r["key"], r["name"], r.get("auto")) for r in self.v2["roles"]],
                         [(r["key"], r["name"], r.get("auto")) for r in self.v1["roles"]])
        names = {c["name"].split("︱")[-1] for cat in self.v2["categories"] for c in cat["channels"]}
        for n in ("welcome", "rules", "announcements", "changelog", "hangout", "levelling", "world-chat", "den-showcase",
                  "den-files", "host-templates", "hosting-help", "workshop-submissions", "claude-and-agents", "companions",
                  "bug-reports", "feature-ideas", "forum"):
            self.assertIn(n, names)

    def test_every_text_channel_has_a_first_post(self):
        for cat in self.v2["categories"]:
            for ch in cat["channels"]:
                if ch.get("type", "text") in ("text", "forum"):
                    base = ch["name"].split("︱")[-1]          # "👋︱welcome": the 30 Oct emoji and bar, then the name
                    self.assertRegex(self.posts, r"(?m)^### #" + re.escape(base) + r"\b", ch["name"])

    def test_channel_names_keep_the_emoji_and_bar(self):
        """Owner, 1 Nov: "keep the emoji thematics and bar too". Every channel is "<emoji>︱<name>"."""
        for cat in self.v2["categories"]:
            for ch in cat["channels"]:
                self.assertRegex(ch["name"], r"^[^\w\s]{1,2}︱[\w][\w -]*$", ch["name"])

    def test_first_posts_follow_the_rules(self):
        blocks = re.findall(r"```\n(.*?)```", self.posts, re.S)
        self.assertGreater(len(blocks), 30)
        for b in blocks:
            self.assertLess(len(b), 2000)
        text = self.posts
        self.assertNotRegex(text, r"(?i)world\s*of\s*warcraft|\bwow\b|warcraft|lich\s*king|blizzard")
        self.assertNotRegex(text, r"\b\d+\.\d+(\.\d+)?[a-z]?\b", "no version numbers")
        hosts = {urllib.parse.urlsplit(u).hostname for u in re.findall(r"https?://[^\s)`]+", text)}
        self.assertLessEqual(hosts, {"leylinelauncher.com", "play.openvanilla.app", "discord.gg", "discord.com", "reddit.com", "www.reddit.com"})
        self.assertNotRegex(text, r"(?i)patreon|ko-?fi|paypal|donat|\btips?\b|buy me")
        self.assertIn("TODO(owner)", text)
        for need in ("How to report a bug", "How to share your Den", "How to host your own realm", "**The rules**"):
            self.assertIn(need, text)


if __name__ == "__main__":
    try:
        unittest.main(verbosity=1)
    finally:
        shutil.rmtree(TMP, ignore_errors=True)
