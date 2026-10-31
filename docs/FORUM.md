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

## Settings

`WU_FORUM=0` turns the forum off. `WU_FORUM_NEW_HOURS` (72) is how long an account counts as new to the forum.
`WU_NAME_CHANGES_PER_DAY` (3) limits a player's own name changes. Nothing else is new: no service, port or secret.

## Deploy

`signin` ships the portal (forum.py, profiles.py, social.py, admin.py, hq.py, demo.py). `ship` ships the launcher
(the Forum page's posting). No realm restart.
