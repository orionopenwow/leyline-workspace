# Forge's Den mode

Forge is the Den's ground editor: Noggit Red, renamed, as its own GPL-3.0 program (`forge/`). The Leyline launcher,
Den Lite and the browser client are MIT and never link Forge; they start `Forge.exe` with the flags below and read the
files it writes. Plan: `docs/NOGGIT-DEN-PLAN.md` (sections 1.3, 2 and 8).

## Starting Forge on a Den

```
Forge.exe --game "<game folder>" --project "<Den folder>" --map 1 --tile 1,1 --locked-tiles --den "<name>.den.json"
```

| Flag | Meaning |
|---|---|
| `--game <folder>` | The player's own game folder. Read only: Forge opens its archives read only and never writes there. |
| `--project <folder>` | The Den folder (the launcher's Den workspace). The only place Forge writes. It must not be inside the game folder (or hold it); Forge refuses to start otherwise. |
| `--map <id>` | The map to open (the Den: 1). |
| `--tile <x,y>` | The Den's tile, as in the tile file's name (`<map>_<x>_<y>.adt`; the Den: `1,1`). Forge opens straight on it: no map list, no id fix. |
| `--locked-tiles` | Only the `--tile` tile can be saved. With a file after it (`--locked-tiles tiles.txt`), the tiles in the file can be saved too: one `x,y` per line (`#` starts a comment), or a JSON list (`[[1,1],[1,2]]` or `["1,1"]`). |
| `--den <file.den.json>` | The Den file. Its props are drawn as light-blue boxes (reference only: not part of the map, never saved). Its `name` names the delta. |
| `--name <name>` | The delta's name, if not the Den file's (default: the Den folder's name). Letters, digits, `-` and `_`. |
| `--origin <x,y,z>` | The Den's middle in world yards (default: the island room, `16226.2,16257.0,13.2`). Delta model positions and the Den file's props are relative to it. |

`--game` and `--project` turn Den mode on, and then `--map` and `--tile` are required. With none of these flags Forge
behaves as Noggit Red did (except that a new install's project folder is `Documents/Forge/project`, not the game
folder). A bad command line shows one message and exits with code 2; a bad game folder exits with code 1.

## What Den mode changes

- **Writes stay in the Den folder.** Every map, image, database and id file Forge writes goes under `--project`; a write
  that would land anywhere else (or in the game folder) is refused and listed after the save. Settings live in
  `<Den folder>/.forge/` (an ini file, not the user's registry), the log in `<Den folder>/.forge/log.txt`.
- **Only the Den's tiles are saved.** Saving another tile (Ctrl+S, "save current tile", "save all") skips it and says
  "Only your Den's ground can be saved", with the list. The map's own WDT is never rewritten.
- **New ground models get ids from 0x7F000000 up** (2130706433, ...), above any id the base map uses; ids placed in an
  earlier session are kept (Forge starts above the highest one it finds on the Den's tiles and in `uid.ini`).
- **The base snapshot.** When a Den tile is loaded straight from the game's archives (no edited copy in the Den
  folder yet), Forge writes `<Den folder>/.forge/base/<map>_<x>_<y>.json`: a fingerprint per chunk and per model, no
  game data. The next saves compare against it, so the delta holds only what changed.

## The terrain delta (`<name>.terrain.json`)

Written into the Den folder on every save, one per allowed tile: the `--tile` tile as `<name>.terrain.json`, others as
`<name>.<x>_<y>.terrain.json`. The schema is `docs/terrain-delta.schema.json` (format `leyline-terrain`, version 1; the
shape is the plan's section 2.5). Forge's own MIT counterpart is the `leyline-terrain` crate; they share only this file.

| Key | Forge writes |
|---|---|
| `format`, `version` | `"leyline-terrain"`, `1` |
| `base` | kept from the delta already in the Den folder (`own:…`, `pack:…`, `blank`); else `own:<map>/<x>/<y>` |
| `open` | kept from the existing delta (the launcher decides which chunks are open); else the chunks that changed |
| `pad` | kept from the existing delta, if it has one |
| `chunks` | every chunk that differs from the base snapshot and is open; each entry is the chunk's **whole** state |
| `models` | every M2/WMO placement on the tile that isn't an unmoved base model: new ones, and moved base ones (their own uid) |
| `made` | `{"by": "forge", "at": "<UTC time>"}` |

A chunk entry:

- `i`: 0-255, `row * 16 + column` (the map file's chunk order).
- `heights`: base64 of 145 little-endian floats, absolute heights in yards, in the file's vertex order.
- `normals`: always `false` (readers compute them).
- `layers`: bottom first, at most 4: `tex` (the texture's path in the game), `flags`, and for layers 1-3 `alpha`
  (base64, 64 x 64 bytes).
- `holes`: the 16-bit hole mask. `area`: the area id. `impassable`: the flag.
- `water` (no key: no water): the first liquid layer: `type` (liquid id), `min`, `max`, `mask` (base64, 8 bytes, a
  little-endian 64-bit mask of the 8 x 8 cells).
- `colors` (no key: none): base64 of 145 x 4 bytes as the file stores them (blue, green, red, alpha; 127 = 1.0).

A model: `path`, `uid`, `x`/`y`/`z` (world yards from the Den's middle: `x` along the world's x axis, `y` along its y
axis, `z` above the middle's height), `rx`/`ry`/`rz` (degrees, the map file's three rotation values in its order),
`scale` (1.0 normal; WMOs always 1.0).

**Applying a delta** (what a reader does): start from the base tile; replace each listed chunk's heights, layers,
holes, water, colours, area and flag with the entry's; for each model, replace the base placement with the same `uid`
or add it.

**What version 1 can't carry yet** (Forge says so after the save, when it happens): a base model that was deleted
(the delta can add and move, not remove); a second liquid layer or per-vertex water heights in a chunk (only the first
layer, flat between `min` and `max`); changed chunks outside `open` (left out, counted in the save message). With no
base snapshot (the tile was already edited when Forge first saw it), every open chunk is written: correct, just larger.

## Not verified yet

Built only in CI (no Windows here): the flags, the refusals, the snapshot and the delta have not been run against a
real game folder. The phase 3 smoke test in the plan is the check: open the Den with the flags, raise a hill, save,
look at `<name>.terrain.json`, try to save a tile next door.
