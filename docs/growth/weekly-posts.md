# Weekly post templates (drafts)

Five kinds of post, each with where it goes, what fills it in, and a template. **Nothing is posted.** The parts in
your voice are `TODO(owner)` (a draft is beside each); the rest is filled from facts: the commits, the bug manifest,
the realm list, the reel kit's videos.

The usual rules: no game or publisher names, no version numbers in sentences (a badge like "3.3.5a" in a table is
fine), "Log in" not "Sign in", links only to leylinelauncher.com, the Discord and Reddit, no money links. Name a player
or a host only with their OK.

| Post | When | Where | Fills it in |
|---|---|---|---|
| Patch notes | after a launcher or realm release | #announcements, then Reddit | the commits since the last notes; HQ's recent changes |
| Feature reel | one a week | Reddit (the GIF), Shorts/Reels/TikTok (the 9:16 MP4), #showcase | a reel from `tools/reels/out` |
| Guide | one a week | Reddit, then #guides-and-faq | an episode of [reddit-guides.md](reddit-guides.md) |
| Den or host spotlight | one a week | #showcase or #host-a-realm, then Reddit | a Den from #showcase, a realm from the Community list |
| State of Leyline | once a month | #announcements, Reddit | the numbers below, the bug manifest, the roadmap |

---

## 1. Patch notes

```
**Leyline update: <date>**

TODO(owner): one line on what this update is about. Draft: "Smoother logins and a better Den this week."

**New**
- <what a player can now do, in one line each> (e.g. "Single player: Start sets up your own realm on your PC")

**Better**
- <what works better> (e.g. "Switching between community realms and Wrath Unleashed needs nothing by hand")

**Fixed**
- <the bug, as players saw it> (from the bug manifest: what was reported, now fixed)

**Getting it:** the launcher updates itself; a **Restart to update** button shows when it's ready (it never restarts
on its own).

Report a bug: in the game type `.bug`, or https://leylinelauncher.com/wrath/bugs · Discord: #bug-reports
```

Notes: one line per change, what a player sees, never file or module names. The restart line follows the owner's
standing rule (3 Nov: the Restart-to-update button just appears and stays optional).

## 2. Feature reel

```
**<the reel's title>** (e.g. "Build your Den: carry, place, turn")

TODO(owner): one line. Draft for den-carry: "Pick a piece, carry it, set it down. Your Den, your rules."

[the GIF on Reddit / the 9:16 video on Shorts, Reels and TikTok]

How: <the guide episode's title and link> · Get Leyline: https://leylinelauncher.com/wrath/start
```

Which file: Reddit takes the `.gif` (or the 16:9 MP4 as a video post); Shorts, Reels and TikTok take
`<reel>-reel.mp4`; YouTube and the site take `<reel>-wide.mp4`. Each has an `.srt` for captions where the app allows
one. A video with a DRAFT tag isn't ready: the tag goes when its script has no `TODO(owner)` left (then run reel-make).

Suggested order: login-to-playing, den-carry, realm-switch, single-player, den-fly, mods-hd, flow-host, phone, queue.

## 3. Guide

Use the episode as written in [reddit-guides.md](reddit-guides.md). On the Discord, post it in #guides-and-faq with the
GIF and the full guide's link, then pin it there if it is one of the evergreen ones (1, 3, 5, 11).

## 4. Den or host spotlight

```
**Den of the week: <Den name> by <player>**   (or: **Realm of the week: <realm name>, hosted by <host>**)

TODO(owner): two lines on why it caught your eye. Draft: "Bramble built a whole harbour town, lanterns and all.
The boardwalk alone took a weekend."

[2-3 screenshots or a short clip the player sent]

Den: open the shared Den from the Workshop: <its Workshop page>   (with the player's OK)
Realm: <the realm card's line: rules, version badge, players>. Join it from the launcher's Realms > Community.

Want yours here? Post it in #showcase (Dens) or #host-a-realm (realms).
```

Encouraging, not a contest: thank them by name, say what is good about it, and how others can see it. Ask first;
leave out anyone who'd rather not be named. TODO(owner): whether a pick is yours each week or a vote in #showcase.

## 5. State of Leyline (monthly)

```
**State of Leyline: <month>**

TODO(owner): your opening, a few lines on the month. Draft: "A month since launch. Here's where we are, what we fixed,
and what's next."

**The network**
- <N> realms on the network (<official> official, <community> community)
- <N> players this month, <N> hosts, <N> community nodes carrying the relay
- <N> Dens built, <N> shared on the Workshop

**What we fixed:** <the month's top fixed reports, from the bug manifest>

**What's next:** <three lines from the roadmap: what is being built now>

**Thank you:** <hosts, node keepers, guides on the Discord, by name with their OK>

TODO(owner): your sign-off.
```

Where the numbers come from: the site's realm list and network numbers (the launcher's Realms page shows
"N playing on M realms"), the relay's node list, the Workshop, the bug manifest (Control step `bugs`). Never the
browser game's seat numbers (owner 4 Nov).
