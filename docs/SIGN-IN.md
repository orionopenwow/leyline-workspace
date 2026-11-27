# Sign-in and profiles (website and launcher)

Owner, 2026-10-30: "Allow manage account within launcher, bio profile pic etc basic stuff, smooth out the login process".
Owner, 2 Nov 21:04: "remove discord auth from Leyline". **Sign-in is the Hero Key, with Google as the other way.
Discord sign-in is not used**: the portal never offers it, whatever the website's own `/login` shows
(`stream/profiles.py` `ways()` always answers `"discord": false`). The wording everywhere is "Log in".

Player-facing help: [What is a Hero Key? / Lost your key?](leyline/hero-key.md) (also at `/wrath/docs/hero-key` on the site).

## Who does what

```
 browser / launcher                our portal (stream/, Python)                 the website (Rust, /opt/wenilla-realm)
 ------------------                ----------------------------                 --------------------------------------
 /wrath/signin  ───────────────▶   Log in: the Hero Key (key file, paste, or
                                   the key kept in this browser), then Google,
                                   then Create a Hero Key
 /wrath/signin/go/<way> ───────▶   sets ly_next (ours, Path=/wrath) and
                                   wr_next=/wrath/signin/welcome (the website's
                                   own "come back to" cookie), then  ─────────▶ /key or /auth/google/start
                                                                                signs in, makes the account the first time,
                                                                                sets wr_session, lands on wr_next
 /wrath/signin/welcome ◀────────────────────────────────────────────────────────
                                   first time only: name, picture, bio (Skip works)
 back to where they came from
```

- Sign-in itself (sessions, accounts, Google, Hero Keys) stays in the Rust site. Nothing in Python sees a
  password, makes an account or reads its database; the portal learns who is signed in through `GET /api/me` and
  `GET /api/play`, as before.
- `wr_next` is the Rust site's own cookie (`auth/oauth.rs` `next_cookie`, `landing`): it only ever sends a browser to a
  path on this site (`safe_next`). We set it to `/wrath/signin/welcome`, nothing else.
- Which ways show: `stream/profiles.py` `ways()` reads the website's `/login` (no cookie) every 5 minutes and looks for
  `/auth/google/start`. The Hero Key is always there. If `/login` can't be read: Hero Key and Google. Discord: never
  (the old `discord` key stays in the answer, always `false`, so older launchers don't break).
- The old `/login` and `/key` pages redirect to `/wrath/signin` and `/wrath/key` (Caddy, `stream/fastloop.sh`).

## The launcher

The launcher's sign-in screen comes first (`launcher/ui/gate.js`): Log in with your browser, Google, the Hero Key
file (drop or choose), a pasted key, or name and password; Sign up (Create your Hero Key); offline with a saved
sign-in still gets in; with the website down, "Continue without logging in".

Log in with your browser is the device code flow (`stream/social.py`, `/wrath/link`): it opens
`/wrath/link?code=XXXX-XXXX&via=herokey|google`, which goes straight to that way's sign-in, then back to the code to
approve. The launcher polls and continues by itself; the first time, it opens its own Manage account page for the
profile step. (`via=discord` is still accepted from old launchers and lands on the normal chooser.)

## Profiles

- Name and `#tag`: stream/social.py's (one name everywhere). Refused: the game and company names, also hidden behind
  separators (`profiles.name_problem`).
- Bio: plain text, up to 300 characters, the word filter's words starred out, game and company names replaced.
- Picture: one of 12 original preset shapes (`profiles.PRESETS`) or the initial. Served at `/wrath/avatar/p/<id>.svg`
  (a week) and `/wrath/avatar/u/<public id>.svg?v=<n>` (a day; `v` changes when the picture or name does).
- The admin console's Profiles tab resets a picture, bio or name. Every change, the player's own too, is kept in
  `profile_history` (profiles.sqlite3) and staff resets are in the audit log. Nothing is deleted.

### Guests

What "guest" means here, and what each kind can do:

| Kind | What it is | `/api/me` | Name and picture |
|---|---|---|---|
| **Website guest account** | "Play as guest" on the Rust site (`auth/guest.rs`): a real account with no password, a Hero Key made at once, characters kept | `username: guest-xxxxxxxx`, `display_name: "Guest XXXXXXXX"` | like anyone: `/wrath/profile`, the launcher's Manage account, the welcome step when it signs in through `/wrath/signin`. Its first Leyline name is `GuestXXXXXXXX`; the pages flag it (`name_generated`) and the forum's forms link "Pick your own name and picture" |
| **Hero Key account** | any account signing in with its Hero Key (a guest's or anyone's) | as its account | the same |
| **Guest pass** (`stream/demo.py`, `WU_GUEST_DEMO=1`) | a shared demo character, no account, reset after each visit | none (no sign-in) | not possible: nothing is kept. Its pages say so, and "Make a free account" goes to `/wrath/signin` |

What stopped guests before (31 Oct): nothing refused them, but the site header and the forum showed the website's display
name ("Guest ABCD2345") even after a rename, for up to a minute on the forum and forever in the header. Now
`social.cookie_user` and the forum show the Leyline name at once. Nothing else was guest-specific.

The same rules for everyone: `social.name_error`, the brand rule, the word filter, the presets; names that pass for the
team (Admin, Moderator, Leyline..., GM, Staff, Owner) and guest-style names are refused; at most
`WU_NAME_CHANGES_PER_DAY` (3) name changes of your own a day (staff resets aren't counted). The forum's stricter limits
for accounts new to it stay (they count from the forum's first sight of the account, so a rename doesn't reset them).

Not ours to change (the Rust site): an idle guest account is removed after `OPENVANILLA_GUEST_IDLE_DAYS` (30) even when
it saved its Hero Key. The sweep (`auth/guest.rs sweep`) doesn't look at keys. TODO(owner): if Hero Key accounts must
be kept, that sweep needs `AND NOT EXISTS (SELECT 1 FROM hero_keys ...)` in the website's repository.

### Not built, and what each would need

- Uploading a picture: the portal runs on the stock `python:3.12-alpine` image, which has no image library to decode and
  re-encode an upload safely (strip metadata, 256 px WebP). Needs a portal image with Pillow (e.g. a two-line Dockerfile
  `FROM python:3.12-alpine` + `RUN pip install --no-cache-dir Pillow==<pinned>`, used by stream-lib.sh and
  fastloop.sh), then an upload endpoint in profiles.py. TODO(owner): say if you want this.
- The Google picture: the Rust site asks Google for `openid profile`, but keeps
  only the display name; `/api/me` answers `{username, display_name, role}`. Needs, in the Rust site: store the
  provider's picture URL on the identity (`identities.avatar_url`), and add `"avatar_url"` to `/api/me`. The portal
  would then offer "Use my Google picture" and proxy it (never hotlink).
- Showing which ways are linked, and unlinking: `/api/me` doesn't say. Needs `"providers": ["google", ...]` in
  `/api/me`; unlinking needs a POST on the Rust site (it has none). Today the profile page links to `/account/key`
  (the Hero Key page) and to `/auth/<way>/start?link=1` (the Rust site's own "link this way to my account").

## Owner checklist

Nothing below types a password or makes an account on another service for you.

- [ ] Google: already on (the website has offered it for months). Nothing to do.
- [x] Discord sign-in: **not used** (owner, 2 Nov: "remove discord auth from Leyline"). The portal never offers it. Keep
      `DISCORD_SIGNIN=off` in `/opt/wenilla-realm/.env` too, so the website's own `/login` doesn't show it either.
      How it was switched off, and how it would come back, is in the git history of this file (before 12 Nov).
- [ ] Read the sign-in and welcome words (TODO(owner) in stream/profiles.py).

No new environment variables or secrets on our portal. `WU_PROFILES=0` turns profiles and /wrath/signin off.
