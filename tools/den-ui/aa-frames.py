"""Lists AzerothAdmin's window parts (names, groups, parents, sizes, anchors, English labels) as a Lua table.

    python3 -I tools/den-ui/aa-frames.py <AzerothAdmin folder> > tools/den-ui/azerothadmin-frames.lua

AzerothAdmin (github.com/superstyro/AzerothAdmin, the v24 + QoL build from github.com/Dandenas/coa-azerothadmin that
addons/README.md lists) is third-party GPL code we never copy into this repo. Its window is built by FrameLib calls
(FrameLib:BuildButton({ name = ..., setpoint = {...} })) and a few CreateFrame("EditBox", ...) blocks: this script
reads that source as text (it runs none of it) and writes down the facts the Leyline skin (WrathUnleashedUI
SkinGM.lua) is laid out against, so its test (stream/tests/test_wuui_gmskin.lua) and the picture
(tools/den-ui/gm-scene.lua) can build stand-in frames of the same names without the addon.
"""
import os
import re
import sys

KEYS = ("kind", "name", "group", "parent", "type", "inherits", "w", "h", "pos", "rel", "relPos", "x", "y", "text", "tex")
FILES = ("Start", "Tabs", "MiniMenu", "LookupButtons", "PopupFrames", "SmallPopupFrames", "SectionMain", "SectionTele",
         "SectionChar", "SectionTicket", "SectionMisc", "SectionServer", "SectionNpc", "SectionGO")


def read(path):
    with open(path, encoding="utf-8-sig", errors="replace") as h:
        return h.read()


def locale(root):
    s = read(os.path.join(root, "Locales", "enUS", "enUS.lua"))
    s = s[s.find('NewLocale("AzerothAdmin", "enUS"'):]
    out = {}
    for m in re.finditer(r'\[\s*"([^"]+)"\s*\]\s*=\s*"((?:[^"\\]|\\.)*)"', s):
        out.setdefault(m.group(1), m.group(2))
    return out


def block(s, i):
    depth = 0
    for j in range(i, len(s)):
        if s[j] == "{":
            depth += 1
        elif s[j] == "}":
            depth -= 1
            if depth == 0:
                return s[i:j + 1]
    return s[i:]


def top_level(b):
    out, depth = [], 0
    for ch in b[1:-1]:
        if ch == "{":
            depth += 1
        if depth == 0:
            out.append(ch)
        if ch == "}":
            depth -= 1
    return "".join(out)


def sub(b, key):
    m = re.search(r"\b" + key + r"\s*=\s*\{", b)
    return block(b, m.end() - 1) if m else ""


def val(b, key, loc):
    m = re.search(r"\b" + key + r'\s*=\s*("([^"]*)"|Locale\["([^"]+)"\]|-?[\d.]+|[A-Za-z_][\w.]*)', b)
    if not m:
        return None
    if m.group(2) is not None:
        return m.group(2)
    if m.group(3) is not None:
        return loc.get(m.group(3), m.group(3))
    return m.group(1)


def rows(root):
    loc = locale(root)
    out = []
    for f in FILES:
        s = read(os.path.join(root, "Frames", f"Frames_{f}.lua"))
        s = re.sub(r"--\[\[.*?\]\]", "", s, flags=re.S)
        s = re.sub(r"--[^\n]*", "", s)
        calls = [(m.start(), "lib", m) for m in re.finditer(r"FrameLib:Build(Frame|Button|FontString|Texture|EditBox)\(\s*\{", s)]
        calls += [(m.start(), "eb", m) for m in re.finditer(r'CreateFrame\("EditBox",\s*"(\w+)",\s*(\w+)\)', s)]
        for _, how, m in sorted(calls, key=lambda c: c[0]):
            if how == "lib":
                b = block(s, m.end() - 1)
                flat = top_level(b)
                sp, sz, tx = sub(b, "setpoint"), sub(b, "size"), sub(b, "texture")
                text = val(flat, "text", loc)
                out.append(dict(kind=m.group(1).lower(), name=val(flat, "name", loc), group=val(flat, "group", loc),
                                parent=val(flat, "parent", loc), type=val(flat, "type", loc), inherits=val(flat, "inherits", loc),
                                w=val(sz, "width", loc), h=val(sz, "height", loc), pos=val(sp, "pos", loc), rel=val(sp, "relTo", loc),
                                relPos=val(sp, "relPos", loc), x=val(sp, "offX", loc), y=val(sp, "offY", loc),
                                text=None if text and text.endswith("..") else text, tex=val(tx, "name", loc)))
            else:
                # an EditBox made by hand: its group, size and anchor are the next few lines
                tail = s[m.end():m.end() + 600]
                g = re.search(r'AddGroupFrame\("(\w+)"', tail)
                size = re.search(r"SetSize\(([-\d.]+),\s*([-\d.]+)\)", tail)
                pt = re.search(r'SetPoint\("(\w+)",\s*(\w+),\s*"(\w+)",\s*([-\d.]+),\s*([-\d.]+)\)', tail)
                out.append(dict(kind="editbox", name=m.group(1), group=g and g.group(1), parent=m.group(2), type="EditBox",
                                inherits=None, w=size and size.group(1), h=size and size.group(2), pos=pt and pt.group(1),
                                rel=pt and pt.group(2), relPos=pt and pt.group(3), x=pt and pt.group(4), y=pt and pt.group(5),
                                text=None, tex=None))
    return out


def lua(v):
    if v is None or v == "nil":
        return "nil"
    if re.fullmatch(r"-?\d+(\.\d+)?", str(v)):
        return str(v)
    return '"' + str(v).replace("\\", "\\\\").replace('"', '\\"') + '"'


def main():
    root = sys.argv[1]
    print("-- generated by tools/den-ui/aa-frames.py from AzerothAdmin v24's Frames/*.lua (read as text): names, groups,")
    print("-- parents, sizes, anchors and English labels of its window's parts, for the stand-ins in tools/den-ui/aa-stub.lua.")
    print("-- fields: " + ", ".join(KEYS))
    print("return {")
    for r in rows(root):
        print("  { " + ", ".join(lua(r[k]) for k in KEYS) + " },")
    print("}")


if __name__ == "__main__":
    main()
