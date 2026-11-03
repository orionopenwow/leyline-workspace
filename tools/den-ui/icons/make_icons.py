"""The Den editor's own icons (owner, 3 Nov 10:36: "Don't use the game's icons for the UI. Use relative, sense-making,
minimalistic icons for the building UI"): flat white line icons on transparent, drawn here as vector shapes at 4x and
downsampled, so they are consistent and ours. The addon tints them (Texture:SetVertexColor: white, gold when on).

Written as the 3.3.5a client reads a texture from an addon folder: uncompressed 32-bit TGA (image type 2, 8 alpha
bits), power-of-two sizes (64 x 64), rows stored bottom-up (the TGA default origin, descriptor 0x08). Loaded as
Interface\\AddOns\\WrathUnleashedUI\\Media\\Icons\\<name> (no extension).

    python3 -I tools/den-ui/icons/make_icons.py      (from the repo root; needs Pillow)
"""
import math
import os
import struct
import sys

from PIL import Image, ImageDraw

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
OUT = os.path.join(ROOT, "stream", "fui-web", "Interface", "AddOns", "WrathUnleashedUI", "Media", "Icons")
SIZE, K = 64, 4
N = SIZE * K          # the drawing's size (256)
LW = 18               # a line's width at 4x (4.5 px at 64)
W = (255, 255, 255, 255)


def P(x, y):
    """A point in 0..64 icon units -> the 4x drawing."""
    return (x * K, y * K)


class Pen:
    def __init__(self):
        self.im = Image.new("RGBA", (N, N), (255, 255, 255, 0))
        self.d = ImageDraw.Draw(self.im)

    def line(self, *pts, w=LW, close=False):
        pts = [P(*p) for p in pts]
        if close:
            pts = pts + [pts[0]]
        self.d.line(pts, fill=W, width=w, joint="curve")
        r = w / 2 - 0.5
        for x, y in (pts if not close else pts[:-1]):   # round ends and joints
            self.d.ellipse((x - r, y - r, x + r, y + r), fill=W)

    def rect(self, x0, y0, x1, y1, w=LW, r=6, fill=False):
        if fill:
            self.d.rounded_rectangle((*P(x0, y0), *P(x1, y1)), radius=r * K, fill=W)
        else:
            self.d.rounded_rectangle((*P(x0, y0), *P(x1, y1)), radius=r * K, outline=W, width=w)

    def circle(self, cx, cy, r, w=LW, fill=False):
        box = (*P(cx - r, cy - r), *P(cx + r, cy + r))
        if fill:
            self.d.ellipse(box, fill=W)
        else:
            self.d.ellipse(box, outline=W, width=w)

    def arc(self, cx, cy, r, a0, a1, w=LW):
        self.d.arc((*P(cx - r, cy - r), *P(cx + r, cy + r)), a0, a1, fill=W, width=w)
        for a in (a0, a1):
            x, y = cx + r * math.cos(math.radians(a)), cy + r * math.sin(math.radians(a))
            self.dot(x, y, LW / K / 2 - 0.1)

    def dot(self, x, y, r=3):
        self.circle(x, y, r, fill=True)

    def poly(self, *pts, fill=True):
        self.d.polygon([P(*p) for p in pts], fill=W if fill else None, outline=None if fill else W)

    def head(self, x, y, ang, s=9):
        """An arrow head at (x, y) pointing at `ang` degrees."""
        a = math.radians(ang)
        l, r = a + math.radians(150), a - math.radians(150)
        self.poly((x + s * 0.3 * math.cos(a), y + s * 0.3 * math.sin(a)), (x + s * math.cos(l), y + s * math.sin(l)),
                  (x + s * math.cos(r), y + s * math.sin(r)))


ICONS = {}


def icon(name):
    def reg(fn):
        ICONS[name] = fn
        return fn
    return reg


# ---------------------------------------------------------------- tools
@icon("select")
def _(p):   # the pointer arrow
    p.poly((16, 8), (16, 50), (27, 40), (35, 56), (42, 53), (34, 37), (49, 37))


@icon("place")
def _(p):   # a box with a down arrow onto it
    p.line((14, 30), (32, 22), (50, 30), (32, 38), close=True)
    p.line((14, 30), (14, 46), (32, 55), (50, 46), (50, 30))
    p.line((32, 38), (32, 55))
    p.line((32, 4), (32, 15))
    p.head(32, 17, 90, 8)


@icon("move")
def _(p):   # the four-way cross of arrows
    p.line((32, 10), (32, 54))
    p.line((10, 32), (54, 32))
    for x, y, a in ((32, 7, -90), (32, 57, 90), (7, 32, 180), (57, 32, 0)):
        p.head(x, y, a, 10)


@icon("rotate")
def _(p):   # an open arc with an arrow
    p.arc(32, 34, 19, 200, 470 - 360 + 360)
    p.head(50, 20, -60, 11)


@icon("scale")
def _(p):   # two corners and a diagonal
    p.line((10, 24), (10, 10), (24, 10))
    p.line((54, 40), (54, 54), (40, 54))
    p.line((16, 16), (48, 48))


@icon("path")
def _(p):   # dots along a curve
    for t in range(5):
        x = 10 + t * 11
        y = 46 - 26 * math.sin(t / 4 * math.pi * 0.9)
        p.dot(x, y, 4.5)


@icon("stamp")
def _(p):   # a rubber stamp
    p.rect(24, 8, 40, 26, r=4)
    p.line((30, 26), (30, 36))
    p.line((34, 26), (34, 36))
    p.rect(12, 36, 52, 46, r=3)
    p.line((10, 55), (54, 55))


@icon("ground")
def _(p):   # hills
    p.line((6, 50), (20, 28), (30, 40), (40, 22), (58, 50))
    p.line((6, 54), (58, 54))


@icon("dojo")
def _(p):   # a target
    p.circle(32, 32, 22)
    p.circle(32, 32, 11)
    p.dot(32, 32, 4)


@icon("undo")
def _(p):
    p.arc(34, 36, 16, 180, 450)
    p.line((18, 36), (18, 37))
    p.head(18, 40, 90, 10)


@icon("redo")
def _(p):
    p.arc(30, 36, 16, 90, 360)
    p.head(46, 40, 90, 10)


@icon("snap")
def _(p):   # a magnet
    p.arc(32, 30, 16, 180, 360)
    p.line((16, 30), (16, 50))
    p.line((48, 30), (48, 50))
    p.line((10, 46), (22, 46))
    p.line((42, 46), (54, 46))


@icon("grid")
def _(p):
    p.rect(10, 10, 54, 54, r=4)
    p.line((25, 10), (25, 54))
    p.line((39, 10), (39, 54))
    p.line((10, 25), (54, 25))
    p.line((10, 39), (54, 39))


@icon("files")
def _(p):   # a page with a folded corner
    p.line((16, 8), (40, 8), (50, 18), (50, 56), (16, 56), close=True)
    p.line((40, 8), (40, 18), (50, 18))
    p.line((24, 32), (42, 32))
    p.line((24, 42), (42, 42))


@icon("invite")
def _(p):   # a person and a plus
    p.circle(24, 22, 9)
    p.arc(24, 54, 17, 200, 340)
    p.line((48, 22), (48, 42))
    p.line((38, 32), (58, 32))


@icon("menu")
def _(p):
    for y in (18, 32, 46):
        p.line((12, y), (52, y))


@icon("help")
def _(p):
    p.arc(32, 22, 11, 180, 400)
    p.line((40, 29), (32, 36), (32, 41))
    p.dot(32, 52, 4)


@icon("close")
def _(p):
    p.line((16, 16), (48, 48))
    p.line((48, 16), (16, 48))


@icon("play")
def _(p):
    p.poly((20, 12), (20, 52), (52, 32))


@icon("build")
def _(p):   # a hammer
    p.line((20, 50), (40, 24))
    p.line((26, 14), (44, 30), w=26)


@icon("class")
def _(p):   # a shield
    p.line((32, 8), (52, 16), (50, 36), (32, 56), (14, 36), (12, 16), close=True)
    p.line((32, 16), (32, 46))


@icon("lock")
def _(p):
    p.rect(14, 28, 50, 56, r=5)
    p.arc(32, 26, 11, 180, 360)
    p.line((21, 26), (21, 28))
    p.line((43, 26), (43, 28))
    p.dot(32, 42, 4)


@icon("duplicate")
def _(p):
    p.rect(8, 8, 38, 38, r=5)
    p.rect(26, 26, 56, 56, r=5)


@icon("delete")
def _(p):   # a bin
    p.line((10, 16), (54, 16))
    p.line((24, 16), (26, 8), (38, 8), (40, 16))
    p.line((16, 16), (20, 56), (44, 56), (48, 16))
    p.line((28, 26), (28, 46))
    p.line((36, 26), (36, 46))


@icon("floor")
def _(p):   # a box dropped onto a line
    p.rect(20, 10, 44, 34, r=4)
    p.line((32, 38), (32, 44))
    p.head(32, 47, 90, 8)
    p.line((8, 54), (56, 54))


@icon("search")
def _(p):
    p.circle(27, 27, 15)
    p.line((38, 38), (54, 54))


# ---------------------------------------------------------------- categories
@icon("all")
def _(p):   # four squares
    for x, y in ((10, 10), (36, 10), (10, 36), (36, 36)):
        p.rect(x, y, x + 18, y + 18, r=4)


@icon("seating")
def _(p):   # a chair from the side
    p.line((20, 8), (20, 56))
    p.line((20, 34), (46, 34), (46, 56))


@icon("tables")
def _(p):
    p.line((8, 24), (56, 24))
    p.line((16, 24), (16, 54))
    p.line((48, 24), (48, 54))


@icon("barrels")
def _(p):
    p.d.ellipse((*P(14, 8), *P(50, 56)), outline=W, width=LW)
    p.line((18, 22), (46, 22))
    p.line((18, 42), (46, 42))


@icon("crates")
def _(p):
    p.rect(10, 12, 54, 54, r=3)
    p.line((10, 12), (54, 54))
    p.line((54, 12), (10, 54))


@icon("lights")
def _(p):   # a lantern
    p.line((32, 6), (32, 12))
    p.rect(20, 14, 44, 50, r=6)
    p.line((24, 56), (40, 56))
    p.circle(32, 32, 5, fill=True)


@icon("fires")
def _(p):   # a flame
    pts = []
    for i in range(41):
        t = i / 40 * 2 * math.pi
        r = 18
        x = 32 + r * math.sin(t)
        y = 40 - r * 0.9 * math.cos(t)
        if math.cos(t) > 0:   # the top half pinched to a point
            k = math.cos(t)
            x = 32 + (x - 32) * (1 - 0.7 * k)
            y = y - 12 * k * k
        pts.append((x, y))
    p.line(*pts, close=True)
    p.line((20, 58), (44, 58))


@icon("banners")
def _(p):
    p.line((16, 6), (16, 58))
    p.line((16, 10), (48, 10), (48, 40), (32, 32), (16, 40))


@icon("tents")
def _(p):
    p.line((6, 52), (32, 12), (58, 52), close=True)
    p.line((32, 12), (32, 52))
    p.line((32, 52), (24, 40))


@icon("fences")
def _(p):
    for x in (14, 32, 50):
        p.line((x, 16), (x, 56))
    p.line((8, 28), (56, 28))
    p.line((8, 44), (56, 44))


@icon("nature")
def _(p):   # a tree
    p.circle(32, 24, 16)
    p.line((32, 40), (32, 58))


@icon("decor")
def _(p):   # a vase
    p.line((24, 8), (40, 8))
    p.line((26, 8), (26, 18), (16, 34), (22, 56), (42, 56), (48, 34), (38, 18), (38, 8))


@icon("structure")
def _(p):   # a column
    p.line((14, 10), (50, 10))
    p.line((14, 54), (50, 54))
    p.line((22, 10), (22, 54))
    p.line((42, 10), (42, 54))


@icon("buildings")
def _(p):   # a house
    p.line((8, 30), (32, 10), (56, 30))
    p.line((14, 26), (14, 56), (50, 56), (50, 26))
    p.rect(26, 38, 38, 56, r=2)


@icon("furniture")
def _(p):   # a bed / sofa
    p.line((8, 20), (8, 52))
    p.line((8, 38), (56, 38), (56, 52))
    p.rect(14, 26, 30, 38, r=4)


@icon("objects")
def _(p):   # a cube
    p.line((32, 8), (54, 20), (54, 44), (32, 56), (10, 44), (10, 20), close=True)
    p.line((10, 20), (32, 32), (54, 20))
    p.line((32, 32), (32, 56))


@icon("models")
def _(p):   # a wireframe sphere
    p.circle(32, 32, 22)
    p.d.ellipse((*P(22, 10), *P(42, 54)), outline=W, width=LW - 4)
    p.line((10, 32), (54, 32))


@icon("favourites")
def _(p):   # a star
    pts = []
    for i in range(10):
        a = -math.pi / 2 + i * math.pi / 5
        r = 24 if i % 2 == 0 else 10
        pts.append((32 + r * math.cos(a), 33 + r * math.sin(a)))
    p.line(*pts, close=True)


@icon("recent")
def _(p):   # a clock
    p.circle(32, 32, 22)
    p.line((32, 18), (32, 32), (42, 38))


@icon("prefabs")
def _(p):   # three stacked boxes
    p.rect(8, 34, 30, 56, r=3)
    p.rect(34, 34, 56, 56, r=3)
    p.rect(21, 8, 43, 30, r=3)


@icon("storage")
def _(p):   # a chest
    p.rect(8, 24, 56, 54, r=4)
    p.arc(32, 26, 24, 180, 360)
    p.line((8, 34), (56, 34))
    p.rect(28, 30, 36, 40, r=2, fill=True)


@icon("market")
def _(p):   # a stall with an awning
    p.line((8, 22), (14, 10), (50, 10), (56, 22), close=True)
    for x in (8, 24, 40):
        p.arc(x + 8, 22, 8, 0, 180)
    p.line((12, 30), (12, 56))
    p.line((52, 30), (52, 56))
    p.line((12, 44), (52, 44))


@icon("dock")
def _(p):   # planks on posts over a wave
    p.line((8, 22), (56, 22))
    p.line((16, 22), (16, 42))
    p.line((48, 22), (48, 42))
    p.arc(16, 52, 8, 180, 360)
    p.arc(32, 52, 8, 180, 360)
    p.arc(48, 52, 8, 180, 360)


@icon("keep")
def _(p):   # a tower with battlements
    p.line((14, 56), (14, 12), (22, 12), (22, 18), (28, 18), (28, 12), (36, 12), (36, 18), (42, 18), (42, 12), (50, 12), (50, 56))
    p.line((10, 56), (54, 56))
    p.rect(28, 40, 36, 56, r=2)


@icon("guide")
def _(p):   # an open book
    p.line((32, 16), (32, 52))
    p.line((32, 16), (22, 12), (8, 12), (8, 48), (22, 48), (32, 52))
    p.line((32, 16), (42, 12), (56, 12), (56, 48), (42, 48), (32, 52))
    p.line((15, 24), (25, 24))
    p.line((15, 33), (25, 33))
    p.line((39, 24), (49, 24))


@icon("keys")
def _(p):   # a key cap with a smaller one beside it
    p.rect(6, 16, 38, 48, r=6)
    p.line((14, 40), (30, 40))
    p.rect(42, 26, 58, 48, r=4)


@icon("noimage")
def _(p):   # a dashed box
    for a, b in (((10, 10), (24, 10)), ((40, 10), (54, 10)), ((10, 54), (24, 54)), ((40, 54), (54, 54)),
                 ((10, 10), (10, 24)), ((10, 40), (10, 54)), ((54, 10), (54, 24)), ((54, 40), (54, 54))):
        p.line(a, b)


def tga(im: Image.Image) -> bytes:
    """An uncompressed 32-bit TGA: type 2, bottom-up rows (descriptor 0x08: 8 alpha bits, origin bottom-left)."""
    w, h = im.size
    head = struct.pack("<BBBHHBHHHHBB", 0, 0, 2, 0, 0, 0, 0, 0, w, h, 32, 8)
    px = im.convert("RGBA").transpose(Image.FLIP_TOP_BOTTOM).tobytes("raw", "BGRA")
    return head + px


def render(name):
    p = Pen()
    ICONS[name](p)
    return p.im.resize((SIZE, SIZE), Image.LANCZOS)


def main():
    os.makedirs(OUT, exist_ok=True)
    for f in os.listdir(OUT):
        if f.endswith(".tga") and f[:-4] not in ICONS:
            os.remove(os.path.join(OUT, f))
    for name in sorted(ICONS):
        with open(os.path.join(OUT, name + ".tga"), "wb") as f:
            f.write(tga(render(name)))
    if "--sheet" in sys.argv:   # a contact sheet to look at (not shipped)
        names = sorted(ICONS)
        cols = 10
        sheet = Image.new("RGBA", (cols * 80, ((len(names) + cols - 1) // cols) * 80), (20, 22, 28, 255))
        for i, n in enumerate(names):
            sheet.alpha_composite(render(n), ((i % cols) * 80 + 8, (i // cols) * 80 + 8))
        sheet.save(sys.argv[sys.argv.index("--sheet") + 1])
    print("wrote %d icons to %s" % (len(ICONS), os.path.relpath(OUT, ROOT)))


if __name__ == "__main__":
    sys.exit(main())
