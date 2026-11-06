#!/usr/bin/env python3
"""The reel kit: its look follows the launcher's and the site's CSS, the cards are the right size and shape, every
script is valid and says nothing players must never see, and the edit really cuts takes into finished videos (small
synthetic takes made here with ffmpeg: a recorded one and a takes.cjs frames folder).

    python3 -I tools/reels/test_reels.py
"""
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)
import brand as B  # noqa: E402
import cards  # noqa: E402
import edit  # noqa: E402

FF = edit.tool("ffmpeg")


def read(rel):
    with open(os.path.join(ROOT, rel), encoding="utf-8") as f:
        return f.read()


def probe(path):
    r = subprocess.run([edit.tool("ffprobe"), "-v", "error", "-show_entries", "stream=codec_type,width,height,r_frame_rate:format=duration",
                        "-of", "json", path], capture_output=True, text=True)
    return json.loads(r.stdout)


class Look(unittest.TestCase):
    def test_colours_are_the_launchers_and_the_sites(self):
        style = read("launcher/ui/style.css")
        dark = style[style.index(":where(:root) {"):]
        dark = dark[:dark.index("}")]
        portal = read("stream/wrath_portal.py")
        for name, (src, token) in B.SOURCES.items():
            text = dark if src.endswith("style.css") else portal
            m = re.search(re.escape(token) + r"\s*:\s*(#[0-9a-fA-F]{6})", text)
            self.assertIsNotNone(m, "%s: %s not in %s" % (name, token, src))
            self.assertEqual(m.group(1).lower(), getattr(B, name).lower(), "%s follows %s %s" % (name, src, token))

    def test_fonts_and_art_are_the_repos(self):
        for p in (B.FONT_DISPLAY, B.FONT_BODY, B.FONT_BODY_BOLD, B.MARK, B.BACKDROP):
            self.assertTrue(os.path.isfile(p), p)
        f = B.font("display", 40)
        self.assertGreater(f.size, 0)
        name = " ".join(f.getname()) if hasattr(f, "getname") else ""
        if "Big Shoulders" not in name:
            self.skipTest("this Pillow can't read .woff2 (fell back to %s)" % name)

    def test_the_legal_line_is_the_only_brand_word(self):
        self.assertEqual(B.LEGAL, "Not affiliated with or endorsed by Blizzard Entertainment.")
        self.assertEqual(B.SITE, "leylinelauncher.com")
        self.assertEqual(B.FORMATS, {"reel": (1080, 1920), "wide": (1920, 1080)})


class Cards(unittest.TestCase):
    def test_each_card_is_the_frame_size(self):
        for fmt, size in B.FORMATS.items():
            self.assertEqual(cards.title_card(fmt, "Build your Den", "Your Den", "A line under it").size, size)
            self.assertEqual(cards.end_card(fmt, "A line").size, size)
            lt = cards.lower_third(fmt, "Pick a piece from the gallery", "and place it")
            self.assertEqual((lt.size, lt.mode), (size, "RGBA"))
            self.assertEqual(cards.draft_badge(fmt).size, size)
            self.assertEqual(cards.placeholder(fmt, "game-den-fly", "what it shows").size, size)

    def test_the_lower_third_covers_only_its_panel(self):
        for fmt, (w, h) in B.FORMATS.items():
            lt = cards.lower_third(fmt, "Click to pick it up and carry it")
            a = lt.getchannel("A")
            self.assertEqual(a.getpixel((5, 5)), 0, "the top corner stays clear")
            box = a.point(lambda v: 255 if v > 200 else 0).getbbox()
            self.assertIsNotNone(box)
            # under the picture on a reel (brand.STAGE), in the lower part of a wide frame; never in the bottom 10%
            self.assertGreater(box[1], h * (B.STAGE[fmt][1] + B.STAGE[fmt][3] - 0.05) if fmt == "reel" else h * 0.6)
            self.assertLess(box[3], h * 0.9 + 2)
            self.assertLess(box[2] - box[0], w * 0.95)

    def test_a_long_caption_wraps_and_shrinks_to_fit(self):
        lt = cards.lower_third("reel", "Or skip it: your own PC, host a realm, or help carry the network for everyone else")
        box = lt.getchannel("A").getbbox()
        self.assertLessEqual(box[2], 1080)
        self.assertGreaterEqual(box[0], 0)

    def test_the_end_card_says_the_address_in_the_sites_cyan(self):
        im = cards.end_card("wide")
        cyan = B.rgb(B.SITE_CYAN)
        hits = sum(1 for px in im.getdata() if all(abs(a - b) < 12 for a, b in zip(px, cyan)))
        self.assertGreater(hits, 500)

    def test_the_sting_is_one_second_and_the_mark_comes_in(self):
        for fmt in B.FORMATS:
            fr = cards.sting_frames(fmt)
            self.assertEqual(len(fr), int(B.STING_SECONDS * B.FPS))
            w, h = B.FORMATS[fmt]
            box = (w // 2 - w // 10, int(h * 0.3), w // 2 + w // 10, int(h * 0.55))

            def light(im):
                return sum(sum(px) for px in im.crop(box).resize((40, 40)).getdata())
            self.assertGreater(light(fr[-1]), light(fr[0]) * 1.2, "the mark is there at the end, not at the start")


class Scripts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.takes = edit.catalog()
        cls.scripts = edit.all_scripts()

    def test_every_script_is_valid(self):
        problems = []
        for s in self.scripts.values():
            problems += edit.validate(s, self.takes, self.scripts)
        self.assertEqual(problems, [])

    def test_the_reels_the_owner_asked_for_are_there(self):
        # owner 4 Nov: reels of the features, flow videos (host a server, open the Den, open any client), a launch video
        for sid in ("login-to-playing", "den-carry", "den-fly", "realm-switch", "single-player", "mods-hd", "phone", "queue",
                    "flow-host", "flow-den", "flow-any-client", "launch-v2"):
            self.assertIn(sid, self.scripts)
        self.assertEqual(self.scripts["launch-v2"]["kind"], "launch")
        reels_in_launch = {p["reel"] for p in self.scripts["launch-v2"]["parts"]}
        self.assertGreaterEqual(len(reels_in_launch), 8, "the launch video is cut from the reels")

    def test_takes_say_how_they_are_recorded(self):
        cjs = read("tools/reels/takes.cjs")
        scenes = set(re.findall(r"^  '?([a-z-]+)'?: \{ (?:o|site|run)", cjs, re.M))
        rec = read("tools/reels/record.py")
        for tid, t in self.takes.items():
            self.assertIn(t["source"], edit.SOURCES, tid)
            self.assertTrue(t.get("what"), tid)
            self.assertIsNotNone(re.fullmatch(r"[a-z0-9-]+", tid), tid)
            if t["source"] == "launcher-fake":
                self.assertIn(t["scene"], scenes, "%s: takes.cjs has scene %s" % (tid, t["scene"]))
            elif t["source"] in ("site", "phone") and t.get("offline"):
                self.assertIn(t["flow"], scenes, "%s: takes.cjs has scene %s" % (tid, t["flow"]))
            elif t["source"] == "game":
                self.assertIn('flow == "%s"' % t["flow"], rec, "%s: record.py plays flow %s" % (tid, t["flow"]))
            elif t["source"] == "hand":
                self.assertTrue(t.get("prompt"), tid)
                self.assertIn(t.get("window"), ("screen", "game", "launcher"), tid)
            for label in (t.get("guess") or {}):
                self.assertIn(label, t["marks"], tid)

    def test_the_marks_scripts_use_are_the_ones_takes_make(self):
        cjs = read("tools/reels/takes.cjs")
        rec = read("tools/reels/record.py")
        made = set(re.findall(r"rec\.mark\('([a-z-]+)'\)", cjs)) | set(re.findall(r'rec\.mark\("([a-z-]+)"\)', rec))
        for tid, t in self.takes.items():
            for m in t.get("marks", []):
                self.assertTrue(m in made or m in (t.get("guess") or {}), "%s: nothing makes mark %s" % (tid, m))

    def test_words_on_screen_follow_the_rules(self):
        for s in self.scripts.values():
            for w in edit.words(s):
                self.assertEqual(edit.text_problems(w), [], "%s: %s" % (s["id"], w))
        for t in self.takes.values():
            for k in ("what", "prompt"):
                self.assertNotRegex(t.get(k, "").lower(), r"\b(world of warcraft|wow|warcraft|lich king|blizzard)\b")

    def test_the_owners_words_are_drafts(self):
        # taglines and end lines are his to write: each script has at least one TODO(owner), so its videos say DRAFT
        for s in self.scripts.values():
            self.assertTrue(edit.is_draft(s, self.scripts), s["id"])

    def test_the_rules_catch_what_they_should(self):
        self.assertTrue(edit.text_problems("Play WoW free"))
        self.assertTrue(edit.text_problems("TODO(owner): the Lich King awaits"))
        self.assertTrue(edit.text_problems("Add your 3.3.5a folder here"))
        self.assertTrue(edit.text_problems("Sign in with your Hero Key"))
        self.assertEqual(edit.text_problems("Log in with your Hero Key"), [])
        self.assertEqual(edit.shown("TODO(owner): Free, and yours."), "Free, and yours.")

    def test_a_bad_script_is_refused(self):
        bad = {"id": "Bad Id", "kind": "short", "title": "", "formats": ["square"],
               "shots": [{"take": "nope", "in": "x", "out": ["no-such-mark", 1], "speed": 9, "fit": "stretch"}], "gif": {"shots": [4]}}
        p = " | ".join(edit.validate(bad, self.takes))
        for want in ("id must be", "kind is", "no title", "unknown format", "isn't in takes.json", "seconds or [mark, offset]",
                     "speed between", "fit is", "the GIF names shot"):
            self.assertIn(want, p)
        p = " | ".join(edit.validate(dict(bad, id="x-y", kind="reel", title="T", formats=["wide"], gif=None,
                                          shots=[{"take": "lf-login", "in": 0, "out": ["nope", 0]}]), self.takes))
        self.assertIn("has no mark nope", p)
        p = " | ".join(edit.validate({"id": "l-v", "kind": "launch", "title": "T", "parts": [{"reel": "den-fly", "shots": [99]}, {"reel": "zz"}]},
                                     self.takes, self.scripts))
        self.assertIn("den-fly has no shot 99", p)
        self.assertIn("unknown reel zz", p)

    def test_mark_references(self):
        meta = {"seconds": 10.0, "marks": {"carry": 4.0}}
        self.assertEqual(edit.at(2, meta), 2.0)
        self.assertEqual(edit.at(-1.5, meta), 8.5)
        self.assertEqual(edit.at(["carry", -1], meta), 3.0)
        self.assertEqual(edit.at(["carry", 99], meta), 10.0)
        with self.assertRaises(edit.ScriptError):
            edit.at(["drop", 0], meta)

    def test_the_launch_video_takes_its_shots_from_the_reels(self):
        got = edit.shots_of(self.scripts["launch-v2"], self.scripts)
        self.assertEqual(got[0], ("login-to-playing", self.scripts["login-to-playing"]["shots"][0]))
        self.assertEqual(len(got), sum(len(p.get("shots") or self.scripts[p["reel"]]["shots"]) for p in self.scripts["launch-v2"]["parts"]))


@unittest.skipUnless(FF and edit.tool("ffprobe"), "ffmpeg isn't installed here")
class Cut(unittest.TestCase):
    """The edit end to end on small synthetic takes: one recorded (an MP4 with its marks), one from takes.cjs (frames
    and their times in slow motion)."""

    @classmethod
    def setUpClass(cls):
        from PIL import Image, ImageDraw
        cls.tmp = tempfile.mkdtemp(prefix="reels-test-")
        cls.takes_dir = os.path.join(cls.tmp, "takes")
        os.makedirs(cls.takes_dir)
        subprocess.run([FF, "-y", "-v", "error", "-f", "lavfi", "-i", "testsrc2=size=640x360:rate=30:duration=6",
                        "-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p", os.path.join(cls.takes_dir, "t-rec.mp4")], check=True)
        with open(os.path.join(cls.takes_dir, "t-rec.json"), "w") as f:
            json.dump({"seconds": 6.0, "marks": {"go": 2.0}}, f)
        # a takes.cjs folder: 20 frames over 4 s of slow motion 2 (2 s of take), a mark at the middle
        d = os.path.join(cls.takes_dir, "t-frames")
        os.makedirs(d)
        frames = []
        for i in range(20):
            im = Image.new("RGB", (480, 720), (20 + i * 10, 40, 90))
            ImageDraw.Draw(im).rectangle((i * 20, 300, i * 20 + 60, 360), fill=(240, 240, 240))
            im.save(os.path.join(d, "%05d.jpg" % i))
            frames.append({"file": "%05d.jpg" % i, "t": 100.0 + i * 0.2})
        with open(os.path.join(d, "frames.json"), "w") as f:
            json.dump({"t0": 100.0, "t1": 104.0, "frames": frames, "marks": [{"label": "mid", "t": 102.0}], "scale": 1, "slow": 2, "moves": [], "clicks": []}, f)
        cls.cat = {"t-rec": {"source": "game", "flow": "x", "what": "a recorded take", "marks": ["go"]},
                   "t-frames": {"source": "launcher-fake", "scene": "x", "what": "a launcher take", "marks": ["mid"]},
                   "t-missing": {"source": "hand", "what": "not recorded", "prompt": "p", "window": "screen", "marks": []}}
        cls.script = {"id": "t-reel", "kind": "reel", "formats": ["reel", "wide"], "title": "Test reel", "kicker": "Test",
                      "sub": "TODO(owner): a line", "end_line": "The end",
                      "shots": [{"take": "t-rec", "in": ["go", -1], "out": ["go", 1], "caption": "First caption"},
                                {"take": "t-missing", "in": 0, "out": 2, "caption": "Missing"},
                                {"take": "t-frames", "in": 0, "out": ["mid", 0.5], "caption": "Second caption", "speed": 0.5}],
                      "gif": {"format": "wide", "shots": [0, 2], "max_seconds": 4}}
        cls.out = os.path.join(cls.tmp, "out")
        cls.result = edit.make(cls.script, {"t-reel": cls.script}, cls.cat, cls.takes_dir, cls.out, fast=True, log=lambda *a: None)

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.tmp, ignore_errors=True)

    def test_frames_become_a_steady_take(self):
        mp4, meta = edit.find_take(self.takes_dir, "t-frames")
        self.assertTrue(mp4.endswith("t-frames.mp4"))
        self.assertAlmostEqual(meta["seconds"], 2.0, places=2)
        self.assertAlmostEqual(meta["marks"]["mid"], 1.0, places=2)
        p = probe(mp4)
        self.assertAlmostEqual(float(p["format"]["duration"]), 2.0, delta=0.15)
        self.assertEqual(p["streams"][0]["r_frame_rate"], "30/1")

    def test_both_formats_their_size_length_and_sound(self):
        self.assertEqual(self.result["missing"], ["t-missing"])
        self.assertTrue(self.result["draft"])
        # sting + title + 2 s (recorded) + 3 s (frames: 1.5 s of take at half speed) + end, less the cross-fades
        want = B.STING_SECONDS + B.TITLE_SECONDS + 2.0 + 3.0 + B.END_SECONDS - 4 * B.XFADE
        for fmt, (w, h) in B.FORMATS.items():
            p = probe(os.path.join(self.out, "t-reel", "t-reel-%s.mp4" % fmt))
            v = [s for s in p["streams"] if s["codec_type"] == "video"][0]
            self.assertEqual((v["width"], v["height"]), (w, h))
            self.assertEqual(v["r_frame_rate"], "30/1")
            self.assertTrue([s for s in p["streams"] if s["codec_type"] == "audio"], "a silent track for the apps")
            self.assertAlmostEqual(float(p["format"]["duration"]), want, delta=0.3)
            self.assertTrue(os.path.isfile(os.path.join(self.out, "t-reel", "t-reel-%s-sheet.jpg" % fmt)))

    def test_captions_and_the_gif(self):
        srt = open(os.path.join(self.out, "t-reel", "t-reel-wide.srt"), encoding="utf-8").read()
        self.assertIn("First caption", srt)
        self.assertIn("Second caption", srt)
        self.assertNotIn("Missing", srt, "a take not recorded is left out")
        self.assertIn("00:00:02,", srt.split("\n")[1], "the first caption comes after the sting and the title")
        g = os.path.join(self.out, "t-reel", "t-reel.gif")
        self.assertTrue(os.path.isfile(g))
        from PIL import Image
        im = Image.open(g)
        self.assertEqual(im.width, 640)
        self.assertGreater(getattr(im, "n_frames", 1), 20)
        self.assertLess(os.path.getsize(g), 10 * 1024 * 1024)

    def test_the_draft_tag_and_final(self):
        from PIL import Image
        sheet = Image.open(os.path.join(self.out, "t-reel", "t-reel-wide-sheet.jpg")).convert("RGB")
        corner = sheet.crop((4, 4, 120, 30)).getdata()   # the first tile's top-left corner: the red DRAFT tag
        red = sum(1 for (r, g, b) in corner if r > 170 and g < 120 and b < 120)
        self.assertGreater(red, 100)
        with self.assertRaises(edit.ScriptError):
            edit.make(self.script, {}, self.cat, self.takes_dir, self.out, final=True, fast=True, log=lambda *a: None)

    def test_placeholders_show_what_is_missing(self):
        s = dict(self.script, id="t-ph", formats=["wide"], gif=None)
        r = edit.make(s, {}, self.cat, self.takes_dir, self.out, placeholders=True, fast=True, log=lambda *a: None)
        p = probe(os.path.join(self.out, "t-ph", "t-ph-wide.mp4"))
        self.assertAlmostEqual(float(p["format"]["duration"]),
                               B.STING_SECONDS + B.TITLE_SECONDS + 2.0 + 2.0 + 3.0 + B.END_SECONDS - 5 * B.XFADE, delta=0.3)
        self.assertEqual(r["missing"], ["t-missing"])

    def test_nothing_recorded_cuts_nothing(self):
        s = dict(self.script, id="t-none", shots=[self.script["shots"][1]], gif=None)
        r = edit.make(s, {}, self.cat, self.takes_dir, self.out, fast=True, log=lambda *a: None)
        self.assertEqual(r["files"], [])
        self.assertFalse(os.path.isdir(os.path.join(self.out, "t-none")) and os.listdir(os.path.join(self.out, "t-none")))


class GrowthDocs(unittest.TestCase):
    """docs/growth: drafts only; the standing rules for words and links; the guide series as the owner asked for it."""
    DOCS = ("README.md", "reddit-guides.md", "weekly-posts.md", "discord-onboarding.md", "host-recruiting.md", "seeded-season.md",
            "reddit-lessons.md")
    LEGAL = "Not affiliated with or endorsed by Blizzard Entertainment."

    def text(self, name):
        return read("docs/growth/" + name)

    def test_words_and_links(self):
        for name in self.DOCS:
            t = self.text(name).replace('"Log in" not "Sign in"', "").replace(self.LEGAL, "")   # the rule itself; the legal line
            for bad in (r"(?i)\b(world of warcraft|wow|warcraft|lich king|blizzard|turtle)\b", r"(?i)\bsign(ed)?[ -]in\b|\bsign up\b",
                        r"(?i)patreon|ko-fi|paypal|donat|\btip\b"):
                m = re.search(bad, t)
                self.assertIsNone(m, "%s: %r" % (name, m and t[max(0, m.start() - 40):m.end() + 40]))
            for url in re.findall(r"https?://[^\s)`>\]]+", t):
                host = re.match(r"https?://([^/]+)", url).group(1)
                self.assertIn(host, ("leylinelauncher.com", "discord.gg", "www.reddit.com", "reddit.com", "projectreforged.github.io"),
                              "%s: %s" % (name, url))
            self.assertIn("TODO(owner)", t, name + ": the owner's words are his")

    def test_twelve_guides_of_five_steps(self):
        t = self.text("reddit-guides.md")
        eps = re.split(r"\n## \d+\. ", t)[1:]
        self.assertEqual(len(eps), 12)
        gifs = {os.path.splitext(n)[0] for n in os.listdir(os.path.join(HERE, "scripts"))}
        for i, ep in enumerate(eps, 1):
            steps = re.findall(r"^([1-9])\. ", ep, re.M)
            self.assertEqual(steps, ["1", "2", "3", "4", "5"], "episode %d" % i)
            self.assertIn("**Stuck?**", ep, "episode %d" % i)
            self.assertIn("- **Title:**", ep)
            self.assertIn("TODO(owner)", ep.split("- **Intro:**")[1].split("\n")[0], "episode %d: the intro is his" % i)
            gif = ep.split("- **GIF:**")[1].split("\n")[0]
            named = re.findall(r"`([a-z0-9-]+)\.gif`", gif)
            self.assertTrue(named or "TODO" in gif, "episode %d names its GIF" % i)
            for g in named:
                self.assertIn(g, gifs, "episode %d: %s.gif comes from a reel script" % (i, g))

    # The lessons (reddit-lessons.md): stricter than the other docs, because each body is pasted as it is.
    LESSON_BANNED = (
        ("a game or publisher name", r"(?i)world of warcraft|warcraft|lich king|blizzard|\bwow|wow\b|turtle"),
        ("a client version number", r"\b\d+\.\d+\.\d+[a-z]?\b|\b(1\.12|2\.4|3\.3)\b|\b12340\b"),
        ("money", r"(?i)patreon|ko-?fi|paypal|venmo|cash ?app|donat|\btips?\b|buy me a|support us|bitcoin|crypto|[$\u20ac\u00a3]"),
        ("the wrong login words", r"(?i)\bsign(ed)?[ -]?(in|up)\b"),
        ("a client from elsewhere", r"(?i)decompil|leaked|\brepack|wowdecomp|waystone"),
        ("the publisher's live game", r"(?i)\bforever\b"),
    )
    LESSON_LINKS = ("https://leylinelauncher.com", "https://github.com/OWNER/Leyline", "https://discord.gg/RfChVr8J7T")

    def lesson_problems(self, t):
        """What breaks the lesson rules in t (the whole file or one body), as (what, text around it)."""
        out = []
        t = t.replace(self.LEGAL, "")
        # The sub's own name (a note for the owner, never in a body) and the GitHub owner's name are not words players read.
        t = re.sub(r"- \*\*Subreddit:\*\*.*?(?=\n- \*\*Title:\*\*)", "", t, flags=re.S)
        t = t.replace("github.com/orionopenwow/", "github.com/OWNER/")
        for what, rx in self.LESSON_BANNED:
            for m in re.finditer(rx, t):
                out.append((what, t[max(0, m.start() - 30):m.end() + 30]))
        for url in re.findall(r"https?://[^\s)`>\]]+", t):
            url = url.rstrip(".,;:")
            ok = any(url == a or url.startswith(a + "/") or url.startswith(a + "?") or url.startswith(a + "#")
                     for a in self.LESSON_LINKS)
            if not ok or url.startswith("https://github.com/OWNER/Leyline-"):
                out.append(("a link off the list", url))
        for host in re.findall(r"(?<![\w/.@-])((?:[a-z0-9-]+\.)+(?:com|org|net|io|gg|app|dev|co|me|tv))\b", t):
            if host not in ("leylinelauncher.com", "github.com", "discord.gg"):
                out.append(("a site off the list", host))
        return out

    def lessons(self):
        t = self.text("reddit-lessons.md")
        return re.split(r"\n## (?=\d+\. )", t)[1:]

    def test_lessons_follow_the_rules(self):
        t = self.text("reddit-lessons.md")
        self.assertEqual(self.lesson_problems(t), [])
        posts = self.lessons()
        self.assertTrue(8 <= len(posts) <= 10, len(posts))
        for p in posts:
            head = p.split("\n")[0]
            for field in ("- **Subreddit:**", "- **Title:**", "- **Body:**", "- **Call to action:**", "- **Check before posting:**"):
                self.assertIn(field, p, "%s: %s" % (head, field))
            body = p.split("```text\n", 1)[1].split("```", 1)[0]
            n = len(body.split())
            self.assertTrue(150 <= n <= 300, "%s: the body is %d words" % (head, n))
            self.assertTrue(body.lstrip().startswith("TODO(owner):"), head + ": the opening line is the owner's")
            self.assertEqual(body.rstrip().split("\n")[-1], self.LEGAL, head + ": ends with the legal line")
            self.assertEqual(body.count("Blizzard"), 1, head + ": the legal line is the only brand word")
            self.assertIn("https://discord.gg/RfChVr8J7T", body, head)
            self.assertRegex(body, r"https://leylinelauncher\.com/", head + ": links its full guide")
            cta = p.split("- **Call to action:**")[1].split("\n")[0].strip()
            self.assertTrue(cta and "\n" not in cta, head + ": a one-line call to action")
            checks = re.findall(r"^  - ", p.split("- **Check before posting:**")[1], re.M)
            self.assertGreaterEqual(len(checks), 3, head + ": things to check live")
            self.assertIn("rules", p.split("- **Subreddit:**")[1].split("- **Title:**")[0], head + ": the sub's rules risk")
        titles = " ".join(p.split("\n")[0].lower() + " " + p.split("- **Title:**")[1].split("\n")[0].lower() for p in posts)
        for topic in ("own realm", "24/7", "friends-only", "be a node", "modding", "den", "llm agents", "scaling",
                      "browser", "phones", "boss fight"):
            self.assertIn(topic, titles, "a lesson on " + topic)
        self.assertIn("when live", posts[-1].split("\n")[0].lower(), "boss fights are marked until they are live")
        guides = self.text("reddit-guides.md")
        for p in posts:
            title = p.split("- **Title:**")[1].split("\n")[0].strip()
            self.assertNotIn(title, guides, "a lesson is not a copy of a guide episode")
        self.assertIn("reddit-lessons.md", self.text("README.md"))

    def test_the_lesson_rules_catch_what_they_should(self):
        bad = ["Our realm for World of Warcraft fans", "a WoW server", "on the 3.3.5a client", "the 1.12 client", "build 12340",
               "Patreon here", "leave a tip", "costs $5", "sign up today", "Sign in first", "a decompiled client", "a repack",
               "like Forever", "- **Title:** r/wowservers", "github.com/someone/wow", "https://example.com/x", "see docker.com", "https://github.com/orionopenwow/leyline-workspace",
               "https://github.com/orionopenwow/Leyline-Launcher", "https://discord.gg/other"]
        for b in bad:
            self.assertTrue(self.lesson_problems(b), b)
        good = ["Not affiliated with or endorsed by Blizzard Entertainment.", "1.5x experience, rested twice as fast",
                "https://leylinelauncher.com/wrath/docs/play", "https://github.com/orionopenwow/Leyline/releases",
                "https://discord.gg/RfChVr8J7T", "about 4 GB, 128 connections, 512 KB/s", "Log in with a Hero Key",
                "- **Subreddit:** r/wowservers (check its rules)\n- **Title:** a lesson"]
        for g in good:
            self.assertEqual(self.lesson_problems(g), [], g)

    def test_every_weekly_template(self):
        t = self.text("weekly-posts.md")
        for h in ("## 1. Patch notes", "## 2. Feature reel", "## 3. Guide", "## 4. Den or host spotlight", "## 5. State of Leyline"):
            self.assertIn(h, t)
        self.assertIn("never the", t.lower().replace("never\nthe", "never the").replace("**never**", "never"))


class Main(unittest.TestCase):
    def test_check_and_list(self):
        self.assertEqual(edit.main(["--check"]), 0)
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(edit.main(["--list", "--takes", d]), 0)
            self.assertEqual(edit.main(["no-such-reel", "--takes", d, "--out", d]), 2)

    def test_takes_and_out_stay_out_of_git(self):
        ignore = read(".gitignore")
        self.assertIn("/tools/reels/takes/", ignore)
        self.assertIn("/tools/reels/out/", ignore)


if __name__ == "__main__":
    unittest.main()
