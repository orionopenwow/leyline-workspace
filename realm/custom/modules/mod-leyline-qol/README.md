# mod-leyline-qol

An AzerothCore module (3.3.5a, the mod-playerbots fork pinned in `realm/custom/sources.lock`) with small
quality-of-life rules for Leyline's Wrath Unleashed realm. The owner, 5 Nov 2026: "make dalaran an instant log out
zone too and duels avail anywhere on the island on our server".

**Instant logout in Dalaran.** The core logs a player out at once only when they rest (an inn, a capital, a rest
zone) and are out of combat, or are on a flight path (`WorldSession::HandleLogoutRequestOpcode`). Dalaran rests only
in parts, so most of the city gave the 20-second countdown. The module answers the logout request itself
(`ServerScript::CanPacketReceive` on `CMSG_LOGOUT_REQUEST`, called just before the core's handler, in the same
thread) when the player stands in a listed area and nothing would stop an instant logout: not in combat, not falling,
not in a duel, not frozen by a GM, not held by the AFK rules (`AFK.PreventLogout`). It then does what the core does
for a resting player: releases loot, sends `SMSG_LOGOUT_RESPONSE` with result 0 and the **instant** flag (the
client shows no countdown) and calls `LogoutPlayer(true)`. In every other case the core's handler answers as usual.
The resting flag is never touched, so rested XP doesn't change.

**Duels anywhere in Dalaran.** The core refuses a duel ("Dueling isn't allowed here") when the caster's or the
target's area lacks `AREA_FLAG_ALLOW_DUELS` (`Spell::EffectDuel`). At start-up (`OnBeforeWorldInitialized`, after
the AreaTable loads) and on a config reload, the module sets that flag on every AreaTable row of map 571 under the
listed ids, the way the core's own `SpellInfoCorrections` sets area flags. Only the duel flag changes: the sanctuary
flag stays, so nobody but the two duelists can attack (`Unit::_IsValidAttackTarget` lets a duel's two players hit
each other before its sanctuary check, and no one else), and the guards stay out of duels. A reload with the setting
off puts the rows back.

| File | What |
|---|---|
| `src/LeylineQol.cpp` | the scripts: the settings, the duel flags, the logout answer |
| `src/QolLogic.h` | pure helpers: id lists, "is this area under a listed zone", the logout decision, the duel rows |
| `src/mod_leyline_qol_loader.cpp` | `Addmod_leyline_qolScripts()` |
| `conf/mod_leyline_qol.conf.dist` | settings (all optional; the defaults are built in) |
| `tests/test_qol_logic.cpp` | `g++ -std=c++17 -Wall -Wextra -Werror -I../src test_qol_logic.cpp -o t && ./t` (no core needed) |

**Settings** (env on the realm, `realm/profiles/main.env`):

| Key | Env | Default |
|---|---|---|
| `LeylineQol.Enable` | `AC_LEYLINE_QOL_ENABLE` | 1 |
| `LeylineQol.Dalaran.InstantLogout` | `AC_LEYLINE_QOL_DALARAN_INSTANT_LOGOUT` | 1 |
| `LeylineQol.Dalaran.LogoutAreas` | `AC_LEYLINE_QOL_DALARAN_LOGOUT_AREAS` | `4395,2817` (the city, the forest under it) |
| `LeylineQol.Dalaran.DuelsAnywhere` | `AC_LEYLINE_QOL_DALARAN_DUELS_ANYWHERE` | 1 |
| `LeylineQol.Dalaran.DuelAreas` | `AC_LEYLINE_QOL_DALARAN_DUEL_AREAS` | `4395` (the city) |
| `LeylineQol.Dalaran.MapId` | `AC_LEYLINE_QOL_DALARAN_MAP_ID` | 571 (the Dalaran arena is its own map, never counted) |

An area counts when it, or any area above it (sub-area, area, zone), is listed: 4395 covers every street, Krasus'
Landing, the Underbelly and every inn and shop in them.

**Start-up line.** The world server log says `mod-leyline-qol: on; Dalaran instant logout on (areas 4395,2817);
duels anywhere on (areas 4395, N rows opened to duels)`.

**Build.** `realm/custom/fetch.sh` copies this folder into `acore/modules/` (without `tests/`), so the `wu-images`
workflow builds it with the realm image; it reaches the live realm with the next image (Control: `ship`, then
`realm-image`).

**Unverified.** Written against the pinned core (3f2969dd) by reading its sources and checked with
`g++ -fsyntax-only` against its headers; not yet run in game. To check in game: `/logout` on a Dalaran street (gone
at once, no countdown), `/logout` in combat there (refused, as before); `/duel` on a street and in the Underbelly
(the flag appears), a third player can't hit either duelist, a guard doesn't react. The client's own copy of the
area table still says "no duels" for most of the city; the duel request is decided by the server, so this should not
matter, but it is the first thing to look at if the client refuses before asking.
