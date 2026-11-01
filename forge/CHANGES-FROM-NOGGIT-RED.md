# Changes from Noggit Red

Forge is a modified version of Noggit Red (GPL-3.0). As section 5(a) of the GPL asks, this file lists every change
and when it was made. The unmodified starting point is the first commit that adds `forge/` to this repository
("forge: import Noggit Red source, unmodified"); `git log -- forge` and `git diff <that commit> -- forge` show each
change line by line. Changed source files also say "Modified for Forge" in their first lines.

## 1 November 2026 (first Forge version)

### Left out

- `src/external/dbcd-cpp`: an empty git submodule (no code in the source tree; nothing in `src/noggit` includes it).
  Its CMake lines and its library link were removed.
- `resources/segoeui.ttf`, `resources/segoeuisb.ttf`: Microsoft's Segoe UI fonts, which may not be redistributed.
  Replaced by Inter (below).
- `resources/font_awesome.otf`: it is Font Awesome 5 **Pro** Regular 5.14.0 (its name table), a paid font that may
  not be redistributed. Replaced by Font Awesome Free 5.14.0 Solid (`resources/font_awesome_free_solid.ttf`, SIL OFL
  1.1; `resources/FontAwesome-OFL.txt`, `FontAwesome-Free-LICENSE.txt`); the 13 icons Forge uses are all in it
  (checked against its character map). They look filled instead of outlined. `font_awesome.cpp` loads the new file.
- `media/splash.png`: its text says "All rights reserved". Replaced by Forge's own splash (below).
- `src/external/imguipiemenu`: no licence anywhere in it or found for it; its only use in Noggit Red was a commented-out
  example in `MapView.cpp`, which was removed too, with its `#include`.
- `src/external/tracy`: only the profiler client is kept (`client/`, `common/`, `libbacktrace/`, the top-level headers,
  `TracyClient.cpp`, licence and readme); the profiler GUI, server, its bundled ImGui and zstd, tests and tools were
  left out (not part of the editor's build).
- `src/external/qtimgui/demo-widget`, `demo-window` (demos, off in the build).
- Parts of bundled libraries that the build never uses: Qt Advanced Docking System's `doc/`, `demo/`, `examples/`,
  Python bindings (`PyQtAds/`, `sip/`, `setup.py`, `setup.cfg`, `versioneer.py`, `MANIFEST.in`); Dear ImGui's
  `misc/fonts/` (sample fonts, not compiled in).
- `.gitlab-ci.yml`, `.gitmodules`, `Doxyfile`, `etc/` (two diffs for other game versions), `sql/` (the optional MySQL
  id table; the `USE_SQL` code stays, off by default), `todo_1.4.md`, `uid_fix_concept.md` (developer notes),
  `bin/noggit_template.conf` (an old config template).
- The upstream `.gitignore` (it ignored nearly everything); Forge's ignores only build output.

### Name and look

- The program is **Forge**: window titles (`main_window.cpp`, `MapView.cpp`), the first menu (`&Forge`), the log's first
  line, `QApplication` name "Forge" and organisation "Leyline" (so Forge's settings never mix with a Noggit install),
  messages (`opengl/context.cpp`, `map_index.cpp`, `SettingsPanel.cpp`/`.ui`, `font_noggit.cpp`, `NodeEditor.cpp`).
- The executable is `Forge.exe` (`CMakeLists.txt`: `PROJECT(Forge)`, `OUTPUT_NAME Forge`; the CMake target keeps the
  name `noggit`). Packaging (`cmake/win32_pack.cmake`, `cmake/apple_pack.cmake`) names Forge and installs its licence
  files; the NSIS installer is no longer generated (a zip is).
- The version is Forge's own (`FORGE_VERSION`, set by CI; `cmake/revision.h.in`), instead of "3.UNKNOWN".
- About box (`About.ui`): "Forge", "The Den's ground editor. Forge is based on Noggit Red, GPL-3.0.", "Built on Noggit
  Red, by the Noggit contributors:" above the unchanged list of Noggit authors, the GPL-3 and no-warranty line, and the
  source address. The icon (`media/forge_icon.png`, `media/forge.ico`, `media/res.rc`) and the splash
  (`media/forge_splash.png`) are made from the Leyline mark; the splash is now shown while Forge starts.
- Help menu: "Forum", "Homepage" and "Discord" links replaced by "Source code (GPL-3.0)" (this repository's `forge/`)
  and "Based on Noggit Red" (the Noggit Red project).
- The interface font is **Inter** (SIL Open Font License 1.1, `resources/Inter-OFL.txt`), `resources/Inter-*.ttf`,
  loaded in `application.cpp` and set as the application font.

### No game or publisher names on screen

- About: the line naming the game and its publisher was removed, and the subtitle naming the game and its client
  version was replaced (above).
- Settings: the folder dialog's title (it named the game) and the label "Game Path" -> "Game folder".
- First start: the folder dialog's title (it named the game) -> "Choose your game folder".
- Map creation wizard: the three expansion names -> "Base game (0)", "First expansion (1)", "Second expansion (2)".
- Id fix window: a heading naming the publisher -> "For unedited original maps".
- Example scripts' default seed text "noggit" -> "forge".
- `tools/check_ui_words.py` (new) fails the build when such a word is in a user-visible string; CI runs it first.
- Code names (class names, archive file names) are unchanged; players don't see them.

### Licence files added (they were missing from the Noggit Red tree)

- `LICENSE`: Noggit Red's `COPYING` (the GNU GPL version 3 text), renamed.
- `THIRD-PARTY-NOTICES.md`: every bundled and linked part with its licence.
- `src/external/glm/copying.txt` (from upstream GLM) and `SOURCE.txt`.
- `src/external/qtimgui/LICENSE` (MIT, from upstream qtimgui) and `SOURCE.txt`.
- `src/external/qtgradienteditor/LICENSE.LGPL` (the LGPL-2.1 text its headers name) and `SOURCE.txt`.
- `src/external/PNG2BLP/LICENSE` (MIT, from its upstream, wowdev/pywowlib) and `SOURCE.txt` (where it is from, and its
  bundled parts' licences); `PNG2BLP/libimagequant/COPYRIGHT` (from upstream libimagequant 2.12.2, the version in the
  tree); `PNG2BLP/zlib/LICENSE` (the licence text from `zlib.h`).

### Den mode (new: `src/forge/den_mode.hpp`, `src/forge/den_mode.cpp`; `docs/DEN-MODE.md`)

- Command line (`application.cpp`, `main_window.cpp`): `--game`, `--project`, `--map`, `--tile`, `--locked-tiles
  [<file>]`, `--den`, `--name`, `--origin`. Noggit Red read no arguments. With the Den flags Forge opens straight on
  the Den's tile (no map list, no id fix prompt, no "Return to menu").
- Writes only into the Den folder: `MPQFile::SaveFile` (`MPQ.cpp`) refuses any path outside it or inside the game
  folder; settings are an ini file in `<Den folder>/.forge/` (not the registry); the log is `<Den folder>/.forge/log.txt`
  (`Log.cpp`); a bad game folder is an error, not a folder prompt.
- Saves only the Den's tiles: `MapTile::saveTile`, `MapIndex::saveTile` and `MapIndex::saveChanged` (`MapTile.cpp`,
  `map_index.cpp`) skip other tiles and report them; `MapIndex::save` leaves the map's WDT alone in Den mode.
- New ground models get unique ids from 0x7F000000 up (`MapIndex::newGUID`, new `MapIndex::ensureUIDAbove` in
  `map_index.hpp`).
- A base snapshot (fingerprints only) of each Den tile loaded straight from the game's archives (`MapTile::finishLoading`
  notes it, `MapView::tick` makes it), and on every save the terrain delta `<name>.terrain.json` (`MapView::save`):
  the format in `../docs/terrain-delta.schema.json`.
- The Den file's props are drawn as reference boxes (`World::draw`).

### Other behaviour changes

- Outside Den mode, a new install's project folder defaults to `Documents/Forge/project/` instead of the game folder,
  so Forge never writes into a game folder unless the user sets that.
- The Tracy profiler is off by default (`NOGGIT_ENABLE_TRACY_PROFILER`): when on, it listens on the network.
- `cmake/win32_prefind.cmake`: `Boost_USE_STATIC_LIBS` can be set from the command line (CI uses Boost DLLs).
- `CMakeLists.txt`: minimum CMake 3.16 (FetchContent and C++20 need it anyway), the `src/forge` sources.
- `CMakeLists.txt`: FastNoise2's NoiseTool and tests are switched off as cache values (`FORCE`), so newer CMake
  really skips them (the NoiseTool's bundled Magnum no longer configures).
- Boost 1.89 (vcpkg): `src/opengl/shader.cpp` drops the unused `boost/filesystem/string_file.hpp` include (removed
  from Boost); `src/noggit/scripting/script_filesystem.cpp` and `script_settings.cpp` include `<fstream>` themselves.

### Build and release (new)

- `tools/github-workflows/forge.yml` (in this repository's `.github/workflows/`): Windows build with pinned tools,
  `Forge.zip` with the Qt runtime and every licence, its SHA-256, and the source tarball of the same commit, on the
  release `forge-latest`.
