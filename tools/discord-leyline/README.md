# Leyline Discord: the launch layout (you run it, from Leyline Control)

This sets your Discord server to the launch layout in [docs/DISCORD-PLAN.md](../../docs/DISCORD-PLAN.md) (3 Nov):
**13 channels in five categories**, each dressed `<emoji>︱<name>` (`👋︱welcome`), with a clear topic, the right
permissions, and pinned guides. It is your server as it is today, kept and tidied: the channels you have are
**renamed and moved, so their history stays**; one already right is left alone. The server's name, icon and banner
are never touched. You run it from your own PC. Claude never connects to Discord and never posts anything.

Why a bot: Discord only lets a program change a server through a bot token. Running it as your own account is
against Discord's terms. So you make a bot, use it for ten minutes, then kick it.

**Safe to run again.** Every run reads the server first and changes only what differs; a second run changes
nothing. The preview lists every change before anything happens.

## What changes on today's server

| | Today | After |
|---|---|---|
| START | 👋 welcome, 📣 announcements | the same, read-only, plus **📖 guides-and-faq** (new, read-only: every guide and an FAQ) |
| PLAY | 💬 general, 🛟 help, 🤝 looking-for-group, 🎧 Hangout (voice) | the same, with topics |
| CREATE | 🧩 mods-and-workshop, 🏰 host-a-beacon, 📷 showcase | 🏰 **host-a-realm** (renamed: players see "Host a realm" now), 📸 showcase; the `/submit` command in mods-and-workshop's topic |
| FEEDBACK | 🐞 bug-reports, 💡 ideas | the same, with topics |
| 🔒 STAFF | mod-log | 🔒︱mod-log, private to Moderator and Helper |
| Roles | Moderator, Helper, Creator, Member | kept as they are (name, colour, place); a missing Moderator or Helper is made |
| Server | | system messages to #welcome (and the Community rules channel, if Community is on); AFK off |
| Old channels | anything not above | into a hidden **ARCHIVE** (Moderators can read it), or deleted if you choose so in the card |
| Pinned posts | | from [docs/discord-launch-posts.md](../../docs/discord-launch-posts.md), if you tick Post and pin |

## In Leyline Control > Discord

Make and invite the bot (steps 1-2 below), then:

1. **Paste the token** and press **Save**. It is kept on this PC only, in `token.txt` next to this README
   (git-ignored, so it never goes to GitHub). Control never shows it again and hides it from every log.
2. **Choose** (both are remembered):
   - **Old channels (not in the layout)**: *Move them to a hidden ARCHIVE* (the default: nothing is deleted),
     *Delete the empty ones, archive the rest* (a channel with no messages at all), or *Delete them all, even with
     messages* (the backup keeps their last 1000 messages). A channel with messages is never deleted without that
     last choice.
   - **Post and pin the guide posts**: the bot posts and pins each finished block from docs/discord-launch-posts.md
     (#welcome, the guides and the FAQ in #guides-and-faq, and the how-to posts in #help, #mods-and-workshop,
     #host-a-realm, #bug-reports and #ideas). A pin that already says the same is left alone; when you change a block,
     the bot edits its own pin instead of posting again. Your launch post in #announcements has `TODO(owner)` lines,
     so it is always yours to write and pin.
3. **Preview**. Changes nothing. The card lists every change by section: new channels, renames, each new topic's
   text, permissions, what goes to ARCHIVE (and which old channels have no messages), the pinned posts, and anything
   it would delete.
4. **Do it**. Read the list, type `YES`, press **Do it**. It backs up the server to your Desktop first, then makes
   exactly those changes with the choices the preview used. It needs a preview from the last hour, waits for a new
   preview if you changed a choice, and stops, changing nothing, if the server changed since the preview.
5. **Bot settings**. Changes nothing: the lines with your channel ids for the website bot and the game bridge
   (step 5 below).
6. When you're finished, kick the bot (step 7) and press **Remove** in the card.

## The steps

1. **Make the bot.** Open <https://discord.com/developers/applications> > **New Application**, name it
   `Leyline setup`, tick the box, **Create**. Left menu > **Bot** > **Reset Token** > **Copy**. That token is the
   bot's password: paste it in Leyline Control > Discord and nowhere else.
2. **Invite it.** Left menu > **OAuth2** > **OAuth2 URL Generator** > tick **bot**, then under Bot Permissions tick
   **Administrator**. Open the URL at the bottom, pick your server, **Authorize**. In Discord: **Server Settings >
   Roles**, drag `Leyline setup` to the **top**.
3. **Preview, then Do it** in Leyline Control (above).
4. **Your launch post.** Write it in #announcements from the draft in docs/discord-launch-posts.md, then hover >
   **...** > **Pin Message**. Without Post and pin, Do it also prints the other blocks (and saves them as
   `first-posts.txt` in the backup folder) for you to paste and pin.
5. **The bots' settings.** Do it prints (and saves as `settings-for-the-bots.txt`) the lines with your channel ids:
   - `/srv/wrath/portal.env`: `WU_DISCORD_GUILD`, `WU_DISCORD_TEMPLATE=/app/discord_template_launch.json`,
     `WU_FORUM_DISCORD_CHANNELS=news=<announcements id>`. Restart the portal. Then Leyline HQ > Content > Discord >
     **Preview** should list everything under "found"; type `setup` and press **Set up the Discord** once so the
     website bot learns which channel is which (it makes nothing new).
   - `/opt/wu-discord/discord.env` (the game bridge; never paste it anywhere else): `DISCORD_BUG_CHANNEL_ID`
     (`bug-reports`), `WU_DISCORD_BUG_INTAKE`, `DISCORD_GUILD_ID`. The launch layout has no live game-chat channel:
     keep your `DISCORD_CHAT_CHANNEL_ID` as it is, or leave it empty. Then `systemctl restart wu-discord`.
6. **By hand** (Do it prints these too): Community servers: Server Settings > Onboarding, default channels #welcome,
   #announcements, #guides-and-faq, #general, #help. Give Moderator and Helper by hand. TODO(owner): keep or delete
   the old Creator and Member roles.
7. **Kick the bot.** Server Settings > Integrations > `Leyline setup` > **Kick**. Then in the Developer Portal:
   Bot > **Reset Token** again (or delete the app), so the token is dead. Press **Remove** in the card.
8. **Undo, if you need to.** The backup folder has `guild.json`, `roles.json`, `channels.json` (names, categories
   and every permission, as they were) and `what-changed.txt`. Archived channels are only hidden: drag one back out
   of ARCHIVE and copy its old permissions from `channels.json`. Renames and topics can be put back by hand from the
   same file. Deleted channels can't come back (only their last messages are in `messages\`).

Old names it picks up (the full list is `ALIASES` in the script), so a server from any earlier pass lands here:
host-a-beacon / hosting-help > `host-a-realm`, hangout > `general`, launcher-help > `help`, mods-and-addons /
workshop-submissions > `mods-and-workshop`, den-showcase > `showcase`, mod-room > `mod-log`, Dalaran lounge (voice) >
`Hangout`; categories Start here > START, Wrath Unleashed > PLAY, Your Den / Mods & Workshop / Host a server > CREATE,
Team > STAFF. The names are exactly the ones in `stream/discord_template_launch.json`, so the website bot finds them
by exact name.

If the Leyline website bot is already in the server (a bot role with "leyline" in its name, or name it with
`--leyline-bot "<role>"`), it may post in the read-only and private channels. If you add it later, run this again.

**Never touched:** @everyone (only per-channel permissions), roles of bots and integrations, roles above the bot,
roles you have yourself, your members, the server's name, icon, banner and invite background.

## By hand, without Control

`Leyline Discord - preview.bat` and `Leyline Discord - do it.bat` do the same from a window (do it asks for `YES`).
Options, after the .bat name in a command prompt:

| Option | |
|---|---|
| `--old-channels archive / delete-empty / delete-all` | what happens to old channels (as the card's choice). `delete-all` asks you to type the server's name. |
| `--pin-posts` | the card's Post and pin. Asks you to type `POST`. |
| `--delete-archived` | after the backup, delete everything in ARCHIVE and empty old categories. You type the server's name. |
| `--delete-old-roles` | delete roles that aren't in the plan (Member and Owner are kept). You type `DELETE ROLES`. |
| `--wipe-history` | replace every text channel with a fresh empty copy. You type `WIPE`. |
| `--backup-messages N` | messages to back up per channel (default 1000; `0` skips messages). |
| `--guild <id>` | which server, if the bot is in more than one (otherwise it asks). |

## The rules for anything players see

No game or publisher names (World of Warcraft, WoW, Warcraft, Lich King, Blizzard) in channel names, topics or posts,
no game version numbers, and no money or tip links. The script's names, topics and the posts are checked for this,
and the bot refuses to post a block that breaks it.

## For developers

`Control - Discord preview.bat`, `Control - Discord do it.bat` and `Control - Discord bot settings.bat` are Leyline
Control's steps `discord-preview`, `discord-apply` and `discord-check` (tools/control/control.py); they never ask
anything (`--from-control`). The card's choices reach the script as `LEYLINE_DISCORD_OLD` (archive, empty, all) and
`LEYLINE_DISCORD_PINS` (1); Control records them with the preview and gives Do it the preview's. `--yes-from-control`
works only when Control runs it as `discord-apply` (`LEYLINE_CONTROL_STEP`), never with the role, wipe or archive
delete options, and with the preview's number of changes (`LEYLINE_DISCORD_EXPECT_CHANGES`).
`python3 -I tools/control/test_control_discord.py` tests the card end to end.

`python3 tools/discord-leyline/test_discord_leyline.py` runs the tests against a fake Discord on localhost
(`LEYLINE_DISCORD_API` points the script at it), including a copy of today's server from the owner's screenshot.
`LAYOUT` in the script and `stream/discord_template_launch.json` must agree (a test checks names, types, topics and
permissions): change both together. Standard library only. The bigger 2 Nov plan is `stream/discord_template_v2.json`
and [docs/discord-initial-posts.md](../../docs/discord-initial-posts.md), kept for later.
