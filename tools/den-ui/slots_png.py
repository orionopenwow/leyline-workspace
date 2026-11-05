#!/usr/bin/env python3
"""python3 tools/den-ui/slots_png.py <scenes dir> <WxH pixels> [out dir]

Draws tools/den-ui/slots.lua's JSON scenes (every visible region's rect in UI units, y up) as PNGs with Pillow: the
game's own frames as grey outlines with their names, each Den window filled dark with its own coloured edge, its text
where its letters are, and a red box wherever two Den windows overlap. A rough picture to look at, not the game's
look (tools/den-ui/render.cjs draws that)."""
import json
import os
import sys

from PIL import Image, ImageDraw, ImageFont

FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"
PALETTE = [(224, 164, 58), (87, 171, 224), (79, 209, 139), (139, 108, 240), (242, 193, 78), (255, 127, 118),
           (120, 220, 220), (220, 120, 200)]


def font(size):
    try:
        return ImageFont.truetype(FONT, max(6, int(size)))
    except OSError:
        return ImageFont.load_default()


def draw(scene, pw, ph, out):
    sw, sh = scene["screen"]["w"], scene["screen"]["h"]
    k = ph / sh  # pixels per UI unit

    def box(r):
        return (r["x"] * k, (sh - r["y"] - r["h"]) * k, (r["x"] + r["w"]) * k, (sh - r["y"]) * k)

    img = Image.new("RGB", (pw, ph), (58, 70, 40))
    d = ImageDraw.Draw(img, "RGBA")
    roots, colours = {}, {}
    for e in scene["regions"]:
        if e.get("stock"):
            d.rectangle(box(e), outline=(200, 200, 200, 200), width=1)
            d.text((e["x"] * k + 4, (sh - e["y"] - e["h"]) * k + 2), e["stock"], fill=(220, 220, 220), font=font(12))
            continue
        root = e.get("root") or ""
        if root.startswith("#") or e["alpha"] < 0.05:
            continue
        if e.get("name") == root:
            if e["w"] >= sw - 1 and e["h"] >= sh - 1:
                continue  # a frame round the whole screen (build mode's edge): not a window
            roots[root] = e
            colours.setdefault(root, PALETTE[len(colours) % len(PALETTE)])
    # the windows, then what is in them (in drawing order)
    for e in scene["regions"]:
        root = e.get("root") or ""
        if root not in roots or e["alpha"] < 0.05:
            continue
        c = colours[root]
        if e.get("name") == root:
            d.rectangle(box(e), fill=(10, 11, 14, 215), outline=c + (255,), width=2)
        elif e["kind"] in ("Button", "EditBox", "CheckButton", "Slider"):
            d.rectangle(box(e), fill=(26, 27, 32, 230), outline=(140, 140, 150, 160), width=1)
        elif e["kind"] == "FontString" and e.get("text"):
            size = (e.get("font") or 13) * k * 0.88  # DejaVu is wider than the game's font
            f = font(size)
            text = e["text"]
            for code in ("|r",):
                text = text.replace(code, "")
            while "|c" in text:
                i = text.index("|c")
                text = text[:i] + text[i + 10:]
            x0, y0, x1, y1 = box(e)
            bw = x1 - x0
            tc = tuple(int(v * 255) for v in (e.get("tcolor") or [0.92, 0.92, 0.94, 1])[:3])
            # as the game draws it: a box two lines tall or more wraps by words; one line ends in "..."
            lines = [text]
            if not e.get("autoW") and d.textlength(text, font=f) > bw + 1:
                if (y1 - y0) >= size * 1.9 and e.get("wrap") is not False:
                    lines, cur = [], ""
                    for word in text.split(" "):
                        t = (cur + " " + word).strip()
                        if d.textlength(t, font=f) > bw and cur:
                            lines.append(cur)
                            cur = word
                        else:
                            cur = t
                    lines.append(cur)
                else:
                    while text and d.textlength(text + "...", font=f) > bw:
                        text = text[:-1]
                    lines = [text.rstrip() + "..."]
            j = e.get("justify") or "CENTER"
            lh = size * 1.2
            y = y0 + max(0, (y1 - y0 - lh * len(lines)) / 2) if e.get("justifyV") != "TOP" else y0
            for line in lines:
                tw = d.textlength(line, font=f)
                if e.get("autoW") or j == "LEFT":
                    x = x0
                elif j == "RIGHT":
                    x = x1 - min(tw, bw)
                else:
                    x = x0 + max(0, (bw - tw) / 2)
                d.text((x, y), line, fill=tc, font=f)
                y += lh
    # the Den's windows on each other: red
    names = list(roots)
    bad = []
    for i, a in enumerate(names):
        for b in names[i + 1:]:
            ra, rb = roots[a], roots[b]
            ox = min(ra["x"] + ra["w"], rb["x"] + rb["w"]) - max(ra["x"], rb["x"])
            oy = min(ra["y"] + ra["h"], rb["y"] + rb["h"]) - max(ra["y"], rb["y"])
            if ox > 2 and oy > 2:
                bad.append(f"{a}/{b}")
                x = max(ra["x"], rb["x"])
                y = max(ra["y"], rb["y"])
                d.rectangle(box({"x": x, "y": y, "w": ox, "h": oy}), outline=(255, 40, 40, 255), width=3)
    for name, e in roots.items():
        x0, y0, _, _ = box(e)
        d.text((x0 + 2, y0 - 12 if y0 > 14 else y0 + 2), name, fill=colours[name], font=font(10))
    d.text((8, ph - 20), f'{scene["title"]}  ({scene["notes"]})  overlaps: {", ".join(bad) or "none"}',
           fill=(255, 255, 255), font=font(13))
    img.save(out)
    return bad


def main():
    src, px = sys.argv[1], sys.argv[2]
    out = sys.argv[3] if len(sys.argv) > 3 else src
    pw, ph = (int(v) for v in px.split("x"))
    os.makedirs(out, exist_ok=True)
    worst = 0
    for name in sorted(os.listdir(src)):
        if not name.endswith(".json"):
            continue
        with open(os.path.join(src, name), encoding="utf-8") as fh:
            scene = json.load(fh)
        bad = draw(scene, pw, ph, os.path.join(out, name[:-5] + ".png"))
        print(name, "overlaps:", ", ".join(bad) or "none")
        worst = max(worst, len(bad))
    sys.exit(1 if worst else 0)


if __name__ == "__main__":
    main()
