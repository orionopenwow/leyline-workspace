"""Den of the week (stream/denweek.py): what staff may set (plain words, no brand words or client numbers, https
screenshot links only, a Visit link that is https or leyline://realm/<id>), the file, the HQ actions with the role
staff need, the front page card (escaped, links only, no pictures), home.json's den_week for the launcher and the
in-game window (stream/wrath_portal.py art_bytes), and the hub's front-page hook.

    python3 stream/tests/test_denweek.py
"""
import importlib.util
import json
import os
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
STREAM = os.path.dirname(HERE)


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, os.path.join(STREAM, file))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


class FakeAdmin:
    class Refused(Exception):
        pass

    def __init__(self):
        self.ACTIONS = {}

    def act(self, user, action, target, label, back, fields="", confirm=None, danger=False, block=False):
        return f'<form data-action="{action}" data-target="{target}">{fields}<button>{label}</button></form>'


class DenWeek(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp(prefix="wu-dw-")
        os.environ["WU_DEN_WEEK_FILE"] = os.path.join(self.dir, "hq", "den-week.json")
        self.dw = load("wu_denweek_test", "denweek.py")
        self.A = FakeAdmin()
        self.dw.setup(self.A)
        self.user = {"account": "staff1", "role": "mod"}

    def tearDown(self):
        os.environ.pop("WU_DEN_WEEK_FILE", None)

    def pick(self, **kw):
        form = {"title": "Mossy Hollow", "by": "Ana", "line": "A cottage by a moonwell pond.",
                "shots": "https://i.example.org/a.png\nhttps://i.example.org/b.png", "visit": "leyline://realm/abcdefghijk2345a"}
        form.update(kw)
        return self.dw.a_set(self.user, "new", form)

    def test_actions_registered_for_staff(self):
        self.assertEqual(self.A.ACTIONS["den.week.set"][1], "mod")
        self.assertEqual(self.A.ACTIONS["den.week.clear"][1], "mod")

    def test_set_and_read(self):
        before, after = self.pick()
        self.assertIsNone(before)
        cur = self.dw.current()
        self.assertEqual(cur, after)
        self.assertEqual((cur["title"], cur["by"], cur["visit"]), ("Mossy Hollow", "Ana", "leyline://realm/abcdefghijk2345a"))
        self.assertEqual(cur["shots"], ["https://i.example.org/a.png", "https://i.example.org/b.png"])
        self.assertRegex(cur["week"], r"^\d{4}-\d{2}-\d{2}$")
        doc = json.load(open(os.environ["WU_DEN_WEEK_FILE"], encoding="utf-8"))
        self.assertEqual((doc["format"], doc["by_staff"]), (1, "staff1"))

    def test_clean_words_and_links(self):
        c = self.dw.clean
        self.assertIsNone(c({"title": "", "by": "Ana"}), "a name is needed")
        self.assertIsNone(c({"title": "Den", "by": ""}), "and its maker")
        self.assertIsNone(c({"title": "My WoW house", "by": "Ana"}), "no brand words")
        self.assertIsNone(c({"title": "Warcraft keep", "by": "Ana"}))
        self.assertEqual(c({"title": "Keep", "by": "Ana", "line": "Built on 3.3.5a"})["line"], "", "no client numbers")
        self.assertEqual(c({"title": "  Two\nlines\x00 ", "by": "Ana"})["title"], "Two lines")
        self.assertEqual(len(c({"title": "x" * 99, "by": "Ana"})["title"]), self.dw.TITLE_MAX)
        shots = c({"title": "K", "by": "A", "shots": ["http://plain.example/a.png", "javascript:alert(1)", "https://ok.example/1.png",
                                                       "https://ok.example/1.png", "data:image/png;base64,AAAA", "https://a.example/\"><x>"]})["shots"]
        self.assertEqual(shots, ["https://ok.example/1.png"], "https only, no repeats, nothing odd")
        many = c({"title": "K", "by": "A", "shots": [f"https://ok.example/{i}.png" for i in range(9)]})["shots"]
        self.assertEqual(len(many), self.dw.SHOTS_MAX)
        self.assertEqual(c({"title": "K", "by": "A", "visit": "leyline://realm/short"})["visit"], "")
        self.assertEqual(c({"title": "K", "by": "A", "visit": "https://relay.example.org/join/abc#XYZ"})["visit"], "https://relay.example.org/join/abc#XYZ")

    def test_refusals(self):
        with self.assertRaises(FakeAdmin.Refused):
            self.pick(title="")
        with self.assertRaises(FakeAdmin.Refused):
            self.pick(visit="javascript:alert(1)")
        with self.assertRaises(FakeAdmin.Refused):
            self.dw.a_clear(self.user, "pick", {})
        self.pick()
        before, after = self.dw.a_clear(self.user, "pick", {})
        self.assertEqual(before["title"], "Mossy Hollow")
        self.assertIsNone(after)
        self.assertIsNone(self.dw.current())

    def test_hand_edited_file_is_held_to_the_rules(self):
        os.makedirs(os.path.dirname(os.environ["WU_DEN_WEEK_FILE"]), exist_ok=True)
        with open(os.environ["WU_DEN_WEEK_FILE"], "w", encoding="utf-8") as f:
            json.dump({"format": 1, "pick": {"title": "<script>", "by": "Blizzard fan", "shots": ["https://x.example/a"]}}, f)
        self.assertIsNone(self.dw.current(), "a brand word in the maker: no pick")
        with open(os.environ["WU_DEN_WEEK_FILE"], "w", encoding="utf-8") as f:
            f.write("not json")
        self.assertIsNone(self.dw.current())

    def test_front_page_card(self):
        self.assertEqual(self.dw.section_html(), "", "nothing picked: no card")
        self.pick(title="Tom & Jerry's <Den>", line='A "quote"')
        h = self.dw.section_html()
        self.assertIn("Den of the week", h)
        self.assertIn("Tom &amp; Jerry&#x27;s &lt;Den&gt;", h)
        self.assertNotIn("<Den>", h)
        self.assertIn('href="https://i.example.org/a.png"', h)
        self.assertIn(">Screenshot 1</a>", h)
        self.assertIn(">Visit</a>", h)
        self.assertNotIn("<img", h, "screenshots are links only")

    def test_home_json_merge(self):
        body = json.dumps({"slides": {"hangout": [{"file": "hangout.webp"}]}}).encode()
        self.assertEqual(self.dw.merge_home(body), body, "no pick: home.json as it is")
        self.assertIsNone(self.dw.merge_home(None))
        self.pick()
        merged = json.loads(self.dw.merge_home(body))
        self.assertEqual(merged["slides"]["hangout"][0]["file"], "hangout.webp")
        self.assertEqual(merged["den_week"]["title"], "Mossy Hollow")
        self.assertEqual(json.loads(self.dw.merge_home(None)), {"den_week": merged["den_week"]})
        self.assertEqual(self.dw.merge_home(b"[1,2]"), b"[1,2]", "a list is left alone")

    def test_hq_panel(self):
        self.assertIn("No Den picked this week.", self.dw.panel_html(self.user, "/wrath/admin"))
        self.pick()
        h = self.dw.panel_html(self.user, "/wrath/admin")
        self.assertIn('data-action="den.week.set"', h)
        self.assertIn('data-action="den.week.clear"', h)
        self.assertIn("<b>Mossy Hollow</b> by Ana", h)
        self.assertIn("2 screenshot links, a Visit link", h)
        for word in ("warcraft", "blizzard", "lich king"):
            self.assertNotIn(word, h.lower())


class PortalAndHub(unittest.TestCase):
    """The portal serves home.json with den_week; the hub's front page calls the card."""

    def test_portal_merges_home_json(self):
        src = open(os.path.join(STREAM, "wrath_portal.py"), encoding="utf-8").read()
        self.assertIn("return den_week_home(body if ok else None)", src)
        self.assertIn("return den_week_home(None) if name == HOME_JSON else None", src)
        art = tempfile.mkdtemp(prefix="wu-art-")
        os.makedirs(os.path.join(art, "home"))
        os.environ["WU_DEN_WEEK_FILE"] = os.path.join(art, "dw.json")
        try:
            dw = load("wu_denweek", "denweek.py")
            sys.modules["wu_denweek"] = dw
            dw.save({"title": "Mossy Hollow", "by": "Ana", "line": "", "shots": [], "visit": "", "week": ""})
            ns = {}
            code = src[src.index("def den_week_home("):src.index("def art_bytes(")]
            import re as _re
            ns.update({"sys": sys, "_load_module": lambda n, f: None, "json": json, "re": _re})
            exec(code, ns)   # noqa: S102 (our own source, the one function)
            out = json.loads(ns["den_week_home"](b'{"slides": {}}'))
            self.assertEqual(out["den_week"]["by"], "Ana")
            self.assertEqual(json.loads(ns["den_week_home"](None))["den_week"]["title"], "Mossy Hollow")
        finally:
            os.environ.pop("WU_DEN_WEEK_FILE", None)
            sys.modules.pop("wu_denweek", None)

    def test_hub_front_page_hook(self):
        src = open(os.path.join(STREAM, "hub.py"), encoding="utf-8").read()
        self.assertIn("self.how_strip() + den_week_html() + self.featured_mod(items)", src)
        self.assertIn("def den_week_home(", open(os.path.join(STREAM, "wrath_portal.py"), encoding="utf-8").read())

    def test_hq_hook(self):
        src = open(os.path.join(STREAM, "hq.py"), encoding="utf-8").read()
        self.assertIn("dw.setup(admin)", src)
        self.assertIn("denweek().panel_html(user, back)", src)


if __name__ == "__main__":
    unittest.main()
