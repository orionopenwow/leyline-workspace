# Duel tournaments in the Den

The host runs a tournament in their Den; friends sign up in the game, fight in a ring, and everyone else watches.
Open it with the **Arena** button on the Den bar (or `/tourney`), or type `.den tourney`.

## Running one (the host)

1. Pick a format and how many rounds a match has, then press **New**
   (`.den tourney new single|robin|king [bo1|bo3|bo5|bo7] [title]`).
2. Stand where the fights should be and press **Ring here** (`.den tourney ring [yards]`, 6 to 30 across).
   `.den dojo ring` builds a fenced ring there so everyone can see it.
3. Optional: a house rule (**Rule**, then **Set**; `.den tourney twist <rule>`).
4. Everyone presses **Join** (`.den tourney join`). You can add or remove someone by name
   (`.den tourney add|remove <name>`).
5. Press **Start**. The matches are called in order: "Next: Rowan vs Briar", both fighters into the ring,
   3-2-1, "Fight!".
6. A round ends when a fighter **Yields** (`.den tourney yield`), steps out of the ring for more than a moment
   (ring out), or you call it (**Rowan won**; `.den tourney win <name>`). A fighter who doesn't come to the ring in a
   minute loses the match.
7. The champion is announced. The results are kept in the Den file (the last 10), and the Saved Dens card in
   Leyline shows "Champion: Rowan won the Spring Cup (round robin, 4 players)".

Everyone else is a spectator: they hear every call, see the bracket in the window, and are asked once to step
out of the ring.

**Honest limit:** Den Lite has no player-against-player damage, so a fight is played out and its end is said by
the players (yield), the ring (ring out) or the host. On the full Den the realm's own duels could decide it
(ov_duels.lua); that hookup is still to do.

## The formats built

| Format | How it works |
|---|---|
| Single elimination | Signup order is the seeding; top seeds get the byes; seed 1 and 2 can only meet in the final |
| Round robin | Everyone fights everyone (up to 10 players); most match wins takes it, then fewest losses, then rounds won |
| King of the hill | The winner stays on, the loser goes to the back of the line; three wins in a row take the crown |
| Best of 1, 3, 5 or 7 | Any format: first to the rounds needed wins the match |
| Ring out | With a ring, leaving it during a fight loses the round |
| Sudden death | House rule: the final is one round |

## Ideas (not built yet, or built only as an announced house rule)

1. **Class-locked:** everyone plays the same class (`.den class` for all). Announced now; could be enforced.
2. **Gear-normalised:** everyone in the same set (`.den gearset`), so only skill counts.
3. **Mirror match:** both fighters pick the same class and spec.
4. **2v2:** pairs sign up together; the ring holds four.
5. **Last one standing:** everyone in the ring at once; out of the ring is out; the last one wins.
6. **Handicap:** the higher seed fights at half speed (`.den speed`), or with fewer action bars.
7. **Double elimination:** a losers' bracket, so one bad match doesn't end your night.
8. **Swiss rounds:** pairs of equal records each round, for big groups.
9. **Ladder night:** challenge anyone up to two places above you; the ladder is kept in the Den file.
10. **Mounted joust:** fight from your mount; falling off loses the round.
11. **Costume rules:** dress the part (a theme per tournament) or forfeit.
12. **No potions, melee only:** house rules for a cleaner fight.
13. **Shrinking ring:** the ring gets smaller every 30 seconds.
14. **Spectator bets for fun:** spectators pick a winner before each match; the best picker is named at the end
    (no money, ever).
15. **Seasons:** results from several tournaments add up to a Den season champion on the showcase card.

## For developers

- Logic and the glue: `labs/den-lite/src/tourney.rs` (unit tests beside it); ticked from `hosting.rs` four times a
  second; the chat commands in `world.rs` (`.den tourney`, `.den t`, `.tourney`; guests may use join, leave, yield
  and the bracket).
- Window: `stream/fui-web/Interface/AddOns/WrathUnleashedUI/DenTourney.lua` (lines `wuden tourney`, `tsign`, `tbr`),
  test `stream/tests/test_wuui_tourney.lua`.
- Results in the Den file: `tournaments`; kept when a Den is opened (`den.rs`), shown by the launcher
  (`host/dens.rs champion_line`).
