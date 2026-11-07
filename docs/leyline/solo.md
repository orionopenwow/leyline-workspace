# Leyline Solo

Leyline Solo is a small Windows app for the first era of the game. It runs a world of your own on your PC, from your
own game folder: companions at your side, no account to make, no internet once it is set up. It logs you in by
itself, keeps your characters in one list, and lets a friend join when you want company. The page with the download
is [Leyline Solo](/wrath/solo).

It is its own app, apart from the Leyline launcher. The launcher's [Single player](single-player.md) plays on the
later client; Leyline Solo is for the first-era client, the one the game first came out with.

## What you need

- **A Windows PC.** The first release is for Windows only.
- **Your own first-era game folder**: the folder with the game's program and its `Data` folder. Leyline Solo never
  downloads, ships or uploads any of the game. Your world is made from your folder, on your PC.
- **About 8 GB free** for the world's maps and database.
- **The internet once**, to download the app. After setup your world plays offline.

## Install

1. Download the installer from [the Leyline Solo page](/wrath/solo) (it goes to our GitHub releases). The `.sha256`
   file beside it is its checksum, if you want to check the download.
2. Run the installer. Windows may show "Windows protected your PC" for a new app: press **More info**, then **Run
   anyway**.
3. Open **Leyline Solo** from the Start menu.

## First run

1. **Point it at your game folder.** Press **Choose your game folder** and pick the folder with the game's program in
   it. Leyline Solo checks it is the first-era client and says so in one line.
2. **Setup.** The first time, Leyline Solo reads the maps from your game folder and fills your world. A progress line
   shows each part. It can take up to an hour, mostly the part that reads the world's buildings and caves. Your game
   folder is only read: setup changes nothing in it.
3. **Play.** Your world starts on your PC, the game opens and logs you in by itself, and you land on character
   select. The first time there are no characters: make one on the game's own creation screen, or use
   [Quick start](#quick-start).

After the first time, **Play** takes a minute or so.

## Quick start

Press **Quick start** to skip the creation screen: pick a race, a class, male or female and a name (or keep the one
suggested), then press **Quick start**. The character is made for you and goes straight into the world at level 1.

## Playing with companions

Your world starts quiet: no strangers, just you. Companions come when you call them, by typing in the game's chat:

| Type | What happens |
|---|---|
| `.bot group` | Four companions join your party at your level: a tank, a healer and two who deal damage |
| `.bot group size=3` | The same, with a smaller party |
| `.bot create class=priest` | One companion of that class; add `race=`, `level=`, `gender=` or `name=` if you like |
| `.bot add Name` | One of your own characters comes along as a companion, with its gear and talents |
| `.bot remove Name` | Sends that companion away |
| `.bot list` | Who is with you now |
| `.bot gear Name` | The best gear for that companion's level |
| `.bot prepare Name` | Gear, food and potions |
| `.bot levelup Name` | One level up for that companion |
| `.bot delete Name` | Deletes that companion's character |
| `.bot help` | The full list, in the game |

Then give the whole party orders by saying one word in party chat:

| Say | They |
|---|---|
| `follow` | follow you |
| `stay` | stay where they are |
| `attack` | attack your target |
| `flee` | run back to you |
| `summon` | come to you, wherever they are |
| `release`, `revive` | let go of their spirit, and come back to life |

Companions are real characters on your own account. They show on character select, and they count toward the ten
characters you can have in your world.

## Characters

**Characters** lists every hero you have made: name, level and class. Pick one and press **Play**: the world starts if
it isn't running, and that character goes straight in.

Your characters live on your PC, in Leyline Solo's own folder, and nowhere else. Stopping your world saves everyone.
For our online realms, use the Leyline launcher: those characters live on the realm.

## Invite a friend

While your world runs, press **Invite a friend**. Leyline Solo makes one link for one friend, with a login made just
for them, and checks the way in before you send it. Send the link in a private message.

- **On the same network** (your home, a LAN party): your friend comes straight in.
- **Anywhere else**: through the Leyline relay. Nothing to open on your router, and your address is never shown.

Your friend needs Leyline Solo and their own first-era game folder. They open the link (or paste it into **Join a
friend**), and their game logs in to your world. Game files never travel between you. Links stop working when you stop
sharing; make a new one for the next friend.

## Settings

**Sign-in** chooses how the game logs in to your world:

- **Quick sign-in** (the default): the game logs in by itself and goes to character select. Leyline Solo adds one
  small archive of its own to your game's `Data` folder and sets two of the game's settings files (the realm list
  file and the saved account name), after backing them up.
- **Simple sign-in**: only the two settings files are set. The login screen shows your world's account name already
  filled in; type its password once (Settings shows it), and the game remembers the account.
- **Remove sign-in**: puts your game folder back exactly as it was, byte for byte, and takes our archive out. Close the
  game first: it holds the archive open while it runs.

## Troubleshooting

| What you see | What to do |
|---|---|
| "This is not the first-era client" | Pick the folder of the first-era game, the one with the game's program and the `Data` folder in it. A later client's folder doesn't work here; for that, use the Leyline launcher |
| "A port is in use by another program" | Another server is running on this PC on the same ports (often another private server, or a world left running). Stop it, or restart the PC, then press **Play** again |
| Windows Firewall asks about the server programs | Your world only listens on your own PC, so **Cancel** is fine for playing alone. To let a friend on the same network in, allow **Private networks** only |
| Your antivirus flags or deletes a server program | The world's programs are new to it. They are built in the open on GitHub from public source. Add Leyline Solo's folder to your antivirus's exceptions, then press **Repair** (or run setup again) to put the program back |
| Setup seems stuck | Reading the buildings and caves is the slow part and can take up to an hour on an older PC. The progress line still moves; leave it. If it stopped, press **Try again**: it carries on from the part that stopped |
| "Login interface files are corrupt" (or the game won't open its login screen) | Your game refused our sign-in archive. In Settings, choose **Simple sign-in** (or **Remove sign-in**), then press **Play** again |
| The game opens on the normal login screen | Quick sign-in didn't run: type the account name and password Settings shows, once. If it keeps happening, choose **Simple sign-in** |
| `.bot group` does nothing | Type it in the chat box, with the dot first. Your party must have room: leave other groups first |
| My friend can't get in | Keep your world running and your PC awake. Press **Invite a friend** again: it checks the way in and says what to fix |

Still stuck? Ask on the [Discord](https://discord.gg/RfChVr8J7T), and say what the progress line or the message said.

## Removing everything

1. In Leyline Solo, **Settings**, then **Remove sign-in**. Your game folder is put back as it was.
2. Close Leyline Solo. It stops your world and saves it first.
3. Uninstall it from Windows **Settings**, **Apps**, **Leyline Solo**.
4. Your world and your characters stay in `%APPDATA%\Leyline Solo` until you delete that folder. Delete it to remove
   everything; keep it to pick up where you left off after installing again.

Nothing of Leyline Solo is anywhere else: not on our servers, not in your game folder once sign-in is removed.
