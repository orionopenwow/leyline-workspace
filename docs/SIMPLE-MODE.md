# Simple mode: Play, Host, Visit, Back up

Owner, 10 Nov 2026: a more simplified launcher, staged. First, a mode inside Leyline. Then the same screen published as a
forkable reference launcher in the neutral Commonhold organisation. It follows the Commonhold growth plan: "four
buttons, everything else automatic".

## Stage 1: inside Leyline (this change)

- **The switch:** *Simple mode* in the side menu's foot. While it's on (`html.ll-simple`), the side menu shows only
  Home (the four buttons), the switch (now *Show everything*), Settings and Help, and the play bar is hidden. The
  choice is remembered (`leyline.simple`). It's off by default. **TODO(owner):** decide whether new players should
  start in Simple mode.
- **The page:** `ui/simplehome.js` and `ui/simplehome.css`, `#page-simple`. It reuses the launcher's own paths,
  never copies of them:

| Button | What it does | Path |
|---|---|---|
| **Play** | Sets up (once) and starts your own world, then signs you in. Pick Single player or the original world. | `LeylineWorlds.run` (vanilla.js): `world_status`, `world_setup`/`world_start`, `engine_progress`, `world_play` |
| **Host** | Play with friends: Invite a friend in one step | `LeylineHostFriends.open` (hostfriends.js) |
| **Visit** | Join with a link or code a friend sent, or find a world in the realm list | Realms' invite box (`joinFriendsDen`, app.js), `join_invite` |
| **Back up** | Backs up every world that's set up; keeps the newest 10 per world; **Back up every day** (on by default) | `world_backup` (app.rs, host/engine.rs `backup`) |

- **Daily back-up:** when the launcher or this page opens, if the last back-up made here is over 20 hours old (or
  there's never been one), it backs up every set-up world once, quietly. This happens in Simple mode and in the full
  launcher alike, unless it's turned off.
- **Edge cases:**
  - No game folder, or no Built-in engine for this PC: Play goes to the Single player page, which walks through
    both.
  - Another world running: Play asks "Stop it and play", and never stops anything unasked.
- **Tests:** `launcher/tests/simplehome.browser.test.cjs`.

## What's not here yet

- **Restore from a back-up.** The back-ups are SQL dumps zipped in `<app data>/engine*/backups/`. Restoring needs a
  core command that stops the world and imports the dump. It comes next, with a confirmation step in the page.
- **Visit with your own character.** This needs the Commonhold passport (spec in the Commonhold repo); the Visit card
  says it's coming.
- **Auto-updates:** already on for the launcher, the engine and the Host pack. Simple mode adds nothing there.

## Stage 2: the reference launcher in the Commonhold organisation

Once the passport works between two worlds:

1. **A frontend of its own:** `launcher/ui-simple/` (as `ui-solo/` is for Leyline Solo), built from this page with
   its own `tauri.simple.conf.json`, using the same core.
2. **Branding with `launcher/scripts/rebrand.py`** (docs/FORKING.md): name, colours, update channel and relays from
   one `branding.json`, so anyone can fork it as their own door into the network.
3. **Published in the neutral org** as "the simple launcher anyone can reskin". Leyline stays its own launcher, and
   CoreCraft, Project Eternal and others can ship theirs from the same code.
4. **Visit becomes the passport flow:** pick a world, see the fit, go, come home, approve.

Stage 2 shouldn't start before stage 1 has been used by real players, because the four buttons need to be right before
they're forked.
