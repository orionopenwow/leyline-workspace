# Forge: third-party parts and their licences

Forge as a whole is under the GNU General Public License, version 3 (`LICENSE`), as Noggit Red is. Every part below is
under a licence that allows it in a GPL-3.0 program. In `Forge.zip` each text is in `licenses/`; in the source tree it
is next to the part. Checked 1 November 2026.

## In the source tree (`src/external/`, `resources/`)

| Part | Licence | Text |
|---|---|---|
| Noggit Red (everything not listed below), by the Noggit contributors | GPL-3.0 | `LICENSE` |
| NodeEditor (Dmitry Pinaev) | BSD-3-Clause | `src/external/NodeEditor/LICENSE` |
| Qt Advanced Docking System | LGPL-2.1 | `src/external/QtAdvancedDockingSystem/LICENSE` |
| libnoise | LGPL-2.1 | `src/external/libnoise/LICENSE.md` |
| qt-color-widgets | LGPL-3.0 | `src/external/qt-color-widgets/COPYING` |
| qtgradienteditor (Qt 4 tools sources, Nokia 2009) | LGPL-2.1 (the alternative its headers offer; Forge takes it) | `src/external/qtgradienteditor/LICENSE.LGPL`, `SOURCE.txt` |
| framelesshelper | MIT | `src/external/framelesshelper/LICENSE` |
| ImGuizmo | MIT | `src/external/imguizmo/LICENSE` |
| qtimgui (Ryohei Ikegami) | MIT (from upstream: github.com/seanchas116/qtimgui) | `src/external/qtimgui/LICENSE` |
| Dear ImGui 1.76 | MIT | `src/external/qtimgui/imgui/LICENSE.txt` |
| robin-map (tsl) | MIT | header of `src/external/tsl/robin_map.h` |
| Tracy (client only; off by default) | BSD-3-Clause | `src/external/tracy/LICENSE` |
| GLM | MIT (GLM offers MIT or the Happy Bunny licence) | `src/external/glm/copying.txt` |
| PNG2BLP (from github.com/wowdev/pywowlib, `blp/PNG2BLP/native`; files identical) | MIT (that repository's licence) | `src/external/PNG2BLP/LICENSE`, `SOURCE.txt` |
| libimagequant 2.12.2 (in PNG2BLP) | GPL-3.0-or-later, base code under pngquant's original licence | `src/external/PNG2BLP/libimagequant/COPYRIGHT` |
| libpng (in PNG2BLP) | libpng licence | `src/external/PNG2BLP/libpng/LICENSE` |
| png++ (in PNG2BLP) | BSD-3-Clause | `src/external/PNG2BLP/pngpp/COPYING` |
| zlib 1.2.11 (in PNG2BLP) | zlib licence | `src/external/PNG2BLP/zlib/LICENSE` |
| libtxc_dxtn (in PNG2BLP) | MIT-style (Roland Scheidegger) | header of `src/external/PNG2BLP/libtxc_dxtn/txc_dxtn.h` |
| Inter (the interface font) | SIL Open Font License 1.1 | `resources/Inter-OFL.txt` |
| Font Awesome Free 5.14.0, Solid (`resources/font_awesome_free_solid.ttf`, icons) | SIL Open Font License 1.1 | `resources/FontAwesome-OFL.txt`, `resources/FontAwesome-Free-LICENSE.txt` |
| `resources/noggit_font.ttf` (Noggit's own tool icons) | Noggit Red's file, "Copyright (c) Noggit" in the font (GPL-3.0 with the program); see the note below | `LICENSE` |
| Forge icon and splash (`media/forge_*`) | made for Forge from the Leyline mark | `LICENSE` |

## Fetched when building (CMake FetchContent) or linked from vcpkg

| Part | Licence | In `Forge.zip` |
|---|---|---|
| Qt 5.15 (Core, Gui, Widgets, OpenGL, Network, Xml, Svg, Quick) | LGPL-3.0, used as unmodified, replaceable DLLs; source: download.qt.io/archive/qt/5.15/ | `licenses/Qt-LGPL-3.0.txt`, `SOURCE.txt` |
| Boost | Boost Software License 1.0 | `licenses/boost-thread.txt` |
| StormLib (Ladislav Zezula), with zlib and bzip2 | MIT; zlib licence; bzip2 licence | `licenses/stormlib.txt`, `zlib.txt`, `bzip2.txt` |
| Lua | MIT | `licenses/lua.txt` |
| nlohmann_json | MIT | `licenses/json.txt` |
| lodepng | zlib licence | `licenses/lodepng.txt` |
| FastNoise2 (tswow fork) | MIT | `licenses/fastnoise2.txt` |
| sol2 (tswow fork) | MIT | `licenses/sol2.txt` |
| Microsoft C++ runtime DLLs | Microsoft Visual C++ redistributable terms | app-local copies |

Note on `noggit_font.ttf`: its name table says "Copyright (c) Noggit", but its description field is copied from Font
Awesome ("The web's most popular icon set and toolkit."). Whether any of its 109 glyphs were drawn from Font Awesome
(Free or Pro) is not stated anywhere in the tree. To check with the Noggit Red authors; if they came from Pro, the
font must be redrawn or its icons swapped for Font Awesome Free ones.

## Not included

- No game data of any kind. Forge reads the player's own game folder, read only.
- Noggit Red's Segoe UI fonts, its Font Awesome **Pro** font (a paid, non-redistributable font; replaced by Font
  Awesome Free, whose Solid style has all 13 icons Forge uses), its "All rights reserved" splash, its unlicensed pie
  menu and its empty `dbcd-cpp` submodule were left out (`CHANGES-FROM-NOGGIT-RED.md`).

## Source

Forge's complete source is the `forge/` folder of github.com/orionopenwow/leyline-workspace; each `forge-latest`
release also carries `forge-source.tar.gz` of the exact commit it was built from (`SOURCE.txt` in the zip names it).
