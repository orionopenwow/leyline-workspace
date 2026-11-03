"""Leyline feature videos: the edit. Recorded scenes (tools/video/record.cjs: screencast frames, the cursor's path and
its clicks, in slow motion) and stills from the repo's art become the finished videos, drawn frame by frame here and
piped to ffmpeg: a smooth cursor with a soft halo and a ripple on each click, gentle lower-third subtitles, the Leyline
mark at the start and the end, crossfades between shots. No music, no voice. Also writes an .srt per video and a
contact sheet (one frame every 2 s).

  python3 -I tools/video/assemble.py <rec dir> <out dir> <fonts dir> [video ...]

<fonts dir> holds Nunito-700.ttf (subtitles) and Marcellus.ttf (the wordmark), converted from the fontsource packages.
"""
import json
import math
import os
import subprocess
import sys
from bisect import bisect_right

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
ART = os.path.join(ROOT, "stream", "www-files", "art")
UIART = os.path.join(ROOT, "launcher", "ui", "art")
REC, OUT, FONTS = (os.path.abspath(a) for a in sys.argv[1:4])
ONLY = sys.argv[4:]
FPS = 30
XF = 0.45   # crossfade


def font(name, size):
    return ImageFont.truetype(os.path.join(FONTS, name), size)


def ease(t):
    t = max(0.0, min(1.0, t))
    return t * t * (3 - 2 * t)


# ---------------------------------------------------------------- recorded scenes
class Clip:
    def __init__(self, name):
        self.name = name
        d = os.path.join(REC, name)
        j = json.load(open(os.path.join(d, "frames.json")))
        self.dir, self.t0, self.slow, self.scale = d, j["t0"], j["slow"], j["scale"]
        self.files = [f["file"] for f in j["frames"]]
        self.times = [(f["t"] - self.t0) / self.slow for f in j["frames"]]
        self.dur = (j["t1"] - self.t0) / self.slow
        self.marks = {m["label"]: (m["t"] - self.t0) / self.slow for m in j["marks"]}
        mv = sorted(j["moves"])
        self.mt = [(m[0] - self.t0) / self.slow for m in mv]
        self.mxy = [(m[1], m[2]) for m in mv]
        self.clicks = [((c[0] - self.t0) / self.slow, c[1], c[2]) for c in j["clicks"]]
        self._i, self._im = None, None

    def at(self, ref):
        """a time in the clip: a number, or (mark, offset)"""
        if isinstance(ref, (int, float)):
            return float(ref)
        return self.marks[ref[0]] + ref[1]

    def frame(self, t):
        i = max(0, bisect_right(self.times, t) - 1)
        if i != self._i:
            self._i, self._im = i, Image.open(os.path.join(self.dir, self.files[i])).convert("RGB")
        return self._im

    def cursor(self, t):
        if not self.mt:
            return None
        i = bisect_right(self.mt, t)
        if i == 0:
            return self.mxy[0]
        if i >= len(self.mt):
            return self.mxy[-1]
        ta, tb = self.mt[i - 1], self.mt[i]
        (xa, ya), (xb, yb) = self.mxy[i - 1], self.mxy[i]
        k = 0 if tb <= ta else (t - ta) / (tb - ta)
        return xa + (xb - xa) * k, ya + (yb - ya) * k


CLIPS = {}


def clip(name):
    if name not in CLIPS:
        CLIPS[name] = Clip(name)
    return CLIPS[name]


# ---------------------------------------------------------------- the cursor and its ripple
def cursor_sprite(s):
    """the arrow with a soft warm halo; s = pixels per CSS pixel. Returns (image, hotspot)."""
    S = 4   # drawn big, then scaled down: smooth edges
    W = int(70 * s)
    big = Image.new("RGBA", (W * S, W * S), (0, 0, 0, 0))
    hx = hy = W * S // 2
    halo = Image.new("L", big.size, 0)
    r = 22 * s * S
    ImageDraw.Draw(halo).ellipse((hx - r, hy - r, hx + r, hy + r), fill=120)
    halo = halo.filter(ImageFilter.GaussianBlur(9 * s * S))
    big.paste((255, 236, 190, 255), (0, 0), halo)
    pts = [(3, 2), (3, 24), (8.6, 18.6), (12.4, 27.2), (16, 25.6), (12.3, 17.2), (20, 17.2)]
    poly = [(hx + (x - 3) * s * S, hy + (y - 2) * s * S) for x, y in pts]
    sh = Image.new("L", big.size, 0)
    ImageDraw.Draw(sh).polygon([(x, y + 2 * s * S) for x, y in poly], fill=150)
    sh = sh.filter(ImageFilter.GaussianBlur(3 * s * S))
    big.paste((0, 0, 0, 255), (0, 0), Image.composite(sh, Image.new("L", big.size, 0), sh))
    d = ImageDraw.Draw(big)
    d.polygon(poly, fill=(255, 255, 255, 255), outline=(27, 27, 34, 255), width=int(1.6 * s * S))
    im = big.resize((W, W), Image.LANCZOS)
    return im, (W // 2, W // 2)


def ripple(canvas, x, y, p, s):
    """a ring that grows and fades, p in 0..1"""
    if p < 0 or p > 1:
        return
    q = 1 - (1 - p) ** 2.4
    r = (5 + 30 * q) * s
    a = (1 - p) ** 1.3
    R = int(r + 14 * s)
    lay = Image.new("RGBA", (2 * R, 2 * R), (0, 0, 0, 0))
    d = ImageDraw.Draw(lay)
    w = max(2, int(3 * s))
    d.ellipse((R - r, R - r, R + r, R + r), outline=(255, 214, 130, int(200 * a)), width=w + 4)
    lay = lay.filter(ImageFilter.GaussianBlur(4 * s))
    d = ImageDraw.Draw(lay)
    d.ellipse((R - r, R - r, R + r, R + r), outline=(255, 238, 190, int(255 * a)), width=w)
    canvas.alpha_composite(lay, (int(x - R), int(y - R)))


# ---------------------------------------------------------------- segments: each draws itself at a time in 0..dur
class ClipSeg:
    """a stretch of a recorded scene; zoom: [(time in this shot, x, y, z)] keyframes (x, y: the point to centre, as
    fractions of the frame; z: how far in), eased between, so a reel can lean in on what is clicked"""

    def __init__(self, name, a, b, box=None, zoom=None):
        self.c = clip(name)
        self.a, self.b = self.c.at(a), self.c.at(b)
        self.dur = self.b - self.a
        self.box = box   # (x, y, w, h) on the canvas, else the whole canvas
        self.zoom = zoom or [(0, 0.5, 0.5, 1.0)]

    def view(self, t):
        z = self.zoom
        if t <= z[0][0]:
            return z[0][1:]
        for (ta, *va), (tb, *vb) in zip(z, z[1:]):
            if t <= tb:
                k = ease((t - ta) / (tb - ta)) if tb > ta else 1
                return tuple(va[i] + (vb[i] - va[i]) * k for i in range(3))
        return z[-1][1:]

    def draw(self, canvas, t):
        c, ts = self.c, self.a + t
        im = c.frame(ts)
        x0, y0, w, h = self.box or (0, 0) + canvas.size
        fx, fy, z = self.view(t)
        cw, ch = im.width / z, im.height / z
        cx = min(max(fx * im.width - cw / 2, 0), im.width - cw)
        cy = min(max(fy * im.height - ch / 2, 0), im.height - ch)
        k = w / cw   # canvas pixels per source pixel
        if (cx, cy, cw, ch) == (0, 0, w, h):
            pic = im
        else:
            pic = im.resize((w, h), Image.LANCZOS, box=(cx, cy, cx + cw, cy + ch))
        canvas.paste(pic, (x0, y0))
        s = c.scale * k   # canvas pixels per CSS pixel
        to = lambda px, py: (x0 + (px * c.scale - cx) * k, y0 + (py * c.scale - cy) * k)
        for tc, px, py in c.clicks:
            ripple(canvas, *to(px, py), (ts - tc) / 0.7, s)
        p = c.cursor(ts)
        if p:
            spr, (hx, hy) = sprite(s)
            X, Y = to(*p)
            canvas.alpha_composite(spr, (int(X - hx), int(Y - hy)))


_sprites = {}


def sprite(s):
    key = round(s * 20) / 20
    if key not in _sprites:
        _sprites[key] = cursor_sprite(key)
    return _sprites[key]


class StillSeg:
    """an in-game still with a gentle pan and push-in: wide fills the frame; in a reel it is a card on its own blurred copy"""

    def __init__(self, path, dur, frm=(0.0, 0.0, 1.0), to=(1.0, 1.0, 1.08), card=None):
        self.im = Image.open(path).convert("RGB")
        self.dur, self.frm, self.to, self.card = dur, frm, to, card
        self.bg = None

    def view(self, size, t):
        k = ease(t / self.dur)
        fx, fy, fz = (self.frm[i] + (self.to[i] - self.frm[i]) * k for i in range(3))
        W, H = size
        iw, ih = self.im.size
        base = max(W / iw, H / ih)   # cover
        z = base * fz
        vw, vh = W / z, H / z
        x = (iw - vw) * fx
        y = (ih - vh) * fy
        return self.im.resize((W, H), Image.LANCZOS, box=(x, y, x + vw, y + vh))

    def draw(self, canvas, t):
        if not self.card:
            canvas.paste(self.view(canvas.size, t), (0, 0))
            return
        if self.bg is None:
            W, H = canvas.size
            iw, ih = self.im.size
            z = max(W / iw, H / ih) * 1.1
            bg = self.im.resize((int(iw * z), int(ih * z)), Image.LANCZOS).filter(ImageFilter.GaussianBlur(40))
            bg = bg.crop(((bg.width - W) // 2, (bg.height - H) // 2, (bg.width - W) // 2 + W, (bg.height - H) // 2 + H))
            self.bg = Image.blend(bg, Image.new("RGB", (W, H), (8, 9, 16)), 0.45)
        canvas.paste(self.bg, (0, 0))
        x0, y0, w, h = self.card
        v = self.view((w, h), t)
        m = Image.new("L", (w, h), 0)
        ImageDraw.Draw(m).rounded_rectangle((0, 0, w - 1, h - 1), radius=28, fill=255)
        sh = Image.new("RGBA", (w + 120, h + 120), (0, 0, 0, 0))
        ImageDraw.Draw(sh).rounded_rectangle((60, 70, w + 60, h + 70), radius=28, fill=(0, 0, 0, 170))
        canvas.alpha_composite(sh.filter(ImageFilter.GaussianBlur(24)), (x0 - 60, y0 - 60))
        canvas.paste(v, (x0, y0), m)


def starfield(size):
    bg = Image.open(os.path.join(UIART, "leyline-bg.webp")).convert("RGB")
    W, H = size
    z = max(W / bg.width, H / bg.height)
    bg = bg.resize((math.ceil(bg.width * z), math.ceil(bg.height * z)), Image.LANCZOS)
    return bg.crop(((bg.width - W) // 2, (bg.height - H) // 2, (bg.width - W) // 2 + W, (bg.height - H) // 2 + H))


class CardSeg:
    """the Leyline mark on the starfield: the intro (mark and wordmark) and the end card (a line and the address)"""

    def __init__(self, dur, line=None, small=None, tall=False):
        self.dur, self.line, self.small, self.tall = dur, line, small, tall
        self.bg = None

    def draw(self, canvas, t):
        W, H = canvas.size
        if self.bg is None:
            self.bg = starfield((W, H)).convert("RGBA")
            self.mark = Image.open(os.path.join(UIART, "brand", "leyline-mark-512.png")).convert("RGBA")
        z = 1.0 + 0.04 * t / self.dur   # a slow push-in
        bg = self.bg.resize((int(W * z), int(H * z)), Image.BILINEAR)
        canvas.paste(bg.crop(((bg.width - W) // 2, (bg.height - H) // 2, (bg.width - W) // 2 + W, (bg.height - H) // 2 + H)), (0, 0))
        u = min(W, H)
        a = ease(t / 0.9)
        ms = int(u * (0.36 if self.tall else 0.34) * (0.92 + 0.08 * a))
        mk = self.mark.resize((ms, ms), Image.LANCZOS)
        cy = int(H * (0.40 if self.tall else 0.36))
        glow = Image.new("RGBA", (ms * 2, ms * 2), (0, 0, 0, 0))
        ImageDraw.Draw(glow).ellipse((ms * 0.55, ms * 0.55, ms * 1.45, ms * 1.45), fill=(110, 120, 255, int(90 * a)))
        canvas.alpha_composite(glow.filter(ImageFilter.GaussianBlur(ms * 0.18)), (W // 2 - ms, cy - ms))
        mk.putalpha(ImageChops.multiply(mk.getchannel("A"), Image.new("L", mk.size, int(255 * a))))
        canvas.alpha_composite(mk, (W // 2 - ms // 2, cy - ms // 2))
        b = ease((t - 0.35) / 0.8)
        y = cy + ms // 2 + int(u * 0.05)
        word = text_img("Leyline", font("Marcellus.ttf", int(u * 0.12)), (236, 232, 255), shadow=True)
        paste_alpha(canvas, word, (W // 2 - word.width // 2, y + int((1 - b) * 14)), b)
        y += word.height + int(u * 0.02)
        if self.line:
            c = ease((t - 0.7) / 0.8)
            ln = text_img(self.line, font("Nunito-700.ttf", int(u * 0.058)), (255, 255, 255), shadow=True)
            paste_alpha(canvas, ln, (W // 2 - ln.width // 2, y + int((1 - c) * 12)), c)
            y += ln.height + int(u * 0.018)
        if self.small:
            c = ease((t - 1.0) / 0.8)
            sm = text_img(self.small, font("Nunito-600.ttf", int(u * 0.036)), (200, 205, 235), shadow=True)
            paste_alpha(canvas, sm, (W // 2 - sm.width // 2, y), c)


class ReelSeg:
    """a reel's recorded shot: the tall launcher on the starfield, the reel's title above it"""

    def __init__(self, inner, title):
        self.inner, self.title, self.dur = inner, title, inner.dur
        self.bg = None

    def draw(self, canvas, t):
        if self.bg is None:
            W, H = canvas.size
            bg = starfield((W, H))
            self.bg = Image.blend(bg, Image.new("RGB", (W, H), (10, 11, 20)), 0.35).convert("RGBA")
            mk = Image.open(os.path.join(UIART, "brand", "leyline-mark-512.png")).convert("RGBA").resize((92, 92), Image.LANCZOS)
            tt = text_img(self.title, font("Nunito-700.ttf", 54), (255, 255, 255), shadow=True)
            x = (W - (92 + 22 + tt.width)) // 2
            self.bg.alpha_composite(mk, (x, 112))
            self.bg.alpha_composite(tt, (x + 92 + 22, 112 + 46 - tt.height // 2))
            x0, y0, w, h = self.inner.box
            sh = Image.new("RGBA", (w + 100, h + 100), (0, 0, 0, 0))
            ImageDraw.Draw(sh).rectangle((50, 60, w + 50, h + 60), fill=(0, 0, 0, 200))
            self.bg.alpha_composite(sh.filter(ImageFilter.GaussianBlur(22)), (x0 - 50, y0 - 50))
        canvas.paste(self.bg, (0, 0))
        self.inner.draw(canvas, t)


# ---------------------------------------------------------------- text
def text_img(s, f, color, shadow=True, stroke=0):
    l, t, r, b = f.getbbox(s)
    pad = int(f.size * 0.5)
    w, h = r - l + 2 * pad, b - t + 2 * pad
    im = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    if shadow:
        m = Image.new("L", (w, h), 0)
        ImageDraw.Draw(m).text((pad - l, pad - t + f.size * 0.04), s, font=f, fill=200)
        m = m.filter(ImageFilter.GaussianBlur(f.size * 0.16))
        im.paste((0, 0, 0, 255), (0, 0), m)
        m2 = Image.new("L", (w, h), 0)
        ImageDraw.Draw(m2).text((pad - l, pad - t + f.size * 0.03), s, font=f, fill=140)
        m2 = m2.filter(ImageFilter.GaussianBlur(f.size * 0.05))
        im.alpha_composite(Image.merge("RGBA", (Image.new("L", (w, h), 0),) * 3 + (m2,)))
    ImageDraw.Draw(im).text((pad - l, pad - t), s, font=f, fill=color + (255,))
    return im


def paste_alpha(canvas, im, xy, a):
    if a <= 0:
        return
    if a < 1:
        im = im.copy()
        im.putalpha(ImageChops.multiply(im.getchannel("A"), Image.new("L", im.size, int(255 * a))))
    canvas.alpha_composite(im, (int(xy[0]), int(xy[1])))


def subtitle_img(lines, size):
    f = font("Nunito-700.ttf", size)
    ims = [text_img(x, f, (255, 255, 255)) for x in lines]
    gap = -int(size * 0.62)
    w = max(i.width for i in ims)
    h = sum(i.height for i in ims) + gap * (len(ims) - 1)
    out = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    y = 0
    for i in ims:
        out.alpha_composite(i, ((w - i.width) // 2, y))
        y += i.height + gap
    return out


# ---------------------------------------------------------------- the timeline
def render(name, size, segs, subs, sub_y, sub_size):
    """segs: [segment]; each one after the first starts XF before the last one ends (a crossfade).
    subs: [(start, end, "line" or "line one|line two")] in the video's time."""
    starts, t = [], 0.0
    for i, s in enumerate(segs):
        starts.append(t)
        t += s.dur - (XF if i < len(segs) - 1 else 0)
    total = t
    W, H = size
    subimgs = [(a, b, subtitle_img(x.split("|"), sub_size)) for a, b, x in subs]
    vignette = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    vg = Image.new("L", (W, H), 0)
    band = int(H * 0.30)
    for yy in range(band):   # a soft dark band under the subtitles, only as dark as needed
        ImageDraw.Draw(vg).line((0, H - band + yy, W, H - band + yy), fill=int(120 * (yy / band) ** 1.6))
    vignette.putalpha(vg)
    os.makedirs(OUT, exist_ok=True)
    mp4 = os.path.join(OUT, name + ".mp4")
    ff = subprocess.Popen(["ffmpeg", "-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{W}x{H}", "-r", str(FPS), "-i", "-",
                           "-c:v", "libx264", "-preset", "slow", "-crf", "19", "-pix_fmt", "yuv420p", "-profile:v", "high", "-movflags", "+faststart",
                           "-r", str(FPS), mp4], stdin=subprocess.PIPE)
    n = int(round(total * FPS))
    for fi in range(n):
        T = fi / FPS
        canvas = None
        for i, s in enumerate(segs):
            a = starts[i]
            if not (a - 1e-9 <= T < a + s.dur):
                continue
            c = Image.new("RGBA", (W, H), (0, 0, 0, 255))
            s.draw(c, T - a)
            if canvas is None:
                canvas = c
            else:   # the next shot fades in over this one
                canvas = Image.blend(canvas, c, ease((T - a) / XF))
        if canvas is None:
            canvas = Image.new("RGBA", (W, H), (0, 0, 0, 255))
        live = [(a, b, im) for a, b, im in subimgs if a <= T < b]
        if live and W > H:
            canvas.alpha_composite(vignette)
        for a, b, im in live:
            k = min(ease((T - a) / 0.3), ease((b - T) / 0.3))
            paste_alpha(canvas, im, ((W - im.width) // 2, sub_y - im.height // 2 + int((1 - k) * 10)), k)
        ff.stdin.write(canvas.convert("RGB").tobytes())
        if fi % 150 == 0:
            print(f"  {name}: {fi}/{n}", flush=True)
    ff.stdin.close()
    ff.wait()
    write_srt(os.path.join(OUT, name + ".srt"), subs)
    sheet(mp4, os.path.join(OUT, name + "-contact.png"), total, W > H)
    print(name, "%.2f s" % total, flush=True)
    return total


def stamp(t):
    ms = int(round(t * 1000))
    return "%02d:%02d:%02d,%03d" % (ms // 3600000, ms // 60000 % 60, ms // 1000 % 60, ms % 1000)


def write_srt(path, subs):
    subs = [(a, min(b, subs[i + 1][0] - 0.05) if i + 1 < len(subs) else b, x) for i, (a, b, x) in enumerate(subs)]   # no overlaps
    with open(path, "w") as f:
        for i, (a, b, x) in enumerate(subs, 1):
            f.write(f"{i}\n{stamp(a)} --> {stamp(b)}\n{x.replace('|', chr(10))}\n\n")


def sheet(mp4, png, total, wide):
    cols = 6 if wide else 8
    rows = math.ceil(total / 2 / cols)
    w = 400 if wide else 225
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", mp4, "-vf",
                    f"fps=1/2,scale={w}:-2,drawtext=fontfile={os.path.join(FONTS, 'Nunito-700.ttf')}:text='%{{pts\\:hms}}':x=6:y=6:fontsize=16:fontcolor=white:box=1:boxcolor=black@0.6,"
                    f"tile={cols}x{rows}:padding=4:margin=4", "-frames:v", "1", png], check=True)


# ---------------------------------------------------------------- the videos
def youtube():
    S = (1920, 1080)
    still = lambda f, d, frm, to: StillSeg(os.path.join(ART, f), d, frm, to)
    segs = [
        CardSeg(2.4),
        ClipSeg("w-home", ("hero", -0.8), ("clicked", 1.3)),
        still("leyline-shot-dalaran.webp", 2.4, (0.5, 0.2, 1.0), (0.5, 0.6, 1.1)),
        ClipSeg("w-den", ("den", -0.3), ("play", 1.2)),
        still("leyline-shot-forge.webp", 2.8, (0.3, 0.3, 1.02), (0.45, 0.45, 1.16)),
        ClipSeg("w-den", ("terrain", -0.2), ("saved", 1.4)),
        ClipSeg("w-host", ("host", -0.3), ("form", 1.6)),
        ClipSeg("w-host", ("typed", -1.1), ("live", 1.6)),
        ClipSeg("w-host", ("realms", -0.2), ("row", 1.7)),
        ClipSeg("w-mods", ("mods", -0.2), ("discover", 1.0)),
        ClipSeg("w-mods", ("install", -1.2), ("install", 1.5)),
        ClipSeg("w-forum", 1.2, ("discord", 1.4)),
        CardSeg(3.8, "Free and open source", "leylinelauncher.com"),
    ]
    return S, segs


def reel(kind):
    S = (1080, 1920)
    box = (0, 250, 1080, 1440)
    card = (40, 560, 1000, 562)
    title = {"play": "Play in one click", "den": "Your Den", "host": "Host your own server"}[kind]
    R = lambda name, a, b, zoom=None: ReelSeg(ClipSeg(name, a, b, box, zoom), title)
    st = lambda f, d, frm, to: StillSeg(os.path.join(ART, f), d, frm, to, card=card)
    end = CardSeg(3.0, "Free and open source", "leylinelauncher.com", tall=True)
    if kind == "play":
        segs = [CardSeg(1.8, tall=True),
                R("t-home", ("hero", -1.6), ("clicked", 1.6), [(0.3, 0.5, 0.5, 1.0), (1.6, 0.64, 0.8, 1.45)]),
                st("leyline-shot-dalaran.webp", 3.4, (0.5, 0.2, 1.0), (0.5, 0.6, 1.12)),
                R("t-pick", ("open", -1.0), ("play", 1.2), [(0.2, 0.5, 0.5, 1.0), (1.4, 0.6, 0.82, 1.5)]),
                end]
    elif kind == "den":
        segs = [CardSeg(1.8, tall=True),
                R("t-den", ("den", -0.5), ("play", 1.6), [(0.6, 0.5, 0.5, 1.0), (1.8, 0.62, 0.22, 1.5)]),
                st("leyline-shot-forge.webp", 3.2, (0.3, 0.3, 1.0), (0.5, 0.5, 1.15)),
                R("t-den", ("invite", -0.8), ("friends", 2.0), [(0.0, 0.62, 0.22, 1.5), (3.0, 0.62, 0.22, 1.5), (4.0, 0.74, 0.28, 1.5)]),
                end]
    else:
        segs = [CardSeg(1.8, tall=True),
                R("t-host", ("host", -0.4), ("form", 1.8), [(0.4, 0.5, 0.5, 1.0), (1.6, 0.62, 0.35, 1.35)]),
                R("t-host", ("typed", -1.2), ("live", 1.6), [(0.0, 0.62, 0.5, 1.3), (1.8, 0.62, 0.5, 1.3), (2.6, 0.62, 0.22, 1.4)]),
                R("t-site-realms", ("scroll", -0.6), ("row", 1.8)),
                end]
    return S, segs


def starts_of(segs):
    out, t = [], 0.0
    for i, s in enumerate(segs):
        out.append(t)
        t += s.dur - XF
    return out


# Subtitles: DRAFT for the owner's voice pass (TODO(owner)). Each is tied to a shot: (shot index, from, to) in that
# shot's own time; a negative "to" counts from the shot's end.
SUBS = {
    "leyline-feature-45s": [(1, 0.3, -0.2, "Leyline: your realms, your friends, one launcher"),
                            (2, 0.0, -0.3, "One click, and you're in"),
                            (3, 0.3, -0.2, "Your Den: a white room to build in"),
                            (4, 0.2, -0.3, "Place a piece, shape the ground"),
                            (5, 0.2, -0.2, "Save it, share it with friends"),
                            (6, 0.3, -0.2, "Host your own server from a template"),
                            (7, 0.2, -0.2, "Name it, press Host"),
                            (8, 0.2, -0.2, "It joins the network list"),
                            (9, 0.3, None, "Mods and addons|from the Workshop"),
                            (10, None, -0.2, "Mods and addons|from the Workshop"),
                            (11, 0.2, -0.3, "Questions? The forum and Discord are open")],
    "reel-1-play": [(1, 0.3, -0.2, "Leyline opens on tonight's realm"),
                    (2, 0.0, -0.3, "One click, and you're in"),
                    (3, 0.2, -0.2, "Or pick any realm,|your Den or your own server")],
    "reel-2-den": [(1, 0.3, -0.2, "Your Den: a white room,|all yours"),
                   (2, 0.2, -0.3, "Build: place pieces,|shape the ground"),
                   (3, 0.2, -0.2, "Invite a friend with one link")],
    "reel-3-host": [(1, 0.3, -0.2, "Pick a template"),
                    (2, 0.2, -0.2, "Name it, press Host,|and you're live"),
                    (3, 0.2, -0.2, "Now it's on the network list")],
}


def subs_for(name, segs):
    st = starts_of(segs)
    out = []
    for i, a, b, txt in SUBS[name]:
        s0 = st[i] + (a if a is not None else 0)
        e = st[i] + segs[i].dur + (b if b is not None else 0)
        if out and out[-1][2] == txt and abs(out[-1][1] - s0) < 1.0:   # one card running on across a cut
            out[-1] = (out[-1][0], e, txt)
        else:
            out.append((s0, e, txt))
    return out


def main():
    jobs = {"leyline-feature-45s": youtube, "reel-1-play": lambda: reel("play"), "reel-2-den": lambda: reel("den"), "reel-3-host": lambda: reel("host")}
    for name, fn in jobs.items():
        if ONLY and name not in ONLY:
            continue
        size, segs = fn()
        subs = subs_for(name, segs)
        wide = size[0] > size[1]
        render(name, size, segs, subs, sub_y=int(size[1] * 0.86) if wide else 1795, sub_size=56 if wide else 54)


if __name__ == "__main__":
    main()
