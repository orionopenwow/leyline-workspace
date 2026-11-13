"""Leyline weekly v0 (stream/weekly.py): the "This week" row (one featured pick per mode, each until featured_until),
what an item may say (plain words, no brand words, client numbers or money links; links https or leyline://realm/<id>),
home.json's this_week for the launcher (stream/wrath_portal.py den_week_home -> this_week_home), the front page strip
(escaped) and its hook in stream/hub.py, and the committed stream/weekly.json.

    python3 stream/tests/test_weekly.py
"""
import calendar
import importlib.util
import json
import os
import tempfile
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
STREAM = os.path.dirname(HERE)
NOW = calendar.timegm((2026, 11, 12, 12, 0, 0))


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, os.path.join(STREAM, file))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def item(mode="brawl", title="Brawl night", start="2026-11-09", until="2026-11-16T00:00:00Z", **kw):
    return dict({"mode": mode, "title": title, "line": "Everyone starts at 80.", "visit": "", "featured_from": start,
                 "featured_until": until}, **kw)


class Weekly(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp(prefix="wu-wk-")
        self.file = os.path.join(self.dir, "weekly.json")
        os.environ["WU_WEEKLY_FILE"] = self.file
        self.w = load("wu_weekly_test", "weekly.py")

    def tearDown(self):
        os.environ.pop("WU_WEEKLY_FILE", None)

    def put(self, items, fmt=1):
        with open(self.file, "w", encoding="utf-8") as f:
            json.dump({"format": fmt, "items": items}, f)

    def test_one_per_mode_live_now_newest_start_wins(self):
        self.put([item(), item(title="Older pick", start="2026-11-02"), item(mode="keystones", title="Twist", until="2026-11-14"),
                  item(mode="minigolf", title="Last week", start="2026-11-02", until="2026-11-09"),
                  item(mode="coin-dash", title="Next week", start="2026-11-16", until="2026-11-23")])
        row = self.w.this_week(NOW)
        self.assertEqual([(x["mode"], x["title"]) for x in row], [("brawl", "Brawl night"), ("keystones", "Twist")])
        self.assertEqual(row[0]["until"], calendar.timegm((2026, 11, 16, 0, 0, 0)))
        self.assertEqual(set(row[0]), {"mode", "title", "line", "visit", "until"}, "the row's data shape")
        self.assertEqual(self.w.this_week(calendar.timegm((2026, 11, 17, 0, 0, 0)))[0]["title"], "Next week")

    def test_at_most_six(self):
        self.put([item(mode=f"m{i}", title=f"Pick {i}") for i in range(9)])
        self.assertEqual(len(self.w.this_week(NOW)), self.w.ROW_MAX)

    def test_words_and_links_are_cleaned(self):
        bad = [item(mode="Bad Mode"), item(title=""), item(until="not a time"), item(start="2026-11-20", until="2026-11-19"),
               item(mode="a", title="Best Warcraft pick"), item(mode="b", title="Patch 3.3.5a night"),
               item(mode="c", title="Support us on patreon.com/x")]
        for d in bad:
            self.assertIsNone(self.w.clean(d), d)
        it = self.w.clean(item(line="Donate to the guild", visit="javascript:alert(1)"))
        self.assertEqual((it["line"], it["visit"]), ("", ""))
        self.assertEqual(self.w.clean(item(visit="leyline://realm/abcdefghijk2345a"))["visit"], "leyline://realm/abcdefghijk2345a")
        self.assertEqual(self.w.clean(item(visit="https://www.patreon.com/x"))["visit"], "")
        self.assertEqual(self.w.clean(item(title="  Brawl‮\nnight  "))["title"], "Brawl night")

    def test_missing_or_broken_file(self):
        self.assertEqual(self.w.this_week(NOW), [])
        with open(self.file, "w") as f:
            f.write("{nope")
        self.assertEqual(self.w.this_week(NOW), [])
        self.put([item()], fmt=2)
        self.assertEqual(self.w.this_week(NOW), [], "a newer format is not guessed at")

    def test_home_json_merge(self):
        self.put([item()])
        out = json.loads(self.w.merge_home(b'{"slides": {"x": []}}', NOW))
        self.assertEqual(out["slides"], {"x": []})
        self.assertEqual(out["this_week"]["format"], 1)
        self.assertEqual(out["this_week"]["items"][0]["mode"], "brawl")
        self.assertEqual(json.loads(self.w.merge_home(None, NOW))["this_week"]["at"], NOW)
        self.assertEqual(self.w.merge_home(b"[1]", NOW), b"[1]", "not an object: left as it was")
        self.put([])
        self.assertEqual(self.w.merge_home(b'{"a": 1}', NOW), b'{"a": 1}', "nothing live: unchanged")

    def test_front_page_strip_is_escaped(self):
        self.put([item(title="Brawl <b>night</b>", visit="https://example.org/a?b=1&c=2")])
        h = self.w.section_html(now=NOW)
        self.assertIn("This week", h)
        self.assertIn("Brawl &lt;b&gt;night&lt;/b&gt;", h)
        self.assertIn('href="https://example.org/a?b=1&amp;c=2"', h)
        self.assertIn("Wrath Brawl", h, "the mode's own name from playmodes.json")
        self.assertIn("Until Mon 16 Nov", h)
        self.put([])
        self.assertEqual(self.w.section_html(now=NOW), "")


class Hooks(unittest.TestCase):
    def test_portal_and_hub(self):
        portal = open(os.path.join(STREAM, "wrath_portal.py"), encoding="utf-8").read()
        self.assertIn("return this_week_home(body)", portal)
        self.assertIn('_load_module("wu_weekly", "weekly.py")', portal)
        hub = open(os.path.join(STREAM, "hub.py"), encoding="utf-8").read()
        self.assertIn("+ this_week_html()", hub)

    def test_portal_merge_end_to_end(self):
        import sys
        d = tempfile.mkdtemp(prefix="wu-wk2-")
        os.environ["WU_WEEKLY_FILE"] = os.path.join(d, "weekly.json")
        os.environ["WU_DEN_WEEK_FILE"] = os.path.join(d, "none.json")
        until = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(time.time() + 86400))
        with open(os.environ["WU_WEEKLY_FILE"], "w") as f:
            json.dump({"format": 1, "items": [item(start="2026-01-01", until=until)]}, f)
        src = open(os.path.join(STREAM, "wrath_portal.py"), encoding="utf-8").read()
        code = src[src.index("def den_week_home("):src.index("def art_bytes(")]

        def _load_module(name, file):
            return load(name + "_t", file)
        ns = {"sys": sys, "_load_module": _load_module}
        saved = {k: sys.modules.pop(k) for k in ("wu_weekly", "wu_denweek") if k in sys.modules}
        try:
            exec(code, ns)
            out = json.loads(ns["den_week_home"](b'{"slides": {}}'))
            self.assertEqual(out["this_week"]["items"][0]["title"], "Brawl night")
            self.assertNotIn("den_week", out)
            self.assertIsNone(ns["den_week_home"](None), "no home.json (or a broken one) stays a 404")
        finally:
            for k in ("wu_weekly", "wu_denweek"):
                sys.modules.pop(k, None)
            sys.modules.update(saved)
            os.environ.pop("WU_WEEKLY_FILE", None)
            os.environ.pop("WU_DEN_WEEK_FILE", None)


class Committed(unittest.TestCase):
    def test_the_seed_file_reads_clean(self):
        os.environ.pop("WU_WEEKLY_FILE", None)
        w = load("wu_weekly_seed", "weekly.py")
        with open(os.path.join(STREAM, "weekly.json"), encoding="utf-8") as f:
            raw = json.load(f)["items"]
        self.assertEqual(len(w.load()), len(raw), "every committed item passes the cleaning")
        modes = {m["id"] for m in json.load(open(os.path.join(STREAM, "playmodes.json"), encoding="utf-8"))["modes"]}
        self.assertTrue({x["mode"] for x in raw} <= modes, "only modes the catalog has")
        self.assertEqual(len({x["mode"] for x in raw}), len(raw), "one per mode")


if __name__ == "__main__":
    unittest.main()
