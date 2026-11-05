"""Leyline's look for reels and videos: the colours, fonts and art the launcher and the website already use.

The colours are the launcher's Dark tokens (launcher/ui/style.css, `:where(:root)`) and the website's nav cyan
(stream/wrath_portal.py SITE_CSS `--n-cyan`); test_reels.py reads both files so a change there shows up here as a
failing test, not as videos in an old colour. Fonts are the ones both ship (SIL OFL 1.1, licences beside them):
Big Shoulders Display SemiBold ("Leyline Display", the site's and the launcher's display face) for titles, Inter for
everything else. Pillow reads the .woff2 files as they are.

Owner 4 Nov: "reels showing features in Leyline's style".
"""
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
UIART = os.path.join(ROOT, "launcher", "ui", "art")

# ---------------------------------------------------------------- colours (launcher/ui/style.css, Dark)
BG = "#12151b"            # --t-bg
CHROME = "#15171e"        # --t-chrome
SURFACE = "#1a1d25"       # --t-surface
SURFACE_2 = "#20242e"     # --t-surface-2
RAISED = "#262b37"        # --t-raised
BORDER_STRONG = "#343a47"  # --t-border-strong
TEXT = "#eef0f4"          # --t-text
TEXT_2 = "#c8cdd7"        # --t-text-2
MUTED = "#9aa1ae"         # --t-muted
ACCENT = "#5b8def"        # --t-accent (the blue of the mark)
ACCENT_STRONG = "#7aa3f5"  # --t-accent-strong (kickers: .hs-kick)
ACCENT_2 = "#8b6cf0"      # --t-accent-2 (the purple of the mark)
OK = "#4fd18b"            # --t-ok
GOLD = "#f1cf86"          # --t-gold (the client version chip)
# the website (stream/wrath_portal.py SITE_CSS .ly-nav)
SITE_CYAN = "#5fe3ff"     # --n-cyan: links and the address on the end card
SITE_NIGHT = "#060816"    # the nav's rgba(6,8,22,.9)

# the CSS each colour comes from: (file, token) -> the value above (test_reels.py checks them)
SOURCES = {
    "BG": ("launcher/ui/style.css", "--t-bg"), "SURFACE": ("launcher/ui/style.css", "--t-surface"),
    "SURFACE_2": ("launcher/ui/style.css", "--t-surface-2"), "RAISED": ("launcher/ui/style.css", "--t-raised"),
    "TEXT": ("launcher/ui/style.css", "--t-text"), "TEXT_2": ("launcher/ui/style.css", "--t-text-2"),
    "MUTED": ("launcher/ui/style.css", "--t-muted"), "ACCENT": ("launcher/ui/style.css", "--t-accent"),
    "ACCENT_STRONG": ("launcher/ui/style.css", "--t-accent-strong"), "ACCENT_2": ("launcher/ui/style.css", "--t-accent-2"),
    "OK": ("launcher/ui/style.css", "--t-ok"), "GOLD": ("launcher/ui/style.css", "--t-gold"),
    "SITE_CYAN": ("stream/wrath_portal.py", "--n-cyan"),
}

# ---------------------------------------------------------------- fonts and art
FONT_DISPLAY = os.path.join(UIART, "leyline-display.woff2")             # Big Shoulders Display 600
FONT_BODY = os.path.join(UIART, "fonts", "inter-latin-400-normal.woff2")
FONT_BODY_BOLD = os.path.join(UIART, "fonts", "inter-latin-600-normal.woff2")
MARK = os.path.join(UIART, "brand", "leyline-mark-512.png")             # the swirl and the crystal
BACKDROP = os.path.join(UIART, "leyline-bg.webp")                       # the launcher's starfield
# Windows / Linux fallbacks when this Pillow can't read .woff2 (no brotli in its FreeType)
FALLBACKS = {
    "display": ["C:/Windows/Fonts/bahnschrift.ttf", "C:/Windows/Fonts/segoeuib.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"],
    "body": ["C:/Windows/Fonts/segoeui.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"],
    "bold": ["C:/Windows/Fonts/segoeuib.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"],
}

# ---------------------------------------------------------------- formats
FPS = 30
FORMATS = {
    "reel": (1080, 1920),   # 9:16: Shorts, Reels, TikTok, a phone-first Reddit post
    "wide": (1920, 1080),   # 16:9: the launch video, flow videos, YouTube, the website
}
STING_SECONDS = 1.0
TITLE_SECONDS = 1.8
END_SECONDS = 2.6
XFADE = 0.25                 # the cross-fade between parts
SITE = "leylinelauncher.com"
LEGAL = "Not affiliated with or endorsed by Blizzard Entertainment."   # the one allowed line naming it (GR-RULES)

# Words players never see (the standing brand rule; the only exception is LEGAL above)
BRANDS = ("world of warcraft", "wow", "warcraft", "lich king", "blizzard", "wotlk")


def rgb(h, a=None):
    h = h.lstrip("#")
    t = tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))
    return t if a is None else t + (int(a),)


def font(kind, size):
    """A Pillow font: kind is display, body or bold. The repo's own faces first, the system's if Pillow can't."""
    from PIL import ImageFont
    first = {"display": FONT_DISPLAY, "body": FONT_BODY, "bold": FONT_BODY_BOLD}[kind]
    for p in [first] + FALLBACKS[kind]:
        try:
            return ImageFont.truetype(p, size)
        except OSError:
            continue
    return ImageFont.load_default(size)
