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
   your game folder and fills the world. This takes 5 to 20 minutes (most of it is filling the world). You can use
   the rest of Leyline meanwhile.
4. Your game starts and logs in by itself. You land on your characters, or on character creation if you have none.

After the first time, the same button takes a minute or two.

The original world is the world as it first was: every character is held in its first era, the level cap is 60, and
the races and classes are that era's. Experience comes twice as fast as normal, so a solo player keeps moving.

## Quick start: straight into the world

Press **Quick start: a new character** under the big button. Pick a race, a class, male or female, and a name (or keep
the one suggested), then press **Quick start**. Leyline makes the character on the game's own creation screen and takes
you straight into the world at level 1.

## Your companions

In the game, open the **Bots window**: press **Ctrl-B**, click the **Bots** button by the minimap, or type `/comp`.
From the window you bring bots, pick their roles and specs, see their gear and send them home. Walk into a 5-player
dungeon and dungeon buddies fill your party by themselves; they go home when you leave.

Want bots to join by themselves when you log in? On the **Single player** page, **Your world** has companions, dungeon
buddies, a roaming population and the experience rate. A new world starts quiet: nobody joins until you ask.

The Leyline button on the minimap (or `/guide`) opens this world's own guide in the game.

Or type:

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

Your friend needs Leyline and their own game folder. No Hero Key and no account: their Leyline proves it's them.
Game files never travel between you. In the game, `/together` (or Friends, the Leyline tab) makes the link too. See
[Host for friends](host-for-friends.md) for the checklist and the fixes.

Their character stays on your world, theirs to keep. If your friend reinstalls or moves to a new PC, they keep their
characters by bringing back their saved key (**Settings**, **Your Leyline network key**). If they didn't save it,
invite them again and use **Give their characters to…** on their old row in your friends list.

## Your world: saving, stopping, offline

- Your world is saved as you play.
- It stops (everyone saved) a minute after the game closes, and when Leyline closes. Both are switches in **Single
  player settings**. With friends in your world, Leyline asks before it closes.
- **Offline**: after the first setup, your world plays with no internet. The internet is only needed for the first
  setup, engine and add-on updates, logging in with your Hero Key (your characters stay reachable without it), and
  inviting a friend through the relay (a friend on the same network needs none).
- **Updates**: when the engine has a fix, the world's card says **Update your world engine**. Your characters, your
  friends' characters and your settings stay; your world is backed up first.
- **Your characters and your Hero Key**: logged in or not, Play goes in on the login that holds your characters on
  this PC. If they're on an earlier login, the card offers **Move them to the login I use now**.

## The first era on its own: Leyline Solo

Have the first-era game folder? [Leyline Solo](solo.md) is a small app of its own for it: a first-era world on your
PC with companions, offline, logged in by itself, your characters in one list, and a friend when you want one.

## Not on Windows

The one-click engine is made for Windows PCs for now. On another computer, the same original world runs in Docker:
the card's button takes you to **Vanilla+** on the Host page; then follow [Host a realm](host-a-realm.md). Single player itself
also runs in Docker there (its page shows the way).

## When something goes wrong

| What you see | What to do |
|---|---|
| "Add your game folder first" | Add it under **Game folders**, then press the button again |
| Setting up stopped | Press **Try again**: it carries on from the step that stopped. **Logs** shows what happened |
| "is running on this PC, and one world runs at a time" | Press **Stop it and play**: the other world's characters are saved |
| The game doesn't log in by itself | Type the login shown on the card once; Leyline made it for you and keeps it on your PC |
| Something is broken (a file gone, the world won't start) | Press **Repair** on the error card: it checks every engine file, puts back only what's broken and keeps your saves |
| You want an earlier copy of your world | **Back-ups and Restore** on the world's card: a back-up is made once a day, before every engine update and before Repair (the newest 10 are kept). **Restore** backs up your world as it is now first, so you can undo it |
| An antivirus warning | Allow Leyline's folder in it, then press **Repair** |
