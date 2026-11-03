"""A community's own name, colours, logo and links for the website (docs/BLANK-SLATE.md, tools/commons).

The portal reads it only when WU_BRAND_FILE names a brand.json. Unset, missing or refused: nothing changes, and every
page reads exactly as it does on leylinelauncher.com. "Make my own Leyline.bat" (tools/commons/commons.py) writes the
file; the same file feeds the browser kit's look and the launcher's branding.json.

    {
      "format": 1,
      "name": "Emberline",                         1-40 characters, no game or game-maker names
      "colours": {"bg": "#12151b", "text": "#eef0f4", "accent": "#5b8def", "accent_strong": "#7aa3f5"},
      "logo": "brand-logo.png",                    optional: a .png or .webp in the site's art folder
      "links": {"site": "https://example.org", "discord": "https://discord.gg/abc", "reddit": ""}
    }

What it changes on the site, with the file set:
  * the colour tokens every page reads (--lk-*), added after the site's own so they win;
  * the round mark becomes the logo (no turning swirl), and the tab icon is the logo;
  * the word "Leyline" in text people read becomes the name (scripts, styles, addresses and ids are left alone);
  * the "Join the Discord" link, when links.discord is set.
Standard library only. Never raises on a bad file: load() returns None and problems() says why.
"""
from __future__ import annotations

import json
import re

FORMAT = 1
DEFAULTS = {
    "format": FORMAT,
    "name": "Leyline",
    "colours": {"bg": "#12151b", "text": "#eef0f4", "accent": "#5b8def", "accent_strong": "#7aa3f5"},
    "logo": "",
    "links": {"site": "", "discord": "", "reddit": ""},
}
COLOUR = re.compile(r"^#[0-9a-fA-F]{6}$")
LOGO = re.compile(r"^brand-logo\.(png|webp)$")
# The standing rules: no game or game-maker names (export.py BANNED), no money links of any kind.
BANNED = [re.compile(r"world\s*of\s*warcraft", re.I), re.compile(r"warcraft", re.I), re.compile(r"lich\s*king", re.I),
          re.compile(r"blizzard", re.I), re.compile(r"\bwow\b", re.I)]
MONEY = re.compile(r"patreon|ko-?fi|paypal|buymeacoffee|buy-me-a-coffee|opencollective|liberapay|github\.com/sponsors"
                   r"|cash\.app|venmo|streamlabs|donat|tip(?:s|jar|eee)?\b|gofundme|kickstarter|stripe|subscribestar", re.I)
DISCORD = re.compile(r"^https://(?:discord\.gg|(?:www\.)?discord\.com/invite)/[A-Za-z0-9-]{2,32}/?$")
REDDIT = re.compile(r"^https://(?:www\.|old\.)?reddit\.com/r/[A-Za-z0-9_]{2,21}/?$")
SITE = re.compile(r"^https://[A-Za-z0-9.-]+\.[A-Za-z]{2,}/?$")
# Text people read: what is NOT a script, a style, an id/address attribute or a URL (the portal's _BRAND_SKIP).
SKIP = (r'<script\b.*?</script>|<style\b.*?</style>'
        r'|\b(?:href|src|action|id|class|name|for|data-[\w-]+)="[^"]*"|https?://[^\s"\'<>]+')
WORD = r"(?<![\w/.-])Leyline(?![\w-]|\.\w)"


def problems(b) -> list[str]:
    """Why this brand file is refused ([] = fine)."""
    if not isinstance(b, dict):
        return ["brand.json must be a JSON object"]
    out = []
    if b.get("format") != FORMAT:
        out.append("format must be 1")
    name = b.get("name")
    if not isinstance(name, str) or not 0 < len(name.strip()) <= 40 or "<" in name or ">" in name or "&" in name:
        out.append("name must be 1-40 characters, without < > &")
    elif any(rx.search(name) for rx in BANNED):
        out.append("name may not use the game's or its maker's names")
    cols = b.get("colours")
    if not isinstance(cols, dict):
        out.append("colours must be an object")
    else:
        for k in DEFAULTS["colours"]:
            if not COLOUR.match(str(cols.get(k, ""))):
                out.append(f"colours.{k} must be #rrggbb")
    logo = b.get("logo", "")
    if logo and not (isinstance(logo, str) and LOGO.match(logo)):
        out.append("logo must be brand-logo.png or brand-logo.webp (or empty)")
    links = b.get("links", {})
    if not isinstance(links, dict):
        out.append("links must be an object")
        links = {}
    for k, rx in (("site", SITE), ("discord", DISCORD), ("reddit", REDDIT)):
        v = links.get(k, "")
        if v and not (isinstance(v, str) and rx.match(v)):
            out.append(f"links.{k} is not a {k} address" + (" (https://discord.gg/<code>)" if k == "discord" else ""))
    for k, v in links.items():
        if k not in DEFAULTS["links"]:
            out.append(f"links.{k}: only site, discord and reddit links are allowed")
        elif isinstance(v, str) and MONEY.search(v):
            out.append(f"links.{k}: no money, tip or donation links")
    return out


def load(path: str | None) -> dict | None:
    """The brand file at path, checked, or None (unset, missing, unreadable or refused)."""
    if not path:
        return None
    try:
        with open(path, encoding="utf-8") as f:
            b = json.load(f)
    except (OSError, ValueError):
        return None
    if problems(b):
        return None
    out = json.loads(json.dumps(DEFAULTS))
    out["name"] = b["name"].strip()
    out["colours"].update({k: b["colours"][k].lower() for k in DEFAULTS["colours"]})
    out["logo"] = b.get("logo", "") or ""
    out["links"].update({k: v for k, v in (b.get("links") or {}).items() if k in DEFAULTS["links"] and v})
    return out


def _rgb(h: str) -> tuple[int, int, int]:
    return int(h[1:3], 16), int(h[3:5], 16), int(h[5:7], 16)


def mix(a: str, b: str, t: float) -> str:
    """a moved t of the way to b (both #rrggbb)."""
    x, y = _rgb(a), _rgb(b)
    return "#" + "".join(f"{round(p + (q - p) * t):02x}" for p, q in zip(x, y))


def tokens(b: dict) -> dict[str, str]:
    """The site's --lk-* colour tokens from the four brand colours (surfaces step from the background to the text)."""
    c = b["colours"]
    bg, text, acc = c["bg"], c["text"], c["accent"]
    r, g, bl = _rgb(acc)
    return {
        "--lk-bg": bg, "--lk-chrome": mix(bg, text, .02), "--lk-surface": mix(bg, text, .05),
        "--lk-surface-2": mix(bg, text, .08), "--lk-raised": mix(bg, text, .11), "--lk-border": mix(bg, text, .10),
        "--lk-border-strong": mix(bg, text, .18), "--lk-text": text, "--lk-text-2": mix(text, bg, .18),
        "--lk-muted": mix(text, bg, .38), "--lk-accent": acc, "--lk-accent-strong": c["accent_strong"],
        "--lk-accent-soft": f"rgba({r},{g},{bl},.16)",
    }


def css(b: dict, art: str = "/wrath/art/") -> str:
    """One <style> body: the tokens (both colour schemes: a brand picks one look) and, with a logo, the mark."""
    t = ";".join(f"{k}:{v}" for k, v in tokens(b).items())
    out = f":root{{{t}}}@media (prefers-color-scheme:light){{:root{{{t}}}}}html.ly-brand{{{t}}}"
    if b.get("logo"):
        out += (f".ly-spin::before{{display:none!important;animation:none!important}}"
                f".ly-spin::after{{background-image:url({art}{b['logo']})!important}}")
    return out


def tab_icon(b: dict, art: str = "/wrath/art/") -> bytes | None:
    if not b.get("logo"):
        return None
    kind = "webp" if b["logo"].endswith(".webp") else "png"
    return f'<link rel="icon" type="image/{kind}" href="{art}{b["logo"]}">'.encode()


_PAGE = re.compile(f"({SKIP})|({WORD})", re.S)
_JSON = re.compile(f"(https?://[^\\s\"'<>]+)|({WORD})")


def rename(text: str, name: str, kind: str = "text/html") -> str:
    """'Leyline' in what people read becomes name (HTML: outside scripts, styles, ids and addresses; JSON: values
    that are not addresses). Unchanged when the name is Leyline."""
    if name == "Leyline":
        return text
    rx = _PAGE if kind == "text/html" else _JSON
    return rx.sub(lambda m: m.group(1) or name, text)


def apply(b: dict | None, body: bytes, ctype: str, art: str = "/wrath/art/") -> bytes:
    """A page or JSON answer as the brand file says it should read (b None: unchanged)."""
    if not b or not body:
        return body
    kind = ctype.split(";", 1)[0].strip().lower()
    if kind not in ("text/html", "application/json"):
        return body
    try:
        text = body.decode("utf-8")
    except UnicodeDecodeError:
        return body
    out = rename(text, b["name"], kind)
    if kind == "text/html" and "</head>" in out and 'id="brand-file"' not in out:
        icon = tab_icon(b, art)
        if icon and 'rel="icon"' not in out[:8192]:
            i = out.find("<title", 0, 8192)
            if i >= 0:
                out = out[:i] + icon.decode() + out[i:]
        out = out.replace("</head>", f'<style id="brand-file">{css(b, art)}</style></head>', 1)
    return body if out == text else out.encode("utf-8")
