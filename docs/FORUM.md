# The Leyline forum

Owner, 2026-10-31: "Decide how we are handling backend with Forum etc and ease of access 1 click and making sure forum
moderation is in my ultimate admin control panel."

## The decision: keep the built-in forum

The forum stays our own: `stream/forum.py`, a small SQLite store inside the portal, using the same sign-in as the
rest of Leyline. It's not a separate forum program (Discourse, Flarum, NodeBB, phpBB).

Why:

- **One sign-in.** The forum uses the website's own session (`wr_session`: Hero Key, Google, Discord, guest accounts)
  and the launcher's own sign-in token. Players never make or link a second account, and nobody gets a "connect your
  forum account" step. A separate forum would need SSO glue (DiscourseConnect or OAuth) plus an account in its own
  database for every player.
- **Nothing else to run or secure.** No second web app, database, mail server, background workers or admin panel to
  patch, back up and watch. The forum is one Python file and one SQLite file in `/data`, it ships with the portal
  (the `signin` step) and is covered by the portal's tests. A Discourse install alone wants 2 GB of RAM and its own
  container stack. That's a lot for a small community, on the same box as the realm.
- **Moderation in HQ.** Reports, hiding, locking, pinning, merging, a poster's history, muting from the forum and
  account bans all live in Leyline HQ (`/wrath/admin/forum`). They use HQ's roles (owner > admin > mod), its
  sanctions and its audit log. A ban covers the account everywhere: forum, messages, Workshop, hosting, Hero Key
  realm sign-in. A separate forum would have its own staff list, its own bans and its own log.
- **JSON for the launcher.** The forum's JSON (`/wrath/forum/api/...`) was made for the launcher's Forum page:
  plain text cut into text and link parts, so the launcher can show posts without parsing HTML. Posting from the
  launcher uses the token it already has.
- **Our rules in one place.** The same name rules, word filter, game and company name rule, rate limits and
  sanctions as everywhere else in Leyline.

What we give up: rich text, images and uploads, @mentions, notifications and email digests, user badges and trust
levels, full-text search (we search thread titles), plugins. For a community this size, plain text with links is
enough, and the Discord covers live chat.

### What would make us move to a separate forum later

Any one of these, kept up for a few weeks:

- **Size**: more than ~200 posts a day, or more than ~50,000 posts in all. SQLite copes, but a single portal process
  would be doing a lot of the forum's work.
- **Features players keep asking for** that would be big to build: notifications (replies, mentions) by email or
  push, image uploads, rich text, full-text search, private messages, polls.
- **Moderation load**: more than one or two moderators working every day, who need trust levels, auto-flagging, spam
  scoring (Akismet), a review queue for new users' first posts, or per-category moderators.
- **A separate team** wants to run the community side on its own server.

If we move: pick Discourse (DiscourseConnect lets our website stay the only sign-in), keep `/wrath/leyline/forum`
as a redirect, import the SQLite tables (threads, posts and authors map one to one), keep the launcher's Forum page
reading a small JSON shim, and put Discourse's moderation link in HQ's Forum section. Until then, nothing here locks
us in: the data is four plain tables.

## One-click access

- **Website**: Forum is in the site header on every page (`stream/hub.py` nav). Reading needs no sign-in. Signed in,
  New thread, Reply and Report are on the forum's own pages, with no second sign-in.
- **Launcher**: the Forum page reads the forum. Signed in on the launcher (Home or Manage account), New thread,
  Reply and Report work right in the app: `forum_post` in `src-tauri/src/forum.rs` sends the launcher's own Leyline
  sign-in token (`Authorization: Bearer`, the device-code token from `stream/social.py`). Signed out, New thread and
  Reply open the website's pages in the browser, as before.
- **In the game and on the Discord**: the address to share is `play.openvanilla.app/wrath/leyline/forum`. The
  in-game login line (`ov_discord.lua` `WELCOME`, the owner's words) still names `.wu` and the Discord only. Adding
  "or the forum: play.openvanilla.app/wrath/leyline/forum" there is a one-line change that needs a Lua reload on the
  realm, so it wasn't made here. TODO(owner): say if you want it, and add the address to the Discord's welcome or
  rules channel if you want it there (we never post on the Discord ourselves).
- **Guests**: a website guest account (made by "Play as guest", with a Hero Key) reads and posts like anyone else.
  The forms show "That name was made for you. Pick your own name and picture" until the guest picks a name
  (`/wrath/profile`). A guest pass (`stream/demo.py`, no account) can only read. Its pages say plainly that nothing
  is kept, and "Make a free account" goes to the Leyline sign-in.

## Posting from the launcher (the JSON)

```
POST /wrath/forum/api/new     {cat, title, body}        {ok, thread: {...}}
POST /wrath/forum/api/reply   {thread, body}            {ok, post, thread, page}
POST /wrath/forum/api/report  {post, reason, note?}     {ok, text}
Authorization: Bearer <the launcher's Leyline sign-in token>
```

- CSRF-safe: only the Bearer token is accepted, never the website cookie. Another site's page has no token to send,
  and the answer has no CORS header. The reading calls stay GET only.
- The same rules as the website's forms: title 4-100 characters, posts up to 5000, the word and brand filters, the
  rate limits (stricter for accounts new to the forum), locked threads, Announcements for the team only. A launcher
  token never carries staff powers (team posts are made on the website). The Team badge the website set is kept.
- The same sanctions: a site-wide mute or suspend, or "No forum posts" (`noforum`), refuses with the reason and
  until when. Reporting always works.
- Refusals: `{error}` with 400, 401 (signed out: the launcher forgets the token and asks to sign in again), 403,
  404 or 429.

## Moderation (Leyline HQ, `/wrath/admin/forum`)

Forum is one of HQ's main sections. Reports also show in **Needs you**.

| What | Who | Action (`POST /wrath/admin/act` or `/wrath/admin/api/act`) |
|---|---|---|
| Hide / show a post | mod | `forum.post.hide`, `forum.post.unhide` (target: post id) |
| A report: done (with what you did), dismiss, reopen | mod | `forum.report.done`, `forum.report.dismiss`, `forum.report.reopen` (target: report id) |
| Pin / unpin, lock / unlock a thread | mod | `forum.thread.pin`, `.unpin`, `.lock`, `.unlock` |
| Hide / show a thread | mod | `forum.thread.hide` (type the thread id), `forum.thread.unhide` |
| Merge a duplicate into another thread | admin | `forum.thread.merge` (field `into`; type the duplicate's id) |
| Mute from the forum (no new threads or replies) | mod: 1-168 hours; admin: any, 0 = until lifted | `forum.mute` (fields `hours`, `reason`; type the game account) |
| Lift the forum mute | mod | `forum.unmute` (target: game account) |
| Ban the account (suspend everywhere) | admin | `sanction.set` with `type=suspend` (type the game account) |
| Ban from the game | admin | `game.ban` on the person's page |

A poster's page (`/wrath/admin/forum?acct=WR...`, or "their forum history" from a report) shows their posts (hide or
show each), the reports about them, their sanctions and the mute and ban tools. Their HQ person page has an
**On the forum** box. Every action checks the role on the server, needs the typed confirm where marked, and goes in the
audit log (`admin/audit.jsonl`), refused ones too.

The JSON for a later launcher admin mode: `GET /wrath/admin/api/forum?view=reports&status=open|done|dismissed|all`,
`?view=threads&q=`, `?view=hidden`, `?view=poster&acct=`. Each answer lists the `actions` this person's role may call.

## The Discord link (forum and Discord, one conversation)

Owner, 2026-10-31: "merge forum with discord somehow as the backend and talking". Code: `stream/forum_discord.py`
(loaded by `forum.py`), tests: `stream/tests/test_forum_discord.py` (a fake Discord API, every flow below).

The same bot also sets up the Discord's channels and roles, posts live realms, Den of the week and the featured
mode, runs Find a group, gives roles from accounts and pushes our blocked words to AutoMod: docs/DISCORD.md (each part
off until the owner turns it on; it needs Manage Channels and Manage Roles on top of the permissions below).

**Our forum stays the source of truth.** Accounts, moderation, bans, blocked words and rate limits all stay ours.
Discord is a live window onto it. It is **off** until the owner sets it up (the checklist below).

| What happens | Where it shows |
|---|---|
| A new forum thread in a linked category | A Discord thread with the same title, its first post as the starter: the author's name and a link to the post |
| A forum reply (website or launcher) | A message in that thread, posted by the bot: the name, a link, the text (no pings, no link previews, markdown shown as typed) |
| A reply in that Discord thread from someone who **linked** their Leyline account | A forum post as their Leyline account, marked "via Discord · <Discord name>" (also `via` in the JSON) |
| The same from someone who **didn't** link | Nothing on the forum. They get one note ("Link your Leyline account to post here", with the link) and their message is taken off |
| A new thread a linked person starts in a linked Discord **Forum** channel | A forum thread (threads from before the link was turned on are left alone) |
| A Discord post the forum refuses (rate limit, a sanction, a bad title, a locked thread) | Not posted; they get the reason in a note and the message is taken off |
| A Discord post the word filter changes | Posted on the forum with the stars; the Discord copy is replaced by the bot's, showing the forum's text |

Discord only lets a bot answer "only you can see this" to a slash command. So `/link` answers that way, and the
one-time note for an unlinked person is a **DM** instead; with DMs closed, it is a reply that pings only them and is
deleted after 2 minutes. The note goes at most once a week per person.

**Linking.** In the Discord, `/link` answers (privately) with a one-time link, good for 15 minutes. Opening it signed
in on the website shows "Link the Discord account X to your Leyline account Y?" with a button. Anyone can open
`/wrath/leyline/forum/discord` to see or undo their link. `/unlink` in Discord undoes it too. When the website's own
Discord sign-in (Profile, Ways to sign in, "Link Discord") reports the Discord id in `/api/me` (`discord_id`, or
`discord.id`), the page offers "Link my Discord" in one click. **Unverified:** we haven't seen whether the website's
`/api/me` sends that field; without it, `/link` is the way.

**Same rules, same sanctions.** A Discord post goes through `store.reply` / `store.new_thread`, the same code as a
website post: lengths, the word and brand filters, the rate limits (the stricter ones for accounts new to the forum),
locked threads, Announcements for the team. It also passes `admin.blocked("post")`: a ban (suspend), a mute or
"Mute from the forum" in HQ stops it at once.

**Moderation both ways.**

| In Leyline HQ | On Discord |
|---|---|
| Hide a post | Our copy is edited to "This post was removed by a moderator."; a Discord person's own message is deleted |
| Show it again | Our copy is edited back; a deleted one is posted again by the bot |
| Lock / unlock a thread | The Discord thread is locked / unlocked |
| Hide a thread | The Discord thread is deleted. Show again: a new Discord thread with the first post |
| Merge a duplicate | A "merged into <title>" note in its Discord thread, then it is locked and archived |
| Pin | Not copied (a Discord Forum channel can pin only one thread) |

| On Discord | In the forum |
|---|---|
| A message deleted (ours or a linked person's) | That post is hidden, within seconds (the thread's message count drops) or within 10 minutes (the sweep) |
| A whole thread deleted | The forum thread is hidden |

Only people with **Manage Messages** in those channels can delete other people's messages on Discord, so that is the
Discord's mods. A person deleting their own message hides their own post. Every hide from Discord is in HQ's audit
log as `discord` ("deleted in Discord"), and it can be undone in HQ like any hide. `WU_FORUM_DISCORD_DELETE_HIDES=0`
turns that direction off. Edits made in Discord are not copied: the forum keeps what was first posted.

**If Discord is down.** Every change for Discord goes to an outbox table in `forum.sqlite3` first. A worker thread in
the portal sends it in order, thread by thread. It follows Discord's rate-limit headers, waits out a short 429 and
hands a long one back. It retries with back-off (5 seconds, doubling up to 15 minutes). Missing permissions are retried
every hour. Nothing is dropped except what Discord says can never work (a 400) or what is gone (a 404). A refused token
pauses the link and keeps the outbox. HQ's Forum page has one line: the link's state, what is waiting and the last
problem. Discord is read with plain REST, no gateway: one call every `WU_FORUM_DISCORD_POLL` seconds lists the
server's active threads, and only threads with new messages are read.

### Owner setup checklist (about 10 minutes)

1. **Discord Developer Portal > New Application** (or the bridge's existing "Leyline" app). Note the **Application
   ID** and the **Public Key** (General Information).
2. **Bot** tab: copy the token (Reset Token). Turn on **Message Content Intent**; without it, Discord sends empty
   texts. No other intent is needed.
3. **General Information > Interactions Endpoint URL**:
   `https://play.openvanilla.app/wrath/forum/api/discord/interactions` (Discord checks it once the portal has the
   settings below, so do step 6 first, then save this).
4. **OAuth2 > URL Generator**: scopes **bot** and **applications.commands**; bot permissions **View Channel, Send
   Messages, Send Messages in Threads, Create Public Threads, Read Message History, Manage Messages, Manage Threads**
   (permission integer `326417591296`). Nothing else: no Administrator, no Mention Everyone, no Manage Roles, no
   Embed Links. Open the URL and add the bot to the server. Manage Messages is for taking off unlinked or refused posts
   and deleting hidden ones. Manage Threads is for lock, archive and delete.
5. In Discord, make one **Forum channel** per category you want on Discord (or use a text channel: its threads are
   used). Turn on Developer Mode (User Settings > Advanced), then right-click each channel > Copy Channel ID. One Forum
   channel with a tag per category also works: `help=<channel id>/<tag id>`. Optional but tidy: let only the team
   post in the Announcements channel.
6. On the server, edit `/srv/wrath/portal.env` (never commit or paste it anywhere), add:
   ```
   WU_FORUM_DISCORD=1
   DISCORD_BOT_TOKEN=<the token>
   DISCORD_APP_ID=<the application id>
   DISCORD_PUBLIC_KEY=<the public key>
   WU_FORUM_DISCORD_CHANNELS=help=<id>,general=<id>,realms=<id>,mods=<id>,dens=<id>,news=<id>
   ```
   Categories: `realms mods dens help general news`. Leave one out to keep it off Discord. Then restart the portal
   (`docker restart wu-portal`, or run the `signin` step). No realm restart.
7. Check: `docker logs wu-portal 2>&1 | grep forum-discord` says `running for N categories` (or exactly what is
   missing). Post a test thread on the forum and watch it appear. Type `/link` in the Discord.
8. Tell the community: "Type /link in the Discord to reply to forum threads from there."

To turn it off: `WU_FORUM_DISCORD=0` (or remove the line) and restart the portal. The outbox and the links stay in
`forum.sqlite3` for when it is on again.

Other settings: `WU_FORUM_DISCORD_POLL` (5 seconds), `WU_FORUM_DISCORD_UNLINKED=keep` (leave unlinked and refused
messages on Discord; they still never reach the forum), `WU_FORUM_DISCORD_DELETE_HIDES=0`.

## Settings

`WU_FORUM=0` turns the forum off. `WU_FORUM_NEW_HOURS` (72) is how long an account counts as new to the forum.
`WU_NAME_CHANGES_PER_DAY` (3) limits a player's own name changes. The Discord link's settings are above; it is the
only part with a secret (the bot token, in `portal.env` only).

## Deploy

`signin` ships the portal (forum.py, forum_discord.py, profiles.py, social.py, admin.py, hq.py, demo.py). `ship` ships the launcher
(the Forum page's posting). No realm restart.
