"""PNG screenshots to small WebP files for the mobile release page (tools/mobile-shots/mobile_shots.cjs).

    python3 -I tools/mobile-shots/to_webp.py <png folder> <out folder>

Only the names in NAMES; at most 720 px wide (1100 for the sideways ones); quality 78; no metadata."""
import sys
from pathlib import Path

from PIL import Image

NAMES = ["phone-play", "phone-play-landscape", "phone-realms", "phone-mods", "phone-account", "phone-join", "phone-solo",
         "phone-touch"]
WIDE = {"phone-play-landscape", "phone-touch"}


def main(src: str, out: str) -> int:
    s, o = Path(src), Path(out)
    o.mkdir(parents=True, exist_ok=True)
    for n in NAMES:
        im = Image.open(s / f"{n}.png").convert("RGB")
        w = 1100 if n in WIDE else 720
        if im.width > w:
            im = im.resize((w, round(im.height * w / im.width)), Image.LANCZOS)
        im.save(o / f"{n}.webp", "WEBP", quality=78, method=6)
        print(f"{n}.webp {im.width}x{im.height} {(o / f'{n}.webp').stat().st_size // 1024} KB")
    return 0


if __name__ == "__main__":
    sys.exit(main(*sys.argv[1:3]))
