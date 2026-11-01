# The Leyline Discord: channel upgrade plan

Owner, 1 Nov 04:18: "create a Discord channel upgrade plan to suit all of the features and desired visions that we
have so far, and initial posts". The first posts are drafts in [discord-initial-posts.md](discord-initial-posts.md).

**Nothing here is posted or changed by us.** The owner applies it by hand, or presses Set up the Discord in Leyline HQ
himself; we run that step for him only after his explicit OK, and it only ever adds what is missing.

## Where we start from

Two earlier passes exist, and the live server may hold either (or both):

| Pass | What it made | Code |
|---|---|---|
| Rebrand and reset (30 Oct, owner-run) | START (welcome, announcements), PLAY (general, help, looking-for-group, Hangout voice), CREATE (mods-and-workshop, host-a-beacon, showcase), FEEDBACK (bug-reports, ideas); roles Moderator, Helper, Creator, Member; old channels in a hidden ARCHIVE. Channel names may carry a glyph or emoji prefix (`✦︱welcome`) | `tools/discord-leyline/` |
| Growth plan (31 Oct, Set up the Discord in HQ) | Start here, Play, Build, Make, Host, Leyline, Voice; 8 roles (Core Team, Moderator, Guide, Realm Host, Mode Maker, Builder, Den Architect, Node Keeper) | `stream/discord_template.json`, docs/DISCORD.md |

Since then Leyline has become: **Wrath Unleashed** as the one official realm to hang out and level on, **Your Den**
(one button in the launcher, Den files shared on the Workshop), **Host a server** (the launcher's Host page with
rulesets and presets, the Host pack), **Mods & Workshop** (List your mod, mode packs, the addon marketplace), the
launcher's **Agents** page (your own Claude) and the in-game **companion bots**, the **forum** linked to Discord, and
now the **bugs manifest** (docs/BUGS-MANIFEST.md). The Builders' Realm is hidden in the launcher, so it gets no channel.

## The new layout

The bot's version of it is `stream/discord_template_v2.json` (checked by the bot's own template checker). Names keep
the 30 Oct look (owner, 1 Nov: "keep the emoji thematics and bar too"): an emoji, the bar `︱`, then the name below
(`👋︱welcome`, `💬︱hangout`, `🎧︱Dalaran lounge`). Set up the Discord finds channels **by exact name**, so the template
carries the full names, the same as the owner-run script's `EMOJI` (tools/discord-leyline; a test keeps them equal).
The server's name and icon stay as they are (owner, 1 Nov). The table uses the names without their emoji.

| Category | Channel | Who writes | What goes there | Fed by |
|---|---|---|---|---|
| **Start here** | `welcome` | owner, team | what Leyline is, the three links, where to go next | the owner's post (pinned) |
| | `rules` | owner, team | the rules | the owner's post (pinned) |
| | `announcements` | Core Team, Moderator | news | the owner; forum Announcements threads (forum link, `news=`) |
| | `changelog` | Core Team | the launcher's release notes, as each release ships | GitHub release webhook (step 8) |
| **Wrath Unleashed** | `hangout` | everyone | general talk about the realm | people |
| | `levelling` | everyone | routes, quests, `.guide`, gear | people |
| | `world-chat` | everyone | live with the game's World channel ("[Discord] Name: text" in game) | the bridge (`DISCORD_CHAT_CHANNEL_ID`) |
| | `looking-for-group` | everyone | groups | people; Find a group (bot, `WU_DISCORD_LFG`) |
| | `live-realms` | bot | realms as they go live | bot (`WU_DISCORD_FEED`) |
| | `events` | Core Team, Moderator, Guide | weekly Dalaran events | the team |
| **Your Den** | `den-showcase` | everyone | screenshots of Dens | people; Den of the week (bot) |
| | `den-files` | everyone, 60 s slowmode | Workshop links to shared Den files | people (Share on the Workshop in the launcher) |
| | `den-help` | everyone | edit mode, props, paths, save/load, invites | people |
| **Host a server** | `host-templates` | Core Team, Guide | the rulesets and presets, one post each | the team |
| | `hosting-help` | everyone | help with the Host page and the Host pack | people |
| | `list-your-realm` | everyone, 10 min slowmode | hosts tell people about their realm | people |
| | `nodes` | everyone | running a node | people |
| | `host-lounge` | Realm Host (private) | hosts among themselves | people |
| | `node-keepers` | Node Keeper (private) | network changes first | the team |
| **Mods & Workshop** | `workshop-submissions` | everyone, 10 min slowmode | a Workshop link to something you made, for the team to look at | people |
| | `mods-and-addons` | everyone | talk, questions, problems with a mod | people |
| | `modes-and-forks` | everyone | game modes and forks | people; the featured mode (bot) |
| | `scripting-help` | everyone | realm scripts and mode files | people |
| | `mode-jam` | everyone | the monthly mode jam (no prizes) | people |
| | `jam-judges` | Mode Maker (private) | judging | people |
| **Claude & the companion** | `claude-and-agents` | everyone | the launcher's Agents page: connecting Claude, what it may do | people |
| | `companions` | everyone | the in-game bots: `.recruit`, `.buddy`, `/comp` | people |
| **Feedback** | `bug-reports` | everyone | one message per bug: what, where, what you expected | people; the in-game `.bug` feed (bridge). **Every person's message goes into the bugs manifest** |
| | `feature-ideas` | everyone (Forum channel) | one idea per post; react to vote | people |
| | `launcher-help` | everyone | installing, signing in, updates | people |
| **Forum** | `forum` (Forum channel) | linked accounts | the website forum, live: a post per forum thread, replies both ways; tags per forum category | the forum link (`WU_FORUM_DISCORD_CHANNELS`) |
| **Team** | `roadmap` | Core Team (private) | what's next | the team |
| | `mod-room` | Moderator, Guide (private) | moderation | the team |
| **Voice** | Dalaran lounge, Group room 1-3, Showcase stage (Community servers) | everyone | | |

### Roles

The growth plan's eight, unchanged: **Core Team**, **Moderator**, **Guide** (by hand, in Discord), and **Realm Host**,
**Mode Maker**, **Builder**, **Den Architect**, **Node Keeper** (earned, given by the nightly role sync from Leyline
facts, `WU_DISCORD_ROLES=1`). From the reset pass: **Helper** becomes **Guide** (rename it by hand to keep who has it),
**Creator** is covered by Mode Maker and Builder (keep it or delete it, TODO(owner)), **Member** is not needed (keep it
if you use it for onboarding). Nobody gets Administrator; the bot's roles have no powers and can't be pinged.

### Permissions

- **Read-only** channels (welcome, rules, announcements, changelog, live-realms, events, host-templates): @everyone
  can read but not send or start threads; the named roles and the bot can post. The bot sets these overwrites when
  it makes a channel; for a channel you keep from before, copy them by hand (Edit Channel > Permissions).
- **Private** channels: @everyone can't see them; only the named roles. The bot never makes one if a role it is
  private to is missing (so it can't end up public by mistake).
- **Moderator**: give it Manage Messages and Timeout Members by hand. **Guide**: what you want (Manage Threads is
  useful in the help channels).
- The bot (one Discord app for both the portal and the bridge, recommended): View Channel, Send Messages, Send
  Messages in Threads, Create Public Threads, Read Message History, Manage Messages, Manage Threads, Manage Channels,
  Manage Roles (and Manage Server for AutoMod): permission integer **326686026800**. Message Content intent on. Its
  role above Realm Host, Mode Maker, Builder, Den Architect and Node Keeper.

### Who posts where (bots and webhooks)

| Poster | Where | Switch | Notes |
|---|---|---|---|
| Leyline bot, forum link | `forum` (Forum channel, one tag per category), `announcements` (forum Announcements) | `WU_FORUM_DISCORD=1`, `WU_FORUM_DISCORD_CHANNELS` | stream/forum_discord.py; replies come back only from linked accounts |
| Leyline bot, growth | `live-realms`, `den-showcase`, `modes-and-forks`, `looking-for-group`; roles; AutoMod | `WU_DISCORD_FEED`, `WU_DISCORD_LFG`, `WU_DISCORD_ROLES`, `WU_DISCORD_AUTOMOD` | stream/discord_growth.py; channel by template key, so the new template keeps the same keys |
| Bridge (wu-discord) | `world-chat` both ways; `bug-reports`: the `.bug` feed out, people's messages in to the manifest | `DISCORD_CHAT_CHANNEL_ID`, `DISCORD_BUG_CHANNEL_ID`, `WU_DISCORD_BUG_INTAKE` | hosting/discord; its own new-realm announcements stay **off** (`live-realms` covers it) |
| GitHub webhook | `changelog` | a Discord webhook on the repo | Releases only; the text is the launcher's `CHANGELOG.md` section for that release |

Every bot post: no pings, no link previews. The bridge skips its own and other bots' messages, so the `.bug` feed in
`bug-reports` is never counted twice in the manifest (game reports come into it from the realm's own table).

## Old channel to new (by hand, before Set up)

Set up never renames or moves. Rename these first so it finds them and history is kept; then drag each under its
new category. Names on the left without their glyph or emoji; give each its new emoji name from the template. **Or let the owner-run script do all of it** (steps 2
and 6-7's ids included): `tools/discord-leyline/` ("Leyline Discord - do it.bat"; its README is the guide). It builds
exactly this layout with these names, renames and moves the old channels, archives the rest, backs up first.

| Today (reset pass / growth plan) | Becomes | In |
|---|---|---|
| general / wrath-unleashed | `hangout` | Wrath Unleashed |
| help | `launcher-help` (or `hosting-help`; TODO(owner): which one it mostly was) | Feedback |
| looking-for-group | `looking-for-group` | Wrath Unleashed |
| mods-and-workshop / mods / addons | `mods-and-addons` | Mods & Workshop |
| host-a-beacon | `hosting-help` | Host a server |
| showcase / builders-realm | `den-showcase` | Your Den |
| building-help | `den-help` | Your Den |
| ideas | `feature-ideas` is a Forum channel: keep `ideas` read-only for its history, or move it to the archive | Feedback |
| Hangout (voice) | `Dalaran lounge` | Voice |
| category Leyline / FEEDBACK | `Feedback` | |
| category Build / CREATE | `Your Den` | |
| category Make | `Mods & Workshop` | |
| category Host | `Host a server` | |
| category Play / PLAY | `Wrath Unleashed` | |
| roadmap, mod-room | the same names | Team |

Anything else left over goes into the hidden ARCHIVE category (the reset script's pattern); nothing needs deleting.

## Applying it: the owner's steps (about 30 minutes)

0. **Names.** Decided (owner, 1 Nov): the emoji and bar names. To change an emoji, change it in both
   `discord_template_v2.json` and `EMOJI` in tools/discord-leyline/discord_leyline.py (the tests check they agree).
1. **Ship.** After this branch is merged, run the Control step **signin** (it installs `discord_template_v2.json`
   beside the current template; nothing changes yet), then **gm** and **bugs** for the bugs manifest.
2. **Rename and move by hand** (the table above), in Discord.
3. **Point the bot at the new plan.** In `/srv/wrath/portal.env` add `WU_DISCORD_TEMPLATE=/app/discord_template_v2.json`
   and restart the portal (`docker restart wu-portal`, or the `signin` step).
4. **Preview.** Leyline HQ > Content > Discord > **Preview**. Read "would make" and "found". If something you renamed
   shows under "would make", its name or type doesn't match: fix it in Discord and preview again.
5. **Set up.** Type `setup`, press **Set up the Discord**, reload after a minute and read "Last setup". Discord
   refusing a Forum channel or the stage means the server isn't a Community server yet (Server Settings > Enable
   Community), then run Set up again. Run it as often as you like.
6. **The bridge.** In `/opt/wu-discord/discord.env` (never paste it anywhere):
   `DISCORD_CHAT_CHANNEL_ID=<world-chat id>`, `DISCORD_BUG_CHANNEL_ID=<bug-reports id>`,
   `WU_DISCORD_BUG_INTAKE=/srv/wrath/bugs/intake/discord.jsonl`, `DISCORD_GUILD_ID=<server id>`. Deploy the bridge
   (`hosting/discord/Deploy Discord bridge.bat`; it makes the intake folder), then `systemctl restart wu-discord`.
7. **The forum link.** In `forum` (the Forum channel) make one tag per forum category: Share your realm, Mods &
   addons, Dens & builds, Help, General (right-click a tag > Copy ID). In portal.env:
   `WU_FORUM_DISCORD_CHANNELS=realms=<forum id>/<tag>,mods=<forum id>/<tag>,dens=<forum id>/<tag>,help=<forum id>/<tag>,general=<forum id>/<tag>,news=<announcements id>`,
   restart the portal. docs/FORUM.md "The Discord link" has the rest.
8. **Changelog.** In Discord: `changelog` > Edit Channel > Integrations > Webhooks > New Webhook, name it
   "Leyline releases", Copy Webhook URL. On GitHub: the Leyline repo > Settings > Webhooks > Add webhook, Payload URL
   = the webhook URL with `/github` on the end, content type `application/json`, "Let me select individual events":
   **Releases** only. The URL is a secret: don't paste it in a chat or a file in the repo.
9. **First posts.** Paste the drafts from [discord-initial-posts.md](discord-initial-posts.md) yourself (fill in the
   TODO(owner) lines), then hover > ... > **Pin Message**. The bots never post in Start here.
10. **Onboarding** (Community servers): Server Settings > Onboarding: rules = `rules`, default channels = welcome,
    announcements, hangout, bug-reports; a question "What do you do?" with Play / Build a Den / Host / Make mods,
    each pointing at its category.
11. **Turn on the bot's parts** one at a time (docs/DISCORD.md step 8) and check HQ > Content > Discord.

**Undo.** Remove `WU_DISCORD_TEMPLATE` from portal.env and restart the portal: the bot goes back to the old plan.
Nothing it made is deleted by that; channels you don't want go to the archive by hand.

**If the owner says "you do it".** Only steps 4-5 can be done for him (Preview, then Set up the Discord, which only
adds), and only with his explicit OK in that conversation. Every rename, move, permission, webhook and post stays his.
