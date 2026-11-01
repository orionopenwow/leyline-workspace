# Leyline Discord: the redo (you run it, once)

This sets your Discord server up the way [docs/DISCORD-PLAN.md](../../docs/DISCORD-PLAN.md) describes: ten
categories, the eight roles, and the 30 Oct look: an emoji and the bar in front of every channel (`👋︱welcome`).
The server's name, icon and banner are never touched. Channels you already have, from the 30 Oct reset (`✦︱welcome`,
`◆︱general`, `⟡︱host-a-beacon`...) or from before, are **renamed and moved, so their history stays**; one already named right (`🤝︱looking-for-group`) keeps its name. Anything the
plan doesn't have goes into a hidden ARCHIVE. You run it from your own PC. Claude never connects to Discord and never
posts anything.

Why a bot: Discord only lets a program change a server through a bot token. Running it as your own account is
against Discord's terms. So you make a throwaway bot, use it for ten minutes, then kick it.

## The steps (about 30 minutes)

1. **Make the bot.** Open <https://discord.com/developers/applications> > **New Application**, name it
   `Leyline setup`, tick the box, **Create**. Left menu > **Bot** > **Reset Token** > **Copy**. That token is the
   bot's password. Paste it when the script asks (it hides it), or save it in `token.txt` next to the script (that
   file is never put on GitHub). Nothing else ever writes the token anywhere.
2. **Invite it.** Left menu > **OAuth2** > **OAuth2 URL Generator** > tick **bot**, then under Bot Permissions tick
   **Administrator**. Open the URL at the bottom, pick your server, **Authorize**. In Discord: **Server Settings >
   Roles**, drag `Leyline setup` to the **top**.
3. **Community first (recommended).** Server Settings > **Enable Community** (pick any channel for rules for now).
   Without it Discord won't make the two forum channels (`feature-ideas`, `forum`) or the stage; the script skips
   them and tells you, and makes them when you run it again later.
4. **Preview.** Double-click **`Leyline Discord - preview.bat`**. It prints every change and **changes nothing**.
   Check that your old channels show under "Reused channels" with the name you expect.
   TODO(owner): your old `#help` becomes `launcher-help`. If it was mostly about hosting, change `ALIASES` in the
   script (move `"help"` to the `hosting-help` line) before step 5.
5. **Do it.** Double-click **`Leyline Discord - do it.bat`**. It shows the plan again; type `YES`. It **backs up
   the server to your Desktop first** (`Leyline-discord-backup-<date>`), then makes the changes. Safe to run again:
   a second run changes nothing, and a run that stopped halfway carries on.
6. **First posts.** At the end it prints every first post from
   [docs/discord-initial-posts.md](../../docs/discord-initial-posts.md) (also saved as `first-posts.txt` in the
   backup folder). Fill in the `TODO(owner)` lines in your own words, paste each one into its channel, then hover >
   **...** > **Pin Message**. Start with `#welcome` and `#rules`.
7. **The bots' settings.** It also prints (and saves as `settings-for-the-bots.txt`) the lines with your channel
   ids, ready to paste on the server:
   - `/srv/wrath/portal.env`: `WU_DISCORD_GUILD`, `WU_DISCORD_TEMPLATE=/app/discord_template_v2.json`,
     `WU_FORUM_DISCORD_CHANNELS=...` (the forum channel and its tags, made by the script). Restart the portal.
   - `/opt/wu-discord/discord.env` (the game bridge, never paste it anywhere else): `DISCORD_CHAT_CHANNEL_ID`
     (`world-chat`), `DISCORD_BUG_CHANNEL_ID` (`bug-reports`), `WU_DISCORD_BUG_INTAKE`, `DISCORD_GUILD_ID`. Deploy the
     bridge (`hosting/discord/Deploy Discord bridge.bat`), then `systemctl restart wu-discord`.
   Then Leyline HQ > Content > Discord > **Preview** should list everything under "found" and nothing under "would
   make" (DISCORD-PLAN.md steps 4-5). Then type `setup` and press **Set up the Discord**: it makes nothing new,
   it just learns which channel is which, so its posts land in the right place.
8. **The rest by hand** (the script prints these too):
   - **Onboarding**: Server Settings > Onboarding. Rules channel `rules`; default channels `welcome`,
     `announcements`, `hangout`, `bug-reports`; a question "What do you do?" with Play / Build a Den / Host / Make
     mods, each pointing at its category.
   - **Changelog**: `#changelog` > Edit Channel > Integrations > Webhooks > New Webhook "Leyline releases" > Copy
     Webhook URL. On GitHub: the Leyline repo > Settings > Webhooks > Add webhook: that URL with `/github` on the end,
     `application/json`, "Let me select individual events": **Releases** only. The URL is a secret.
   - **Forum tags**: the script makes the five (Share your realm, Mods & addons, Dens & builds, Help, General). Rename
     or recolour them in `#forum` > Edit Channel > Tags if you like; the ids stay the same.
   - **Roles**: give Moderator, Guide and Core Team by hand. The earned roles come from the website bot
     (`WU_DISCORD_ROLES=1`). Creator and Member from the reset are left alone. TODO(owner): keep or delete them.
9. **Kick the bot.** Server Settings > Integrations > `Leyline setup` > **Kick**. Then in the Developer Portal:
   Bot > **Reset Token** again (or delete the app), so the token is dead. Delete `token.txt`.
10. **Undo, if you need to.** The backup folder has `guild.json`, `roles.json`, `channels.json` (names, categories
    and every permission, as they were) and `what-changed.txt` (every change made). Archived channels are only
    hidden: drag one back out of ARCHIVE and copy its old permissions from `channels.json`. Renames can be put back
    by hand from the same file. Deleted channels and wiped chats can't come back (only their last messages are in
    `messages\`), which is why those options ask you to type a word first.

## What it changes

| Category | Channels | Notes |
|---|---|---|
| **Start here** | `👋︱welcome`, `📜︱rules`, `📣︱announcements`, `📝︱changelog` | read-only; Core Team and Moderator post news, Core Team the changelog |
| **Wrath Unleashed** | `💬︱hangout`, `📈︱levelling`, `🌍︱world-chat`, `🤝︱looking-for-group`, `🟢︱live-realms`, `🎉︱events` | `🟢︱live-realms` and `🎉︱events` read-only; `🌍︱world-chat` is live with the game |
| **Your Den** | `📸︱den-showcase`, `📦︱den-files` (1 min slowmode), `🔨︱den-help` | |
| **Host a server** | `📋︱host-templates`, `🏰︱hosting-help`, `📢︱list-your-realm` (10 min slowmode), `🔗︱nodes`, `☕︱host-lounge`, `🔑︱node-keepers` | `📋︱host-templates` read-only; `☕︱host-lounge` only Realm Hosts, `🔑︱node-keepers` only Node Keepers |
| **Mods & Workshop** | `📤︱workshop-submissions` (10 min slowmode), `🧩︱mods-and-addons`, `🎲︱modes-and-forks`, `💻︱scripting-help`, `🎮︱mode-jam`, `🔍︱jam-judges` | `🔍︱jam-judges` only Mode Makers |
| **Claude & the companion** | `🤖︱claude-and-agents`, `🐾︱companions` | |
| **Feedback** | `🐞︱bug-reports`, `💡︱feature-ideas` (forum channel), `🛟︱launcher-help` | every message in `🐞︱bug-reports` goes on the bug list |
| **Forum** | `📚︱forum` (forum channel, 5 tags) | the website forum, live |
| **Team** | `🧭︱roadmap`, `🔒︱mod-room` | private: Core Team; Moderators and Guides |
| **Voice** | `🎧︱Dalaran lounge`, `🔊︱Group room 1`-`3`, `🎤︱Showcase stage` (Community only) | |
| Roles | Core Team, Moderator, Guide, Realm Host, Mode Maker, Builder, Den Architect, Node Keeper | colours from the plan; Helper is renamed Guide; nobody gets Administrator |
| Server | system messages to `👋︱welcome`; rules channel `📜︱rules`; AFK off | the name, icon, banner and invite background are never changed |
| Old channels | into the hidden **ARCHIVE** | only Moderators (and you) can read it; nothing is deleted |

Old names it picks up (the full list is `ALIASES` in the script): general / wrath-unleashed > `hangout`, help >
`launcher-help`, host-a-beacon > `hosting-help`, mods-and-workshop / mods > `mods-and-addons`, showcase /
builders-realm > `den-showcase`, building-help > `den-help`, Hangout (voice) > `Dalaran lounge`; categories START >
Start here, PLAY > Wrath Unleashed, CREATE / Build > Your Den, Make > Mods & Workshop, Host > Host a server, FEEDBACK /
Leyline > Feedback. `ideas` can't turn into a forum channel, so it goes to the archive with its history and
`feature-ideas` is new. The names (emoji, the bar `︱`, then the name) are exactly the ones in
`stream/discord_template_v2.json`, so the website bot finds them by exact name; a channel whose emoji differs (the
older `✦︱welcome`) gets the plan's (`👋︱welcome`). The emoji for each channel are `EMOJI` in the script.

If the Leyline website bot is already in the server (a bot role with "leyline" in its name, or name it with
`--leyline-bot "<role>"`), it is allowed to post in the read-only and private channels. If you add it later, run the
script again.

**Never touched:** @everyone (only per-channel permissions), roles of bots and integrations, roles above the bot,
roles you have yourself, your members.

## Options

Add them after the .bat name in a command prompt (`"Leyline Discord - do it.bat" --post-first-posts`):

| Option | |
|---|---|
| `--post-first-posts` | lets the bot post and pin the first posts that have **no** `TODO(owner)` left, after you type `POST`. Never in Start here, never in the forum channels; the rest are printed for you. Off by default: they read better from you. |
| `--delete-archived` | after the backup, delete the archived channels and empty old categories. You type the server's name. |
| `--delete-old-roles` | delete roles that aren't in the plan (Member and Owner are kept). You type `DELETE ROLES`. |
| `--wipe-history` | replace every text channel with a fresh empty copy. You type `WIPE`. (`Leyline Discord - reset chats.bat` does this plus `--delete-archived`.) |
| `--backup-messages N` | messages to back up per channel (default 1000; `0` skips messages). |
| `--guild <id>` | which server, if the bot is in more than one (otherwise it asks). |

## The rules for anything players see

No game or publisher names (World of Warcraft, WoW, Warcraft, Lich King, Blizzard) in channel names, topics or posts,
and no money or tip links (no Patreon, Ko-fi, PayPal, donations). The script's names and topics are checked for this,
and `--post-first-posts` refuses any draft that breaks it. Please keep to it in what you write too.

## For developers

`python3 tools/discord-leyline/test_discord_leyline.py` runs the tests against a fake Discord on localhost
(`LEYLINE_DISCORD_API` points the script at it). `LAYOUT` in the script and `stream/discord_template_v2.json` must
agree (a test checks names, types, topics, slowmodes and permissions): change both together. Standard library only.
