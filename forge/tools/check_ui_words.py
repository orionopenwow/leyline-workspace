#!/usr/bin/env python3
# Part of Forge (GPL-3.0). Forge change, not in Noggit Red.
"""Fails when a word players must never see is in Forge's user-visible strings.

    python3 forge/tools/check_ui_words.py

Scans Qt Designer files (<string> texts) and the string literals of Forge's own C++ sources (not src/external, not
comments). File paths inside the game's archives (world/..., *.MPQ names) are code, not text, and are skipped.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WORDS = re.compile(r"world of warcraft|warcraft|lich ?king|blizzard|\bwow\b|wotlk", re.I)
SKIP = re.compile(r"^(world|textures|dbfilesclient|interface)[/\\]|\.mpq$|^\{locale\}", re.I)


def literals(text):
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    text = re.sub(r"(?m)//.*$", "", text)
    return re.findall(r'"((?:[^"\\\n]|\\.)*)"', text)


def main():
    bad = []
    src = ROOT / "src"
    for p in sorted(src.rglob("*")):
        if "external" in p.relative_to(src).parts or not p.is_file():
            continue
        if p.suffix == ".ui":
            strings = re.findall(r"<string[^>]*>([^<]*)</string>", p.read_text(encoding="utf-8", errors="replace"))
        elif p.suffix in (".cpp", ".h", ".hpp", ".inl"):
            strings = literals(p.read_text(encoding="utf-8", errors="replace"))
        else:
            continue
        for s in strings:
            if WORDS.search(s) and not SKIP.search(s.strip()):
                bad.append(f"{p.relative_to(ROOT)}: {s[:100]}")
    for line in bad:
        print(line)
    print(f"{len(bad)} user-visible strings with a game or publisher name")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
