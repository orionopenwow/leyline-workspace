# Your Den, next (5 Nov 2026): the mouse proven, tales, Jaina writing them, Sky Island, Dens as codes

Owner, 5 Nov: "continued development of the den features to match the movement on the mouse and everything, invent
novel scripting and smoke tests and auto tests and whatever you need to compare it and guarantee it works, suggest and
polish and add more features to the den that make sense as a viral feature and allow people to create worlds and
questlines and more with the novelty of claude in jaina and jaina's brain and more". Later the same night: "don't
stream the island to github but definitely make a local version for me and implement its core idea into dens so we can
make dens super light and novel too without much server setup and super lightweight local and passing around".

Branch `agent/den-next`. The network / transport layer (main.rs, login.rs, the socket plumbing) is untouched: the
phone-solo branch owns it. Everything new is in feature modules.

**What was and wasn't run here.** crates.io is blocked from the cloud, so Den Lite's crate was not compiled. The new
logic was written std-only where it could be and compiled and tested here with plain `rustc --test`: `tale.rs` (6
tests, including a 3 000-case fuzz), `movemodel.rs` (5), `denpack.rs` (3), `skyisland.rs` (3). The launcher's
`ui/denpack.js` (4 node tests), `ui/den.js` (22 browser tests, one new), the Agents page (4), the in-game tracker
`DenTale.lua` (19 Lua checks under texlua; every other WrathUnleashedUI Lua test still passes with it in the .toc), and
`tools/den-check` (59 Python tests, 9 new) were all run here. **Not compiled:** `replay.rs`, `taleplay.rs`, `packs.rs`,
the hooks in `world.rs`, `lib.rs`, `den.rs`, `combat.rs`, the new `tests/client.rs` tests, and the launcher's
`deeplink.rs` and `agents/den.rs` changes. Press **`den-replay`** (or `den-lite-all`) first; it shows any compile error.

## 1. The mouse, proven: the replay harness

The editor's mouse behaviour is now written down twice: once in gizmo.rs (what the Den does) and once as an
**expected model** in `labs/den-lite/src/movemodel.rs`, written again from the rules in DEN-EDITOR-PARITY.md and
DEN-PLACEMENT.md, not from gizmo.rs. The **replay harness** (`labs/den-lite/src/replay.rs`) plays an input script
through gizmo.rs's own `cmd`, the same verbs Gizmo.lua sends, on a headless Den. After every line it compares the editor
with the model:

- whether something is in hand;
- where the copy is (within 2 cm and a third of a degree);
- where every prop is.

A disagreement fails that frame, with its line number and both answers.

The model's rules (Noggit's, and the owner's LANDSCAPE editor's):

1. A card picked is armed, turned as the last one placed (or as you face). Nothing shows until the cursor is over the
   ground; a first ray past the horizon shows it 5 yards ahead.
2. In hand, each hover puts the foot where the ray meets the ground: on the grid when Snap is on, a grid step inside
   the Den's edge, on the top of a prop under it (within 2.5 yards of the ground, never its own), else on the ground
   plus its own lift.
3. A still click on a prop picks it up without a jump: its foot keeps its offset from the ray's ground point, at most
   its own footprint. The next click puts it down; Esc puts it back.
4. A press-and-drag keeps the grabbed point under the cursor on the plane at the grabbed height.
5. Q / E and the wheel turn what is in hand; the next card starts turned as the last one placed.
6. One commit is one undo.

### The `.moves` script

One command a line, `#` comments. Places are yards from the Den's middle; screen points are u, v from -1 to 1 (right,
up), as Gizmo.lua sends them.

```text
cam fov 1.57 pivot 2 dist 12 pitch 0.5   # the build camera (default: Den Lite's first guesses)
at 5 5 face 135                          # the player walked here
grid 1 | zoom 40 | aspect 1.6
prop chair 6 0 [face 40]                 # set-up: a prop already there
mode build | mode play
card chair [u v]                         # a gallery card picked (preview <entry> [u v])
hover u v | click u v [carry] | drag u v
turn 15 | commit | esc | undo | wait 40
expect ghost x y z [deg] [tol]           # where the copy in hand must be
expect prop <n|last> x y z [deg] [tol]   # where a prop is
expect props 3 | expect nothing
```

The owner's flows are in `labs/den-lite/moves/` and built into the tests:

| Script | What it checks |
|---|---|
| `card-glides` | The video's card: it glides, then is placed |
| `carry-and-drop` | Click to carry, onto a table's top |
| `press-and-drag` | Press and drag, and Esc |
| `turn-snap-edge` | Turning, the grid, the edge, walking |
| `undo-and-esc` | One commit is one undo |

`replay::fuzz` plays 300 seeded random scripts. A failure names its seed and line, and the same seed replays the same
script.

Three more checks over the wire, in `tests/client.rs` (the test client speaks the game's protocol, no window):

- `the_mouse_over_the_wire_matches_the_model`: addon messages in, the `where` answer against the model;
- `the_moves_scripts_replay_clean`;
- the Python smoke below, against the real program.

### den-move made sturdier (the game window that never opened)

`tools/den-check/den_check.py`:

- **Preflight**, before anything starts:
  - the game's program is in the folder;
  - no game is already running (a second one never opens its window);
  - the Den's ports, and room on the disk.

  Each check is said plainly with its fix.
- **Start again:** `start_den_play` starts the game again when the launcher's report shows the game's window never
  opened (or there was no report at all). den-move tries at most twice more (`--retries`). A start that reached the
  login screen and then failed is not retried, so a real failure is still seen.

**`den-smoke`** (`tools/den-check/den_smoke.py`) needs no game window. It runs this PC's real Den Lite on spare ports
with a scratch folder, and the headless client checks:

1. the way in (timed);
2. Sky Island, its landing and its tale;
3. a fall off the island;
4. a placement;
5. the mouse against the model (den_check's `project`);
6. the island as a code (decoded in Python: raw deflate and the check);
7. home again;
8. Den Lite's memory.

## 2. Tales: questlines anyone can write

A **tale** is a small questline in the Den, written in a plain-text script (`labs/den-lite/src/tale.rs`). Nothing in
it is ever run as code: it is parsed, checked and played by a small state machine. Rewards are looks only.

### The tale script

```text
tale The Lost Lantern                       # first, always: its title (48 letters)
by Wren                                     # optional
about A small tale on the island.           # optional
npc mira "Mira" look villager at 4 2 [face 90]
item lantern "Mira's Lantern" prop lantern at 12 -6
sign welcome at 0 3 "Welcome!"
portal up at -10 0 to 20 20
step Find Mira's lantern
  say mira "My lantern rolled off. Would you find it?"
  goal collect lantern
  hint It glows near the rocks.
step Bring it back
  goal talk mira
  done mira "You found it! Thank you."
step Train a little
  goal defeat 2 dummies
when enter 0 0 6 do say "The old oak hums."
when near welcome do say mira "Read it twice." again
when timer 30 do say "A breeze."
when step 3 do show lantern
reward title Lantern Finder
reward cheer
```

**What a tale may hold:**

| Kind | Forms |
|---|---|
| Things | `npc <id> "<Name>" look <look> at x y [face deg]`; `item <id> "<Name>" prop <catalog word> at x y`; `sign <id> at x y "<text>"`; `portal <id> at x y to x y` |
| Looks | villager, guard, mage, dwarf, elf, ghost, dummy (the game's own creatures, by AzerothCore entry; a fallback model) |
| Jaina | `jaina` is always there and is never declared |
| Goals | `talk <npc\|jaina>` (within 4 yards); `collect <item>` (walk over it, 2.5 yards); `visit <id>` (3 yards); `defeat <n> dummies` (a training dummy refilled counts as one down: combat.rs `dummy_down`); `reach x y [r]`; `wait <s>` |
| Step lines | `say`, `done`, `hint`: six at most per step |
| Triggers | `when enter x y r` \| `near <id>` \| `timer <s>` \| `step <n>`, then `do say [who] "..."` \| `teleport x y` \| `show <id>` \| `hide <id>`; `again` re-arms enter and near after leaving |
| Rewards | `title <text>` \| `badge <text>` \| `cheer` \| `glow`. Titles and badges are kept in `<dens>/tale-rewards.json`; nothing touches gear, gold or levels |

**Limits:**

- 8 KB in all;
- 24 steps, 24 things, 24 triggers, 4 rewards;
- texts up to 160 letters;
- places within 200 yards, and inside the Den when it plays;
- ids are a-z, 0-9, - and _, starting with a letter;
- no links, no brand words.

Every problem names its line in plain words ("line 4: goal talk bob: no such npc"). `tale::write` gives the one
canonical form, the one a code carries: `parse(write(t)) == t`.

**In game** (`labs/den-lite/src/taleplay.rs`):

| Command | What it does |
|---|---|
| `.den tale list` | The tales here: the Den's own (they travel in its file and its code), `<dens>/tales/*.tale`, and `tale-*` Den files (what the launcher makes of a tale's code) |
| `.den tale play <name>` | Plays one |
| `stop`, `status` | Stops the tale, or says where it stands |
| `keep` | Puts the tale in the Den's file |
| `code` | Makes a code and a `.denpack` file |
| `use <id>` | As if you walked up to `<id>` |

NPCs are units, friendly and selectable, and their names come from Den Lite's creature query. Items, signs and
portals are the player's own catalog props, shown only to the host. The tracker (`DenTale.lua`, `/tale`) shows:

- the step and its goal;
- the hint;
- rewards;
- Jaina's draft, with **Play it** / **Not now**;
- the Den's code, ready for Ctrl+C.

## 3. Jaina writes tales with you

You say: "Jaina, make a quest where you find my lost lantern, then beat 3 dummies, reward the title Lantern Finder".
Then:

1. If you **connected your own assistant** (the launcher's Agents page) and it is listening, Jaina passes your words to
   it through the existing ask path. They go nowhere else: never our keys, never a service of ours. The assistant has
   a new tool, **`den_tale_draft`** (Tier Ask: the launcher shows its card first, and the player approves it). Its
   description carries the whole script. The Den reads the draft from `<dens>/tale-draft.json`, checks it whole, and
   Jaina shows it. A broken draft gets one line naming the problem, and Jaina offers her own instead.
2. Otherwise, or after 60 s with no answer, **Jaina writes it herself** at once (`tale::draft`: keyword templates in
   the order you asked, always a tale that parses).
3. She shows it in three lines at most (her no-spam rule): the title, the steps, "Say Jaina, yes to play it, or
   Jaina, no". The Den bar gets the draft too.
4. **"Jaina, yes"** keeps it in `<dens>/tales/` and plays it. **"Jaina, no"** drops it. Nothing plays without the yes.

Jaina is also a quest NPC herself: `goal talk jaina` and `say jaina "..."` work in any tale.

## 4. Sky Island and light dens

**Sky Island** is a look, not props: `room.look = "sky"` (`labs/den-lite/src/skyisland.rs`; den.rs LOOKS and
build_room). Its 105 pieces are all the player's own game's models:

- a disc of stone platform tiles about 60 yards across from its middle (scale 3, about 60 tiles);
- a rim of rock walls with a gap to the south (the view);
- rocks under it, so it reads as floating;
- a big tree north of the middle;
- a sign post.

So a Sky Island Den file carries only what the player added, and the whole island fits in a link (778 letters).

| What | How it works |
|---|---|
| The spot | Map 1, one map tile further out to sea than the home's on both axes, 180 yards up: the launcher's grass is never under it. TODO(owner): the live check that this shows open sky (else `.den here` from a spot you like) |
| `.den island` | The Den is kept for the old spot (as `.den here`); the island look; you land south of the tree; three training dummies by it; the welcome tale (4 steps, reward title Sky Walker) the first time |
| `.den island off` | Home, on the open plane |
| Instant entry | Unchanged and already there: the launcher's one click (denclick.rs) signs in and enters with Den Lite's ready-made character. With the Den on the island (its autosave keeps `place`), the next start lands on the island |
| Safe edges | Fallen 14 yards under the top: back by the tree. Sunk into the ground: back on top. For friends too |
| The launcher | Your Den has a **Sky Island** tile: Play Sky Island (its file into Saved Dens, opened, then Your Den), and Copy its code. A Sky Island file opened from Saved Dens floats the Den first (world.rs check_open) |
| Sky | TODO(owner): Den Lite sends the real time of day; a fixed bright noon for the island needs a hook in world.rs's login time, which is the transport branch's file area and is left for after the merge |

**Light:**

- The Den's default room is still the smallest (ring 0).
- The island is 105 objects sent once.
- The sky-island test prints the start and entry times (`den-lite start N ms, in the world N ms`; it fails over 5 s).
- den-smoke prints Den Lite's memory. A first reading on the owner's PC is needed before anything is cut.

### Passing a Den around

A Den, a tale or a stamp is **one short code**. The format is `labs/den-lite/src/denpack.rs`, and `ui/denpack.js` is
the same in the launcher:

```text
ley1.<den|tale|stamp>.<r|z>.<base64url>.<crc32 of the text, 8 hex>      r: the text itself, z: raw deflate
```

- **Ways in:** a pasted code (chat line breaks and words round it are fine), a link `leyline://pack/<code>` (deeplink.rs,
  up to 2 000 letters), or a `<name>.denpack` file with the same text.
- **Checks:** a code is checked whole, in this order:
  - the envelope and its check;
  - an inflate limit per kind (a "deflate bomb" stops at 512 KB, 8 KB or 64 KB);
  - then what it holds, by the Den file's own checks (den.rs, the launcher's den_add) or the tale's.
- **In game:** `.den pack [name]` makes the Den's code. It is kept as a `.denpack` file, and its link goes to the Den
  bar and the tracker. `.den pack open <name>` opens a `.denpack` file from the Dens folder.
- **In the launcher:** Saved Dens has **Open the code** (a paste box) and **A .denpack file**. A tale's code becomes a
  small `tale-<title>` Den file that `.den tale list` reads.
- **Kept in step:** a Den Lite test checks that the launcher's built-in island code decodes to exactly
  `skyisland::den_json`. After a change to skyisland.rs that test fails and writes the new island file; then
  `py -3 tools/den-check/sky_island.py --make-code <that file>` sets the launcher's code.

### Friends in one click

Nothing new is needed. A friend's invite (the Den bar's Invite, the launcher's Invite friends, docs/DEN-INVITE.md)
brings them into the host's Den Lite, and a guest comes in at the Den's spot, so on the island they land on the
island. Its edge catches them too.

What still needs the relay: a friend who is neither on the same network nor able to reach the host's own address. Those
are the other two ways in the invite; tonight's relay fix (the join page's knock, refused since 2 Nov) is what makes
that work again. Same network and own address need no server.

## 5. Control presses (in this order)

| Step | Does |
|---|---|
| `den-replay` | Compiles Den Lite and runs the new unit tests: replay, model, tales, codes, island, and over the wire. **The first compile of this branch's Rust** |
| `den-lite-test` | The editor's tests, now with the replay |
| `den-lite` | Builds Den Lite for Your Den |
| `den-lite-all` | Every Den Lite test |
| `ui-local` | DenTale.lua into the game (a new file in the .toc) |
| `den-smoke` | No game window: the real program against the island, the mouse and the code |
| `sky-island` | The smoke, then the island in Your Den and My Den started: you land on it |
| `den-move` | The game-driving test, now with its preflight and starting again |
| `ship` | When the above pass |

## 6. Suggestions (the viral ones first)

Built tonight, because they were cheap: tale codes; Jaina's drafts; Sky Island; Den codes and links; the tracker.

Next, in order of what they would spread:

1. **Tale of the day**: one featured tale code a day on the website's Den page (Discord posts it after the owner's OK).
   Play it, get its badge. Needs only a list and the code format.
2. **Remix button**: `.den tale fork` copies a tale into `tales/` with `by you (after <maker>)`, as stamps keep their
   credits.
3. **Speedrun clocks**: a tale's finish time per player, in `tale-rewards.json`, and a "beat my time" line in its code.
4. **Jaina's brain remembers tales**: the tales you finished feed her ideas ("you liked finding things: here is a
   treasure hunt"), through jaina.rs's memory.
5. **Island archipelago**: several islands joined by portals (one Den each, a code that carries them all).
6. **More goals**: `carry <item> to <npc>`, `sit on <prop>`, `emote <word> at <npc>`; branching (`choose`) with two
   answers.
7. **NPCs that walk a path** (dojo.rs's mover, given points), and NPC barks on a timer.
8. **Tale editor in the launcher**: a form that writes the script, with live checks from `tale.rs`'s rules.

## 7. TODO(owner) and unverified

1. Compile: press **den-replay** first. The Rust in replay.rs, taleplay.rs, packs.rs, the world.rs hooks and the new
   client tests was written to the crate's own APIs, but not compiled here.
2. Sky Island live check:
   - open sky at the spot;
   - the scaled platform tiles have collision you walk on (if not, the edge rescue lifts you, but walking will feel
     wrong: tell Claude);
   - the tree's size;
   - landing south of it.
3. The island's tree, rim and ground looks: skyisland.rs `TREE`, `RIM`, `GROUND` (TODO(owner) in the file).
4. NPC looks: the AzerothCore creature entries in taleplay.rs `LOOK_CREATURES`. If one shows the dummy, its model
   isn't in creatures.rs's table.
5. The Sky Island tile's words (TODO(owner) in den.js).
6. A noon sky for the island (world.rs login time; after the transport merge).
7. The rescue's 1.5 s rate limit is one clock for everyone in the Den. Make it per player if friends fall together
   often.
