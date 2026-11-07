# Single player

Play the game on your own PC with no server to run and no account to make. Leyline starts a small world inside the
launcher, signs you in by itself and takes you to your characters. Your companions fill your party, and a friend can
join you whenever you like.

<!-- TODO(owner): the opening lines in his words -->

## What you need

- **Leyline**, the launcher, on a Windows PC (the one-click engine is a Windows build for now; on other computers,
  see [Not on Windows](#not-on-windows)).
- **Your own game folder**, added once under **Game folders**. Leyline never downloads, ships or uploads the game:
  your world reads its maps from your folder, on your PC.
- Several GB of free disk space (the world's database and the maps it reads), and some minutes the first time.

## Play the original world in one click

1. Open **Single player** in Leyline's side menu (or press **Original world** on Home).
2. Press **Play the original world (level 60) with companions**.
3. The first time, Leyline gets its engine (one download, shared by every single player world), reads the maps from
   your game folder and fills the world. This takes 10 to 30 minutes. You can use the rest of Leyline meanwhile.
4. Your game starts and logs in by itself. You land on your characters, or on character creation if you have none.

After the first time, the same button takes a minute or two.

The original world is the world as it first was: every character is held in its first era, the level cap is 60, and
the races and classes are that era's. Experience comes twice as fast as normal, so a solo player keeps moving.

## Quick start: straight into the world

Press **Quick start: a new character** under the big button. Pick a race, a class, male or female, and a name (or keep
the one suggested), then press **Quick start**. Leyline makes the character on the game's own creation screen and takes
you straight into the world at level 1.

## Your companions

In the game, your companions are a few words away:

| Type | What happens |
|---|---|
| `.recruit party` | Fills your party with companions for quests and dungeons |
| `.recruit tank`, `.recruit healer`, `.recruit dps` | One companion of that role |
| `.buddy` | One companion who follows you around the world |
| `.dismiss` | Sends them home (`.dismiss all` for everyone) |

Dungeons and raids scale to the size of your group, so you and your companions can clear them. Companions always pass
on loot: it's yours.

## Characters: on this PC and online

Press **Characters** on the Single player page (or on Home). It has two tabs:

- **On this PC**: just you, and the friends you invite. Each of your worlds (Single player, the original world, Boss
  Rush) keeps its own characters on your PC. Press **Play** on one: Leyline starts that world if it isn't running and
  takes that character into the game.
- **Online**: the shared realm. Your characters there live on the realm. The ones you played from this PC are listed;
  **Play** joins the realm, and you pick your character after you log in.

One world runs at a time on your PC. If another one is running, Leyline asks before it stops it. Your characters are
always saved when a world stops.

## Play with a friend

While your world runs, press **Invite a friend** on its card. Leyline makes one link for one friend, with a login made
just for them inside, and checks the way in before you send it.

- A friend **on the same network** (your home or office) comes straight in.
- A friend **anywhere else** comes in through the Leyline relay. Nothing to open on your router; your address is never
  shown.

Your friend needs Leyline, their own game folder and a Hero Key. Game files never travel between you. See
[Host for friends](host-for-friends.md) for the checklist and the fixes.

## The first era on its own: Leyline Solo

Have the first-era game folder? [Leyline Solo](solo.md) is a small app of its own for it: a first-era world on your
PC with companions, offline, logged in by itself, your characters in one list, and a friend when you want one.

## Not on Windows

The one-click engine is made for Windows PCs for now. On another computer, the same original world runs in Docker:
press **Open Vanilla+ on the Host page** on the card, and follow [Host a realm](host-a-realm.md). Single player itself
also runs in Docker there (its page shows the way).

## When something goes wrong

| What you see | What to do |
|---|---|
| "Add your game folder first" | Add it under **Game folders**, then press the button again |
| Setting up stopped | Press **Try again**: it carries on from the step that stopped. **Logs** shows what happened |
| "is running on this PC, and one world runs at a time" | Press **Stop it and play**: the other world's characters are saved |
| The game doesn't log in by itself | Type the login shown on the card once; Leyline made it for you and keeps it on your PC |
