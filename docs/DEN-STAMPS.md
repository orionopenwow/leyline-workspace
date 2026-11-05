# Den stamps: ready-made places that look real

Owner, 3 Nov: "Jaina not very good at making stamps / setups: do a research plan and copy general or relative doodad
setups from the actual game using screenshots that make sense, and fix her build ability." And later: "a stamp should
also have the ability to create the 1:1 Lion's Pride from Goldshire, for testing how it works."

What changed:

- **The stamps are composed, not typed in.** `tools/den-stamps/compose.py` lays each one out by the rules below with
  each piece's size (`tools/den-stamps/footprints.json`, our own estimates) and a fixed seed, and writes
  `realm/custom/lua_scripts/den-stamps/<name>.den.json`. Nine stamps, 15 to 34 pieces each.
- **Den Lite fits them to the real models** (`labs/den-lite/src/stamps.rs`): with the player's own model table
  (GameObjectDisplayInfo, already copied into the Dens folder for the gallery) a small thing over a table stands on
  that table's real top, and two standing pieces that would be in each other are moved apart.
- **Jaina builds like a person** (`labs/den-lite/src/dojo.rs`): she walks into the place's middle, builds the big pieces
  first and the small ones last, turns and casts at each, and then steps back on your side to look at it.
- **Copy a place from your own game** (`.den copy lionspride`): see the last section.
- **4 Nov: the stamp library, pictures, turn / mirror / size, placing with the mouse and saving a selection as a
  stamp** are the builder's Tools window: docs/DEN-TOOLS.md.

## 1. How the game's own small places are put together

We looked at how the game's camps, inns, markets, farms and docks are set out (screenshots of the zones, and the
general environment-art and level-design guides below). These rules come up again and again:

1. **One focal point.** Every small scene has one thing you notice first: the fire, the bar, the forge, the statue.
   Level designers call it the "dominant": the composition is planned so that it is the first object you notice, and
   nothing covers it ([Composition in level design, Game Developer](https://www.gamedeveloper.com/design/composition-in-level-design)).
2. **Things face what they are for.** Seats face the fire or the table; tents and stalls open onto the fire or the
   street; a statue looks down the path you came up. Benches round a fire stand in a ring facing it, with a gap for
   the way in.
3. **Clusters of 3 to 5, by an anchor.** Small things are never spread evenly: crates and barrels gather by a tent or a
   wall, goods in front of a stall, food on a table. One guide warns against over-arranging natural clutter: real
   mess is uneven ([Game Developer](https://www.gamedeveloper.com/design/composition-in-level-design)).
4. **Mixed sizes, big to small.** "Objects that are all the same size will make an impression of a flat and chaotic
   scene" ([Game Developer](https://www.gamedeveloper.com/design/composition-in-level-design)). A place is laid out
   large (building, tent, stall), then medium (tables, benches, racks), then small (bread, bottles, candles).
5. **Spacing from sizes.** Pieces never stand in each other; chairs are pulled in to a table's side, not into it.
6. **Natural jitter.** A few degrees of turn and a hand's width of offset on everything that people move (chairs,
   crates, stools); none on what is built (a building, a fence run, a bar counter).
7. **Paths stay clear.** The way in from you to the focal point, the street of a market, the walk down a dock, the
   path between crop rows. Tavern guides say the same: keep a clear route from the entry to the bar and the hearth,
   with clutter behind the counter and not in the walkways
   (a cozy tavern interior guide on the Boostroom blog).
8. **Spaces serve a purpose.** "The space should feel like it fits in the world and serves a purpose"
   ([5 rules of environment design, 80.lv](https://80.lv/articles/5-rules-of-environment-design)): a forge has its
   quench barrel within a step or two, and smiths keep forge, vise and anvil "no more than 2 to 3 steps" apart in a
   work circle ([forge set-up, I Forge Iron](https://www.iforgeiron.com/topic/8213-set-up/)).

Scene by scene:

| Scene | Focal point | Around it |
|---|---|---|
| Camp | the fire | benches and stools in a ring facing it, a gap toward you; bedrolls further out, their foot to the fire; a tent behind, opening to the fire; stores by the tent; a table with food; a weapon rack and torches at the edge |
| Inn | the bar | kegs behind it, stools facing it; tables with chairs pulled in at their sides and food on them; a rug under a table; a hearth in the corner with a stool; lanterns at the door, a mailbox, barrels by the wall outside |
| Market | the street | stalls in two rows facing it, goods set out in front of each; a counter with food; a bench; lamp posts at the ends; a banner |
| Garden | the path | fence on three sides and a gap on yours; rows of one kind of plant each side of the path; hay in a far corner; basket and water barrel; a bench outside looking in; lanterns at the gap |
| Dock | the end | boards in a line; barrels and crates along the edges; a clear walk; lanterns at the corners; a fishing chair at the end looking out |
| Small house | the room | bed along the back wall with a candle beside it; a small table with two chairs and a book; a rug; a barrel in the corner; lantern and mailbox at the door; flowers |
| Smithy | the forge | its fire beside it; the work bench between the smith and you; the quench barrel a step away; racks along the back; crates of finished work; coal sacks; an armour stand out front for buyers |
| Reading nook | the little table | two high-backed chairs turned to it on a rug; a book and a candle; a bench; a side table of books; a lantern; potted plants |
| Shrine | the statue | braziers either side; candles in an arc at its feet; prayer mats and stone benches facing it; banners and bushes behind; flowers |

## 2. The composer's rules (tools/den-stamps/compose.py)

- x is ahead of you, y to your left; you come in from -x. A piece's front is its own +x (seats: the way you sit).
- Every piece has a box (w front to back, d side to side, h) from `footprints.json`, turned by its yaw. Two standing
  pieces' boxes keep 0.1 yard apart (separating axis test); rugs and boards may be under things; a fence run's
  segments join; a building's own box doesn't count (its inside is a room the furniture must stay in).
- Things on a table: inside its top's edge, apart from each other, at the table's height.
- Paths: boxes no standing piece may touch.
- Facing: `facing_point` turns a piece to its target, plus a few degrees; `ring`, `dining` (chairs pulled in at a
  table's sides) and `cluster` (3 to 5 things by an anchor, each the nearest free spot) build on it.
- Order: each recipe lays the anchors, then the medium pieces, then the small ones; a seed per stamp (crc32 of its
  name) makes the same file every time. `python3 tools/den-stamps/compose.py --check` says whether the shipped files
  are its own.
- Tests (`tools/den-stamps/test_compose.py`): 8 to 10 stamps of 10 to 40 pieces; no overlaps; every entry in the
  catalog (objects.rs) with a size; small things inside their table's edge at its height; seats, tents and stalls
  within 15 degrees of their target; paths clear; inn and house furniture inside the walls; the same output twice;
  the shipped files are the composer's.

The sizes are estimates (no game data in the repo). **To check on the owner's PC**: which way a tent, a vendor stall,
a rack and a bed open (the composer assumes the model's front is its +x, as seats are), and whether the estimates are
close. Den Lite corrects heights and overlaps with the real boxes when it has them; the facing it can't know.

## 3. Jaina's build (labs/den-lite/src/dojo.rs, stamps.rs)

"Jaina, build a camp" (or `.den jaina build camp`, `.den stamp camp now`):

1. The stamp goes into the Den at once (saved, one undo), in front of you.
2. She walks to its **focal point**, or the free spot nearest it: never inside a piece (her half yard counted).
3. She starts one long cast, then each piece appears **in build order**: the building, rugs and boards, the big pieces
   (tents, stalls, fences, a bed), furniture, small things, last what stands on a table; within each, nearest her
   first. A big piece takes longer (400 ms; a building 700) than furniture (250) or a small thing (180). She turns to
   each piece and casts at its spot as it appears.
4. When the last piece is up she **steps back on your side**, just past the place's edge, ends facing it, and says it
   is done; 2.5 seconds later she follows you again.
5. Other names: camp, campsite, bonfire; tavern, pub; bazaar, stalls; farm, plot; pier, jetty; cottage, home; forge,
   blacksmith; reading, library, study; altar, temple; training, ring (the Dojo); Lion's Pride, Goldshire
   (`stamps.rs` NAMES). An unknown word is answered with the list.

`tools/den-check/den_check.py features` builds every stamp in turn with Jaina and takes a picture of each when it is
done (`logs\den-features\NN-stamp-<name>.png`), then undoes it, so the owner can judge them on the real game.

## 4. Copy a place from your own game (.den copy)

`.den copy lionspride` (or "Jaina, build the Lion's Pride") builds Goldshire's inn as it is in your game, for testing
how well a real place comes across.

- **Made on your PC, never shipped.** When the launcher starts your Den it reads, from your own game archives, the
  map tile with the place, the inn's building file and your model table, and writes
  `<Dens folder>\stamps-local\lionspride.den.json` (launcher `host/lite.rs` `copy_places`, the conversion in
  `leyline-terrain` `place.rs`). Once: delete the file to copy it again. What ships is only the place's name, the
  map's folder name, the tiles to look in, words for the building's file name and a radius (`place.rs` PLACES).
- **What is copied:** the building's placement in the tile; its doodads (the default set and the placement's own set);
  the loose models within 22 yards. Each becomes a piece when your model table has a display with the same model file
  (Den Lite shows any display through its model carrier). The building itself is placed when a display's model is
  its building file; else the inside only, and the stamp's line says so. The file lists what had no match
  (`skipped`, by file name).

Limits, honestly:

- Many of the game's doodads have no gameobject display (no row in the model table with that file): they are left out.
  Expect gaps: walls' small details, plants, some furniture.
- The building may have no display at all; then you get the furniture floating in the open, at the right heights.
- Positions and turns are converted from the tile's numbers; the building's facing offset (`place.rs` YAW_OFFSET,
  180 degrees) is to check on the real game. A wrong offset turns the whole copy as one; the building's own pieces
  stay right relative to each other; the loose models outside would then be turned on their own spots.
- Lights, sounds, the building's own lighting and its doors are not copied; the ground under it is your Den's.
- It is large: up to 400 pieces and about 40 yards across. A small Den leaves out what falls outside it (`.den grow`
  first). Jaina's build is paced to end within 40 seconds.
- Only the places in `place.rs` PLACES (the Lion's Pride for now). TODO(owner): which others are worth having.
