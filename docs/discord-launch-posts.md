# The Leyline Discord at launch: pinned posts (drafts)

The pinned posts for the launch layout in [DISCORD-PLAN.md](DISCORD-PLAN.md) (13 channels, 3 Nov). The owner's Leyline
Control reads this file: Discord card > Preview shows, for each block below, whether its channel already has it pinned.

- **Nothing is posted unless you say so.** Tick **Post and pin** in the card, then Preview and Do it: the bot posts and
  pins each block that has no `TODO(owner)` line left, or edits its own pin when you changed the block here. A pin
  that already says the same is left alone, so a rerun posts nothing twice. Without the tick, Do it prints them for
  you to paste (hover the message > ... > **Pin Message**).
- **A block with `TODO(owner)` is yours.** The bot never posts it: write it in your own words, then paste and pin it.
- A block is matched to its pin by its **first line**: keep the first line when you edit a block, or the bot posts a
  new pin instead of editing the old one.
- `#name` turns into a link to that channel when the bot posts it. Each block is under Discord's 2000 characters.

Links are our own site and the Discord invite. Every page linked here exists on leylinelauncher.com today
(stream/docs.py PAGES, the site's routes). TODO(owner): if you start a subreddit, add it to #welcome (the only other
social link we use). TODO(owner): if you want a line of your own at the top of #welcome, add it to the block (the bot
then leaves the block to you) or post it separately.

The older, bigger plan's drafts are in [discord-initial-posts.md](discord-initial-posts.md).

---

## START

### #welcome (pinned)

```
**Welcome to Leyline** 👋

Leyline is a free, open-source launcher and realm network. Play on our official realms, build your own private world (Your Den), host a realm for your friends, and add mods in one click. You bring your own game files: we never host or share them.

**Start in 3 steps**
1. **Get the launcher:** https://leylinelauncher.com/wrath/download
2. **Log in** with a Hero Key (free, made in a minute) or Google.
3. **Add your game folder** and press **PLAY**. No game files? Play in your browser: https://leylinelauncher.com/wrath

The four ways in, one click each: https://leylinelauncher.com/wrath/start

**Where to go**
- Guides and common questions: #guides-and-faq
- News: #announcements
- Chat: #general · Stuck? #help · Find a group: #looking-for-group
- Mods: #mods-and-workshop · Hosting: #host-a-realm · Show it off: #showcase
- Found a bug? #bug-reports · Ideas: #ideas

**The rules**
1. Be decent. No harassment, hate or personal attacks.
2. No game files: no client downloads, repacks or links to them.
3. Nobody here will ever ask for your password or your Hero Key. If someone does, it isn't us: tell a Moderator.
4. No selling, no ads, no spam.
5. No cheats or exploits. Found one? Tell a Moderator privately.
6. Keep it in the right channel.

One ban covers your whole Leyline account: the game, the website, the forum and this Discord.

Invite a friend: https://discord.gg/RfChVr8J7T
```

### #guides-and-faq (pinned: the guides)

```
**Guides** 📖

- Getting started: https://leylinelauncher.com/wrath/docs/getting-started
- Your Hero Key (log in, a lost key, your name): https://leylinelauncher.com/wrath/docs/hero-key
- Play (in the browser, your own game folder, community realms): https://leylinelauncher.com/wrath/docs/play
- Your Den: https://leylinelauncher.com/wrath/docs/den
- Invite a friend to your Den: https://leylinelauncher.com/wrath/docs/den-invite
- Host a realm: https://leylinelauncher.com/wrath/docs/hosting
- Mods: https://leylinelauncher.com/wrath/mods
- Make and publish a mod: https://leylinelauncher.com/wrath/docs/publishing
- Game modes: https://leylinelauncher.com/wrath/modes
- The launcher: https://leylinelauncher.com/wrath/docs/launcher
- Fixes for common problems: https://leylinelauncher.com/wrath/docs/getting-started#common-fixes
- Every guide: https://leylinelauncher.com/wrath/docs

Still stuck? Ask in #help.
```

### #guides-and-faq (pinned: the FAQ)

```
**FAQ** ❓

**Is it free?**
Yes. The launcher and the realms are free, and the code is open.

**Do I need the game?**
For your own client, yes: Leyline uses game files you already have and never downloads them for you. Without them you can still play in your browser: https://leylinelauncher.com/wrath

**Where are my game files?**
The folder that holds the game's program and a Data folder. Where to look: https://leylinelauncher.com/wrath/docs/getting-started#where-your-files-usually-are

**I lost my Hero Key.**
While you are logged in, make a new one at https://leylinelauncher.com/wrath/key (the old one stops working at once). Can't log in at all? Ask in #help.

**The game can't connect.**
Check the status dot in the launcher's title bar. If the realm is up, delete the Cache folder next to the game and start it again.

**Can I play with friends?**
Yes: on an official realm, on a community realm, or in your Den with an invite link: https://leylinelauncher.com/wrath/docs/den-invite

**Can I host my own realm?**
Yes, from the launcher's Host page. You need Docker, your own game folder and about 10 GB free: https://leylinelauncher.com/wrath/docs/hosting

**Found a bug?**
Post it in #bug-reports, type .bug in the game, or use https://leylinelauncher.com/wrath/bugs
```

### #announcements (your launch post)

```
TODO(owner): your launch post, in your own words. A shape you could use:

**Leyline is live** 🎉
TODO(owner): a line or two on what Leyline is and why you made it.

**Get started:** read #welcome and #guides-and-faq, then get the launcher: https://leylinelauncher.com/wrath/download
**What's in it:** TODO(owner): your pick of the official realms, Your Den, hosting, mods and game modes.
**Thanks:** TODO(owner): who you want to thank.

Forum announcements show up here too, so you can follow the news from either place.
```

---

## PLAY

### #help (pinned)

```
**Asking for help** 🛟

Say:
1. What you were doing, and what happened (the exact message helps)
2. The launcher or the browser, and which realm
3. A screenshot, if you can

The common fixes are in #guides-and-faq: have a look there first.

Never post your password, your email or your Hero Key. The team will never ask for them.
```

---

## CREATE

### #mods-and-workshop (pinned)

```
**Mods, addons and looks** 🧩

- Browse and install them in the launcher, or on https://leylinelauncher.com/wrath/mods
- Made something? Type **/submit** here with its link: it goes to the team's review queue, and only you see the answer. Or use https://leylinelauncher.com/wrath/submit
- Making a mod: https://leylinelauncher.com/wrath/docs/publishing

The team looks at every new mod before it is listed. Links to your own work or its official page only, please, and no game files.
```

### #host-a-realm (pinned)

```
**Host a realm** 🏰

Host your own realm from your PC: open the launcher's **Host** page and follow the steps. You need Docker, your own game folder and about 10 GB free. The first start takes 5 to 20 minutes, and your PC stays on while people play.

- The guide: https://leylinelauncher.com/wrath/docs/hosting
- Keep your realm up 24/7: https://leylinelauncher.com/wrath/docs/always-on
- Every realm: https://leylinelauncher.com/wrath/realms

Tell people about your realm here: its name, its rules and when it is up.
```

---

## FEEDBACK

### #bug-reports (pinned)

```
**How to report a bug** 🐞

One message per bug. Say:
1. **What you did** (the steps)
2. **What you expected**
3. **What happened instead**
4. **Where**: the realm and zone, the launcher page, or the website page
A screenshot helps.

**Every message here goes on the team's bug list**, together with reports from the game and the website, so the same bug reported twice is one item. Replying to a report adds yours to it.

Other ways:
- In the game: **.bug** and what happened, e.g. **.bug the quest giver in Northshire has no ! over his head** (it saves where you are)
- On the website: https://leylinelauncher.com/wrath/bugs

Never post passwords, your email or your Hero Key. Found an exploit? Tell a Moderator privately instead.
```

### #ideas (pinned)

```
**Ideas** 💡

One idea per message: what you would like, and why. React to the ones you want, and talk them over in a thread.
```
