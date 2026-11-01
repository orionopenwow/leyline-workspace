# Hero Key

A Hero Key is a way to sign in to the website without a password. It is one of the two sign-in options at [play.openvanilla.app/wrath](https://play.openvanilla.app/wrath). The other is Google.

The website's sign-in, Hero Keys included, is run by the Leyline Launcher realm service on play.openvanilla.app. That service is a sibling project and is not in this repo. This page says what this repo shows and marks the rest "to confirm".

## What you can do with it today

- **Sign in.** Use your Hero Key on the sign-in page. The site then shows your game account for Wrath, and makes it on the realm if needed. There are no separate Wrath accounts.
- **Get one.** New here? **Get your Hero Key** at `/wrath/key` (or in the launcher: the profile chip, then Get your Hero Key)
  makes your free account with a name made for you, and shows your key once: hidden until you press Show, with
  Download as a text file and Copy. Change the name later; set a password if you want one (it signs you in to the launcher).
- **See your key.** Signed in, **Your Hero Key** at `/wrath/key` (in the launcher: Manage account) says when it was made
  and last used. A key is shown only when it is new: lost it? **Make a new key** (the old one stops working at once).
- **Sign in with it in the launcher.** Paste it in the launcher's sign-in: no browser needed.
- **Save it in your browser.** "Save in this browser" on the realm site stores the key with your browser's password manager. The next time you open `/wrath` signed out, the page reads the saved key and signs you in by itself.

How the automatic sign-in works, from `stream/wrath_portal.py`:

1. The page asks the browser for a saved password.
2. It only uses one that starts with `ovk1.`. That prefix marks a Hero Key.
3. It sends the key to the site's `/auth/key`, which signs you in and brings you back to `/wrath`.
4. It tries once per tab. With no saved key, a browser without that feature, a refusal or 6 seconds of nothing, you get the normal sign-in page.

## Claiming a hosted realm

A public realm on the realm list shows as "Unclaimed" until its host claims it. The plan:

1. The host signs in on the site with a Hero Key, picks "Claim a realm" and enters the realm id.
2. The site makes a one-time token and shows it.
3. The host runs `leyline claim TOKEN`. The realm's agent puts the token in its next signed heartbeat.
4. The site tells the relay's listing about the token. The listing accepts it only if the realm's latest signed heartbeat carries it, so only the holder of the realm key can claim the realm.
5. The record then shows "claimed by" the Hero Key account's display name.

The listing side is built and tested. The website side, the "Claim a realm" page and the token, is planned.

The design calls this Hero Key v2: an Ed25519 challenge and response, so the host never types a password into anything a realm can see.

## Signing in to community realms (coming)

Built but switched off for now: one Hero Key signs you in to any community realm that takes it, in the browser game
and in the launcher, with no account to make per realm and no password kept in the browser. Each realm sees a
different id for you, so realms can't tell it's the same player. The details, for the curious and for hosts:
`docs/HERO-KEY-SSO.md` in the repo.

## To confirm

These are in the Leyline Launcher repo, not here, so this page can't check them:

- The exact format of a key after `ovk1.`, and how it is checked (`launcher/herokey_v2.py` in Leyline Launcher).
- Whether the v2 challenge-and-response sign-in is live on the website, or only planned.
- What `launcher/passport.py` in Leyline Launcher does, and whether a Hero Key will carry a player's identity across realms (a "passport").

## Keep it safe

Treat your Hero Key like a password. Anyone who has it can sign in as you and see your game login.
