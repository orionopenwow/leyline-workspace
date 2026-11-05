#!/usr/bin/env python3
"""announce.py against a fake Discord (no network): the checks (TODO(owner), brand words, version numbers, money,
links, typed mass pings, minor fixes, length), the ping (allowed_mentions), the crosspost (and a failed one ignored),
the moves to posted/ and rejected/, posting only from Control with the approved draft, and the token never printed.

    python3 -I tools/discord-leyline/test_announce.py
"""
import contextlib, io, os, re, shutil, sys, tempfile, unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE))
import announce as an  # noqa: E402
import discord_leyline as dl  # noqa: E402

TOKEN = "MTAxNTk5.Gx1abc.secret-token-part-zzzzzzzzzzzzzzzz"
SEED = HERE / "announce" / "pending" / "2026-11-05-mobile-and-boss-fights.md"


class Fake:
    """dl.Discord stand-in: one server, #announcements (a news channel unless told), records every call."""
    calls = []
    news = True
    crosspost_fails = False

    def __init__(self, token, allow_writes=False, sleep=None):
        self.allow_writes = allow_writes

    def request(self, method, path, body=None, query=None):
        Fake.calls.append((method, path, body))
        if method != "GET" and not self.allow_writes:
            raise RuntimeError("blocked")
        if path == "/users/@me/guilds":
            return [{"id": "1", "name": "Leyline"}]
        if path == "/guilds/1/channels":
            return [{"id": "7", "type": 4, "name": "START"},
                    {"id": "8", "type": 0, "name": "general"},
                    {"id": "9", "type": 5 if Fake.news else 0, "name": "📣︱announcements", "parent_id": "7"}]
        if method == "POST" and path == "/channels/9/messages":
            return {"id": "555", "content": body["content"]}
        if method == "POST" and path == "/channels/9/messages/555/crosspost":
            if Fake.crosspost_fails:
                raise dl.ApiError(403, "Missing Permissions", 50013)
            return {"id": "555"}
        raise AssertionError((method, path))


def draft(title="Big news", kind="milestone", ping="everyone", body="Boss fights are here.", extra=""):
    return f"---\ntitle: {title}\nkind: {kind}\nping: {ping}\ncreated: 2026-11-05\n{extra}---\n{body}\n"


class Base(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="leyline-announce-"))
        self.root = self.tmp / "announce"
        (self.root / "pending").mkdir(parents=True)
        self.tok = self.tmp / "token.txt"
        self.tok.write_text(TOKEN)
        Fake.calls, Fake.news, Fake.crosspost_fails = [], True, False
        self.real = dl.Discord
        dl.Discord = Fake
        self.env = {k: os.environ.get(k) for k in ("LEYLINE_CONTROL_STEP", "LEYLINE_ANNOUNCE_ID", "LEYLINE_ANNOUNCE_SHA")}
        dl.Out.secret = None

    def tearDown(self):
        dl.Discord = self.real
        for k, v in self.env.items():
            os.environ.pop(k, None)
            if v is not None:
                os.environ[k] = v
        shutil.rmtree(self.tmp, ignore_errors=True)

    def write(self, i, text):
        (self.root / "pending" / f"{i}.md").write_text(text, encoding="utf-8")

    def run_cli(self, *argv):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            rc = an.main([*argv, "--dir", str(self.root), "--token-file", str(self.tok)])
        return rc, out.getvalue()

    def approve(self, i):
        os.environ.update(LEYLINE_CONTROL_STEP="discord-announce", LEYLINE_ANNOUNCE_ID=i,
                          LEYLINE_ANNOUNCE_SHA=an.load(i, self.root)["sha"])

    def probs(self, text):
        f, b, s = an.parse(text)
        return an.problems(f, b, s)

    def writes(self):
        return [c for c in Fake.calls if c[0] != "GET"]


class Checks(Base):
    def test_a_good_draft_passes(self):
        self.assertEqual(self.probs(draft(body="Play at https://leylinelauncher.com/wrath and "
                                               "https://discord.gg/abc, https://www.reddit.com/r/x, "
                                               "https://github.com/orionopenwow/Leyline")), [])

    def test_todo_owner_refused(self):
        p = self.probs(draft(body="TODO(owner): confirm\nHello"))
        self.assertTrue(any("TODO(owner)" in x for x in p), p)

    def test_banned_words_refused(self):
        for bad in ("World of Warcraft", "a WoW server", "Warcraft", "the Lich King", "Blizzard", "WOW"):
            self.assertTrue(any("never see" in x for x in self.probs(draft(body=f"Now with {bad}."))), bad)
        self.assertTrue(self.probs(draft(title="Wrath of the Lich King")))
        self.assertEqual(self.probs(draft(body="Workflow and snowy hills, great.")), [])

    def test_same_brand_check_as_the_website(self):
        src = (ROOT / "stream" / "hub.py").read_text(encoding="utf-8")
        m = re.search(r"BANNED_RE = re\.compile\(\n((?:\s+r\".*\"\n?)+)\)", src)
        self.assertIsNotNone(m)
        hub = "".join(re.findall(r'r"(.*)"', m.group(1)))
        self.assertEqual(hub, an.BANNED_RE.pattern)

    def test_version_numbers_and_money_refused(self):
        self.assertTrue(any("version" in x for x in self.probs(draft(body="For the 3.3.5a client."))))
        self.assertTrue(any("version" in x for x in self.probs(draft(body="Vanilla 1.12.1 too."))))
        self.assertTrue(any("money" in x for x in self.probs(draft(body="Support us on Patreon."))))
        self.assertTrue(any("money" in x for x in self.probs(draft(body="Donations welcome."))))
        self.assertEqual(self.probs(draft(body="Over 2 hours of new content on 5 Nov.")), [])

    def test_links(self):
        for bad in ("https://example.com/x", "http://leylinelauncher.com", "https://github.com/someone/else",
                    "https://evil.leylinelauncher.com.example.org/", "see bit.ly/abc", "www.patreon.org",
                    "https://user:pw@leylinelauncher.com/"):
            p = self.probs(draft(body=f"Look: {bad}"))
            self.assertTrue(any("link isn't allowed" in x for x in p), (bad, p))
        for good in ("https://survivors.leylinelauncher.com", "leylinelauncher.com", "https://redd.it/abc",
                     "https://discord.com/channels/1/2", "[the site](https://leylinelauncher.com/wrath)",
                     "https://play.openvanilla.app/wrath"):
            self.assertEqual(self.probs(draft(body=f"Look: {good}")), [], good)

    def test_minor_fixes_refused_with_a_plain_reason(self):
        for k in ("minor", "fix", "hotfix", "minor-fix"):
            p = self.probs(draft(kind=k))
            self.assertTrue(any("Minor fixes don't get an announcement" in x for x in p), (k, p))
        self.assertTrue(any("milestone, bigfix or schedule" in x for x in self.probs(draft(kind="party"))))
        for k in an.KINDS:
            self.assertEqual(self.probs(draft(kind=k)), [], k)

    def test_ping_and_typed_mass_pings(self):
        self.assertTrue(any("ping must be" in x for x in self.probs(draft(ping="here"))))
        for b in ("Hey @everyone!", "@here look"):
            self.assertTrue(any("set ping: everyone" in x for x in self.probs(draft(body=b))), b)
        f, b, _ = an.parse(draft())
        self.assertTrue(an.message(f, b).startswith("@everyone\n**Big news**\n\n"))
        f, b, _ = an.parse(draft(ping="none"))
        self.assertTrue(an.message(f, b).startswith("**Big news**\n\nBoss"))

    def test_over_length_refused(self):
        f, b, _ = an.parse(draft(body="x" * 1980))
        self.assertGreater(len(an.message(f, b)), an.LIMIT)
        self.assertTrue(any("limit is 2000" in x for x in an.problems(f, b)))
        f, b, _ = an.parse(draft(ping="none", title="T", body="y" * (2000 - len("**T**\n\n"))))
        self.assertEqual(len(an.message(f, b)), 2000)
        self.assertEqual(an.problems(f, b), [])

    def test_shape(self):
        self.assertTrue(self.probs("no front matter"))
        self.assertTrue(any("Unknown front matter" in x for x in self.probs(draft(extra="channel: general\n"))))
        self.assertTrue(any("no title" in x for x in self.probs(draft(title=""))))
        self.assertTrue(any("empty" in x for x in self.probs(draft(body=""))))
        for bad in ("../x", "a/b", "", "X Y", ".hidden"):
            self.assertFalse(an.valid_id(bad), bad)

    def test_the_seeded_draft(self):
        text = SEED.read_text(encoding="utf-8")
        f, b, s = an.parse(text)
        self.assertEqual((f["kind"], f["ping"]), ("milestone", "everyone"))
        self.assertTrue(b.startswith("TODO(owner): confirm what is live before posting"))
        p = an.problems(f, b, s)
        self.assertEqual(len(p), 1, p)
        self.assertIn("TODO(owner)", p[0])
        clean = "\n".join(l for l in text.splitlines() if "TODO(owner)" not in l)
        self.assertEqual(self.probs(clean), [], "only the TODO(owner) line keeps it back")
        for w in ("full screen", "home screen", "Frozen Throne", "leaderboard", "Android"):
            self.assertIn(w, b)

    def test_list_and_check_cli(self):
        self.write("2026-11-05-good", draft())
        self.write("2026-11-05-bad", draft(kind="minor"))
        rc, out = self.run_cli("check", "--all")
        self.assertEqual(rc, 0)
        self.assertIn("1 of 2 draft(s) ready", out)
        self.assertEqual(self.run_cli("check", "2026-11-05-good")[0], 0)
        rc, out = self.run_cli("check", "2026-11-05-bad")
        self.assertEqual(rc, 1)
        self.assertIn("Minor fixes", out)
        self.assertEqual(self.run_cli("list")[0], 0)
        self.assertEqual(Fake.calls, [], "list and check never touch the network")


class Posting(Base):
    def test_post_from_control_with_ping_and_crosspost(self):
        self.write("2026-11-05-a", draft())
        self.approve("2026-11-05-a")
        rc, out = self.run_cli("post", "--yes-from-control")
        self.assertEqual(rc, 0, out)
        self.assertIn("ANNOUNCE POST OK", out)
        w = self.writes()
        self.assertEqual([c[1] for c in w], ["/channels/9/messages", "/channels/9/messages/555/crosspost"])
        self.assertEqual(w[0][2]["allowed_mentions"], {"parse": ["everyone"]})
        self.assertTrue(w[0][2]["content"].startswith("@everyone\n"))
        self.assertFalse((self.root / "pending" / "2026-11-05-a.md").exists())
        posted = (self.root / "posted" / "2026-11-05-a.md").read_text()
        for s in ("message_id: 555", "channel_id: 9", "published: yes", "posted: 20", "Boss fights are here."):
            self.assertIn(s, posted)
        self.assertNotIn(TOKEN, out)
        for part in TOKEN.split("."):
            self.assertNotIn(part, out + posted)

    def test_no_ping_means_no_mentions(self):
        self.write("b", draft(ping="none", kind="schedule"))
        self.approve("b")
        rc, out = self.run_cli("post", "--yes-from-control")
        self.assertEqual(rc, 0, out)
        body = self.writes()[0][2]
        self.assertEqual(body["allowed_mentions"], {"parse": []})
        self.assertNotIn("@everyone", body["content"])

    def test_crosspost_failure_ignored_and_plain_text_channel(self):
        Fake.crosspost_fails = True
        self.write("c", draft())
        self.approve("c")
        rc, out = self.run_cli("post", "--yes-from-control")
        self.assertEqual(rc, 0, out)
        self.assertIn("Couldn't publish", out)
        self.assertIn("published: failed", (self.root / "posted" / "c.md").read_text())
        Fake.calls, Fake.news = [], False
        self.write("d", draft())
        self.approve("d")
        rc, out = self.run_cli("post", "--yes-from-control")
        self.assertEqual(rc, 0, out)
        self.assertEqual(len(self.writes()), 1, "no crosspost outside a news channel")

    def test_refusals_post_nothing(self):
        self.write("e", draft(kind="minor"))
        self.approve("e")
        rc, out = self.run_cli("post", "--yes-from-control")
        self.assertEqual((rc, self.writes()), (1, []))
        self.assertIn("Minor fixes", out)
        self.write("f", draft(body="TODO(owner): x"))
        self.approve("f")
        self.assertEqual(self.run_cli("post", "--yes-from-control")[0], 1)
        # not from Control
        self.write("g", draft())
        self.approve("g")
        os.environ["LEYLINE_CONTROL_STEP"] = "discord-apply"
        rc, out = self.run_cli("post", "--yes-from-control")
        self.assertEqual(rc, 1)
        self.assertIn("only works from Leyline Control", out)
        # a different draft than the approved one, or the file changed since
        self.approve("g")
        self.write("h", draft())
        self.assertEqual(self.run_cli("post", "h", "--yes-from-control")[0], 1)
        self.write("g", draft(body="Changed after the owner read it."))
        rc, out = self.run_cli("post", "--yes-from-control")
        self.assertEqual(rc, 1)
        self.assertIn("changed since you read it", out)
        self.assertEqual(self.writes(), [])
        self.assertTrue((self.root / "pending" / "g.md").exists())

    def test_by_hand_needs_typed_yes(self):
        self.write("i", draft())
        import builtins
        real = builtins.input
        try:
            builtins.input = lambda *_: "yes"
            self.assertEqual(self.run_cli("post", "i")[0], 1)
            self.assertEqual(self.writes(), [])
            builtins.input = lambda *_: "YES"
            rc, out = self.run_cli("post", "i")
            self.assertEqual(rc, 0, out)
        finally:
            builtins.input = real
        self.assertEqual(len(self.writes()), 2)

    def test_no_token_posts_nothing(self):
        self.tok.unlink()
        self.write("j", draft())
        self.approve("j")
        rc, out = self.run_cli("post", "--yes-from-control")
        self.assertEqual(rc, 1)
        self.assertIn("No bot token", out)
        self.assertEqual(Fake.calls, [])

    def test_reject_moves_it(self):
        self.write("k", draft())
        rc, out = self.run_cli("reject", "k", "--why", "not yet")
        self.assertEqual(rc, 0, out)
        self.assertFalse((self.root / "pending" / "k.md").exists())
        t = (self.root / "rejected" / "k.md").read_text()
        self.assertIn("why: not yet", t)
        self.assertIn("rejected: 20", t)
        self.assertEqual(Fake.calls, [])
        self.assertEqual(self.run_cli("reject", "../k")[0], 1)


if __name__ == "__main__":
    unittest.main(verbosity=1)
