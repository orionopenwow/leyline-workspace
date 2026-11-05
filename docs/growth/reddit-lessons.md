# Reddit lessons (drafts)

Written 5 Nov 2026 for the owner's ask (5 Nov): "bank a few reddit posts short lessons on starting a server, hosting
24/7 in there for example, modding, using llm agents to create servers etc".

This is a second set beside the twelve-episode guide series ([reddit-guides.md](reddit-guides.md)). A guide is
"press these five buttons". A **lesson** teaches one thing a reader can use even without Leyline: what hosting at
home really takes, what the numbers are, what agents are good and bad at. Each one is honest about limits and cost.

**Nothing here is posted.** Every post is a draft, and only the owner posts. Each opening line is `TODO(owner)`, in
his own voice, with a draft beside it. The rules for every post:

- No game or publisher names. Say "the game", "Wrath", "classic realms", "the Wrath client". The legal line at the
  end of each body is the only exception.
- No version numbers in the text.
- "Log in", never the other phrasing.
- No money links of any kind.
- Links go only to leylinelauncher.com, github.com/orionopenwow/Leyline and the Discord (https://discord.gg/RfChVr8J7T).
- Players always use their own game folder. Nothing here asks anyone to host, send or share game files, or to use a
  client from anywhere else.
- No advice about the publisher's own live games. If a thread asks about one, the answer is "this is not that".

`tools/reels/test_reels.py` (GrowthDocs) checks these rules on this file.

**Subreddits.** Every sub has its own rules on self-promotion, and they change. Read the sidebar and the pinned
posts the day you post. Some subs allow a project post only on one weekday, under a flair, or in a weekly thread. Post
one lesson per sub per week at most. Stay in the comments for the first hour.

A post's shape:

```
<title>

TODO(owner): <one line, your voice>

<the lesson: what you need, the steps, what it costs, the limits>

<one link to the full guide> · <the Discord>

Not affiliated with or endorsed by Blizzard Entertainment.
```

---

## 1. Your own realm from one page

- **Subreddit:** r/wowservers (read its rules on server ads first: high risk if it reads as an ad, so lead with the
  how-to); also the launcher's own sub if there is one.
- **Title:** Lesson: start your own realm in about ten minutes of clicks (the first setup then runs by itself)
- **Body:**

```text
TODO(owner): your opening line. Draft: "I wanted hosting to be one page, not a weekend of config files. This is where it got to."

What you need:
- A PC that stays on while people play (Windows, macOS or Linux), with 8 GB of memory. The realm uses about 4 GB.
- At least 10 GB of free disk.
- Docker Desktop (free for personal use), or Docker Engine on Linux.
- Your own game folder for the Wrath client. The realm reads the map data it needs from it, on your PC. Nothing is downloaded from us.

The steps:
1. Get the Leyline launcher from leylinelauncher.com and log in.
2. Open Host. The top line says if this PC is ready. If it isn't, there is one button: Get Docker, Start Docker or Get the Host pack.
3. Pick Private (friends with a link) or Public (on the Community realms list).
4. Pick a template: Vanilla+, Classic levelling, Solo with bots, PvP and duel ladder, and more. Every slider is a real server setting.
5. Name it, type your account name (it gets GM) and press Host.

Honest timing: the clicks take a few minutes. The first start then takes 5 to 20 minutes while Docker sets up the database, the login server and the world server. Later starts take a minute or two. Installing Docker comes on top.

Limits: the realm is up only while your PC is on. Players reach you through the Leyline relay, so you open no ports. Each realm is capped at 128 connections (about 64 players) and 512 KB/s through the relay.

Cost: the software is free. You pay for the power your PC uses.

Full guide: https://leylinelauncher.com/wrath/docs/host-a-realm · Discord: https://discord.gg/RfChVr8J7T

Not affiliated with or endorsed by Blizzard Entertainment.
```

- **Call to action:** Open the launcher, press Host, and show us your realm in #host-a-realm.
- **Check before posting:**
  - The Host page's words: "Private or Public", "Pick a template", "Host it", the Host button, and the three fixing
    buttons (launcher/ui/hostflow.js).
  - The template names still match the Host page (docs/leyline/host-a-realm.md).
  - The first-start time (5 to 20 minutes) and the memory and disk needs.
  - The relay caps per realm (hosting/relay/wurelay/relay.py `RELAY_MAX_STREAMS`, `RELAY_KBPS`).
  - Docker Desktop's free-use terms haven't changed.

## 2. Hosting 24/7 on a home PC

- **Subreddit:** r/homelab or r/selfhosted (both have self-promotion rules and dislike thin ads: keep it a real
  how-to; r/selfhosted may want a project flair or a set weekday). Also r/wowservers.
- **Title:** Lesson: what it actually takes to keep a game realm up 24/7 on a PC at home
- **Body:**

```text
TODO(owner): your opening line. Draft: "Our first community realms kept going dark at 2 a.m. It was nearly always sleep or a restart."

What keeps a home realm up:

1. Turn sleep off. A sleeping PC is an offline realm. The screen can still switch off.
2. Start Docker with the PC. Docker Desktop starts when you log on, so after a restart someone logs on, or the PC logs on by itself (only if it's safe at home). Linux: sudo systemctl enable docker.
3. Every part of the realm restarts by itself if it stops. The Vanilla+ ruleset starts "always on".
4. Set Windows Update's active hours to your players' evening, so it doesn't restart then. Don't turn updates off.
5. In the BIOS, set "Restore on AC power loss" to Power On. Characters are saved as people play; a power cut can lose the last few seconds.

No ports: the realm makes one outbound connection to the Leyline relay. Players connect to the relay, so nobody sees your home address. It reconnects by itself.

Backups: stop the realm, copy the Host pack folder. Its state folder is the realm's identity: keep it private. Characters live in Docker's storage for the database, so back that volume up too (docker volume ls finds it).

Power: a mini PC uses about 10 to 20 W, a mostly idle desktop 30 to 100 W. Watts x 720 / 1000 = kWh a month: 50 W is about 36 kWh. Multiply by the price on your power bill.

Limits: home power and internet are the weak points. If they drop often, a small rented Linux server (4 GB, 2 cores) runs the same Host pack.

Full guide: https://leylinelauncher.com/wrath/docs/always-on · Discord: https://discord.gg/RfChVr8J7T

Not affiliated with or endorsed by Blizzard Entertainment.
```

- **Call to action:** Follow the always-on guide once, then tell #host-a-realm how long your realm stayed up.
- **Check before posting:**
  - docs/leyline/always-on.md: the steps, the power numbers, the rented-server size.
  - Whether the Host pack has a one-button character backup by now. If it does, replace the volume line with it.
  - The database volume's real name on a fresh host (`docker volume ls`), so the comment answers are right.
  - That Vanilla+ still starts always on (realm/profiles/vanillaplus.env, the kit's `--always-on`).
  - Don't post the opening line if it isn't true: it's a draft.

## 3. Friends-only or listed: picking how people find your realm

- **Subreddit:** r/wowservers; or a small-group gaming sub (check its rules).
- **Title:** Lesson: friends-only or listed for everyone? How invite links and public listings work on a home realm
- **Body:**

```text
TODO(owner): your opening line. Draft: "Most new hosts should start private. Here's why, and how to go public when you're ready."

Private (friends only):
- You get an invite link. Send it to each friend. They open it once on the PC they play from and make their own account there. Leyline never makes accounts and never sees passwords.
- The secret part of the link sits after the #, so it never lands in a web server's log. The relay keeps only a hash of it.
- A friend's game gets in from the internet address that opened the link, for 24 hours. Friends behind the same mobile carrier address share that; their passwords still protect each account.
- New link stops the old one working. Use it when a link ends up somewhere you didn't mean.

Public (listed):
- Your realm is on the Community realms list (launcher, site and in the game) about a minute after it starts. It sends a signed heartbeat every 60 seconds.
- Anyone on the list can make an account. Write house rules: they are shown before anyone joins.
- The list is ordered by players online, uptime and a small boost while you're new. There is no paid placement.
- Claim it with your Hero Key so it shows as yours.

The lesson: start private with three friends, fix what breaks, then switch with Realm tools, Public or private. The console has kick, ban, an allow list and a health check for the day a stranger misbehaves.

Limits: neither way changes the relay's caps (about 64 players a realm).

Full guide: https://leylinelauncher.com/wrath/docs/hosting · Discord: https://discord.gg/RfChVr8J7T

Not affiliated with or endorsed by Blizzard Entertainment.
```

- **Call to action:** Start private, invite three friends, and go public when it feels solid.
- **Check before posting:**
  - docs/leyline/hosting.md "Private and public": the 24-hour rule, the hash, New link.
  - docs/growth/host-recruiting.md: the listing order and "no paid placement" are still true.
  - "Copy invite link" and Realm tools' "Public or private" are still the names (launcher/ui/hostflow.js).
  - The claim flow (docs/leyline/always-on.md "Claim it for your account").

## 4. Be a node: help realms without playing

- **Subreddit:** r/selfhosted or r/homelab (people who already leave a box on). Medium risk under their self-promotion rules:
  it asks something of the reader, so be plain about what it does and doesn't do. **Post only once Be a node is open.**
- **Title:** Lesson: what a home PC with no open ports can (and can't) do for a game network
- **Body:**

```text
TODO(owner): your opening line. Draft: "People asked how to help without hosting a realm. This is the honest answer."

A PC behind a home router takes no incoming connections unless you open them. That decides what it can do.

What the Be a node switch does (Leyline, Host page, top card):
- Your PC shares Leyline's own files over BitTorrent: the launcher's installers, the Host pack, the Leyline addons and the server software's source. Each file is checked against Leyline's signed list first. It never shares game files or a game client.
- It needs no open ports and never shows your address.
- Limits you set: upload in KB/s, connections at most, MB per day (then it rests until tomorrow). "Only while this PC is idle and plugged in" is on by default. It keeps up to 3 GB of files.

What it doesn't do yet: players' launchers don't download from nodes yet, so today it adds copies on many PCs (if a mirror goes down, the files are still out there) but doesn't yet cut our server's load. That comes when the download side ships.

Carrying players is the part that really moves bytes: each player streams tens of KB/s for hours. That's the Advanced full relay. It needs Docker, a DNS name pointing at your PC, and open ports (TCP 80, 443 and 41000 to 41051). Your PC then sees players' game traffic, chat included, but never passwords. A community relay starts with modest caps: about 25 realms, 256 KB/s and 5 GB a day for each.

Cost: your upload and a little power. Nothing else.

Full guide: https://leylinelauncher.com/wrath/docs/be-a-node · Discord: https://discord.gg/RfChVr8J7T

Not affiliated with or endorsed by Blizzard Entertainment.
```

- **Call to action:** Turn on Be a node on the Host page, set your limits, and say hi in #host-a-realm.
- **Check before posting:**
  - That Be a node is open (the site flag `be_a_node`). Until then the card says "Not open yet": don't post.
  - Whether the download side (phase 2) has shipped. If it has, rewrite "What it doesn't do yet".
  - docs/BE-A-NODE.md and docs/leyline/be-a-node.md: limits, the 3 GB cap, the ports.
  - The community relay defaults (relay.py `VOLUNTEER_DEFAULTS`).

## 5. Modding without breaking anything

- **Subreddit:** r/wowservers, or an addon or modding sub (check that sub's rules on project posts).
- **Title:** Lesson: four layers of modding a realm, from safest to riskiest, and how to undo each
- **Body:**

```text
TODO(owner): your opening line. Draft: "Every mod should come off as easily as it went on. This is how we set it up."

1. Rulesets and settings (safest). On your own realm, the Host page's templates are real server settings: rates, levels, bots, PvP, events. Change one and it says Custom. Save as my template keeps it; Share as a code gives a friend your settings (settings only, never names, links or keys). Save and restart applies changes; anyone playing is dropped for about a minute.

2. Lua game modes, with hot reload. In the game, /modes edit opens a script editor. A mode is one Lua script that runs in a sandbox on the host's client: no chat, no network, a 30 ms budget per call. Save while a test runs and the new script takes over in place, scores kept. If it has an error, the old one keeps running.

3. Addons and looks. The launcher's Mods & Addons installs into the game folder picked in the play bar. Installed lists everything; turn anything off there. The game's own patch files are never touched.

4. Realm mods (riskiest). A Workshop mod's realm half is Lua that runs on a realm. On the official realm the team checks each one, backs up, and does a live reload: a Lua error puts the old scripts back, and nobody is kicked.

And a sandbox for all of it: Your Den is a small world on your own PC, with no realm and no Docker. Break things there first.

Limits: a mod made for another version of the game isn't offered for that folder.

Full guide: https://leylinelauncher.com/wrath/docs/mods · Discord: https://discord.gg/RfChVr8J7T

Not affiliated with or endorsed by Blizzard Entertainment.
```

- **Call to action:** In the game, open /modes edit, change one line, press Save mid-test, and watch it swap.
- **Check before posting:**
  - docs/CREATOR-API.md: the sandbox, the 30 ms budget, hot reload.
  - docs/leyline/publishing.md: the realm half, the live reload and the rollback.
  - docs/leyline/launcher.md: where files go ("the game's own patch files are never touched").
  - Whether hosts can live-reload their own realm's scripts from the console yet. If they can, add it to layer 4.
  - Mods & Addons and Installed are still the names in the launcher.

## 6. Building spaces in the Den and sharing stamps

- **Subreddit:** a building or cozy-games sub, or r/wowservers (check self-promo rules; a GIF of a build does best).
- **Title:** Lesson: how to lay out a small place that looks lived in (a camp, an inn, a market) in your own Den
- **Body:**

```text
TODO(owner): your opening line. Draft: "We studied how the game's own camps and inns are laid out. The rules are simple."

Your Den is a small world on your PC (Den Lite, built into Leyline: no Docker, no realm). Pick Your Den in the launcher's realm picker and press Play.

The layout rules we build our stamps by:
1. One focal point: the fire, the bar, the forge. Nothing covers it.
2. Things face what they're for: seats face the fire, stalls open onto the street.
3. Small things in clusters of 3 to 5, by an anchor: crates by a wall, food on a table.
4. Big to small: building, then tables and benches, then bottles and candles.
5. A little jitter on what people move (chairs, crates), none on what is built.
6. Keep the path from the way in to the focal point clear.

The tools: Build (F7) opens the gallery. A piece follows your mouse; click to place. The Tools window has a stamp library by category (Camps, Markets, Gardens and more). Turn in 15 degree steps, mirror, or size from a quarter to three times, then Enter to keep it. Undo goes back 200 steps.

Sharing: box-select what you made and save it as a stamp, with your name and the date as its credit. Invite a friend into your Den with a link (24 hours, or a week with Keep), and Let build to build together. Your friend needs Leyline and their own game folder; game files never travel.

Coming: Shared Dens, where builds and stamps are shared with everyone after a moderator check.

Full guide: https://leylinelauncher.com/wrath/docs/den · Discord: https://discord.gg/RfChVr8J7T

Not affiliated with or endorsed by Blizzard Entertainment.
```

- **Call to action:** Build one camp by the six rules and post a screenshot in #showcase.
- **Check before posting:**
  - docs/DEN-STAMPS.md (the rules), docs/DEN-TOOLS.md (Tools window, turn, mirror, size, 200 undo steps).
  - docs/DEN-HOSTING.md: invite times, Keep, Let build.
  - Whether Shared Dens is open (`WU_DENS` = 1). If it is, change "Coming" to how to share and add the moderator
    check and the licences (docs/DEN-MARKETPLACE.md).
  - The GIF: `den-carry.gif` from the reel kit.

## 7. Using LLM agents to build and run a server

- **Subreddit:** r/ClaudeAI or r/ChatGPTCoding (on topic; check their rules on project posts). r/LocalLLaMA is about running models locally and we use a
  cloud model, so it is high risk there; post there only if a thread asks. r/selfhosted only under its AI flair, if
  it has one.
- **Title:** Lesson: what AI coding agents were good and bad at while building a game server network (no hype)
- **Body:**

```text
TODO(owner): your opening line. Draft: "Most of Leyline's code was written by Claude agents. I still decide what ships. This is how that works."

How we work:
- One main session plans. Bigger jobs go to background agents, each in its own git worktree and branch, with a short brief: the rules, the files, the tests to run, "commit, don't push".
- Every change has tests: Python suites for the website, browser tests for the launcher, unit tests for the core. Agents run them before anything is merged.
- A small control panel on my PC runs named steps from a queue: deploy the site, ship to GitHub, run an audit. Deploys need my OK. Each has a backup, a self-check and a rollback.
- Agents never post anywhere, never make accounts or type passwords, never print secrets, and never approve a mod for players. I do those.

Good at: wide, boring changes (one wording rule across hundreds of pages), writing tests, reading logs, keeping docs and code in step.

Bad at: knowing what's fun. Anything they can't test: a real game client on a real network still needs a person. They can sound sure when they're wrong, so we check results, not reports. Parallel agents can step on each other: once a worktree was removed while an agent was still using it.

TODO(owner): one or two lines of your own view on working this way.

In the launcher, this is opt-in: the Agents page is off until you turn it on. An assistant can look at your realm's status and logs; installing a mod or starting your realm needs your Approve; accounts, money and bans have no agent button at all.

Full guide: https://leylinelauncher.com/wrath/docs/agents · Discord: https://discord.gg/RfChVr8J7T

Not affiliated with or endorsed by Blizzard Entertainment.
```

- **Call to action:** Ask us anything about the workflow in the comments; the launcher's Agents page is in the guide.
- **Check before posting:**
  - The opening is only true if the owner says so: it is his line to write or cut.
  - docs/leyline/agents.md: what an agent can see, what needs Approve, the "Never" list.
  - The control panel facts (backup, self-check, rollback, owner OK) still hold.
  - Name the model and products only as they are on the day (Claude, Claude Code); no claims about other tools.
  - r/LocalLLaMA's and r/ClaudeAI's current rules on project posts.

## 8. Scaling a home realm: numbers before promises

- **Subreddit:** r/homelab or r/selfhosted (they like real numbers; read their self-promotion rules); r/wowservers.
- **Title:** Lesson: how many players can a home realm take? Our estimates, and why you load test first
- **Body:**

```text
TODO(owner): your opening line. Draft: "Every host asks how many players their PC can take. These are our estimates, not promises."

Memory: the realm (database, login server, world server) uses about 4 GB. 8 GB in the PC is enough for the realm alone; 16 GB if you still use the PC. Bots cost extra: each roaming bot uses some CPU and memory, so add them slowly.

Upload: each player sends and receives roughly 5 to 10 KB/s. Twenty players is about 100 to 200 KB/s through your connection, about 0.8 to 1.6 Mbit/s. Your home upload is usually the first wall, not the CPU.

The relay: each realm is capped at 128 connections (a player uses 2, so about 64 players) and 512 KB/s, which shapes a realm at roughly 50 busy players. A player limit on the Host page puts the rest in a queue.

What we have and haven't proven: our load harness ran 50 headless players through an in-process relay with no errors. That tests our code, not a real network. The one-hour run with 50 players over the internet hasn't been done yet. Until it has, treat every number here as an estimate.

How to test your own: start with friends. Watch the Host console and docker stats while they play. Raise the player limit step by step. Add bots last.

Bigger than about 50 at once means raising relay caps or running your own relay, plus a real load test first.

Full guide: https://leylinelauncher.com/wrath/docs/hosting · Discord: https://discord.gg/RfChVr8J7T

Not affiliated with or endorsed by Blizzard Entertainment.
```

- **Call to action:** Tell us your PC, your upload and how many played at once, so the next host gets real numbers.
- **Check before posting:**
  - Whether the one-hour soak (hosting/tests/soak/SOAK-PLAN.md) has run since. If it has, use its numbers instead.
  - The per-player traffic estimate (hosting/INSTALL-OPERATE.md, "Going live").
  - The relay caps per realm and the "about 50" shaping note (INSTALL-OPERATE.md troubleshooting table).
  - The memory numbers (docs/leyline/always-on.md).
  - Never round any of this up in the comments.

## 9. Playing in the browser and on phones

- **Subreddit:** r/webgpu or r/webdev for the tech side; r/wowservers for players. Low to medium risk under their rules:
  show it, don't sell it.
- **Title:** Lesson: playing an old game client in a browser tab with WebGPU, streaming only what you need
- **Body:**

```text
TODO(owner): your opening line. Draft: "No download, no game files: open a tab and you're in Dalaran."

How it works: the browser streams a small part of the world as you go: Dalaran, the capitals, the starting areas, arenas, dungeons and raids. Your character stays inside that part while you play this way.

What you need: a browser with WebGPU, such as a recent Chrome or Edge on a computer. Without it the page stays blank. That's the honest limit.

The steps:
1. Open leylinelauncher.com and press Play now.
2. Log in with a Hero Key or Google (or make a Hero Key in one click).
3. A new character picks once: level 1, or a one-time boost to 80.

Have your own game folder? Drag it onto the game page to go anywhere in the world. Nothing is uploaded: the tab reads only the parts it needs from your disk. Chrome and Edge remember the folder; Firefox and Safari ask each time.

On a phone or tablet: touch controls switch on by themselves. The stick bottom-left moves you, the buttons on the right attack and cast. Turn the phone sideways. It still needs WebGPU in the phone's browser, so not every phone works yet.

Busy evenings: there's a line. You keep your place, or skip it by playing on your own PC.

Before playing the same character with your own game, press End my browser pass on the account page.

Full guide: https://leylinelauncher.com/wrath/docs/play · Discord: https://discord.gg/RfChVr8J7T

Not affiliated with or endorsed by Blizzard Entertainment.
```

- **Call to action:** Open leylinelauncher.com on your phone, press Play now, and tell us which phone worked.
- **Check before posting:**
  - Which phone browsers have WebGPU on the day (Android Chrome, iOS Safari): say what we tested, nothing more.
  - docs/leyline/play.md: what is streamed, the folder rules, End my browser pass.
  - stream/site_pages.py "browser-and-phone": the touch control words.
  - Whether there is a Leyline phone app yet. There isn't on 5 Nov: don't mention one until it ships.
  - The GIF: `phone.gif` (and `queue.gif`).

## 10. Boss fights and leaderboards (when live)

**Post this only once boss fights are live on leylinelauncher.com.**

- **Subreddit:** r/wowservers; a speedrun-minded sub if one fits (check its rules).
- **Title:** Lesson: an instant boss fight in the browser, and what makes a clear-time board fair
- **Body:**

```text
TODO(owner): your opening line. Draft: "Straight to the boss, no trash, no travel. Then see how your time stacks up."

What it is: The Frozen Throne, the last fight of the citadel, one click from the website. Pick Normal or Heroic, then:
- Fight now with companions: you and helper bots, right away.
- Queue with players: wait for real players. Bots fill the empty spots after a short wait.
- Players only: wait for a full group of real players.

How to play:
1. Open leylinelauncher.com/wrath/bossfights and log in.
2. Pick the difficulty and a button. You're taken into the fight.
3. The bar at the top shows a live timer and Leave. A wipe offers Try again.
In the game, /bf opens the same window (Esc, Boss fights; on a phone, Menu, Boss fights).

The board: the top 10 clear times, with groups of players and with companions kept apart, plus a weekly board. Times come from the realm, not from your client. Companions are counted, not named.

The lesson for anyone running a board: keep bot runs and player runs on separate boards, time on the server, and show only what you need (names, classes, the time).

TODO(owner): one line on what's next (more fights, harder tiers) once it's decided.

Full guide: https://leylinelauncher.com/wrath/bossfights · Discord: https://discord.gg/RfChVr8J7T

Not affiliated with or endorsed by Blizzard Entertainment.
```

- **Call to action:** Clear it once with companions, then queue with players and try to beat your time.
- **Check before posting:**
  - That /wrath/bossfights is open for everyone and the board fills from a real export (stream/bossfights.py).
  - The button words (docs/BOSS-FIGHTS.md): Go now with companions vs Fight now with companions; use what the page
    says on the day.
  - The weekly board exists on the page.
  - The boss is never named: only "The Frozen Throne".
  - Harder tiers are only a plan: keep them out until they are real.
