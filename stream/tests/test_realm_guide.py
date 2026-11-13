#!/usr/bin/env python3
"""The Wrath Unleashed guide: one source (docs/realm-guide/wrath-unleashed.json), two copies kept equal.

    python3 stream/tests/test_realm_guide.py
"""
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
passed = failed = 0


def check(name, ok):
    global passed, failed
    if ok:
        passed += 1
        print("PASS", name)
    else:
        failed += 1
        print("FAIL", name)


gen = os.path.join(ROOT, "tools", "realm-guide", "make_guide.py")
r = subprocess.run([sys.executable, gen, "--check"], capture_output=True, text=True)
check("both copies are up to date (make_guide.py --check)", r.returncode == 0)

d = json.load(open(os.path.join(ROOT, "docs", "realm-guide", "wrath-unleashed.json"), encoding="utf-8"))
check("eight or more pages, each with lines", len(d["pages"]) >= 8 and all(p["lines"] for p in d["pages"]))
keys = [p["key"] for p in d["pages"]]
check("page keys are unique and not the window's own (commands, more)", len(set(keys)) == len(keys)
      and "commands" not in keys and "more" not in keys)

style = open(os.path.join(ROOT, "stream", "fui-web", "Interface", "AddOns", "WrathUnleashedUI", "Style.lua"), encoding="utf-8").read()
icons = set(re.findall(r'"(\w+)"', style[style.index("S.ICONS = {"):style.index("local iconSet")]))
check("every page's icon is one the addon has", all(p["icon"] in icons for p in d["pages"]))

# the guides of the worlds on the player's own PC (Single player, the original world)
for name in ("single-player", "original-world"):
    g = json.load(open(os.path.join(ROOT, "docs", "realm-guide", name + ".json"), encoding="utf-8"))
    ks = [p["key"] for p in g["pages"]]
    check(f"{name}: four or more pages with lines, unique keys", len(g["pages"]) >= 4 and all(p["lines"] for p in g["pages"])
          and len(set(ks)) == len(ks) and "commands" not in ks and "more" not in ks)
    check(f"{name}: every icon is one the addon has", all(p["icon"] in icons for p in g["pages"]))
    text = " ".join(" ".join(p["lines"]) + " " + " ".join(c for cw in p["commands"] for c in cw) for p in g["pages"])
    check(f"{name}: none of the official realm's rules (.hub, 3x, boost to 80, Discord, Death Knights)",
          not re.search(r"\.hub|\b3x\b|boost to 80|discord|death knight", text, re.I))
    check(f"{name}: the Bots window, /comp, /together, offline and Restore", all(w in text for w in ("Ctrl-B", "/comp", "/together", "offline", "Restore")))
    check(f"{name}: a friend needs no Hero Key", "No Hero Key" in text)

text = json.dumps(d).lower()
check("no brand words", not re.search(r"world of warcraft|warcraft|lich king|blizzard|\bwow\b", text))
check("no bots: they are off on the official realm", ".recruit" not in text and "/comp" not in text)

sys.path.insert(0, os.path.join(ROOT, "stream"))
import importlib.util
spec = importlib.util.spec_from_file_location("wu_docs_guide_test", os.path.join(ROOT, "stream", "docs.py"))
docs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(docs)
check("the site lists the notes page", "wrath-unleashed" in dict(docs.PAGES))
status, body = docs.page("/wrath/docs/wrath-unleashed")
body = body.decode("utf-8") if isinstance(body, bytes) else body
check("the notes page renders, with its sections and commands", status == 200 and "Wrath Unleashed notes" in body
      and "Duels and PvP" in body and ".solo" in body)

toc = open(os.path.join(ROOT, "stream", "fui-web", "Interface", "AddOns", "WrathUnleashedUI", "WrathUnleashedUI.toc"), encoding="utf-8").read().split()
check("the addon loads the data before the window", "RealmGuideData.lua" in toc and "RealmGuide.lua" in toc
      and toc.index("RealmGuideData.lua") < toc.index("RealmGuide.lua") and toc.index("Style.lua") < toc.index("RealmGuide.lua"))

print(f"\n{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
