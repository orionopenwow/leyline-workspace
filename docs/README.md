# The docs folder: where to start

`docs/` holds two kinds of writing:

- **Player and host pages** in [`leyline/`](leyline/). These are the pages on the website at `/wrath/docs` (the
  portal reads them from here: `stream/docs.py`). Start with [leyline/index.md](leyline/index.md).
- **Everything else** in this folder: specs of what is built, owner runbooks, plans for what is not built yet, and
  history. Grouped below. A file's own first lines say its status and date; when they disagree with this list, the
  file wins.

Marks: **(superseded)** means a newer page covers it, so read that one first; **(history)** means a record of what
happened, kept as it was.

## For players

The player guides live in [`leyline/`](leyline/): Getting started, Play, Single player, Leyline Solo, Friends,
Your Den, Hero Key (and "Lost your key?"), On your phone, Commands, the Launcher, Agents.

Specs behind them, for the curious:

| File | What it is |
|---|---|
| [SINGLE-PLAYER-ONE-CLICK.md](SINGLE-PLAYER-ONE-CLICK.md) | Single player in one click, no server |
| [BUILTIN-ENGINE.md](BUILTIN-ENGINE.md) | Single player's built-in engine (no Docker) |
| [LEYLINE-SOLO.md](LEYLINE-SOLO.md), [LEYLINE-SOLO-PHASE0.md](LEYLINE-SOLO-PHASE0.md) | Leyline Solo, the standalone first-era app |
| [PHONE-SOLO.md](PHONE-SOLO.md) | Solo play on a phone |
| [COMPANIONS.md](COMPANIONS.md), [BOTS-UPDATES.md](BOTS-UPDATES.md) | Bots for dungeons and raids |
| [LEYLINE-FRIENDS.md](LEYLINE-FRIENDS.md) | Friends with no server: signed friendships, the relay mailbox (built 12 Nov) |
| [SOCIAL.md](SOCIAL.md) | The website's friends and messages (signed-in accounts) |
| [SIGN-IN.md](SIGN-IN.md) | Log in (Hero Key, Google) and profiles, website and launcher |
| [HERO-KEY-SSO.md](HERO-KEY-SSO.md) | Hero Key sign-in on community realms (built, off) |
| [BOSS-FIGHTS.md](BOSS-FIGHTS.md), [BOSS-FIGHT-HEROES.md](BOSS-FIGHT-HEROES.md), [BOSS-RUSH.md](BOSS-RUSH.md) | Instant boss fights and Boss Rush |
| [LEADERBOARDS.md](LEADERBOARDS.md) | Signed leaderboards: Boss Rush and Duels |
| [MODES.md](MODES.md), [leyline-modes-play.md](leyline-modes-play.md) | Game modes and how to play them |
| [SURVIVORS.md](SURVIVORS.md) | Azeroth Survivors |
| [RACE-CLASS-COMBOS.md](RACE-CLASS-COMBOS.md) | Race and class combinations on Wrath Unleashed |
| [TOUCH-CONTROLS.md](TOUCH-CONTROLS.md), [MOBILE-RELEASE.md](MOBILE-RELEASE.md), [ANDROID.md](ANDROID.md) | Phones and tablets |
| [SIMPLE-MODE.md](SIMPLE-MODE.md) | The launcher's Simple mode |
| [THEMES.md](THEMES.md) | Launcher themes |

## Your Den

| File | What it is |
|---|---|
| [DEN.md](DEN.md), [DEN-FEATURES.md](DEN-FEATURES.md) | Dens: what they are, the full feature list |
| [DEN-INVITE.md](DEN-INVITE.md), [DEN-HOSTING.md](DEN-HOSTING.md) | Invite a friend, host your Den |
| [DEN-TOOLS.md](DEN-TOOLS.md), [DEN-STAMPS.md](DEN-STAMPS.md), [DEN-PLACES.md](DEN-PLACES.md), [DEN-PLACEMENT.md](DEN-PLACEMENT.md) | Builder tools, stamps, places, placing with the mouse |
| [DEN-MARKETPLACE.md](DEN-MARKETPLACE.md), [DEN-TOURNAMENTS.md](DEN-TOURNAMENTS.md) | Shared Dens, duel tournaments |
| [JAINA.md](JAINA.md) | Jaina in Den Lite |
| [DEN-NEXT.md](DEN-NEXT.md), [DEN-OVERHAUL.md](DEN-OVERHAUL.md), [DEN-335-PLAN.md](DEN-335-PLAN.md), [DEN-EDITOR-PARITY.md](DEN-EDITOR-PARITY.md), [NOGGIT-DEN-PLAN.md](NOGGIT-DEN-PLAN.md) | Plans and research (dated) |

## For hosts

| File | What it is |
|---|---|
| [HOST-PACK.md](HOST-PACK.md) | The Host pack (what hosts download) |
| [HOST-ANY-CLIENT.md](HOST-ANY-CLIENT.md) | Host any client |
| [BE-A-NODE.md](BE-A-NODE.md) | Be a node (the site flag `be_a_node` is off: not offered to players yet) |
| [QUEUE.md](QUEUE.md) | Server capacity and the browser queue |
| [VANILLA-REMASTERED.md](VANILLA-REMASTERED.md) | The first-era realm as an official Beacon |
| [BUILD-REALM.md](BUILD-REALM.md) | The Builders' Realm |
| [HOSTING-AUDIT-2026-11-05.md](HOSTING-AUDIT-2026-11-05.md) | Hosting and peer-to-peer audit (5 Nov) |

## Mods and creators

| File | What it is |
|---|---|
| [WORKSHOP.md](WORKSHOP.md), [LIST-YOUR-MOD.md](LIST-YOUR-MOD.md) | The Workshop and listing your mod |
| [GAME-MODS.md](GAME-MODS.md) | Game mods and the mods a realm uses |
| [CREATOR-API.md](CREATOR-API.md) ([html](creator-api.html)) | The creator API for Modes |
| [GOLF-COURSES.md](GOLF-COURSES.md) | Making minigolf courses |
| [HUB-INDEX.md](HUB-INDEX.md), [HUB-RANKINGS.md](HUB-RANKINGS.md) | The hub's community index and rankings |
| [MODS-DAILY.md](MODS-DAILY.md) | The daily mods discovery (nothing is approved by the tools) |
| [BUILD-ON-LEYLINE.md](BUILD-ON-LEYLINE.md), [TOOLKIT.md](TOOLKIT.md) | Building on Leyline, every part as a tool |

## The network

| File | What it is |
|---|---|
| [LEYLINE-MESH.md](LEYLINE-MESH.md) | The mesh: a realm list and relays that don't depend on us (flags off live) |
| [COMMONHOLD-V1.md](COMMONHOLD-V1.md) | The network protocol (draft) |
| [P2P-AND-TORRENT.md](P2P-AND-TORRENT.md) | P2P and torrent: built, tested, off |
| [RESILIENCE.md](RESILIENCE.md) | What keeps working when something goes down |
| [AGENT-PROTOCOL.md](AGENT-PROTOCOL.md) | How AI agents build Leyline |
| [leyline-realm-hopping.md](leyline-realm-hopping.md) | Realm hopping (MVP) |
| [HYBRID-CLIENT.md](HYBRID-CLIENT.md), [LAUNCHER-IN-BROWSER.md](LAUNCHER-IN-BROWSER.md), [LAUNCHER-LISTS.md](LAUNCHER-LISTS.md), [LAUNCHER-VERSION-POLICY.md](LAUNCHER-VERSION-POLICY.md), [SERVER-WINDOW.md](SERVER-WINDOW.md) | Launcher internals |

## Owner runbooks (the website, the server, the Discord)

| File | What it is |
|---|---|
| [ADMIN-CONSOLE.md](ADMIN-CONSOLE.md) | Leyline HQ, the admin console |
| [AUDIT.md](AUDIT.md) | The release audit |
| [FIRST-RUN-TEST.md](FIRST-RUN-TEST.md), [FRESH-COMPUTER-TEST.md](FRESH-COMPUTER-TEST.md), [OWNER-TEST-PLAN.md](OWNER-TEST-PLAN.md) | Fresh-machine tests |
| [DOMAIN-MOVE.md](DOMAIN-MOVE.md) | Moving to leylinelauncher.com (the old name keeps answering) |
| [KILL-SWITCH.md](KILL-SWITCH.md), [GAME-DATA-ON-OUR-SERVER.md](GAME-DATA-ON-OUR-SERVER.md) | The kill switch and the game data killswitch |
| [SECURITY-HARDENING.md](SECURITY-HARDENING.md) | Security hardening (31 Oct) |
| [SITE-SPEED-AUDIT.md](SITE-SPEED-AUDIT.md) | Site speed audit (4 Nov; Q3 and M-a fixed 12 Nov: no page waits on SOAP, shared styles are one cached stylesheet) |
| [BUGS-MANIFEST.md](BUGS-MANIFEST.md) | Every bug report in one list |
| [FORUM.md](FORUM.md) | The forum |
| [DISCORD.md](DISCORD.md), [DISCORD-PLAN.md](DISCORD-PLAN.md), [discord-initial-posts.md](discord-initial-posts.md), [discord-launch-posts.md](discord-launch-posts.md) | The Discord: bot, channel plan, post drafts (nothing is posted without the owner's OK) |
| [solo-announce-drafts.md](solo-announce-drafts.md) | Leyline Solo announcement drafts (not posted) |
| [FORKING.md](FORKING.md), [SUCCESSION.md](SUCCESSION.md), [BLANK-SLATE.md](BLANK-SLATE.md), [MOVEMENT.md](MOVEMENT.md) | Keeping Leyline going without us |
| [WORDS.md](WORDS.md) | Words we use |
| [REPO-MAP.md](REPO-MAP.md) | Where the code is |

## Plans (not built, or not all built)

| File | What it is |
|---|---|
| [ROADMAP.md](ROADMAP.md) | The master roadmap |
| [SEEDED-REALM-PLAN.md](SEEDED-REALM-PLAN.md), [SEEDED-REALM-ADDENDUM.md](SEEDED-REALM-ADDENDUM.md) | The seeded first-era realm |
| [SMARTER-BOTS-PLAN.md](SMARTER-BOTS-PLAN.md) | Smarter bots |
| [MOBILE-WEB-OVERHAUL.md](MOBILE-WEB-OVERHAUL.md) | Mobile web overhaul (6 Nov) |

## History

| File | What it is |
|---|---|
| [HANDOFF.md](HANDOFF.md) | **(history, superseded)** the 2 Nov session handoff; newer handoffs and reports are in the owner's project docs |
| [INCIDENT-2026-11-01-SLICE.md](INCIDENT-2026-11-01-SLICE.md) | **(history)** the 1 Nov streamed slice incident |
| [history/](history/) | **(history)** older records |

Folders: `den-ui/`, `host-ui/` and `screens/` hold screenshots; `growth/`, `posts/` and `scout/` hold drafts and
research; `realm-guide/` feeds the Wrath Unleashed notes page; `security/` and `smoke/` hold check records.

TODO(owner): the Host pack players download (`stream/hostpack.json`) was built on 3 Nov; `hosting/kit` has changed
since (5 Nov, 9 Nov, Commonhold v1). Rebuild it with `hosting/pack/make-hostpack.py` and ship `hostpack.json` when you
want hosts to get the newer kit (audit 12 Nov, finding 13; not rebuilt by us).
