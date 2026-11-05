# Jaina in Den Lite: how she thinks, acts and what she spawns

Owner, 4 Nov 2026: "upgrade the way Jaina thinks, acts, and what she spawns". This is Den Lite (labs/den-lite), the
local Den server. The full Den's companion (`realm/custom/lua_scripts/ov_den_companion.lua`) is unchanged: on the full
Den only Follow and Stay of the panel's Jaina rows work.

In game she is Jaina (the owner's choice, 1 Nov: the game's own model and name, in game only). The launcher and the
website say "your companion".

## Where it lives

| File | What |
|---|---|
| `labs/den-lite/src/jaina.rs` | Her brain. Pure logic: no packets, files, clock or locks. Has unit tests. |
| `labs/den-lite/src/jaina-lines.txt` | Everything she says by herself, one `key \| line` each. Read in at build time. TODO(owner): his voice. |
| `labs/den-lite/src/jaina_world.rs` | A child module of world.rs. It carries out what her brain decides: gestures, strolls, lines, previews, critters, tidying, her words. |
| `labs/den-lite/src/companion.rs` | Her body: her spot, walks, hops, look-arounds, turning to you, the assistant's files. Her brain is `Companion::brain`. |
| `labs/den-lite/src/flight.rs` | Flying with you; hops when far (`hop_path`); walks round props (`path`, `smooth`). |
| `stream/fui-web/.../Dojo.lua` | The Dojo panel's two Jaina rows. |

world.rs only has hooks. Each one is a line or a few lines:
- `apply`
- save
- grow
- guest login and leave
- flight start and landing
- her build finishing
- `companion_tick` (the brain tick, hops, landing, removing leftovers when she goes)
- `order` (Follow can hop; Away removes leftovers)
- the "Jaina, ..." chat words
- `.den jaina <word>`
- `creature_answer` (critters)
- `dojo_line` (her panel line)

## Thinking

Once a second (`Brain::think`) she updates four **needs**. Each grows with time, from 0 (met) to 1 (pressing):
- **company:** met by friends arriving and by her gestures;
- **curiosity:** met by looking around;
- **helping:** grows while you place pieces, met by a suggestion;
- **rest:** grows while you stand still.

She also has two feelings:
- **joy:** pieces placed, builds, friends and sets you keep raise it; it drifts back to the middle;
- **energy:** falls when you stand still for a long time, and comes back when you move.

Her **mood** comes from these: calm, cheerful, excited, curious or sleepy. It picks her idle gesture and the line she
uses.

Each thought scores what she could do, and she does the best one if it scores 0.2 or more. Otherwise she does nothing,
which is most of the time. The choices:

| Choice | When | What she does |
|---|---|---|
| React | Something happened in the last 5 s. Fresher and bigger events score higher: a friend arriving 0.9, a fight ending 0.8, your first piece 0.7, a stamp or load 0.6, one piece 0.35. | A gesture, and now and then a line |
| Look around | Following you, after the first minute, and you have stood still for 25 s | A stroll (walking animation) to a spot near you, around the props and inside the Den. She stays there until you walk more than 10 yards away. |
| Idle | You have stood still for 25 s, at most every 45 s | Her mood's gesture, rarely a line |
| Suggest | You placed 3 pieces in the last 2 minutes and paused | A small set as a preview (below) |

Randomness is seeded (`jaina::Rng`, SplitMix64). Each login seeds it with the clock, and a test seeds it with a number,
so the same input gives the same choices.

**Memory of the visit** (`Brain::memory`). She remembers:
- what you placed, and her guess at your favourite piece: the one placed most, at least twice;
- deletes, saves, flights, friends and builds;
- the sets you kept and the sets you sent away;
- the pieces she put down (`mine`).

A new login starts her memory again. How she talks and her pieces carry over while Den Lite runs. "Jaina, what did I
build?" says what she remembers. The heartbeat (`companion-in.json`, field `jaina`) carries the same line as the panel,
so the player's assistant can read it.

## Timing (owner 1 Nov 23:46/47: "not spam the chat"; she speaks when spoken to)

- Unasked words are a **toast in the Den bar**, never the chat (without the bar, notice.rs routes them the old way).
- **No unasked words in the first 90 s** of a visit (`WARMUP_MS`).
- **Normal:** at most one line every 3 minutes and 8 a visit. **Chatty:** one a minute, 30 a visit.
- **Quiet:** no unasked words and no suggestions at all. She still gestures and still answers when spoken to.
- **Gestures:** at most one every 6 s.
- **Suggestions by herself:**
  - the first after 5 minutes, then at most one every 10 minutes, 2 a visit (Chatty: 4);
  - never while she builds or you fly;
  - two sent away, or left unanswered, and she stops suggesting by herself until you change how she talks;
  - "Jaina, help" always works.
- Her answers to commands ("I'll wait here.", "Right behind you.") are the same words as before. The full Den says them
  too, and its tests check them.

## Acting

- **Following:** as before, she follows 2.5 yards behind you and never stands in your way.
- **Hops:** more than 24 yards from her spot on foot (but within 40, where she is put back at once), she hops. The
  game is told she flies, then she flies an arc of 3 to 10 yards up over whatever is between you (`flight::hop_path`,
  14 yards a second). She comes down beside you, and the game is told once that she walks again. While you fly she
  flies with you as before (flight.rs); a hop that lands then leaves the landing to flight.rs.
- **Smoother walks:** a waypoint she doesn't need is dropped (`flight::smooth`). If the waypoint beside a prop would
  stand in another prop and the other side is clear, she goes round the other side.
- **Facing you:** before an unasked line, if she is standing and looking more than 20 degrees away from you, she turns
  to you (`Companion::face_to`).
- **Gestures** (SMSG_EMOTE, one-shot emotes from AzerothCore's `Emote` list), by what happened:

| Gesture | When |
|---|---|
| applaud, point | a piece placed |
| applaud | your first piece |
| exclamation | a stamp or load, many deleted |
| laugh | your favourite again |
| shy | one deleted |
| yes | saved, a set kept |
| no | a set sent away |
| cheer | grew, flying, a win |
| salute | a fight starts |
| wave | a friend comes or goes |
| bow | her build is done |
| point | a suggestion |
| mood gesture (bow, laugh, cheer, point, kneel) | idle |

## Spawning

**Her sets** (`jaina::THEMES`). Each is 4 to 6 pieces from Den Lite's catalog, laid out about 3 yards in front of you:

| Set | Season |
|---|---|
| cozy corner | any time |
| ring of lights | any time |
| market stall | any time |
| flower bed | spring, Mar to May |
| picnic | summer, Jun to Aug |
| harvest corner | autumn, Sep to Nov |
| candlelit corner | October |
| winter tree | Dec to Feb |

How she picks a set:
- in season first;
- then one with your favourite piece;
- never one you sent away twice;
- by herself, never one out of season.

How a suggestion works:
1. **A preview first.** The pieces are shown to you only, sparkling (the placement aids' glow). They aren't in the
   Den: not saved, nobody else sees them. They sit where the stamp would put them (settled the same way; a test checks
   they match).
2. **"Jaina, yes"** (or Keep on the panel) puts the set down for real, as a stamp: saved, and one undo takes the whole
   set back.
3. **"No"**, a minute with no answer, leaving the Den or sending her away: the preview goes.
4. "Jaina, build a winter tree" (or a cozy corner, lights...) is the same preview. Stamps (garden, inn, campfire...) she
   still builds piece by piece, as before.

**Hers and yours.**
- What she put down (a kept set, a stamp she built) stays hers until you move, lock or delete a piece.
- **"Jaina, tidy up"** asks first ("I put down 4 pieces you haven't touched. Take them away?"). Only on "yes" does she
  take away her untouched pieces, through the normal delete, so undo puts each back.
- She never touches your own pieces.
- At most 48 of her pieces in a Den (`MAX_MINE`).

**Critters** ("Jaina, a pet", "a rabbit", "a squirrel"; the panel's Pet button):
- rabbit, squirrel, prairie dog, chicken, fawn or sheep (AzerothCore creature entries; their models from
  creatures.rs);
- at most 3; friendly, can't be attacked;
- they wander within 3 yards of where they came, around props;
- shown to the host only, never saved;
- "pets away", "tidy up", leaving the Den or sending her away and they go.

## Words

Say these in chat as "Jaina, <words>", type `.den jaina <word>`, or press the Den panel's buttons:

| Words | What |
|---|---|
| follow, stay, come, tips, go away | as before (companion.rs `command`) |
| quiet / talk / chatty | how much she says unasked (Quiet on the panel switches between quiet and talk) |
| help, an idea, suggest, surprise me (`.den jaina idea`) | she suggests a set |
| decorate | a set in season |
| build a cozy corner / some lights / a winter tree... | that set as a preview (stamps as before) |
| yes / keep it, no / not now | her open question (a set to keep, a tidy) |
| tidy up, clean up | asks, then takes away only her untouched pieces; her critters go at once |
| a pet, a rabbit, bring a squirrel | a critter |
| pets away | the critters go |
| what did I build? | what she remembers of this visit |

"Jaina, help" used to be a tip. Tips are now "Jaina, tips" or "what next". Anything else is an ask for the player's
assistant, as before.

## The Den panel (Dojo.lua)

There are two rows under "Jaina builds":
- **Jaina:** Follow, Stay, Quiet, Pet.
- **Her ideas:** Idea, Keep, No, Tidy.

Each button sends one `.den jaina <word>`. They light up from her line, `wuden jaina`:
`talk=..;doing=..;mood=..;offer=<set>|-;ask=offer|tidy<n>|-;mine=<n>;critters=<n>;placed=<n>;fav=<entry>|-`.
Den Lite sends this line whenever it changes and whenever the Dojo line goes out.

Keep and No work only while she asks something. The readout says "Jaina's idea: <set> (Keep or No)". With no line
from her (the full Den), only Follow and Stay work, and the others say they need Den Lite's companion.

## Tests

- **jaina.rs (16 tests):**
  - seeded randomness;
  - every line short and plain, and every key has a line;
  - lines vary;
  - warm-up and quiet;
  - unasked words rare and gestures spaced over a busy hour;
  - memory and favourites, her own pieces not counted;
  - suggestions while you build, stopping after two sent away;
  - offers expiring;
  - sets in season, never one sent away twice;
  - layout and turning;
  - tidy only her untouched pieces;
  - critters within limits;
  - looking around only when you stand still;
  - her words;
  - the emote body and the panel line;
  - moods.
- **companion.rs (3 new):** hops and landing once; looking around and coming back when you walk off; turning to you.
- **flight.rs (2 new):** the free side and smoothing; the hop arc.
- **jaina_world.rs (1):** every set's pieces are in the catalog; critters have models and names; the preview is where
  the stamp puts the set; one undo takes a kept set back.
- **stream/tests/test_wuui_dojo.lua:** 69 → 83 checks. The Jaina rows, their commands, lighting up from her line, and
  without it.

jaina.rs, companion.rs and flight.rs were run here under plain rustc: 33 tests passed, using a small serde_json
stand-in (crates.io is blocked in the cloud container). jaina_world.rs and the world.rs hooks need the real crates:
run `cargo test` in labs/den-lite on the PC or CI.

## Still to check live

- The emote ids (273 yes, 274 no, 16 kneel and the others) play as expected on her model.
- The game shows her hop as a flight from the spline flags alone (the same question as her flying in flight.rs).
- The walking animation flag (MoveSplineFlag Walkmode 0x1000) on her stroll.
- Preview pieces sparkle (GAMEOBJECT_DYNAMIC), and a click on one in build mode does nothing harmful.
- The critters' names and models show, and they stay friendly.
- How often she talks feels right: the gaps are the constants at the top of jaina.rs.

## TODO(owner)

- A voice pass on `jaina-lines.txt` (his words; these are drafts). Most of all the lines that carry her personality:
  idle, offer, "place.again", guest.
- Should she suggest sets by herself at all (on by default, rare), or only when asked?
- Which sets and seasons he wants. More sets are a table in jaina.rs; every piece must be in the catalog.
