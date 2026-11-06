# The mobile release

Owner, 5 Nov 2026: "continue building mobile experience launcher, server, game mods until complete and design its
webpage release with screenshots and depictions and guides and unique github repo".

This is where the phone release stands: what is done, what isn't, and how its public repo is made. The player-facing
guide is [On your phone](leyline/mobile.md) (served at `/wrath/docs/mobile`); the showcase page is `/wrath/mobile`.

## What a phone does now

| Part | State | Where |
|---|---|---|
| The Android app: Play, Realms, Mods, Account, a tab bar | built and tested in the fake-core UI tests; **not yet compiled** (the first CI run is the first build) | `launcher/ui/phone.js`, `phone.css`, `platform.js`, `src-tauri/src/mobile.rs`, [ANDROID.md](ANDROID.md) |
| Log in on the phone (Hero Key, Google, name and password, guest) | tested end to end | `launcher/tests/phonejourney.browser.test.cjs` |
| PLAY: the browser game full screen in the app; no WebGPU: Open in Chrome | tested | `phone.browser.test.cjs` |
| Play solo on this phone (the phone's own world, a server inside the page) | tested against a stand-in server; the server's page build comes from CI | [PHONE-SOLO.md](PHONE-SOLO.md) |
| **Join a friend in one tap**: Paste invite link and join, or Realms › Got an invite link? | new, tested | `phone.js join()`, `phonejourney.browser.test.cjs` |
| Community realms for the browser game's client play in the app | new, tested | `phone.js fixRealms()` |
| Mods: each card says whether it works on a phone | new, tested | `phone.js fixMods()` |
| Every page a phone reaches fits upright and sideways, 44 px controls | tested (Play, Realms, Mods, Account, News, Forum, Modes, Help, sign-in) | `phonejourney.browser.test.cjs` |
| Touch controls in the browser game | shipped before this work, untouched | [TOUCH-CONTROLS.md](TOUCH-CONTROLS.md) |
| The APK on a GitHub release (android-latest), only after the emulator passed | new workflow job | `tools/github-workflows/leyline-android.yml` (job `publish`) |
| The release page `/wrath/mobile`, its screenshots, `/wrath/docs/mobile`, menu items | new, tested | `stream/mobile_page.py`, `stream/www-files/art/mobile/`, `docs/leyline/mobile.md` |
| The public repo Leyline-Mobile | prepared (export, Control step); the owner makes the repo | `tools/leyline-repo/mobile_export.py` |

### Server on a phone: the honest answer

A phone does not run a realm for others. It sleeps, changes networks, has no address friends can reach, and Android
gives an app no long-running server. So:

- **The phone's own world is Play solo on this phone**: Den Lite, our own small server, runs in a worker inside the
  game page. Just for you, saved on the phone.
- **Playing together goes through a realm someone hosts**: the official realm, a community realm, or a friend's PC
  (Host for friends). The phone joins with the friend's invite link in one tap.

How Join a friend works: the friend's link (`https://<Leyline relay>/join/<id>#<invite>`, or its
`leyline://join?link=` form) becomes the site's own browser hop, `/wrath?go=r:<id>#inv=<invite>`, opened in the app.
The invite only ever sits in the address's fragment, which browsers never send to a server; the game page
(`wrath-page.js hopInvite`) hands it to the realm's own join page, where the friend makes an account. The portal
finds a private realm through the relay (`private_hop`). A friend's **Den** link (it carries a Den login after `~~`)
or a `DEN-` code is refused with one line: Den worlds are not in the browser game's world.

### Mods on a phone

| Kind | On a phone |
|---|---|
| Interface addons | Yes: the game page's **Your addons**, from the addon's .zip (browser storage, checked like on the PC) |
| Realm mods and Leyline Modes | Yes, on a realm that runs them |
| Mods with a realm half and a game half | the realm half only |
| Client mods, looks, HD packs, themes, graphics settings, loadouts | No: they change a game folder |

## Screenshots

Made from the real pages at phone sizes by `tools/mobile-shots/mobile_shots.cjs` (the phone app with the tests' fake
core; the touch controls over a plain stand-in field, so no game picture), small WebP files in
`stream/www-files/art/mobile/`: `phone-play`, `phone-play-landscape`, `phone-realms`, `phone-mods`, `phone-account`,
`phone-join`, `phone-solo`, `phone-touch`. Served at `/wrath/art/mobile/<name>.webp` (not behind the game data
killswitch: our own UI only). Run it again after a layout change:
`NODE_PATH=$(npm root -g) node tools/mobile-shots/mobile_shots.cjs`.

## Control presses

1. **signin**: the site's `/wrath/mobile`, `/wrath/docs/mobile`, the menu item and the screenshots. It now also
   ships `site_pages.py` (the guides and press pages were never packed by this step before) and checks
   `/wrath/mobile` in its self-check.
2. **ship**: the app changes (Join a friend, community realms, mod lines, layout) reach GitHub; the `leyline` step
   exports them and the `leyline android` workflow (now with `publish`) builds the APK.
3. **leyline-mobile**, after the repo exists (below).

## Not done (honest)

- **The app has never been compiled.** Nothing in the cloud can build Rust for Android; the first `leyline android`
  run in the Leyline repo is the first build. Expect a round of small fixes from its log.
- **Tapping an invite link doesn't open the app yet.** Android App Links need an intent filter in the generated
  Android project and a verified link file on the relay's domain. Until then: copy the link, then Paste invite link
  and join in the app (or Play in your browser on the link's page, in the phone's browser).
- **Log in twice the first time**: once in the app, once on the game page in its web view (separate sign-ins).
  Handing the app's sign-in to the page safely is a next step.
- **Orientation is not locked** in the app (the game page asks you to turn the phone).
- **The Android web view and shared memory** (needed by Play solo) are not verified on a real phone; without it the
  page plays on the realm instead and says so.
- **No store listing.** TODO(owner): whether to list on an app store.
- **Signed releases** need the owner's release key in the repo's secrets ([ANDROID.md](ANDROID.md)); until then the
  release carries the debug-signed APK.

## The public repo: Leyline-Mobile

**Name:** `orionopenwow/Leyline-Mobile` (public, MIT).

**Decision: the app's source stays in the Leyline repo.** The app is the launcher crate built for Android: one Rust
crate for desktop and phone, with its lockfile and the JSON it builds in. A second copy in another repo would drift,
and splitting the core needs compiling, which can't be done here. So Leyline-Mobile's workflow checks out
`orionopenwow/Leyline` at one pinned commit (`LEYLINE_REF`), builds and tests the APK there, and publishes it on
Leyline-Mobile's own releases. Leyline-Mobile carries what is the phone's own:

| Folder | What |
|---|---|
| `web/` | the touch layer (`touch.js`, `touch-driver.js`, `input.js`), the web manifest, and phone solo's page side (`web/solo/*.js`, never the wasm) |
| `app/` | a read-only mirror of the phone parts of the launcher (phone layout, `mobile.rs`, Android config and activity, smoke script, phone UI tests) |
| `screenshots/` | the release page's screenshots, and `tools/mobile-shots/` that makes them |
| `docs/` | ANDROID, TOUCH-CONTROLS, this file, ON-YOUR-PHONE |
| `.github/workflows/android.yml` | made from `leyline-android.yml`: checks out Leyline at `LEYLINE_REF`, apk, smoke, publish |
| `README.md`, `LICENSE`, `.gitignore`, `LEYLINE_REF` | generated; the README has the screenshots, the legal line, no brand words |

The export (`tools/leyline-repo/mobile_export.py`) uses export.py's own checks: game data, secrets, logs, decompiled
code and citations refused; brand words refused except the one legal line; and no client version numbers in the
README's or the docs' sentences. Only the files it lists are copied.

### Owner, once

1. On GitHub, make the empty public repo **orionopenwow/Leyline-Mobile** (no README, no licence: the export writes them).
2. Press **ship** in Leyline Control (the `leyline` step pushes the Leyline repo the app is built from).
3. Press **leyline-mobile**: it reads the Leyline repo's commit from `.repos\leyline-public`, exports into
   `.repos\leyline-mobile` and pushes as Orion. It never creates the repo and never force-pushes.
4. The first build starts by itself (a new `LEYLINE_REF`). When it's green, **Leyline for Android (latest)** is on
   the repo's releases.
5. Optional: set `WU_MOBILE_REPO=https://github.com/orionopenwow/Leyline-Mobile` in the portal's env, so
   `/wrath/mobile` links the mobile repo's releases instead of the Leyline repo's. TODO(owner): your call.
