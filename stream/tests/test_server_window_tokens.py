"""The server windows' one look (design/server-window.tokens.json, docs/SERVER-WINDOW.md): the launcher's host console,
Leyline HQ and the in-game windows read the same tokens.

  - the three generated blocks (hostconsole.css, admin.py, Style.lua) are up to date with the tokens file;
  - the tokens' neutrals are the launcher's own (launcher/ui/style.css --t-*, Dark and Light);
  - the accent is frost blue (after the PLAY button), the same in every place;
  - HQ pages carry the tokens, the server-window skin, Light by the system, and no hard-coded colours of their own;
  - the host console's moderation views take the frost accent; the Start tab keeps the launcher's.

    python3 stream/tests/test_server_window_tokens.py
"""
import json, os, re, subprocess, sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
passed = failed = 0


def check(name, ok, extra=""):
    global passed, failed
    if ok:
        passed += 1
        print("PASS", name)
    else:
        failed += 1
        print("FAIL", name, str(extra)[:400])


def read(*p):
    return open(os.path.join(ROOT, *p), encoding="utf-8").read()


T = json.loads(read("design", "server-window.tokens.json"))
r = subprocess.run([sys.executable, "-I", os.path.join(ROOT, "design", "make_tokens.py"), "--check"], capture_output=True, text=True)
check("the generated blocks are up to date (python3 design/make_tokens.py --check)", r.returncode == 0, r.stdout + r.stderr)

# the neutrals are the launcher's own, Dark and Light
style = read("launcher", "ui", "style.css")


def block(sel):
    m = re.search(re.escape(sel) + r"\s*\{(.*?)\n\}", style, re.S)
    return dict(re.findall(r"--t-([\w-]+):\s*([^;]+);", m.group(1))) if m else {}


ldark, llight = block(":where(:root)"), block(':where(html[data-base="light"])')
bad = [(m, k) for m, src in (("dark", ldark), ("light", llight)) for k in T["launcher_follows"] if T[m][k].lower() != src.get(k, "").lower()]
check("the neutrals equal launcher/ui/style.css (--t-*), Dark and Light", ldark and llight and not bad, bad)
check("shape: the launcher's corners", all(T["shape"][k] == ldark.get(k) for k in ("radius", "radius-sm", "radius-xs")))


def lum(h):
    c = [int(h.lstrip("#")[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    c = [x / 12.92 if x <= 0.03928 else ((x + 0.055) / 1.055) ** 2.4 for x in c]
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]


def contrast(a, b):
    la, lb = sorted((lum(a), lum(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


def hue(h):
    import colorsys
    return colorsys.rgb_to_hsv(*[int(h.lstrip("#")[i:i + 2], 16) / 255 for i in (0, 2, 4)])[0] * 360


check("frost blue: the accent's hue is the PLAY button's (195-215 degrees), Dark and Light",
      all(195 <= hue(T[m]["accent"]) <= 215 for m in ("dark", "light")), [hue(T[m]["accent"]) for m in ("dark", "light")])
check("readable: button text on the accent (3:1 or more), the strong accent on the page (4.5:1 or more)",
      all(contrast(T[m]["on-accent"], T[m]["accent"]) >= 3 and contrast(T[m]["accent-strong"], T[m]["bg"]) >= 4.5 for m in ("dark", "light")),
      [(contrast(T[m]["on-accent"], T[m]["accent"]), contrast(T[m]["accent-strong"], T[m]["bg"])) for m in ("dark", "light")])

# the three places
css = read("launcher", "ui", "hostconsole.css")
check("host console: --sw-accent is the token, its neutrals follow the launcher's --t-* (theme packs still move them)",
      f"--sw-accent: {T['dark']['accent']};" in css and f"--sw-accent: {T['light']['accent']};" in css and "--sw-text: var(--t-text);" in css)
check("host console: its --hc-* names point at the shared tokens", all(f"--hc-{k}: var(--sw-{k});" in css for k in ("edge", "gold", "shadow", "inset")))
check("host console: the console and its confirm take the frost accent, the Start tab keeps the launcher's",
      re.search(r"\.hc-sw:not\(\.hc-start\), \.hc-dlg \{[^}]*--t-accent: var\(--sw-accent\);", css) is not None)
check("host console: no old accent colours left (rgb 91,141,239 / 122,163,245)", "91, 141, 239" not in css and "122, 163, 245" not in css)

lua = read("stream", "fui-web", "Interface", "AddOns", "WrathUnleashedUI", "Style.lua")
check("in game: Style.lua's S.C is the dark set, frost accent", f'accent = rgb("{T["dark"]["accent"].lstrip("#")}")' in lua
      and f'text = rgb("{T["dark"]["text"].lstrip("#")}")' in lua)

src = read("stream", "admin.py")
hq = read("stream", "hq.py")
check("HQ: the tokens, Dark and Light (by the system), the same values", f"--sw-accent:{T['dark']['accent']}" in src
      and "@media (prefers-color-scheme:light){:root{" in src and f"--sw-accent:{T['light']['accent']}" in src)
check("HQ pages: tokens before the look, body.ly.sw, color-scheme dark light", "SW_TOKENS_CSS + ADMIN_CSS + extra" in src
      and src.count('<body class="ly sw">') == 3 and src.count('content="dark light"') == 3 and 'content="dark"' not in src)
m = re.search(r'ADMIN_CSS = """(.*?)"""', src, re.S)
hqm = re.search(r'\nCSS = """(.*?)"""', hq, re.S)
hard = re.compile(r"#[0-9a-fA-F]{3,6}\b|rgba?\(")
check("HQ's look and hq.py's CSS use tokens only (no hard-coded colours)", m and hqm and not hard.search(m.group(1)) and not hard.search(hqm.group(1)),
      (hard.findall(m.group(1)) if m else "") + (hard.findall(hqm.group(1)) if hqm else ""))
check("the tokens file says where it goes", all(x in T["about"] for x in ("hostconsole.css", "admin.py", "Style.lua")))

print(f"{passed}/{passed + failed} passed")
sys.exit(1 if failed else 0)
