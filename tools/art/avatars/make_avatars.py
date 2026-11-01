"""Leyline's player portraits: 24 round flat-graphic fantasy characters on a grey slate medallion.

Owner, 2026-11-01: "grey slate stylize these user icons and not letters ... a cool array of flat graphics art characters".
Original art drawn here by hand (path data below), generic fantasy archetypes named by role; no game's characters, races
or art. The look: flat silhouettes, faces mostly in shadow, bright glowing eyes as the focal point, 2-3 muted tones plus
one accent, a dark tinted inner background inside a grey stone rim; a prop may break out of the circle.

    python3 tools/art/avatars/make_avatars.py     writes stream/www-files/art/avatars/<id>.svg and launcher/ui/art/avatars/

The order of IDS is the default-picture order (stream/profiles.py PORTRAITS, launcher/ui/avatars.js): never reorder,
only append, or players' default pictures change.
"""
from __future__ import annotations

import os
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
OUT = (os.path.join(ROOT, "stream", "www-files", "art", "avatars"), os.path.join(ROOT, "launcher", "ui", "art", "avatars"))


def eyes(pts, c, rx=2.1, ry=1.25, halo=3.8):
    """Glowing eyes: a soft halo, the coloured eye and a hot white centre."""
    out = []
    for x, y in pts:
        out.append(f'<ellipse cx="{x}" cy="{y}" rx="{halo}" ry="{halo * .7:.2g}" fill="{c}" opacity=".3"/>'
                   f'<ellipse cx="{x}" cy="{y}" rx="{rx}" ry="{ry}" fill="{c}"/>'
                   f'<ellipse cx="{x}" cy="{y}" rx="{rx * .45:.2g}" ry="{ry * .5:.2g}" fill="#fff"/>')
    return "".join(out)


def glow(x, y, r, c):
    return f'<circle cx="{x}" cy="{y}" r="{r}" fill="{c}" opacity=".28"/>'


def frame(name, bg, inner, over=""):
    """The medallion: grey stone rim (gradient, speckle, bevel), a dark tinted inside, the figure clipped to it and the
    props drawn over the rim."""
    a, b = bg
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="96" height="96" role="img" aria-label="{name}">'
            '<defs>'
            '<linearGradient id="k" x1="0" y1="0" x2=".4" y2="1"><stop offset="0" stop-color="#b4b4ae"/>'
            '<stop offset=".45" stop-color="#77776f"/><stop offset="1" stop-color="#34342f"/></linearGradient>'
            '<pattern id="s" width="9" height="9" patternUnits="userSpaceOnUse">'
            '<path d="M1 1h2v1H1zM6 4h1v2H6zM3 7h2v1H3z" fill="#000" opacity=".22"/>'
            '<path d="M5 1h1v1H5zM1 5h1v1H1zM8 8h1v1H8z" fill="#fff" opacity=".18"/></pattern>'
            f'<radialGradient id="g" cx=".5" cy=".36" r=".7"><stop offset="0" stop-color="{a}"/><stop offset="1" stop-color="{b}"/></radialGradient>'
            '<clipPath id="c"><circle cx="32" cy="32" r="26.5"/></clipPath>'
            '</defs>'
            '<circle cx="32" cy="32" r="31.5" fill="#1d1d1a"/>'
            '<circle cx="32" cy="32" r="31" fill="url(#k)"/><circle cx="32" cy="32" r="31" fill="url(#s)"/>'
            '<path d="M6 22A28 28 0 0 1 42 5" fill="none" stroke="#e4e4dc" stroke-opacity=".45" stroke-width="1" stroke-linecap="round"/>'
            '<circle cx="32" cy="32" r="27.6" fill="#191917"/>'
            '<circle cx="32" cy="32" r="26.5" fill="url(#g)"/>'
            f'<g clip-path="url(#c)">{inner}</g>'
            '<circle cx="32" cy="32" r="26.2" fill="none" stroke="#000" stroke-opacity=".45" stroke-width="1.2"/>'
            f'{over}</svg>')


# Each: (id, name, (bg light, bg dark), inner, over). Shoulders sit low; the head is centred about (32, 29).
P = []

# Knight: a closed great helm, the visor slit glowing, a red plume sweeping out over the rim.
P.append(("knight", "Knight", ("#3b5680", "#0f1828"),
    '<path d="M6 64C7 51 15 46 24 44H40C49 46 57 51 58 64Z" fill="#4c5562"/>'
    '<ellipse cx="15" cy="50" rx="9" ry="6" fill="#7d8794"/><ellipse cx="49" cy="50" rx="9" ry="6" fill="#7d8794"/>'
    '<path d="M21 41V24C21 15 26 11 32 11S43 15 43 24V41C38 44 26 44 21 41Z" fill="#8f99a6"/>'
    '<path d="M32 11C38 11 43 15 43 24V41C40 43 36 43.6 32 43.8Z" fill="#6c7581"/>'
    '<rect x="22.5" y="25" width="19" height="4" rx="1" fill="#0b0f15"/>'
    '<path d="M27 34h1.6v1.6H27zM30 34h1.6v1.6H30zM33 34h1.6v1.6H33zM36 34h1.6v1.6H36z" fill="#2c3139"/>'
    + eyes([(27.5, 27), (36.5, 27)], "#d4ecff", 2.3, 1.1),
    '<path d="M32.5 12C35 4 45 1 56 5C49 6 43 9 39 15C37 13 35 12.4 32.5 12Z" fill="#c0392b"/>'
    '<path d="M35 11C40 6 47 4 53 5" fill="none" stroke="#7d1e15" stroke-width="1"/>'))

# Ranger: a deep hood, green eyes in the dark, a bow drawn across the right edge.
P.append(("ranger", "Ranger", ("#2e6449", "#0a1a12"),
    '<path d="M6 64C8 51 18 45 32 45S56 51 58 64Z" fill="#33472a"/>'
    '<path d="M18 47C15 34 20 17 34 9C33 12 34 13 36 14C45 20 49 35 46 47C40 43 24 43 18 47Z" fill="#56733f"/>'
    '<path d="M36 14C45 20 49 35 46 47C43 45 40 44 37 44C41 36 40 22 36 14Z" fill="#41592f"/>'
    '<path d="M24 39C23 30 27 22 32 22S41 30 40 39C36 42 28 42 24 39Z" fill="#0b110a"/>'
    + eyes([(28.4, 31), (35.6, 31)], "#86ff9e")
    + '<circle cx="32" cy="47" r="2.4" fill="#e0b04a"/>',
    '<path d="M51 3C63 18 63 46 51 61" fill="none" stroke="#8a5a2b" stroke-width="3" stroke-linecap="round"/>'
    '<path d="M51 3V61" stroke="#e2dccb" stroke-width=".8"/>'))

# Wizard: a tall bent hat, a long pale beard, gold eyes under the brim, a staff with a glowing gem out top-left.
P.append(("wizard", "Wizard", ("#56378a", "#150b25"),
    '<path d="M6 64C8 51 17 45 32 45S56 51 58 64Z" fill="#26346a"/>'
    '<path d="M22 33C22 46 27 56 32 61C37 56 42 46 42 33Z" fill="#c9c3b5"/>'
    '<path d="M32 40V58M28 40C28 48 29 52 30 55M36 40C36 48 35 52 34 55" fill="none" stroke="#9a9384" stroke-width="1"/>'
    '<ellipse cx="32" cy="31" rx="9.5" ry="7.5" fill="#120d1c"/>'
    + eyes([(28.3, 30.5), (35.7, 30.5)], "#ffd27a")
    + '<ellipse cx="32" cy="25" rx="18" ry="3.6" fill="#33468a"/>'
    '<path d="M20 25C25 21 27 12 30 2C33 9 39 15 47 19C43 20 42 22 44 25Z" fill="#3c55a0"/>'
    '<path d="M21.5 23.2C28 25 37 25 43 23.2L43.6 21.4C37 23 28 23 21.9 21.4Z" fill="#e0b04a"/>',
    '<path d="M11 63L13 9" stroke="#6b4a2b" stroke-width="2.8" stroke-linecap="round"/>'
    + glow(13, 7, 6.5, "#6ff0ff") + '<path d="M13 1L16.5 7L13 13L9.5 7Z" fill="#7ff4ff"/><path d="M13 1L16.5 7H9.5Z" fill="#d8feff"/>'))

# Rogue: an angular hood and a scarf over the face, narrow white eyes, a dagger out at the bottom right.
P.append(("rogue", "Rogue", ("#7a2329", "#1e0709"),
    '<path d="M6 64C9 51 20 45 32 45S55 51 58 64Z" fill="#26262b"/>'
    '<path d="M17 48C14 34 19 18 33 10C45 17 50 33 47 48Z" fill="#3d3d45"/>'
    '<path d="M33 10C45 17 50 33 47 48H40C42 34 40 20 33 10Z" fill="#2e2e35"/>'
    '<path d="M24 37C24 28 27 23 32 23S40 28 40 37Z" fill="#0a0a0d"/>'
    '<path d="M21 35C27 38 37 38 43 35L45 47C37 50 27 50 19 47Z" fill="#55555f"/>'
    '<path d="M24 41C29 43 35 43 40 41" fill="none" stroke="#3d3d45" stroke-width="1"/>'
    + eyes([(28.2, 31.2), (35.8, 31.2)], "#f4f4ff", 2.3, .9),
    '<path d="M47 55L58 37.5L60.5 37L59.6 39.6L49.4 57Z" fill="#cdd1d8"/>'
    '<path d="M44 52.5L52.5 59" stroke="#8a8f99" stroke-width="2.6" stroke-linecap="round"/>'
    '<path d="M47.5 57.5L43.5 62" stroke="#3a2a20" stroke-width="3" stroke-linecap="round"/><circle cx="42.6" cy="62.6" r="1.9" fill="#ff4d4d"/>'))

# Cleric: a pale hood with a gold halo behind, gold eyes, a sun on the chest.
P.append(("cleric", "Cleric", ("#8c6a24", "#271b06"),
    '<circle cx="32" cy="26" r="16" fill="none" stroke="#f5c542" stroke-width="2.2"/>'
    '<path d="M6 64C8 51 18 45 32 45S56 51 58 64Z" fill="#a39b86"/>'
    '<path d="M26 45L24 64H29L30 45ZM38 45L40 64H35L34 45Z" fill="#d9a63a"/>'
    '<path d="M19 47C18 32 22 15 32 15S46 32 45 47C40 44 24 44 19 47Z" fill="#ddd6c3"/>'
    '<path d="M32 15C42 15 46 32 45 47C42 45.5 39 45 36 44.6C40 35 38 21 32 15Z" fill="#bdb5a0"/>'
    '<path d="M24 38C24 29 27 24 32 24S40 29 40 38C37 41 27 41 24 38Z" fill="#17130c"/>'
    + eyes([(28.4, 31.5), (35.6, 31.5)], "#ffe9a0")
    + '<circle cx="32" cy="54" r="2.8" fill="#f5c542"/>'))

# Bard: a tilted cap with a long gold feather out top-left, blue eyes, a lute neck out at the right.
P.append(("bard", "Bard", ("#1f7176", "#061d1f"),
    '<path d="M6 64C8 51 18 46 32 46S56 51 58 64Z" fill="#4a1d35"/>'
    '<path d="M26 46L32 54L38 46Z" fill="#c9b48a"/>'
    '<path d="M21 26C20 40 25 44 32 44S44 40 43 26Z" fill="#14100f"/>'
    + eyes([(28, 32), (36, 32)], "#8fdcff")
    + '<path d="M16 25C15 17 25 13 34 14C43 15 50 19 49 23C44 27 22 28 16 25Z" fill="#7a3253"/>'
    '<path d="M16 25C22 28 44 27 49 23L49 25.5C44 29.5 22 30.5 16 27.5Z" fill="#4a1d35"/>',
    '<path d="M24 18C15 11 9 5 3 2C8 9 14 15 21 21Z" fill="#ffd66e"/><path d="M21.5 19.5L5 4" stroke="#b8872a" stroke-width=".9"/>'
    '<ellipse cx="47" cy="57" rx="8.5" ry="6.5" transform="rotate(-38 47 57)" fill="#b07a3a"/>'
    '<circle cx="47.5" cy="56.5" r="2" fill="#3a2410"/>'
    '<path d="M51 52L62 40" stroke="#6b4422" stroke-width="3" stroke-linecap="round"/>'))

# Smith: a squat helmeted figure with a huge copper beard, gold eyes, a hammer out top-right.
P.append(("smith", "Smith", ("#8a4518", "#220e04"),
    '<path d="M3 64C5 50 14 44 32 44S59 50 61 64Z" fill="#3a2a1e"/>'
    '<path d="M22 35C22 25 26 21 32 21S42 25 42 35Z" fill="#140c07"/>'
    + eyes([(28.2, 30.5), (35.8, 30.5)], "#ffcf5a")
    + '<path d="M20 33C20 45 24 55 32 59C40 55 44 45 44 33C40 37 36 38 32 38S24 37 20 33Z" fill="#c46a2a"/>'
    '<path d="M28 40C28 47 29 52 31 56M36 40C36 47 35 52 33 56" fill="none" stroke="#8a4519" stroke-width="1.2"/>'
    '<path d="M24.5 35.5C28 33 31 34 32 36C33 34 36 33 39.5 35.5C36 38.5 28 38.5 24.5 35.5Z" fill="#8a4519"/>'
    '<path d="M21 28C21 19 26 14 32 14S43 19 43 28Z" fill="#848a93"/><rect x="19.5" y="26.5" width="25" height="2.8" rx="1" fill="#5a5f68"/>'
    '<circle cx="32" cy="51" r="1.8" fill="#e0b04a"/>',
    '<path d="M44 48L55 15" stroke="#6b4422" stroke-width="3" stroke-linecap="round"/>'
    '<path d="M48 9L62 13.5L60 19.5L46 15Z" fill="#9aa0a8"/><path d="M48 9L62 13.5L61.4 15.4L47.4 10.9Z" fill="#c8ccd2"/>'))

# Druid: a leaf-green hood crowned with antlers out over the rim, amber eyes.
P.append(("druid", "Druid", ("#4d6a25", "#111a06"),
    '<path d="M6 64C8 51 18 45 32 45S56 51 58 64Z" fill="#2b4a20"/>'
    '<path d="M10 56l4-5 3 5 4-6 3 6 4-6 4 6 4-6 3 6 4-6 3 5 4-5 4 8H6Z" fill="#3f6b2e"/>'
    '<path d="M19 47C18 32 22 19 32 19S46 32 45 47C40 44 24 44 19 47Z" fill="#4c7d36"/>'
    '<path d="M24 39C24 30 27 26 32 26S40 30 40 39C37 42 27 42 24 39Z" fill="#0c120a"/>'
    + eyes([(28.4, 32.5), (35.6, 32.5)], "#ffb347")
    + '<path d="M32 17C29 20 29 23 32 25C35 23 35 20 32 17Z" fill="#a8f06a"/>',
    '<path d="M25 21C21 15 18 9 13 4M20.5 13.5C17 13 14 11 11 8.5M23 17.5C20 18.5 16 18 13 16.5M39 21C43 15 46 9 51 4M43.5 13.5C47 13 50 11 53 8.5M41 17.5C44 18.5 48 18 51 16.5" '
    'fill="none" stroke="#d2bf92" stroke-width="2.4" stroke-linecap="round"/>'))

# Captain: a gold-trimmed tricorn, a dark beard, one bright eye and an eye patch, gold epaulettes.
P.append(("captain", "Captain", ("#25597a", "#08151f"),
    '<path d="M6 64C8 51 18 45 32 45S56 51 58 64Z" fill="#1f2c44"/>'
    '<ellipse cx="15" cy="50" rx="7.5" ry="4" fill="#d9a441"/><ellipse cx="49" cy="50" rx="7.5" ry="4" fill="#d9a441"/>'
    '<path d="M29 45L32 55L35 45Z" fill="#e8e2d2"/>'
    '<path d="M23 37C23 28 27 24 32 24S41 28 41 37Z" fill="#12100f"/>'
    '<path d="M22 35C22 45 27 51 32 51S42 45 42 35C38 39 26 39 22 35Z" fill="#4a4440"/>'
    '<path d="M21 26L41 35" stroke="#000" stroke-width="1.2"/><ellipse cx="28" cy="31" rx="3.2" ry="2.6" fill="#000"/>'
    + eyes([(36, 31)], "#f2f6ff", 2.2, 1.3)
    + '<path d="M9 24C16 27 22 15 32 15S48 27 55 24C51 31 40 28 32 28.5S13 31 9 24Z" fill="#22242b"/>'
    '<path d="M9 24C13 31 24 28.5 32 28.5S51 31 55 24" fill="none" stroke="#d9a441" stroke-width="1.3"/>'
    '<circle cx="32" cy="20" r="1.8" fill="#d9a441"/>'))

# Wanderer: a drooping cowl and a scarf, white eyes in the dark, a lantern held out at the bottom left.
P.append(("wanderer", "Wanderer", ("#525a6b", "#14171d"),
    '<path d="M6 64C8 50 18 44 32 44S56 50 58 64Z" fill="#3b342d"/>'
    '<path d="M16 50C12 34 19 16 32 14C45 16 52 34 48 50C42 45 22 45 16 50Z" fill="#5b5248"/>'
    '<path d="M32 14C45 16 52 34 48 50C45 48 42 46.5 39 46C43 36 40 22 32 14Z" fill="#4a423a"/>'
    '<path d="M23 40C22 31 26 25 32 25S42 31 41 40Z" fill="#0e0c0b"/>'
    '<path d="M21 39C27 42 37 42 43 39L44 46C37 49 27 49 20 46Z" fill="#7a6f60"/>'
    + eyes([(28.2, 33), (35.8, 33)], "#ffffff", 1.9, 1.1),
    '<path d="M17 40V47" stroke="#2a2622" stroke-width="1.2"/>'
    + glow(17, 54, 9, "#ffc85a")
    + '<path d="M13.5 47H20.5L21.5 49V58L20.5 59.5H13.5L12.5 58V49Z" fill="#2a2622"/><rect x="14.3" y="49.3" width="5.4" height="8" rx="1" fill="#ffd36e"/>'
    '<path d="M17 50.5C15.8 52.5 15.8 54.5 17 56C18.2 54.5 18.2 52.5 17 50.5Z" fill="#fff6d8"/><path d="M12.5 59.5H21.5L20 61.5H14Z" fill="#2a2622"/>'))

# Warrior: an iron helm with a nose guard and great bone horns out over the rim, orange eyes, a fur collar.
P.append(("warrior", "Warrior", ("#762020", "#1c0505"),
    '<path d="M4 64C6 51 15 44 32 44S58 51 60 64Z" fill="#4a3424"/>'
    '<path d="M8 52L12 46L15 51L19 45L22 50L26 44L29 49L32 43L35 49L38 44L42 50L45 45L49 51L52 46L56 52L58 60H6Z" fill="#6b4e33"/>'
    '<path d="M22 30H42V41H22Z" fill="#0f0b0a"/>'
    + eyes([(28, 31.5), (36, 31.5)], "#ff8a3d")
    + '<path d="M20 29C20 19 25 14 32 14S44 19 44 29Z" fill="#78736d"/>'
    '<path d="M32 14C39 14 44 19 44 29H38C38 22 36 17 32 14Z" fill="#5d5853"/>'
    '<path d="M20 28H26.5V40L20 38ZM44 28H37.5V40L44 38Z" fill="#5d5853"/>'
    '<path d="M30.7 27H33.3V36L32 37.5L30.7 36Z" fill="#78736d"/>'
    '<path d="M27 40L32 49L37 40Z" fill="#3a2a1e"/>',
    '<path d="M21.5 23C14 23 7 17 5 5C10 11 16 14 23 16Z" fill="#e2d6b8"/><path d="M42.5 23C50 23 57 17 59 5C54 11 48 14 41 16Z" fill="#e2d6b8"/>'
    '<path d="M21.5 23C14 23 7 17 5 5C9 13 15 18 22 19.5ZM42.5 23C50 23 57 17 59 5C55 13 49 18 42 19.5Z" fill="#b3a586"/>'))

# Mage: long pointed ears, pale hair, a gold circlet with a violet crystal, violet eyes.
P.append(("mage", "Mage", ("#17707a", "#04191c"),
    '<path d="M6 64C8 51 18 45 32 45S56 51 58 64Z" fill="#162c46"/>'
    '<path d="M22 45L32 58L42 45Z" fill="#244a6e"/>'
    '<path d="M20 28C18 42 16 52 13 60L25 52L26 32ZM44 28C46 42 48 52 51 60L39 52L38 32Z" fill="#c7ccd6"/>'
    '<path d="M25 30L11 20L24 36ZM39 30L53 20L40 36Z" fill="#3e3653"/>'
    '<path d="M24 32C24 24 27 20 32 20S40 24 40 32C40 38 36 43 32 43S24 38 24 32Z" fill="#13111c"/>'
    + eyes([(28.4, 31.5), (35.6, 31.5)], "#d9a6ff")
    + '<path d="M22 30C21 20 26 15 32 15S43 20 42 30C39 24 35 22.5 32 22.5S25 24 22 30Z" fill="#dfe3ea"/>'
    '<path d="M23.5 24.5C28 22.5 36 22.5 40.5 24.5" fill="none" stroke="#d9b45a" stroke-width="1.3"/>'
    + glow(32, 22.5, 4, "#c58bff") + '<path d="M32 19.5L34 22.5L32 25.5L30 22.5Z" fill="#d6a8ff"/>'))

# Brute: a huge frame with spiked pauldrons, a heavy brow over red eyes, ivory tusks.
P.append(("brute", "Brute", ("#6a6420", "#1b1906"),
    '<path d="M1 64C2 48 12 40 32 40S62 48 63 64Z" fill="#4a3424"/>'
    '<path d="M6 46L9 36L13 44L16 35L19 43ZM58 46L55 36L51 44L48 35L45 43Z" fill="#b3a586"/>'
    '<ellipse cx="13" cy="48" rx="10" ry="6" fill="#5e4430"/><ellipse cx="51" cy="48" rx="10" ry="6" fill="#5e4430"/>'
    '<path d="M20 42C18 34 19 22 24 19C28 17 36 17 40 19C45 22 46 34 44 42C40 46 24 46 20 42Z" fill="#6d7552"/>'
    '<path d="M32 17.5C36 17.5 38 18 40 19C45 22 46 34 44 42C41 44.5 36 45.5 33 45.7C39 36 38 24 32 17.5Z" fill="#5a6143"/>'
    '<path d="M21 28C26 25 38 25 43 28V33H21Z" fill="#14150d"/>'
    '<path d="M20.5 27C26 23.5 38 23.5 43.5 27L43 29C38 26.5 26 26.5 21 29Z" fill="#4b5238"/>'
    + eyes([(27.8, 31), (36.2, 31)], "#ff5a3c", 2, 1.1)
    + '<path d="M22 37C26 41 38 41 42 37V42C38 46 26 46 22 42Z" fill="#4b5238"/>'
    '<path d="M25.5 40L24 32.5L28 39ZM38.5 40L40 32.5L36 39Z" fill="#f0e6c8"/>'
    '<path d="M30 18C30 12 34 10 36 5C37.5 10 35.5 14 34 18Z" fill="#1c1a17"/>'))

# Witch: a crooked wide-brimmed hat out over the rim, wine-dark hair, green eyes, a glowing buckle.
P.append(("witch", "Witch", ("#64297a", "#16081c"),
    '<path d="M6 64C8 51 18 45 32 45S56 51 58 64Z" fill="#1b1722"/>'
    '<path d="M19 29C14 42 16 52 11 61L26 51L25 32ZM45 29C50 42 48 52 53 61L38 51L39 32Z" fill="#6e2c3d"/>'
    '<path d="M24 32C24 25 27 22 32 22S40 25 40 32C40 38 36 43 32 43S24 38 24 32Z" fill="#100c12"/>'
    + eyes([(28.4, 32), (35.6, 32)], "#74ff8e")
    + '<path d="M7 28C15 24 24 25 32 25S49 24 57 28C49 32 15 32 7 28Z" fill="#2a2433"/>'
    '<path d="M21 26.5L22.5 22.5C29 23.8 36 23.8 41.5 22.3L43 26.5C36 28 28 28 21 26.5Z" fill="#4a2d5c"/>'
    '<rect x="29.6" y="23.4" width="4.8" height="3.8" rx=".6" fill="none" stroke="#9cff6a" stroke-width="1.2"/>',
    '<path d="M22.5 22.5C25 15 29 9 38 4C41 2 45 3 47 6C43 6 40 9.5 39.5 13.5C39 17 40.5 20 41.5 22.3C36 23.8 29 23.8 22.5 22.5Z" fill="#2a2433"/>'
    '<path d="M38 4C41 2 45 3 47 6C43 6 40 9.5 39.5 13.5C39 17 40.5 20 41.5 22.3L37 22.9C35 16 35 9 38 4Z" fill="#211c29"/>'))

# Revenant: a tattered hood around a pale skull, cold blue eyes deep in the sockets.
P.append(("revenant", "Revenant", ("#38485f", "#0c1018"),
    '<path d="M5 64C7 50 18 44 32 44S57 50 59 64L54 60L50 64L45 59L40 64L36 59L32 64L28 59L24 64L19 59L14 64L10 60Z" fill="#262a33"/>'
    '<path d="M16 50C13 34 19 15 32 13C45 15 51 34 48 50L44 46L40 50L36 46L32 50L28 46L24 50L20 46Z" fill="#3e434f"/>'
    '<path d="M32 13C45 15 51 34 48 50L44 46L40 50C43 36 40 22 32 13Z" fill="#323642"/>'
    '<path d="M22 42C21 31 26 23 32 23S43 31 42 42Z" fill="#07080b"/>'
    '<path d="M25.5 31C25.5 25 28 22.5 32 22.5S38.5 25 38.5 31C38.5 34.5 37 36 36 38.5H28C27 36 25.5 34.5 25.5 31Z" fill="#9c978a"/>'
    '<path d="M28 38.5H36V42H28Z" fill="#7d796e"/><path d="M30 38.5V42M32 38.5V42M34 38.5V42" stroke="#2a2a2a" stroke-width=".7"/>'
    '<ellipse cx="29" cy="30.5" rx="2.6" ry="2.4" fill="#000"/><ellipse cx="35" cy="30.5" rx="2.6" ry="2.4" fill="#000"/>'
    '<path d="M31.2 33.5H32.8L32 35.5Z" fill="#2a2a2a"/>'
    + eyes([(29, 30.6), (35, 30.6)], "#6fd6ff", 1.4, 1.1, 3.6)))

# Shieldmaid: a silver helm with white wings out over the rim, gold braids, white eyes.
P.append(("shieldmaid", "Shieldmaid", ("#3a6288", "#0f1a26"),
    '<path d="M6 64C8 51 18 45 32 45S56 51 58 64Z" fill="#4a5262"/>'
    '<path d="M10 64C11 55 16 51 22 50L32 54L42 50C48 51 53 55 54 64Z" fill="#5d6678"/>'
    '<path d="M21 31C19 40 20 48 18.5 57H23C24 48 24 40 25.5 34ZM43 31C45 40 44 48 45.5 57H41C40 48 40 40 38.5 34Z" fill="#d9a441"/>'
    '<path d="M19.5 41H24M19.5 47H23.8M40 41H44.5M40.2 47H44.5" stroke="#9c7426" stroke-width="1"/>'
    '<path d="M24 32C24 26 27 23 32 23S40 26 40 32C40 38 36 42 32 42S24 38 24 32Z" fill="#100f14"/>'
    + eyes([(28.4, 31.5), (35.6, 31.5)], "#f4f8ff")
    + '<path d="M21 29C21 19 26 15 32 15S43 19 43 29Z" fill="#bcc3cc"/>'
    '<path d="M32 15C38 15 43 19 43 29H38C38 23 36 18 32 15Z" fill="#8f97a2"/>'
    '<path d="M30.8 27H33.2V36L32 37.5L30.8 36Z" fill="#bcc3cc"/><rect x="20.5" y="27" width="23" height="2.4" rx="1" fill="#d9a441"/>',
    '<path d="M21.5 24C15 21 8 13 6 4C10 8 14 9 17 9C14 11 14 14 16 15C13 17 15 20 21.5 21.5Z" fill="#ece9e1"/>'
    '<path d="M42.5 24C49 21 56 13 58 4C54 8 50 9 47 9C50 11 50 14 48 15C51 17 49 20 42.5 21.5Z" fill="#ece9e1"/>'
    '<path d="M21.5 24C15 21 8 13 6 4C9 12 14 18 21.5 21.5ZM42.5 24C49 21 56 13 58 4C55 12 50 18 42.5 21.5Z" fill="#c4c0b6"/>'))

# Monk: a shaved head in shadow below the brow, a glowing mark on the brow, saffron robe and prayer beads.
P.append(("monk", "Monk", ("#91601a", "#271905"),
    '<path d="M6 64C8 51 18 45 32 45S56 51 58 64Z" fill="#c97a2a"/>'
    '<path d="M18 47L44 64H34L14 50Z" fill="#8f5218"/>'
    '<path d="M21 46C24 53 40 53 43 46" fill="none" stroke="#3a2414" stroke-width="2.2" stroke-dasharray="2 1.2"/>'
    '<circle cx="21" cy="30" r="2.6" fill="#5a4232"/><circle cx="43" cy="30" r="2.6" fill="#5a4232"/>'
    '<circle cx="32" cy="29" r="11" fill="#7a5a44"/>'
    '<path d="M38 19.5C41.5 21.5 43 25 43 29C43 35 38 40 32 40C37 34 39 26 38 19.5Z" fill="#664a37"/>'
    '<path d="M21.2 28C24 30 40 30 42.8 28C42.5 35 38 40 32 40S21.5 35 21.2 28Z" fill="#1d140e"/>'
    + eyes([(28, 31.3), (36, 31.3)], "#7ff0d6", 2, 1)
    + glow(32, 24.5, 3.2, "#5ff0d0") + '<circle cx="32" cy="24.5" r="1.4" fill="#9ffbe6"/>'))

# Alchemist: wild grey hair, brass goggles with glowing lenses for eyes, a bubbling flask out at the bottom right.
P.append(("alchemist", "Alchemist", ("#1f6a52", "#061a14"),
    '<path d="M6 64C8 51 18 45 32 45S56 51 58 64Z" fill="#4e3824"/>'
    '<path d="M24 46L26 64H38L40 46Z" fill="#7a5a3a"/>'
    '<path d="M20 27L18 16L24 20L25 11L30 17L33 9L36 17L41 11L41 20L46 16L44 27Z" fill="#a3a8ae"/>'
    '<path d="M22 28C22 40 26 44 32 44S42 40 42 28Z" fill="#131210"/>'
    '<path d="M20 29H44" stroke="#3a2a1c" stroke-width="2"/>'
    '<circle cx="27.3" cy="29.5" r="4.8" fill="#c9a04a"/><circle cx="36.7" cy="29.5" r="4.8" fill="#c9a04a"/>'
    '<path d="M31.5 29.5H32.5" stroke="#c9a04a" stroke-width="2"/>'
    '<circle cx="27.3" cy="29.5" r="3.3" fill="#c6ff5a"/><circle cx="36.7" cy="29.5" r="3.3" fill="#c6ff5a"/>'
    '<circle cx="26.3" cy="28.4" r="1.2" fill="#fff"/><circle cx="35.7" cy="28.4" r="1.2" fill="#fff"/>'
    '<path d="M28 39C30 40 34 40 36 39" fill="none" stroke="#3a3530" stroke-width="1.2"/>',
    '<path d="M46.5 44H51.5V49.5C56 51 58 54 58 57.5C58 61.6 54.9 64 49 64S40 61.6 40 57.5C40 54 42 51 46.5 49.5Z" fill="#cfe9f2" fill-opacity=".55"/>'
    '<path d="M41 57C44 55.5 54 55.5 57 57C57 61 54 63.3 49 63.3S41 61 41 57Z" fill="#ff5ad1"/>'
    + glow(49, 58, 8, "#ff5ad1") + '<rect x="45.5" y="42.5" width="7" height="2.4" rx=".8" fill="#8a5a2b"/>'
    '<circle cx="47" cy="59.5" r="1" fill="#ffd0f2"/><circle cx="51" cy="58" r=".7" fill="#ffd0f2"/>'))

# Golem: a cracked stone block head, a glowing cyan eye slit, a rune on the chest.
P.append(("golem", "Golem", ("#5e4c34", "#16110b"),
    '<path d="M3 64L5 48L18 42H46L59 48L61 64Z" fill="#615c53"/>'
    '<path d="M5 48L18 42L22 52L8 56ZM59 48L46 42L42 52L56 56Z" fill="#8a8478"/>'
    '<path d="M22 17L42 16L45 37L33 41L20 38Z" fill="#8a8478"/>'
    '<path d="M42 16L45 37L33 41L35 29Z" fill="#6d685f"/>'
    '<path d="M26 18L28 23L25 26M40 34L37 37" fill="none" stroke="#45413b" stroke-width="1.1"/>'
    '<path d="M22 27.5L43 26.5L43.5 31.5L22.5 32.5Z" fill="#0d0d0c"/>'
    + eyes([(27.5, 29.6), (37.5, 29.2)], "#5ff6ff", 2.2, 1.1)
    + glow(32, 53, 6, "#5ff6ff") + '<path d="M32 47L37 53L32 59L27 53Z" fill="none" stroke="#7ff8ff" stroke-width="1.6" stroke-linejoin="round"/><circle cx="32" cy="53" r="1.6" fill="#7ff8ff"/>'))

# Fox
P.append(("fox", "Fox", ("#1d6a72", "#061b1e"),
    '<path d="M13 64C15 52 22 46 32 46S49 52 51 64Z" fill="#a4501c"/>'
    '<path d="M24 50C27 48 37 48 40 50L32 64Z" fill="#efe6d6"/>'
    '<path d="M16 29L17 9L29 19ZM48 29L47 9L35 19Z" fill="#d9752e"/><path d="M18.5 23L19 13L25.5 19ZM45.5 23L45 13L38.5 19Z" fill="#3a1a0a"/>'
    '<path d="M14 26C17 20 24 17 32 17S47 20 50 26C47 33 40 41 32 47C24 41 17 33 14 26Z" fill="#d9752e"/>'
    '<path d="M32 17C40 17 47 20 50 26C47 33 40 41 32 47Z" fill="#c4662a"/>'
    '<path d="M19 31C24 34.5 28 36.5 32 36.5S40 34.5 45 31C41 38 36.5 43 32 47C27.5 43 23 38 19 31Z" fill="#efe6d6"/>'
    '<ellipse cx="32" cy="44.3" rx="2.2" ry="1.6" fill="#1a0e08"/>'
    '<path d="M21.5 27.5L29 28.5L27 31.5ZM42.5 27.5L35 28.5L37 31.5Z" fill="#5a2a10"/>'
    + eyes([(25.8, 29.2), (38.2, 29.2)], "#ffd24a", 1.9, 1.1, 3.2)))

# Owl
P.append(("owl", "Owl", ("#3b3e86", "#0c0d24"),
    '<path d="M13 64C11 48 18 36 32 36S53 48 51 64Z" fill="#5f4a33"/>'
    '<path d="M24 48L28 51L32 48L36 51L40 48M22 54L26 57L30 54M34 54L38 57L42 54" fill="none" stroke="#a88c66" stroke-width="1.5"/>'
    '<path d="M16 19L12 6L25 13ZM48 19L52 6L39 13Z" fill="#7a6044"/>'
    '<path d="M14 30C14 18 22 12 32 12S50 18 50 30C50 40 42 46 32 46S14 40 14 30Z" fill="#8a6e4e"/>'
    '<circle cx="25" cy="29" r="8.6" fill="#cbb791"/><circle cx="39" cy="29" r="8.6" fill="#cbb791"/>'
    + glow(25, 29, 6.5, "#ffc23a") + glow(39, 29, 6.5, "#ffc23a")
    + '<circle cx="25" cy="29" r="5" fill="#ffc23a"/><circle cx="39" cy="29" r="5" fill="#ffc23a"/>'
    '<circle cx="25" cy="29" r="2.6" fill="#120c06"/><circle cx="39" cy="29" r="2.6" fill="#120c06"/>'
    '<circle cx="24" cy="28" r="1" fill="#fff"/><circle cx="38" cy="28" r="1" fill="#fff"/>'
    '<path d="M29.5 33.5H34.5L32 40Z" fill="#e0a83a"/>'))

# Wolf: a grey head under a crescent moon, ice-blue eyes.
P.append(("wolf", "Wolf", ("#2c3c78", "#080c1e"),
    '<circle cx="18" cy="17" r="6.5" fill="#efe3ae"/><circle cx="21" cy="15" r="5.8" fill="#24336a"/>'
    '<path d="M8 64C10 54 18 47 32 49S54 54 56 64Z" fill="#6b737c"/>'
    '<path d="M12 58L16 52L19 57L22 51L25 56L28 50L32 55L36 50L39 56L42 51L45 57L48 52L52 58Z" fill="#878f98"/>'
    '<path d="M19 24L17 6L29 16ZM45 24L47 6L35 16Z" fill="#8a929b"/><path d="M20.5 19L19.5 10L26 15.5ZM43.5 19L44.5 10L38 15.5Z" fill="#3a3f46"/>'
    '<path d="M16 25C18 17 25 14 32 14S46 17 48 25C48 32 42 36 40 42C38 48 35 52 32 52S26 48 24 42C22 36 16 32 16 25Z" fill="#9aa3ad"/>'
    '<path d="M32 14C39 14 46 17 48 25C48 32 42 36 40 42C38 48 35 52 32 52Z" fill="#848c96"/>'
    '<path d="M26 34C28 32 36 32 38 34C38 44 35 50 32 50S26 44 26 34Z" fill="#cdd3d9"/>'
    '<path d="M20 25L30 28.5L27.5 32.5ZM44 25L34 28.5L36.5 32.5Z" fill="#3a3f46"/>'
    '<path d="M29.4 45.5H34.6L32 48.6Z" fill="#14161a"/>'
    + eyes([(26.6, 29.4), (37.4, 29.4)], "#8fe4ff", 1.8, 1, 3.2)))

# Cat: a black cat with big green eyes and a gold bell.
P.append(("cat", "Cat", ("#7a3060", "#1e0a16"),
    '<path d="M15 64C15 52 22 46 32 46S49 52 49 64Z" fill="#26262e"/>'
    '<path d="M17 30L16 8L29 19ZM47 30L48 8L35 19Z" fill="#3d3d4a"/><path d="M19 22L18.5 13L25 18.5ZM45 22L45.5 13L39 18.5Z" fill="#7a4a5a"/>'
    '<path d="M15 34C15 23 22 18 32 18S49 23 49 34C49 42 41 48 32 48S15 42 15 34Z" fill="#3d3d4a"/>'
    '<path d="M32 18C42 18 49 23 49 34C49 42 41 48 32 48C38 41 39 27 32 18Z" fill="#33333f"/>'
    + glow(25, 32, 5.5, "#b6ff4a") + glow(39, 32, 5.5, "#b6ff4a")
    + '<ellipse cx="25" cy="32" rx="4.4" ry="3.4" fill="#b6ff4a"/><ellipse cx="39" cy="32" rx="4.4" ry="3.4" fill="#b6ff4a"/>'
    '<ellipse cx="25" cy="32" rx="1" ry="3" fill="#0a0a0a"/><ellipse cx="39" cy="32" rx="1" ry="3" fill="#0a0a0a"/>'
    '<path d="M30.4 38H33.6L32 39.8Z" fill="#c98a9a"/>'
    '<path d="M27 40L16 38.5M27 41.5L17 43M37 40L48 38.5M37 41.5L47 43" stroke="#8a8a96" stroke-width=".6"/>'
    '<path d="M21 48.5C26 51 38 51 43 48.5V51.5C38 54 26 54 21 51.5Z" fill="#a8323e"/>'
    '<circle cx="32" cy="54.5" r="3" fill="#f2c24a"/><path d="M30.5 55H33.5" stroke="#8a6a1a" stroke-width=".8"/>'))

# Drake: a jade-scaled horned head, fiery slit eyes, horns out over the rim.
P.append(("drake", "Drake", ("#77231a", "#1d0805"),
    '<path d="M16 64C18 54 24 48 32 48S46 54 48 64Z" fill="#2a6049"/>'
    '<path d="M27 52H37M26 56H38M25.5 60H38.5" stroke="#c9b88a" stroke-width="2.2"/>'
    '<path d="M18 31C18 21 24 16 32 16S46 21 46 31C46 39 40 43 38 50C36 54 28 54 26 50C24 43 18 39 18 31Z" fill="#3f8a6a"/>'
    '<path d="M32 16C40 16 46 21 46 31C46 39 40 43 38 50C36 53 33.5 53.5 32 53.5Z" fill="#33765a"/>'
    '<path d="M16 30L11 27L17 35ZM48 30L53 27L47 35ZM18 38L13 38L20 43ZM46 38L51 38L44 43Z" fill="#2a6049"/>'
    '<path d="M20 26C24 24 28 25 30 28L21 30ZM44 26C40 24 36 25 34 28L43 30Z" fill="#1b3f30"/>'
    + eyes([(25.5, 29.6), (38.5, 29.6)], "#ffb03a", 2.2, 1.3)
    + '<ellipse cx="25.5" cy="29.6" rx=".55" ry="1.2" fill="#3a1500"/><ellipse cx="38.5" cy="29.6" rx=".55" ry="1.2" fill="#3a1500"/>'
    '<ellipse cx="29.6" cy="48" rx="1" ry=".7" fill="#0f2a1f"/><ellipse cx="34.4" cy="48" rx="1" ry=".7" fill="#0f2a1f"/>',
    '<path d="M22.5 21C18 15 13 9 6 7C12 12 15 16 19.5 24.5ZM41.5 21C46 15 51 9 58 7C52 12 49 16 44.5 24.5Z" fill="#d9cfb0"/>'
    '<path d="M22.5 21C18 15 13 9 6 7C11 11 15 17 19.5 24.5ZM41.5 21C46 15 51 9 58 7C53 11 49 17 44.5 24.5Z" fill="#b3a586" opacity=".6"/>'))

IDS = [p[0] for p in P]
NAMES = {p[0]: p[1] for p in P}


def build() -> dict:
    out = {}
    for pid, name, bg, inner, *over in P:
        out[pid] = frame(name, bg, inner, over[0] if over else "")
    return out


def main() -> int:
    svgs = build()
    for d in OUT:
        os.makedirs(d, exist_ok=True)
        for pid, body in svgs.items():
            with open(os.path.join(d, pid + ".svg"), "w", encoding="utf-8", newline="\n") as f:
                f.write(body + "\n")
    big = {k: len(v) for k, v in svgs.items() if len(v) > 4096}
    print(f"{len(svgs)} portraits -> {', '.join(os.path.relpath(d, ROOT) for d in OUT)}"
          + (f"; over 4 KB: {big}" if big else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
