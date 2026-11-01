# The Leyline Discord: the bot and its setup

The growth plan's Discord section (owner, 2026-10-31: "yep do it"). Code: `stream/discord_growth.py` and
`stream/discord_template.json`, loaded by `stream/forum_discord.py` (the forum link, docs/FORUM.md "The Discord
link"). It is **one bot**: the same token, REST client, rate limits, outbox and worker as the forum link. Tests:
`python3 -I stream/tests/test_discord_growth.py stream/wrath_portal.py` (a fake Discord API, every flow below).

**Everything is off until the owner sets it up.** We can't make Discord accounts or post anything public ourselves.
The bot only does what portal.env turns on, and Set up the Discord only runs when the owner presses it in HQ.

## What the bot does

| Part | Switch (portal.env) | What happens |
|---|---|---|
| Set up the Discord | none (owner presses it in HQ) | Makes the plan's channels and roles that the server doesn't have yet. Found by name, so running it again is safe. It never deletes, renames or changes anything already there |
| Live realms | `WU_DISCORD_FEED=1` | A realm on the network that goes live (first seen, or back after 6 hours away) gets a card in **live-realms**: name, mode, players, and its link (`/r/<short>` when the realm has one, else its join link). At most `WU_DISCORD_FEED_PER_HOUR` (4) an hour, busiest first; a card held back more than an hour is dropped. Realms already live when the feed is turned on are not announced, and nor is a burst after the portal was down |
| Den of the week, featured mode | `WU_DISCORD_FEED=1` | Staff pick them in HQ; posted in **den-showcase** / **modes-and-forks**. With the feed off the pick is still recorded (and Den of the week still earns the role) |
| Find a group | `WU_DISCORD_LFG=1` | "Alice wants a group for Goblin Brawl (Brawl)" in **looking-for-group**, with their Discord mention (no ping), a note and a Join button. From the website (the forum's Discord page, `/wrath/leyline/forum/discord`, `?group=<realm id>` picks the realm) or the launcher (`POST /wrath/forum/api/discord/group {realm?, what?, note?}`, Bearer token). Linked accounts only, no sanctions, the word and brand filters, one per account every `WU_DISCORD_LFG_GAP` minutes (30), `WU_DISCORD_LFG_PER_HOUR` (20) in all |
| Roles from accounts | `WU_DISCORD_ROLES=1` | Once a night at `WU_DISCORD_ROLES_HOUR` UTC (4) linked people get the roles their Leyline facts earn (below). Every change is in HQ's audit log |
| AutoMod | `WU_DISCORD_AUTOMOD=1` | Our blocked words (Content > Blocked words: slurs, sexual terms, profanity, the owner's words) as one Discord AutoMod keyword rule, "Leyline blocked words". Updated when the list changes and once a day; never deleted; turning it off in Discord is respected |

Every post: no pings, no link previews, the host's markdown shown as typed, links only to our own site or the
realm's own join link. Realms named with the game's name, flagged by the relay or hidden by staff are never posted.

### Roles

| Role | How it's earned |
|---|---|
| Realm Host | Owns a realm (a Host page realm, `hostflow.py`) that has been listed on the network for 7 days. Counted every night |
| Mode Maker | A fact recorded in HQ (Content > Discord > Record a fact) or by code (`growth.record_fact(acct, "mode_maker", why)`) |
| Builder | The same: staff record the builder approval |
| Node Keeper | The same (no account is tied to a node yet) |
| Den Architect | Den of the week; kept for good |
| Guide, Moderator, Core Team | Manual, in Discord. The bot never touches them |

A role the bot gave is taken back when its fact is gone, or when the person unlinks. A role staff gave by hand is
never taken. TODO(when they exist): record Mode Maker automatically from the modes and fork credit ("based on X by
Y"), and Node Keeper from node registrations, by calling `record_fact`.

### The channels (discord_template.json)

Start here (welcome, rules, roles, announcements: read-only), Play (looking-for-group, wrath-unleashed, live-realms
read-only for the bot's cards, events), Build (den-showcase, builders-realm, building-help), Make (modes-and-forks,
mods, addons, mode-jam, scripting-help, jam-judges for Mode Makers), Host (hosting-help, list-your-realm with a
10-minute slowmode, nodes, host-lounge for Realm Hosts, node-keepers for Node Keepers), Leyline (bug-reports, ideas,
launcher-help, roadmap for Core Team, mod-room for Moderators and Guides), Voice (Dalaran lounge, Group room 1-3,
Showcase stage). Template fields: `read_only` (only `posters` roles and the bot can write), `private` (only those
roles see it; never made if one of its roles is missing), `bot_posts`, `slowmode` (seconds), `topic`, `type`
(text, voice, stage, forum), `optional`. `posts` names the channel for each kind of post. Roles are made with no
powers and can't be pinged; `auto` names the fact that earns one, `keep` keeps it.

## Owner checklist (about 15 minutes)

1. Make the Discord server (if there isn't one), then turn on **Developer Mode** (User Settings > Advanced).
   Right-click the server icon > **Copy Server ID**.
2. **Discord Developer Portal**: use the forum link's app (docs/FORUM.md steps 1-3) or make one. Bot tab: copy the
   token; turn on **Message Content Intent** (for the forum link). No other intent is needed (no Server Members
   intent: the bot reads one member at a time).
3. **OAuth2 > URL Generator**: scopes **bot** and **applications.commands**. Bot permissions: the forum link's
   (View Channel, Send Messages, Send Messages in Threads, Create Public Threads, Read Message History, Manage
   Messages, Manage Threads) plus **Manage Channels** and **Manage Roles**. Permission integer **326686026768**.
   For AutoMod also **Manage Server**: **326686026800**. No Administrator, no Mention Everyone. Open the URL and add
   the bot (if it's already in the server, open the new URL again to give it the new permissions).
4. On the server, edit `/srv/wrath/portal.env` (never commit or paste it anywhere):
   ```
   WU_FORUM_DISCORD=1
   DISCORD_BOT_TOKEN=<the token>
   WU_DISCORD_GUILD=<the server id>
   ```
   (`WU_FORUM_DISCORD_CHANNELS` is optional now: without it the bot runs without the forum mirror.) Restart the
   portal (`docker restart wu-portal`, or the `signin` step). No realm restart.
5. Leyline HQ > Content > **Discord**: it should say the forum link is running and show the server id.
   Press **Preview**, read the list, then type `setup` and press **Set up the Discord**. Reload after a minute: "Last
   setup" lists what was made and anything Discord refused (with the reason). Run it again any time.
6. In Discord: Server Settings > **Roles**: drag the bot's own role **above** Realm Host, Mode Maker, Builder, Den
   Architect and Node Keeper (Discord only lets a bot give roles below its own). Give **Moderator** its powers by
   hand (Manage Messages, Timeout Members) and Guide what you want it to have; the bot makes roles with no powers.
   Give yourself and the team Core Team / Moderator.
7. Optional: Server Settings > **Enable Community**, then run Set up the Discord again to make the Showcase stage,
   and use Onboarding / Channels & Roles for the "roles: pick what you do" step. Write the welcome and rules posts
   yourself (the bot never posts in Start here).
8. Turn the parts on, one at a time, in portal.env, then restart the portal:
   ```
   WU_DISCORD_FEED=1        # live realms, Den of the week, featured mode
   WU_DISCORD_LFG=1         # Find a group
   WU_DISCORD_ROLES=1       # roles from accounts, nightly
   WU_DISCORD_AUTOMOD=1     # needs Manage Server (step 3)
   ```
   Optional: `WU_DISCORD_FEED_PER_HOUR` (4), `WU_DISCORD_LFG_GAP` (30 minutes), `WU_DISCORD_LFG_PER_HOUR` (20),
   `WU_DISCORD_ROLES_HOUR` (4, UTC), `WU_DISCORD_TEMPLATE` (another template file).
9. Check HQ > Content > Discord: each part on, "Last posts", "Last problem". Press **Run the role sync now** and
   **Push the words now** once to see them work.

To turn a part off: remove its line (or set it to 0) and restart the portal. Nothing made in Discord is ever deleted
by turning things off.

## Deploy

`signin` ships `discord_growth.py` and `discord_template.json` with `forum_discord.py` (fastloop.sh installs them,
with a backup). No realm restart.
