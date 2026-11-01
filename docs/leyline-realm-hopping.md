# Leyline realm hopping (MVP) and the next step: Hero Key sign-on for Beacons

## What the MVP does

Join in the in-game Leyline window reloads the browser game on that community realm (a Beacon), in the same
tab. "Back to Wrath" is one click in the window and on the page.

### The bridge (hosting/relay/wurelay/relay.py)

- `wss://<relay>/ws/<realm id>/auth` and `/ws/<realm id>/world`: a WebSocket per game connection, turned into a
  stream on the Beacon's existing multiplexed tunnel (service 3724 or 8085 on the host's agent). The older
  `/r/<id>/ws/<port>` path stays.
- Framing is wu-play's: binary messages, one TCP read per message, the bytes reassembled as a stream; either
  side's close closes the other. `?host=` is ignored.
- It only reaches a realm whose agent is connected and signed in (Ed25519 realm key). The relay never dials a
  host; nothing in the request names one.
- Limits: the realm's own caps (streams, KB/s, GB/day, per-IP streams and connection rate), plus WebSocket
  upgrades per IP per minute (`RELAY_WS_PER_MIN_PER_IP`, 30), browser streams in total (`RELAY_MAX_WS_TOTAL`,
  1024), all streams in total (`RELAY_MAX_STREAMS_TOTAL`, 4096), and an Origin allow-list
  (`RELAY_PLAY_ORIGINS` + `RELAY_SITE_ORIGINS`; no Origin = a native tool, allowed like raw TCP).

### The hop, step by step

1. Join on a live community realm: `Leyline.lua` sets the Lua global `WUUI_LEYLINE_HOP = <id>` and whispers
   `WULY H:<id>` to itself (`ov_leyline.lua` echoes it; that only matters to the addon itself, see below).
2. The page (`wrath-page.js` `watchHops`) polls the global every second while the bridge is up
   (`wenilla.lua`: take it and clear it), and goes to `https://play.openvanilla.app/wrath?go=r:<id>` (own game
   files: `/wrath/play?mode=own&go=r:<id>`). The poll is the trigger: the web client gives CHAT_MSG_ADDON to the
   addons but not to the page bridge (see "What the real client does"). The global is the authority: someone
   else's whisper can't make a page hop. Leyline.lua clears an untaken global after 8 s (the desktop client).
3. The portal signs the player in as usual and carries `go` to the gate. `/__gate` checks the id against the
   relay listing (live realms only) and marks the session as a hop. Anything else lands on Wrath.
4. `/__env.js` for a hop session: `hop`, `hop_name`, `hop_ws`, `hop_join`, `host` (the relay), `home`, and no
   Wrath login.
5. `wrath-early.js` (before the client starts) wraps `WebSocket`: the client's own `/ws/3724` becomes
   `<hop_ws>/auth`, any other port `<hop_ws>/world`. It fills in the login this browser keeps for that realm
   (`localStorage["wu.leyline.login.<id>"]`).
6. No login kept yet: the page frames the realm's join page (`/join/<id>?hop=1&from=<origin>`, served with COEP,
   CORP and `frame-ancestors` for the play origins only). The player makes an account there (SRP verifier made
   in the browser, as before) or types one they have. The join page hands the login to the game page only
   (postMessage to an allowed origin). The page keeps it and reloads. The login is never sent to our servers;
   the game proves it to the Beacon with SRP.
7. "Back to Wrath": the page bar's button, or the window's (it hops to `wrath`), goes to `/wrath?go=h:dalaran`, a
   fresh session with no hop that lands in Dalaran (the go intent, `ov_go.lua`; a brand-new character's first
   login is never moved, and where level-1 characters start is unchanged).

### Known limits

- The streamed slice only has its own part of the world. On a Beacon, the full world needs "Play with my own
  game files" (that mode is kept across a hop).
- A hop goes through the Wrath sign-in, so Wrath's realm must be answering.
- Private Beacons aren't listed, so they can't be hopped to yet (the bridge takes `?invite=`).
- The kept password is in this browser's localStorage, readable by script on the game's origin.
  "Use another account" on the bar forgets it.

## The realm browser in game, and the fast hop (2026-10-31)

Owner, 31 Oct: "the realm swap should be able to happen in game with a list of all realms on the p2p network and a
process to observe what mods they have etc before joining and who is there ... server hop should be as quick as
possible to the character create / select screen of their traversing hero key account ... hopping between client
versions should be easy too".

### Where it shows

- **The Leyline window** (`WrathUnleashedUI/Leyline.lua`, `/leyline`, Esc > Realm List). In the browser game: a
  search box, the filters **In browser**, **Own files**, **Has players**, **Version** (cycles through the version
  chips the list has), pages of 20, and **View** on each row: the preview opens beside the window. Its **Join** hops.
  The desktop client (no page) keeps the realm's own list (`ov_leyline.lua`, the WULY protocol above, unchanged).
- **Character select** (`GlueXML/ForeverUIGlueLeyline.lua`, listed in `ForeverUIGlueWeb.xml`; the name starts with
  ForeverUI so `fui-overlay.py` takes it): Forever UI's **REALMS** button opens the same browser while the page offers
  it. Without the page, or with Shift held, it does what it always did (this server's realm list; the browser also
  has a "This server's realms" button). Without that top-bar button, a **Realms** button of our own sits top left.
  Every step is pcall-guarded.

### How the data gets into the game (the page bridge)

The client can't make HTTP calls; the page does and hands the answers in through the bridge, the way the game
folder's state goes in (`stream/www-files/wrath-page.js`, block `leyline-browse`):

```
game -> page   WUUI_LEYLINE_ASK = "list|<page>|<filters>|<search>"   filters: b in browser, o own files, p has players,
                                                                      v<build> that version (comma separated)
               WUUI_LEYLINE_ASK = "preview|<realm id>"               one realm's details (+ the Hero Key prefetch)
               WUUI_LEYLINE_ASK = "launch|<realm id>"                another game version: the launcher bar
page -> game   WUUI_LEYLINE_NET = { page, pages, total, network = {realms, players, bots}, rows = { {id, name, here,
                 live, online, bots, build, chip, browser, own, hk, up, featured, verified, hangout, mods, desc}, ... } }
               WUUI_LEYLINE_PREVIEW = { id, name, desc, rules, live, online, bots, who (only if shared), mods = {
                 {name, req, client}, ... }, build, chip, up, hk ("ready"|"yes"|"no"|"unknown"), browser, own,
                 verdict ("ok"|"own"|"launcher"|"offline"), why }
               then WUUI_LEYLINE_ONNET(kind) and WUUI_LEYLINE_ONNET_GLUE(kind), each pcall'd
```

The page polls the ask every 0.4 s while the bridge has a VM and sets `WUUI_LEYLINE_BROWSE = 1` so the game knows it
can ask. A page that doesn't answer within 6 s: the window clears the flag and asks the realm instead. Pages are 20
rows, a preview at most 50 names and 32 mods (a few KB of Lua). Every string is escaped (`\ddd`) in the Lua it writes.

Sources, merged by realm id: the network listing `<home>/wrath/realms.json` (staff hides, featured, the official
order) and the relay's own listing `/api/listing/realms.json` (or `/api/mesh/realms.json` when the site's
`mesh_realms` flag is on, falling back to the listing). Without the network listing, the relay's alone. Flags from
`<home>/wrath/leyline/flags.json`. Cached 30 s. No cookies go to any listing.

### The listing contract (what the relay listing adds per realm, read defensively)

Each field is read from the record, else from its `community` object; missing or malformed = unknown, never an error.

| field | type | shown as | when missing |
|---|---|---|---|
| `build` | `"3.3.5a"` (or the number) | the version chip; In browser / Own files; the verdict | `profile.build`, `profile.version`, else 3.3.5a |
| `online` | int | "N players online now" | `community.lastHeartbeat.players` |
| `who` | list of up to 50 names (only when the realm opted in) | "Who's on" | "This realm doesn't share who's on." |
| `rules` | short text (a list is joined) | "House rules", 300 chars | "The host hasn't written house rules." |
| `mods` | `[{id, name, required, client}]` (ids as `gamemods.py`) | Required / Optional / Uses, "(on your side)" for client ones | `required` unknown -> "Uses"; `client` guessed from `up:` / `mode:` |
| `herokey` | bool | "Hero Key sign-in" chip; the fast hop signs in with it | no chip, no prefetch |
| `uptime_pct` | number | "N% up lately" | `community.uptime.d7`, `.h24`, else "Uptime: new" |
| `own_files` | bool (optional) | needs own game files | from the build |

Host text (names, rules, the tagline, mod names, who) never shows a game brand word, a colour code or a version number
(those are dropped); names in `who` must be 2 to 12 letters. The version is only ever a chip.

### Versions

The verdict (`verdict()` in the page; `L.Verdict` in the window for the desktop client, from `GetBuildInfo`):
offline; **launcher** when the realm needs another game version or takes no browser players (Warden on);
**own** when it needs own game files and this session is streamed (Join then goes through the website's own-files
switch, `/wrath/play?mode=own&go=r:<id>`); else ok. A launcher realm is never hopped to: the preview says so plainly
("This realm needs a different game version than this one. Open it in the Leyline launcher."), its button reads
**Open in launcher**, and in the browser the page shows a bar with **Open in the Leyline launcher**
(`leyline://realm/<id>`, a real click), **Get the launcher** and **Not now**. The desktop client puts the link in the
window's copy box.

### The fast hop

Join (the window's, the glue's, or a row's on the realm list) still sets `WUUI_LEYLINE_HOP`; the page's watcher now
tries `fastHop` first and only falls back to the old way (`/wrath?go=r:<id>` through the website) when it says no:

1. **At the preview** (the realm says `herokey: true`, the site's `herokey_sso` flag is on, not declined in this
   browser): the page asks the portal for an assertion, `POST /__gate?herokey=<id>` (same origin, the session cookie;
   the same rules and limits as the hop env's). Only the website is asked; the realm learns nothing until Join.
2. **Join**: "Going to <realm>…" covers the game with a progress bar. The page redeems the assertion at the relay
   (`credentials: omit`, at most 6 s) and keeps the one-time login in this tab's sessionStorage
   (`wu.leyline.pre.<id>`, `{user, pass, at, ttl}`; at = the redeem, so the realm's 2-minute window is honoured).
3. `POST /__gate?hop=<id>` switches **this** session to that realm (`{"sso": "pre"}` when step 2 worked: the env then
   issues no second assertion) and answers the landing page; the page `location.replace`s to it. No trip through the
   website's sign-in, no SOAP to our realm, so a hop works even while our realm isn't answering.
4. The landing page: `wrath-early.js` takes the kept login while it is fresh (ttl less 15 s): no redeem, no join page,
   nothing held. The "Signing in" cover reads "Going to <realm>…" and hides at the character list, so the login
   screen is never seen. A brand-new Hero Key account lands on an empty character list (create).
5. Anything that fails (no ticket, the realm refuses it, an old portal, a junk answer) falls back: without "pre" the
   env issues its own assertion as before; without a switch, the old way. The old join-page path is unchanged for
   realms without Hero Key or with the flag off.

Also: with Hero Key through the env (the old way), the wasm download no longer waits for the redeem; only its answer
is held, so the download and the redeem overlap. "Back to <home>" is still one click: the bar and the window use the
same switch (`POST /__gate?hop=wrath`: the Wrath login again, Dalaran on arrival), else the old way.

The slice and the game page are the same origin and URL before and after a hop, so the browser cache, the
service worker (when on) and IndexedDB keep everything already loaded; the boot itself is the fast-load work's.

### Measured here, and the plan for live

Local harness (the portal with a fake realm, an 80 ms round trip and 40 ms per SOAP command): the old way is 5 round
trips and 2 SOAP commands to the landing env, median 373 ms; the fast hop is 3 round trips, median 245 ms (30 ms /
20 ms: 181 vs 106 ms). On top of that the redeem no longer serialises with the wasm download.

Live (signed in, on Wrath, in the world, a Hero Key realm listed): Join a realm from the preview, then on the new page
`window.__wuHop.timing` gives `{via, page, charselect, sso}` (ms from Join to the page and to the character list; the
console prints one line). Do it 5 times each way: `localStorage['wu.leyline.fasthop'] = 'off'` for the old way,
remove it for the fast one; also once from character select (REALMS). Compare the medians; `sso` should be `pre` on
the fast way. Also check: the listing fields show in the preview (the network agent's relay), a realm with
`who` off says so, a realm on another version shows the launcher bar, Back is one click.

### What the launcher needs (launcher/src-tauri/src/deeplink.rs)

`leyline://realm/<id>` already shows the realm on the Realms page with its PLAY button. For "easy version hops" the
launcher should: pick (or offer to add) a game folder of the build the realm needs when that link arrives (the
listing's `build`), sign in with Hero Key when the realm has `herokey` (its `play_realm_herokey` exists), and start it
in one click; and say plainly when no folder of that version is set up. The browser game only hands over the link.

## Wrath Unleashed (the owner's realm as a Beacon)

The owner hosts it; nothing runs anywhere but on the realm's own box. `hosting/hangout/hangout-up.sh` (the
`hangout` step; `hangout-off` takes it off Leyline, keeping its key) runs two containers next to the realm:

- `wu-hangout-agent`: the host kit's own agent with `setup.py official`'s host.json: public, `signup: off`,
  `official: true` (the agent never touches `acore_auth.realmlist`, accounts or GM rights), players and bots
  from `http://wu-portal:8091/wrath/status.json`. Its key is made on the box (`state/realm.key`, 0600). It
  tunnels to the relay's web side on the same box (`ws://<RELAY_HTTP_BIND>/agent`) and is listed as
  "Wrath Unleashed".
- `wu-hangout-auth`: a second login server for relay players: the realm's own authserver image and settings
  (copied from `ov-ac-authserver`), the same database, no published port, but `AC_WRONG_PASS_MAX_COUNT=0`.
  Every relay player reaches a realm from the agent's address; the realm's own IP ban (10 wrong passwords,
  10 minutes) would lock every hopper out at once. The relay counts wrong passwords per player IP instead
  (`RELAY_AUTH_FAILS_PER_IP`, 10 in `RELAY_AUTH_FAIL_WINDOW_SECS`, 600): it reads only the authserver's
  logon answers. That protects every Beacon the same way.

Accounts: the hangout IS Wrath, so a hop into it uses the player's own Wrath login. `/__env.js` for that hop
session carries `hop_home: "1"` with the session's own `user`/`pass` (the same account the portal already made;
the relay and the agent carry only SRP, never the password), and `wrath-early.js` keeps it (any other Beacon:
dropped, as before). No join page, no second account, nothing typed. The relay refuses signups for it.

Dalaran: `/__gate` hops to it only while it's live on the listing; the portal turns `go=r:<hangout id>` into
the `h:dalaran` go intent, so hop arrivals land in Dalaran; "Back to Wrath" lands there too. The hangout is
first on `/wrath/realms` (with "Play in Dalaran" straight to `/wrath?go=h:dalaran`, no relay needed) and in the
in-game list (`leyline-fetch.py` reads `/srv/wrath/hangout-realm.id`). Limits: through the relay it shares the
per-realm caps (`RELAY_MAX_STREAMS` 128 = 64 players at once, `RELAY_KBPS`); the site's button and "Back to
Wrath" go direct.

## The Leyline Sample Hangout (temporary, 2026-10-29)

Community-hosted realms are the main Leyline network; the official realm becomes a temporary, lighter **Leyline
Sample Hangout** to try the game. It is the main realm with an overlay (`realm/profiles/sampler.env`), switched by
`realm/sampler.sh` (Autopilot `sampler-check`, `sampler`, `sampler-off`):

- **Streamed browser sessions** (marked by the sign-in gate) get Dalaran, their starting area (Northshire Valley,
  the Valley of Trials, Ebon Hold for Death Knights) and every streamed dungeon, raid, arena and battleground. The
  capitals close (`OV_SAMPLER_CITIES=1` opens them). Elsewhere `ov_slices.lua` brings them home with "Add your own
  3.3.5a game folder to explore the whole world ... Or join a community realm from Leyline."
- **Own files** (a desktop client, or "Use my game folder" in the browser, which clears the mark) keep the whole
  world, as on the main realm. With streaming switched off, every session is an own-files session.
- The Dungeon Finder, raids, Companions, the boost, minigolf, Modes, the Workshop, friends and the admin console
  are unchanged. A build area opens on Krasus' Landing (`OV_BUILD_DALARAN=1`).
- 12 random bots, all level 80, living around Dalaran's bankers (`AC_PLAYERBOTS_*` in the overlay); idle grids
  unload after a minute; open-world visibility 75 yards.
- Labels: the realm list name, the play page, `/wrath/realms` and `/wrath/leyline` say "Leyline Sample Hangout"
  and "temporary" (`WU_SAMPLER=1` in portal.env); everyone gets a one-line welcome in game.
- The browser slice `stream/slices/sampler.json` is `mini.json` less the capitals and the Goldshire inn; the
  `slice-sampler` step builds it and `stream/server/slice-serve.sh sampler` serves it beside mini
  (`slice-serve.sh mini` switches back; `sampler.sh off` does it too).

## Website realms: the key is made on the host's PC

`/wrath/host` makes a page id, invite/host secrets and a claim token, never a key. The download (seed v2)
carries those; `setup.py init --seed` makes the key on the PC. The agent's hello carries
`pair_sha256 = sha256("leyline-pair-v1\n" + claim token)`; the website asks `GET /api/pair/<hash>` (rate
limited, first realm to show a hash keeps it, kept in the relay's state file) and from then on shows that realm
id's links and status. Why: keys.py's rule (the key never leaves the host's PC) held for every realm except
website ones, whose key sat in `/srv/wrath/hosted` (and its backups) until the first connect, and in the
downloaded .bat forever. Same steps for the player; the only visible change is that the share and invite
links appear on the page once the PC connects (the realm id comes from the PC's key). Older records (seed v1)
keep working.

## What the real client does (answered from the source, 2026-10-25)

- **WebSocket.** The wasm glue (`wenilla.js`, wasm-bindgen) does `new WebSocket(url)` on the main thread at the
  moment of connecting, against the global, and the URL is `{wss://page host}/ws/{port}?host=...`
  (`benilla-protocol/src/transport/web.rs` `connect_url`; `set_ws_base` is never called). So `wrath-early.js`'s
  wrapper, installed before the module script, is what the client calls, and the returned object is a real
  WebSocket (binaryType and handlers work).
- **CHAT_MSG_ADDON.** It reaches the UI's addons (`benilla-ui/src/script/addon_message.rs`
  `deliver_addon_message`), so the Leyline window's list works. It does NOT reach the page: that function
  calls `fire_event_values` directly, past `fire_event_into`, which is where the bridge's event tap copies
  events. So `wenilla.on('CHAT_MSG_ADDON')` never fires today; the 1 s poll of the global is the trigger. (A
  one-line client fix would be to deliver it through `fire_event_into`; the page needs nothing more.)
- **Self-whisper echo on AzerothCore.** Stock AzerothCore does deliver an addon whisper to yourself
  (`ChatHandler` finds the receiver by name, the level rule skips `receiver == sender`, `Player::Whisper` sends
  CHAT_MSG_WHISPER with LANG_ADDON to the target; `AddonChannel` must be on, the default). With `ov_leyline.lua`
  the original is swallowed and the script answers itself. Either way the page doesn't depend on it now.
- **The framed join page under COOP/COEP.** The game page is `COOP: same-origin` + `COEP: require-corp`
  (wu-play). The relay serves `/join/<id>` with `COEP: require-corp`, `CORP: cross-origin` and
  `frame-ancestors <play origins>`, which is exactly what a require-corp page needs to frame it; tested in
  headless Chromium (`stream/tests/hop-frame.browser.test.cjs`): the frame loads in hop mode, the login comes
  back by postMessage, the page stays `crossOriginIsolated`; without those two headers the frame is blocked;
  an unlisted origin can't frame it. A popup can't work (COOP severs `window.opener`), and a credentialless
  iframe isn't needed. Keep Caddy from stripping the headers (it doesn't today).

## Live test: Wrath's Dalaran -> Wrath Unleashed through the relay -> Back to Wrath

Run in the owner's browser pane, signed in to play.openvanilla.app. Deployed first: `relay`, `hangout` (its last
lines print the hangout's realm id, below `RID`), `signin`, `client`, `scripts`. Nothing here makes an account
or types a password: every login is the player's own, filled in by the page.

1. **The hangout is live.** On any play.openvanilla.app page:
   ```js
   const L = await (await fetch('https://relay.openvanilla.app/api/listing/realms.json')).json();
   const H = L.realms.find((r) => r.name === 'Wrath Unleashed');
   const RID = H.community.realmId;
   [H.state, RID, H.community.lastHeartbeat]          // 'live', the id, {online: true, players: N, bots: N}
   (await (await fetch(`https://relay.openvanilla.app/r/${RID}/status.json`)).json())   // online: true
   ```
   `https://play.openvanilla.app/wrath/realms` shows Wrath Unleashed first among community realms, with
   "Play in Dalaran".
2. **On Wrath, in the world.** Open `https://play.openvanilla.app/wrath`, pick a character, enter the world. On the
   game tab (wrath.openvanilla.app):
   ```js
   wenilla.state.session.state                         // 'inworld'
   window.__wuHop.status()                             // hop: null, sockets: [], watch.polls > 0 (the 1 s poll runs)
   ```
3. **The in-game list.** Open the Leyline window and read the list (or Esc > Mods & Workshop > Leyline realms):
   ```js
   await wenilla.lua('WrathUnleashedUI.ToggleLeyline()');
   await new Promise((r) => setTimeout(r, 3000));
   await wenilla.lua(`local t = {} for i, r in ipairs(WrathUnleashedUI.Leyline.realms) do
     t[i] = r.id .. '|' .. r.name .. '|' .. tostring(r.live) end return table.concat(t, '\n')`)
   // 'this|WrathUnleashed|true' first, then '<RID>|Wrath Unleashed|true'
   ```
4. **Hop to the hangout** (the window's own Join; clicking Join on that row is the same):
   ```js
   await wenilla.lua(`for _, r in ipairs(WrathUnleashedUI.Leyline.realms) do
     if r.name == 'Wrath Unleashed' then WrathUnleashedUI.Leyline.Join(r) return r.id end end`)
   ```
   Within about a second the tab goes to `play.openvanilla.app/wrath?go=r:<RID>` and comes back to the game at
   `wrath.openvanilla.app/?go=r:<RID>`. If it doesn't: `window.__wuHop.status().watch` shows `polls`, `taken`
   and `error`.
5. **On the hangout.** After the reload:
   ```js
   const s = window.__wuHop.status(); s.hop      // {id: RID, name: 'Wrath Unleashed', base: 'wss://relay.openvanilla.app/ws/<RID>', login: true, home: true}
   s.signedIn                                    // true: the Wrath login, no join page
   !!document.querySelector('#wu-hop-join')      // false
   document.querySelector('#wu-hop-bar').textContent   // 'On Wrath Unleashed · Leyline' + 'Back to Wrath' (no 'Use another account')
   ```
   The character list comes up by itself (same account, same characters). Enter the world, then:
   ```js
   window.__wuHop.status().sockets   // [{from: 'wss://wrath.openvanilla.app/ws/3724', to: '.../ws/<RID>/auth'},
                                     //  {from: 'wss://wrath.openvanilla.app/ws/<port>', to: '.../ws/<RID>/world'}]
   await new Promise((r) => setTimeout(r, 7000));       // the go intent fires 5 s after login
   [wenilla.state.map.id, wenilla.state.zone?.name]     // [571, 'Dalaran'] (or 'Crystalsong Forest', the zone under
                                                        // the city); chat says "[Go] Taking you to Dalaran."
   (await (await fetch(`https://relay.openvanilla.app/r/${RID}/status.json`)).json()).players   // counts you
   ```
6. **Play.** Walk, talk, open a menu: `wenilla.chat('/say hello from the Leyline hangout')`, `wenilla.chat('/who')`.
   A 15-minute idle is fine (the client pings).
7. **Back to Wrath** (the bar's button; the window's own Back says "You're on Wrath now" here, since the hangout
   is Wrath):
   ```js
   document.querySelector('#wu-hop-bar button.home').click();
   ```
   The tab goes to `play.openvanilla.app/wrath?go=h:dalaran`, then the game. Then:
   ```js
   window.__wuHop.status()          // hop: null, sockets: [] (straight to Wrath, not through the relay)
   // in the world, after ~7 s:
   [wenilla.state.map.id, wenilla.state.zone?.name]     // [571, 'Dalaran']
   ```
   The same through the poll, from the hangout: `await wenilla.lua('WUUI_LEYLINE_HOP = "wrath"')`.
8. **Checks that should fail.** `(await fetch('https://relay.openvanilla.app/api/pair/' + '0'.repeat(64))).status`
   is 404. A login with a wrong password through the relay counts against your IP for that realm only
   (10 in 10 minutes, then a 429 for 10 minutes): don't try it on the owner's IP.

A player-hosted Beacon (when one is live) tests the framed join page: Join it in step 4; the realm's join page
opens over the game; the player makes an account there (their own password, typed by them) or signs in; the page
reloads signed in to that realm (`status().hop.login: true`, the login in `localStorage` under
`wu.leyline.login.<id>`). Its world beyond the streamed slice needs "Play with my own game files".

## Next step (design only): Hero Key single sign-on for Beacons

**Built (2026-10-30), behind the `herokey_sso` flag, off by default: see [HERO-KEY-SSO.md](HERO-KEY-SSO.md).** The
build differs from the sketch below in one way: the website signs a per-realm assertion (Ed25519, site key pinned in
the host kit) carrying the per-realm pseudonym and the SRP verifier of a one-time password, instead of the browser
deriving a per-realm key from the Hero Key secret. The rest (per-realm ids, one-time SRP login, the relay passing an
opaque blob, a realm opt-in, bans per pseudonym) is as sketched. The original design, kept for the record:

The goal: hop with no account step and no password kept in the browser. A player's Hero Key identity is their
login on every Beacon that opts in.

- **Identity.** Each Hero Key holder has a key pair already (the website's sign-in). For Leyline, they derive a
  separate per-realm key, so Beacons can't link one player across realms unless the player wants that:
  `k_realm = HKDF(hero_secret, "leyline-realm" || realm_id)`.
- **Account on first hop.** The page asks the relay for a challenge from the Beacon. The page signs it with
  `k_realm` and sends `{realm_id, pubkey, sig, display name}`. The Beacon's agent checks the signature and makes
  (or finds) an account bound to that pubkey. The name is derived (for example `HK` + 10 base32 characters of
  the key's hash). No password is involved.
- **Login.** The 3.3.5a client can only do SRP. So the agent sets a fresh random SRP password on that account
  for this login only (a one-time verifier, valid 2 minutes, one use). It returns the password to the page,
  sealed to the page's ephemeral key so the relay can't read it. The page puts it in the env and the client
  logs in as now. The agent rotates the verifier after use.
- **Trust.** The relay only forwards the sealed blobs. The Beacon host sees a pubkey and a derived name, never
  the Hero Key identity, unless the player shares it.
- **Opt-in.** A Beacon setting: `signup: "herokey"` next to `invite`, `open` and `off`. Old Beacons keep the
  join page flow from the MVP.
- **Revocation.** A player can forget a realm (the page drops it). A Beacon can ban a pubkey.
- **To build:** an agent control op (`herokey_login`), a relay route that passes it through
  (`POST /api/realms/<id>/herokey`, rate-limited like accounts), a page call from `wrath-page.js` before
  `wrath-early.js` needs the login (an extra boot step), and tests with the fake Beacon.
