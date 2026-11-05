"""The reel kit's pictures, drawn with Pillow in Leyline's look (brand.py): the 1 s intro sting, the title card, the
lower-third caption, the end card ("leylinelauncher.com"), and the DRAFT badge a video wears while any of its words are
still TODO(owner). Every function takes a format name (brand.FORMATS: "reel" 9:16 or "wide" 16:9) and returns PIL
images the size of the frame, so edit.py lays them over or between clips without any arithmetic of its own.

The look is the launcher's Home banner (launcher/ui/home.css .hs-*): the starfield backdrop, a blue wash from one
corner and a purple one from the other, an uppercase kicker in the strong accent, the title in the display face.

    python3 -I tools/reels/cards.py <out dir>      every card in both formats, as PNGs, to look at
"""
import math
import os
import sys

from PIL import Image, ImageDraw, ImageFilter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import brand as B  # noqa: E402

_cache = {}


def size(fmt):
    return B.FORMATS[fmt]


def ease(t):
    t = max(0.0, min(1.0, t))
    return t * t * (3 - 2 * t)


def ease_out(t):
    t = max(0.0, min(1.0, t))
    return 1 - (1 - t) ** 3


def _cover(im, w, h):
    """im scaled to cover w x h, centred."""
    k = max(w / im.width, h / im.height)
    im = im.resize((max(w, round(im.width * k)), max(h, round(im.height * k))), Image.LANCZOS)
    x, y = (im.width - w) // 2, (im.height - h) // 2
    return im.crop((x, y, x + w, y + h))


def _wash(w, h, cx, cy, radius, colour, strength):
    """A soft radial glow of one colour (the CSS radial-gradient washes), as an RGBA layer."""
    small = 64
    # Pillow's radial gradient is 0 in the middle and 181 at the middle of each edge: a smooth fall to 0 there
    m = Image.radial_gradient("L").resize((small, small))
    m = m.point(lambda v: int(255 * strength * max(0.0, 1 - v / 181.0) ** 2))
    d = int(radius * 2)
    m = m.resize((d, d), Image.BICUBIC)
    # the frame's window onto the glow (crop pads outside with 0): no seams where the square would end
    left, top = int(radius - cx), int(radius - cy)
    mask = m.crop((left, top, left + w, top + h))
    layer = Image.new("RGBA", (w, h), B.rgb(colour, 255))
    layer.putalpha(mask)
    return layer


def backdrop(fmt):
    """The frame's background: the launcher's starfield, dimmed, with the blue and purple washes."""
    key = ("bg", fmt)
    if key in _cache:
        return _cache[key].copy()
    w, h = size(fmt)
    base = Image.new("RGBA", (w, h), B.rgb(B.BG, 255))
    try:
        art = _cover(Image.open(B.BACKDROP).convert("RGB"), w, h).convert("RGBA")
        art.putalpha(150)
        base.alpha_composite(art)
    except OSError:
        pass
    r = max(w, h)
    base.alpha_composite(_wash(w, h, w * 0.88, h * 0.08, r * 0.62, B.ACCENT, 0.42))
    base.alpha_composite(_wash(w, h, w * 0.06, h * 0.98, r * 0.58, B.ACCENT_2, 0.36))
    # the bottom a touch darker, like the launcher's backdrop wash
    shade = Image.linear_gradient("L").resize((w, h)).point(lambda v: int(v * 0.45))
    dark = Image.new("RGBA", (w, h), (9, 11, 17, 255))
    dark.putalpha(shade)
    base.alpha_composite(dark)
    out = base.convert("RGB")
    _cache[key] = out
    return out.copy()


def mark(px):
    key = ("mark", px)
    if key not in _cache:
        _cache[key] = Image.open(B.MARK).convert("RGBA").resize((px, px), Image.LANCZOS)
    return _cache[key]


def wrap(draw, text, fnt, width):
    """Words in lines no wider than width (a word longer than the line gets a line of its own)."""
    lines, cur = [], ""
    for word in str(text).split():
        trial = (cur + " " + word).strip()
        if cur and draw.textlength(trial, font=fnt) > width:
            lines.append(cur)
            cur = word
        else:
            cur = trial
    if cur:
        lines.append(cur)
    return lines


def _fit(draw, text, kind, start, width, max_lines, upper=False):
    """The largest font (from start down) that fits text in max_lines lines of width."""
    text = text.upper() if upper else text
    s = start
    while s > 12:
        f = B.font(kind, s)
        lines = wrap(draw, text, f, width)
        if len(lines) <= max_lines and all(draw.textlength(x, font=f) <= width for x in lines):
            return f, lines, s
        s -= 4
    f = B.font(kind, 12)
    return f, wrap(draw, text, f, width), 12


def _tracked(draw, xy, text, fnt, fill, tracking):
    """Text with letter spacing (CSS letter-spacing), left-aligned at xy; returns its width."""
    x, y = xy
    for ch in text:
        draw.text((x, y), ch, font=fnt, fill=fill)
        x += draw.textlength(ch, font=fnt) + tracking
    return x - xy[0] - tracking


def _tracked_width(draw, text, fnt, tracking):
    return sum(draw.textlength(c, font=fnt) for c in text) + tracking * max(0, len(text) - 1)


# ---------------------------------------------------------------- the title card
def title_card(fmt, title, kicker="", sub=""):
    """The opening card after the sting: kicker (small, uppercase, accent), title (display face, uppercase), sub."""
    w, h = size(fmt)
    im = backdrop(fmt)
    d = ImageDraw.Draw(im)
    tall = h > w
    pad = int(w * (0.09 if tall else 0.08))
    width = w - pad * 2
    f_title, lines, s = _fit(d, title, "display", int(w * (0.135 if tall else 0.085)), width, 4 if tall else 2, upper=True)
    f_kick = B.font("bold", int(w * (0.032 if tall else 0.017)))
    f_sub = B.font("body", int(w * (0.04 if tall else 0.022)))
    sub_lines = wrap(d, sub, f_sub, width) if sub else []
    lh = int(s * 1.02)
    kick_h = int(f_kick.size * 1.9) if kicker else 0
    sub_h = int(f_sub.size * 1.4) * len(sub_lines) + (int(f_sub.size * 0.9) if sub_lines else 0)
    block = kick_h + lh * len(lines) + sub_h
    y = int(h * (0.42 if tall else 0.5)) - block // 2
    m = mark(int(w * (0.16 if tall else 0.075)))
    im.paste(m, (pad - int(m.width * 0.12), y - m.height - int(w * 0.02)), m)
    if kicker:
        _tracked(d, (pad, y), kicker.upper(), f_kick, B.rgb(B.ACCENT_STRONG), f_kick.size * 0.12)
        y += kick_h
    for ln in lines:
        d.text((pad, y), ln, font=f_title, fill=B.rgb(B.TEXT))
        y += lh
    if sub_lines:
        y += int(f_sub.size * 0.9)
        for ln in sub_lines:
            d.text((pad, y), ln, font=f_sub, fill=B.rgb(B.TEXT_2))
            y += int(f_sub.size * 1.4)
    # the accent rule under the kicker's column, the website's divider in two colours
    bar_y = h - int(h * (0.1 if tall else 0.12))
    _rule(im, pad, bar_y, int(width * 0.3), max(4, int(w * 0.004)))
    return im


def _rule(im, x, y, length, thick):
    """A short bar fading from the blue to the purple."""
    g = Image.linear_gradient("L").rotate(90, expand=True).resize((length, thick))
    a = Image.new("RGBA", (length, thick), B.rgb(B.ACCENT, 255))
    b = Image.new("RGBA", (length, thick), B.rgb(B.ACCENT_2, 255))
    bar = Image.composite(b, a, g)
    m = Image.new("L", (length, thick), 0)
    ImageDraw.Draw(m).rounded_rectangle((0, 0, length - 1, thick - 1), radius=thick // 2, fill=255)
    im.paste(bar.convert("RGB"), (x, y), m)


# ---------------------------------------------------------------- the lower third
def lower_third(fmt, text, sub=""):
    """A caption panel over the clip (RGBA, frame-sized, transparent elsewhere): the launcher's panel (surface, 88%),
    a blue-to-purple bar on its left, the caption in Inter SemiBold, an optional muted second line. On a reel it sits
    above the bottom quarter (the apps put their own buttons and text at the very bottom)."""
    w, h = size(fmt)
    tall = h > w
    im = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    margin = int(w * (0.06 if tall else 0.045))
    maxw = w - margin * 2 - int(w * (0.07 if tall else 0.04))
    f, lines, s = _fit(d, text, "bold", int(w * (0.052 if tall else 0.03)), maxw, 3 if tall else 2)
    f_sub = B.font("body", int(s * 0.66))
    sub_lines = wrap(d, sub, f_sub, maxw) if sub else []
    lh, slh = int(s * 1.25), int(f_sub.size * 1.35)
    padx, pady = int(s * 0.9), int(s * 0.62)
    tw = max([d.textlength(x, font=f) for x in lines] + [d.textlength(x, font=f_sub) for x in sub_lines])
    pw = int(tw + padx * 2 + s * 0.25)
    ph = int(lh * len(lines) + slh * len(sub_lines) + pady * 2 - (s * 0.25))
    x0 = margin if not tall else (w - pw) // 2
    y1 = int(h * (0.74 if tall else 0.9))
    y0 = y1 - ph
    r = int(s * 0.55)
    # the soft shadow under it
    sh = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    ImageDraw.Draw(sh).rounded_rectangle((x0, y0 + int(s * 0.25), x0 + pw, y1 + int(s * 0.25)), radius=r, fill=(0, 0, 0, 120))
    im.alpha_composite(sh.filter(ImageFilter.GaussianBlur(int(s * 0.5))))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle((x0, y0, x0 + pw, y1), radius=r, fill=B.rgb(B.SURFACE, 228), outline=B.rgb(B.BORDER_STRONG, 200), width=max(1, w // 960))
    bar = max(5, int(s * 0.16))
    grad = Image.linear_gradient("L").resize((bar, ph - r))
    col = Image.composite(Image.new("RGBA", (bar, ph - r), B.rgb(B.ACCENT_2, 255)), Image.new("RGBA", (bar, ph - r), B.rgb(B.ACCENT, 255)), grad)
    im.alpha_composite(col, (x0 + int(s * 0.38), y0 + r // 2))
    d = ImageDraw.Draw(im)
    y = y0 + pady - int(s * 0.12)
    tx = x0 + padx + int(s * 0.12)
    for ln in lines:
        d.text((tx, y), ln, font=f, fill=B.rgb(B.TEXT))
        y += lh
    for ln in sub_lines:
        d.text((tx, y), ln, font=f_sub, fill=B.rgb(B.MUTED))
        y += slh
    return im


# ---------------------------------------------------------------- the end card
def end_card(fmt, line=""):
    """The last card: the mark, LEYLINE, the address in the site's cyan, an optional line, the legal line small."""
    w, h = size(fmt)
    tall = h > w
    im = backdrop(fmt)
    d = ImageDraw.Draw(im)
    m = mark(int(w * (0.36 if tall else 0.17)))
    cy = int(h * (0.36 if tall else 0.34))
    im.paste(m, ((w - m.width) // 2, cy - m.height // 2), m)
    f_word = B.font("display", int(w * (0.15 if tall else 0.075)))
    word = "LEYLINE"
    track = f_word.size * 0.08
    ww = _tracked_width(d, word, f_word, track)
    y = cy + m.height // 2 + int(w * 0.02)
    _tracked(d, ((w - ww) / 2, y), word, f_word, B.rgb(B.TEXT), track)
    y += int(f_word.size * 1.12)
    f_url = B.font("bold", int(w * (0.058 if tall else 0.03)))
    uw = d.textlength(B.SITE, font=f_url)
    d.text(((w - uw) / 2, y), B.SITE, font=f_url, fill=B.rgb(B.SITE_CYAN))
    y += int(f_url.size * 1.7)
    if line:
        f_line = B.font("body", int(w * (0.036 if tall else 0.019)))
        for ln in wrap(d, line, f_line, w * 0.8):
            lw = d.textlength(ln, font=f_line)
            d.text(((w - lw) / 2, y), ln, font=f_line, fill=B.rgb(B.TEXT_2))
            y += int(f_line.size * 1.4)
    f_legal = B.font("body", int(w * (0.019 if tall else 0.0105)))
    lw = d.textlength(B.LEGAL, font=f_legal)
    d.text(((w - lw) / 2, h - int(h * 0.05)), B.LEGAL, font=f_legal, fill=B.rgb(B.MUTED))
    return im


# ---------------------------------------------------------------- the 1 s intro sting
def sting_frames(fmt, seconds=B.STING_SECONDS, fps=B.FPS):
    """The intro: the mark turns in from a quarter turn and grows into place with a glow, LEYLINE rises in under it,
    held a moment. seconds * fps frames, RGB."""
    w, h = size(fmt)
    tall = h > w
    n = max(2, int(round(seconds * fps)))
    base = backdrop(fmt)
    px = int(w * (0.34 if tall else 0.16))
    src = mark(px * 2)
    f_word = B.font("display", int(w * (0.12 if tall else 0.06)))
    track = f_word.size * 0.1
    probe = ImageDraw.Draw(base)
    ww = _tracked_width(probe, "LEYLINE", f_word, track)
    word = Image.new("RGBA", (int(ww + 4), int(f_word.size * 1.4)), (0, 0, 0, 0))
    _tracked(ImageDraw.Draw(word), (0, 0), "LEYLINE", f_word, B.rgb(B.TEXT), track)
    cy = int(h * (0.44 if tall else 0.42))
    frames = []
    for i in range(n):
        t = i / (n - 1)
        im = base.copy().convert("RGBA")
        k = ease_out(t / 0.62)
        size_now = int(px * (0.72 + 0.28 * k))
        rot = -90 * (1 - k)
        mk = src.rotate(rot, resample=Image.BICUBIC).resize((size_now, size_now), Image.LANCZOS)
        a = ease(t / 0.35)
        if a < 1:
            mk.putalpha(mk.getchannel("A").point(lambda v, a=a: int(v * a)))
        # the glow: the mark blurred, brightest as it lands
        glow_k = math.sin(math.pi * min(1.0, t / 0.8)) * 0.9
        if glow_k > 0.02:
            g = mk.filter(ImageFilter.GaussianBlur(px * 0.09))
            g.putalpha(g.getchannel("A").point(lambda v, g_=glow_k: int(v * g_)))
            im.alpha_composite(g, ((w - size_now) // 2, cy - size_now // 2))
        im.alpha_composite(mk, ((w - size_now) // 2, cy - size_now // 2))
        wk = ease((t - 0.38) / 0.4)
        if wk > 0:
            wd = word.copy()
            wd.putalpha(wd.getchannel("A").point(lambda v, a=wk: int(v * a)))
            y = cy + px // 2 + int(w * 0.03) + int((1 - wk) * f_word.size * 0.5)
            im.alpha_composite(wd, (int((w - ww) / 2), y))
        frames.append(im.convert("RGB"))
    return frames


# ---------------------------------------------------------------- marks on a draft and a missing take
def draft_badge(fmt, text="DRAFT: TODO(owner) words"):
    """A small corner tag on every frame of a video whose script still has TODO(owner) words (edit.py --final drops it
    only when none are left), so a draft never gets posted by mistake."""
    w, h = size(fmt)
    im = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    f = B.font("bold", int(w * (0.026 if h > w else 0.014)))
    tw = d.textlength(text, font=f)
    x, y, p = int(w * 0.03), int(h * 0.025), int(f.size * 0.5)
    d.rounded_rectangle((x, y, x + tw + p * 2, y + f.size + p * 2), radius=p, fill=(229, 72, 77, 220))
    d.text((x + p, y + p * 0.8), text, font=f, fill=(255, 255, 255, 255))
    return im


def placeholder(fmt, take, what=""):
    """A card standing in for a take not recorded yet (edit.py --placeholders): a preview of the cut, never a post."""
    w, h = size(fmt)
    im = backdrop(fmt)
    d = ImageDraw.Draw(im)
    f = B.font("bold", int(w * (0.045 if h > w else 0.026)))
    f2 = B.font("body", int(f.size * 0.7))
    y = h // 2 - f.size
    for ln, ff, col in (("Take not recorded yet: " + take, f, B.GOLD), (what, f2, B.TEXT_2)):
        for part in wrap(d, ln, ff, w * 0.84):
            pw = d.textlength(part, font=ff)
            d.text(((w - pw) / 2, y), part, font=ff, fill=B.rgb(col))
            y += int(ff.size * 1.4)
    return im


def main(argv):
    out = argv[0] if argv else os.path.join(os.path.dirname(os.path.abspath(__file__)), "out", "cards")
    os.makedirs(out, exist_ok=True)
    for fmt in B.FORMATS:
        title_card(fmt, "Log in to playing", kicker="Leyline in 20 seconds", sub="One launcher. Your game folder. In the world.").save(os.path.join(out, fmt + "-title.png"))
        bg = backdrop(fmt).convert("RGBA")
        bg.alpha_composite(lower_third(fmt, "Log in with your Hero Key", "Google works too"))
        bg.convert("RGB").save(os.path.join(out, fmt + "-lower-third.png"))
        end_card(fmt, "Free. Your own game folder. Bring friends.").save(os.path.join(out, fmt + "-end.png"))
        fr = sting_frames(fmt)
        for i in (0, len(fr) // 3, len(fr) * 2 // 3, len(fr) - 1):
            fr[i].save(os.path.join(out, "%s-sting-%02d.png" % (fmt, i)))
    print("cards in", out)


if __name__ == "__main__":
    main(sys.argv[1:])
