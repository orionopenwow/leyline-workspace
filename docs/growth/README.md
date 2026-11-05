# Growth kit: reels, guides and posts for the grand release

Written 4 Nov 2026 for the owner's ask (4 Nov): reels showing features in Leyline's style, a launch video updated with
every feature, flow videos (hosting a server, opening the Den, opening any client), a Reddit guide series, encouraging
posts, and getting players and server hosts.

**Nothing here is posted.** Every post is a draft. Lines in the owner's own voice (titles meant to sell, intros,
taglines, announcements) are `TODO(owner)` with a draft next to them. Links in posts go only to our own site
(leylinelauncher.com), the Discord and Reddit. No money links. No game or publisher names in anything players see, and
no version numbers in sentences.

| File | What |
|---|---|
| [reddit-guides.md](reddit-guides.md) | The Reddit guide series: 12 episodes, each with a title, 5 steps, a "Stuck?" section and the GIF it uses |
| [reddit-lessons.md](reddit-lessons.md) | Reddit lessons: 10 short posts that each teach one thing (hosting, 24/7 at home, invites, nodes, modding, the Den, agents, scaling, the browser, boss fights), with the sub to try, a call to action and what to check before posting |
| [weekly-posts.md](weekly-posts.md) | Weekly post templates: patch notes, feature reel, guide, Den or host spotlight, state of Leyline; the week's rhythm |
| [discord-onboarding.md](discord-onboarding.md) | Discord onboarding, roles and channels, on top of the launch layout in [../DISCORD-PLAN.md](../DISCORD-PLAN.md) |
| [host-recruiting.md](host-recruiting.md) | The host recruiting page: copy for a "Host with us" page on the site and a pinned post |
| [seeded-season.md](seeded-season.md) | The seeded season announcement draft (the plan is [../SEEDED-REALM-PLAN.md](../SEEDED-REALM-PLAN.md)) |

## The reel kit (tools/reels)

One kit makes every video in Leyline's look: the launcher's colours (its Dark tokens in `launcher/ui/style.css`) and
the site's cyan, the display face both use (Big Shoulders Display) and Inter, the starfield backdrop, the swirl mark.

| Piece | What it is |
|---|---|
| A 1 s intro sting | The mark turns in and LEYLINE rises under it |
| A title card | Kicker, title and one line, over the starfield with a blue and a purple wash |
| Lower-third captions | The launcher's panel with a blue-to-purple bar; they fade in and out with each shot |
| An end card | The mark, LEYLINE, **leylinelauncher.com** in the site's cyan, one line, the legal line |
| A DRAFT tag | On every frame of a video whose script still has `TODO(owner)` words |

Each video comes in two shapes: **9:16 reels** (1080x1920: Shorts, Reels, TikTok, a phone-first Reddit post) and
**16:9 videos** (1920x1080: the launch video, the flow videos, YouTube, the site). Every video also gets an `.srt` of its
captions, a contact sheet (a frame every 2 s, to look a cut over without playing it) and a **GIF for Reddit**.

### The scripts

`tools/reels/scripts/` holds one small JSON file per video and `takes.json`, the list of raw takes. A script names its
shots: a take, where to cut in and out (seconds, or a named moment in the take such as `["carry", -1]`), a speed, and
the caption. `launch-v2.json` is cut from the other reels' shots, so running the edit again after a new reel or a new
take brings the launch video up to date by itself.

| Script | Shape | What it shows |
|---|---|---|
| login-to-playing | reel + wide | Log in with a Hero Key, pick a realm, PLAY, logged in by itself, in the world |
| den-carry | reel + wide | Your Den in one click; a piece from the gallery, carried, set down, turned |
| den-fly | reel + wide | Free flight over your Den, snap out, zoom far out and back |
| realm-switch | reel + wide | A community realm and back to the official one, nothing to change by hand |
| single-player | reel + wide | Single player from the side menu, Start, companions and dungeon buddies |
| mods-hd | reel + wide | Mods & Addons, the HD pack by Project Reforged, before and after |
| phone | reel | The website on a phone: Start here, Play in your browser |
| queue | reel + wide | The browser game's line: your place kept, or skip it (never the seat numbers) |
| flow-host | wide + reel | Host a realm: template, name, Start, listed for everyone |
| flow-den | wide + reel | Open your Den: the realm picker, Play, logged in, building |
| flow-any-client | wide + reel | Add the game folder you have, PLAY, into the world |
| launch-v2 | wide + reel | The launch video, cut from all of the above |

### Making them

1. **Record the takes.** In Leyline Control, queue **reel-record** (about 10 minutes; leave the mouse alone). The
   launcher's own tests start your game: PLAY on Wrath Unleashed into the world, Your Den opened, a piece carried,
   the free flight. Your Den is saved first and put back after. The launcher's pages and the website's are recorded
   too when this PC has node and Playwright; the cloud can make those as well (`node tools/reels/takes.cjs`).
2. **Record the hand takes** (optional): queue **reel-record-hand**. A box says what to play (a community realm and
   back, Single player with bots, the HD pack off and on) and for how long. OK records, Cancel skips.
3. **Cut them.** Queue **reel-make**. The videos land in `tools\reels\out\<reel>\`. A reel whose takes aren't all
   recorded yet is cut from what there is; `py -3 tools\reels\edit.py --list` says which takes each one still needs.
4. **Look them over.** Open each `-sheet.jpg` first. If a cut lands a little early or late, nudge the numbers in its
   script (`"in": ["carry", -1]` means one second before the carry) and run reel-make again.
5. **Write your words.** Replace each `TODO(owner): ...` in the scripts with your own line. When none are left, the
   DRAFT tag goes (`edit.py --final` refuses a script that still has one).

Takes and videos stay on the PC (`tools/reels/takes/` and `tools/reels/out/` are not in git).

### Making a new reel

Copy a script, give it a new `id`, list its shots. A new take goes in `takes.json`: a launcher take is a scene in
`tools/reels/takes.cjs`; a game take is a flow in `tools/reels/record.py`; anything else can be a hand take (a prompt
and a length). `python3 -I tools/reels/edit.py --check` checks every script against the rules for words on screen.

## The rhythm (a suggestion)

TODO(owner): pick the days that suit you. A draft:

| Day | What | Where |
|---|---|---|
| Monday | A guide episode (reddit-guides.md) with its GIF | Reddit, then #guides-and-faq |
| Wednesday | A feature reel | Reddit (the GIF), Shorts/Reels (the 9:16), #showcase |
| Friday | Den or host spotlight | #showcase or #host-a-realm, then Reddit |
| Every second Sunday | Patch notes, or State of Leyline once a month | #announcements, Reddit |

## Getting players and hosts

- **Players** come for one thing they can do today: the guides are short and each ends in one action.
- **Hosts** are the network. The host recruiting page and the flow-host video say what hosting gives them: their
  own realm listed for everyone, the launcher doing the setup, the Realm Host role on the Discord.
- **Encouraging posts**: the spotlight template celebrates a player's Den or a host's realm by name (with their OK).
  TODO(owner): whether a small "Den of the week" pick is yours each week or the community's vote in #showcase.
