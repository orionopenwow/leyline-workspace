# Leyline without a server

Written 26 Nov 2026. On 26 Nov the owner shut down Leyline's only server, the one Hetzner host behind Caddy. This
page covers what went with it, what still works and where it comes from now, what Leyline Control retired, and how
to bring the server's features back if there is ever a server again.

## What was shut down

Everything that ran on that one host:

- **The website portal.** This was `stream/wrath_portal.py` and its modules in `/srv/wrath`: sign-in on the website,
  Hero Key accounts, profiles, friends and messages, the forum, the hub and Workshop pages, downloads, bug reports, the
  feedback digest, Phone Lab and **Leyline HQ** (`/wrath/admin`).
- **The official realm, Wrath Unleashed.** That means its world and login servers, the bots, the realm scripts, SOAP,
  the realm image and the GM console. The Builders' Realm, the first-era realm (Vanilla Remastered) and its site went
  too.
- **The browser game.** Its client build, the streamed game files (the slice) and the Forever UI look on
  `play.leylinelauncher.com`.
- **Azeroth Survivors' site**, at survivors.leylinelauncher.com.
- **The relay**, at `relay.leylinelauncher.com`: Beacons, the mesh's first step, the Den invite tunnels and the
  friends mailbox.
- **The Discord bridge** (`/opt/wu-discord`, which linked game chat to #world-chat) and the website's Discord bot.
- **The release mirror** (our copy of the releases), the server-side kill switch helper and the server's firewall
  and Caddy settings.

The Discord server itself is still there, because Discord hosts it. GitHub is untouched.

## What still works, and from where

| What | From |
|---|---|
| **The website** | A static site on **GitHub Pages**, built from the public repo `orionopenwow/Leyline` on every push to its main. The `ship` step pushes it (through `leyline`), so **ship is the website's deploy now**. Where `leylinelauncher.com` points is the owner's call: DNS is his. |
| **The launcher** | GitHub releases on `orionopenwow/Leyline` (`launcher-latest`), built by GitHub CI after `leyline` pushes. The launcher becomes an aggregator: it lists realms and content from GitHub and community sources, not from our server. |
| **The engine, Den Lite, the add-ons, the Host pack and kit** | GitHub releases, under the names the tools already use: `engine-latest`, `solo-engine-latest`, `den-lite-latest`, `leyline-addons`, `host-pack`, `host-kit` (and `android-latest`, `forge-latest`). The `addons` and `hostkit` steps publish them. |
| **Single player, Leyline Solo, Your Den, Boss Rush** | The player's own PC (Den Lite and the engine run locally). No server is needed. |
| **Community realms** | Whoever hosts them. Players reach them directly. Our relay is gone, so anything that went through it (relay tunnels, the relay's realm list, Den invites over the relay) needs a relay someone else runs. |
| **The Discord** | Discord. Control's Discord card (preview, do it, bot settings, read, announcements) talks to Discord's own API with the bot token saved on the PC. It never used the server, so it is kept. |
| **The public repos** | GitHub: `orionopenwow/Leyline`, `orionopenwow/Leyline-Launcher` (its own Pages site `www/` stays), `Leyline-Mobile`, `leyline-solo`. |

## Leyline Control now

`tools/control/control.py` keeps every step's code and `.bat` file. The steps that needed the server are listed in
`RETIRED` (just below `STEPS`), each with a one-line reason. While the server switch is off (see the end of this
page):

- **The queue:** a retired line in `UPDATES-PENDING.txt` never runs. Launch all and Autopilot write
  `-- Skipped '<step>' (<id>): retired, the server was shut down on 26 Nov 2026: <reason>` to the page and to its log
  (`logs\<id>-<step>.log`). The line then leaves the queue (`UPDATES-DONE.txt`) and the run goes on. A skip is not a
  failure, so Autopilot doesn't stop for it. "Up next" shows such lines as "(retired)" before they run.
- **The buttons:** no button runs a retired step. The page hides Release audit, Check server, Open HQ and the GM
  console. "Open the game" becomes "Open the public repo". The Discord & GM card's buttons leave out the retired
  steps: Check server, Pull bug reports and World server log are gone, while Discord preview, Discord bot settings and
  Find new mods stay. `/api/do`, `/api/check`, `/api/audit`, `/api/open-hq` and `/api/gm-console` answer 409 with the
  reason.
- **Health:** the header no longer polls the server's names. Every 10 minutes it asks GitHub's public API (no token)
  for two things: the `launcher-latest` release (it has files) and the public repo's newest finished build on main.
- **Background jobs:** there is no 3-hourly server check and no Leyline HQ sync after each run.
- **The kill switch's step runner** refuses retired steps. The kill switch's server parts (Community message, Realms
  and network, Website) need the server's ssh, so their preview now reports problems. Untick them, or leave the kill
  switch alone: the server they pause is already gone.
- **Quick apply** (`apply-check` / `apply`) refuses manifests with `"deploy": "signin"` or `"deploy": "scripts"`. Use
  `"deploy": "none"` instead: the files are checked and logged, the next `ship` carries them, and the website rebuilds
  from that push. `"deploy": "addons"` (GitHub release) works as before.

### Retired steps (they needed the server)

| Group | Steps |
|---|---|
| Website portal | `signin`, `page`, `frontpage`, `frontpage-off`, `homeart`, `phonelab`, `feedback`, `bugs` |
| Browser game and its files | `client-build`, `client`, `client-back`, `slice`, `slice-health`, `slice-sampler`, `foreverui`, `gamedata-off`, `gamedata-on` |
| Official realm | `realm`, `scripts`, `scripts-reload`, `realm-image`, `combos`, `combos-all`, `combos-modern`, `combos-check`, `bots-off`, `bots-on`, `lean`, `sampler`, `sampler-off`, `sampler-check`, `brand-check`, `brand-leyline`, `brand-off`, `mods`, `gm` |
| Server checks and fixes | `check`, `audit`, `worldlog`, `server-fix`, `tidy`, `tidy-check`, `caddy-check`, `firewall`, `firewall-check`, `h3-off`, `h3-on`, `spike-test` |
| Relay and Beacons | `relay`, `hangout`, `hangout-off`, `vr-beacon`, `vr-beacon-off` |
| First-era realm and data | `ov112-check`, `ov112-off`, `ov112-undo`, `ov112-realm-check`, `ov112-on`, `ov112-realm-off`, `ov112-data-check`, `ov112-data-remove-check`, `ov112-data-remove`, `vr-data-check`, `vr-data`, `vr-site-check`, `vr-site`, `vr-site-off`, `vr-test` |
| Builders' Realm | `build-realm-check`, `build-realm-on`, `build-realm-off`, `build-realm-saves` |
| Azeroth Survivors' site | `sv-site-check`, `sv-site`, `sv-site-off` |
| Names and DNS checks | `domain-check`, `domain-on`, `domain-off` |
| Release mirror | `release-mirror`, `release-mirror-check` |
| Discord bridge | `discord-bridge` (it installed the game chat bridge on the server) |
| Leyline HQ | `hq-sync`, `hqnote`, `hq-inbox` |
| Tests that need the website or the official realm | `first-run-test`, `fresh-test`, `fresh-sandbox` (a test Hero Key on the website, PLAY on Wrath Unleashed), `client-smoke`, `client-smoke-world` (PLAY on Wrath Unleashed) |

The release audit can still run by hand for the parts that don't need the server:
`py -3 tools\audit\audit.py --only den,launcher`. Its `--fake` mode, which runs the site's own code on 127.0.0.1,
works anywhere too.

### Kept steps (local, or GitHub, or Discord)

| Group | Steps |
|---|---|
| Save and publish (GitHub) | `ship` → `leyline` → `leyline-launcher` (chained as before), `ship-again`, `leyline-mobile`, `leyline-solo`, `addons`, `hostkit`, `ovsite` |
| Repo history and releases (GitHub) | `fresh-check`, `fresh-history`, `fresh-leyline-check`, `fresh-leyline`, `wipe-releases`, `retag-leyline-check`, `retag-leyline`, `ghclean-check`, `ghclean`, `builds-why` |
| Den Lite and Your Den (this PC) | `den-lite`, `den-lite-test`, `den-lite-all`, `den-check`, `den-test`, `den-test-variants`, `den-test-invite`, `den-glide`, `den-glide-undo`, `den-replay`, `den-smoke`, `sky-island`, `den-move`, `den-features`, `ui-local` |
| Single player, Solo, Boss Rush (this PC) | `sp-playtest`, `sp-playtest-boss`, `solo-check`, `solo-setup`, `solo-start`, `solo-stop`, `solo-status`, `solo-signin`, `solo-signin-off`, `solo-first-run`, `solo-wasm`, `boss-rush`, `boss-rush-test`, `bots-smoke` |
| Mods discovery (approves nothing) | `mods-daily`, `addons-curate`, `thumbs` |
| Discord (Discord's own API) | `discord-preview`, `discord-apply` (only from the card), `discord-check`, `discord-read`, `discord-announce` (only from the card), `announce-check` |
| Azeroth Survivors on this PC | `sv-record`, `sv-export`, `sv-preview` (they record and preview locally; only `sv-site*` uploaded) |
| Other | `apply-check`, `apply`, `remove-files`, `fork-check`, `commons-check`, `reel-record`, `reel-record-hand`, `reel-make` |

Caveats on the kept steps:
- `discord-check` still prints the channel ids for the old website bot and game bridge. It is harmless, but those
  two are gone.
- `mods-daily`, `addons-curate` and `thumbs` end with "then ship and signin". Now ship is enough, because the site
  rebuilds on push.
- `reel-record` includes takes that open the live website and PLAY on Wrath Unleashed. Those takes will fail until
  there is a site and a realm to film. The Den and launcher takes still work.
- `den-test` invites a test friend. Under test the launcher answers without the relay tunnel, so it still runs on
  this PC.

`UPDATE.bat` (the old double-click runner, without Control) doesn't know about `RETIRED`. Don't use it for the
server's steps: they would try to reach the server and fail.

## Bringing server features back

Nothing was deleted from the repo, so the server's features can return.

1. **A server.** The retired `.bat` files reach the host as `root@play.openvanilla.app` with the key
   `%USERPROFILE%\.ssh\openwow`. Either point that name at the new host (DNS is the owner's) and put its key there,
   or change the host in the `.bat` files you bring back. Use a fresh key for a new server, never the old one.
2. **The switch.** Create `control\server.json` on the PC (it is local and holds no secrets):
   - `{"on": true}` brings every retired step back: the server's buttons, the Live checks every 30 s against the
     server's names, the 3-hourly server check and the HQ sync.
   - `{"steps": ["relay", "check"]}` brings back only those steps. Everything else stays retired, and the header keeps
     showing GitHub.
   - Setting the environment variable `LEYLINE_SERVER=1` before starting Control does the same as `"on"`.

   Control reads the file on every step and button press, with no restart. The header switches over within 10
   minutes. Delete the file, or set `"on": false`, to retire the
   steps again.
3. **The order**, for a whole server like the old one:
   1. `check` (read-only);
   2. `signin` (the portal; it ends "FASTLOOP OK");
   3. `realm-image` and `realm`, then `scripts`;
   4. `relay`;
   5. `hangout` (the realm on Leyline);
   6. `discord-bridge`;
   7. `gm`.

   The browser game needs `client-build`, `client`, then `slice` (hours). Every one keeps its own backup, self-check
   and rollback.
4. **The website:** while GitHub Pages serves it, bringing back `signin` puts the portal on the server. Decide which
   one `leylinelauncher.com` points at first (DNS, the owner's).
5. **Quick apply:** `signin` and `scripts` deploys come back with the switch (`"on"`, or their name in `"steps"`).

To retire a step for some other reason later, add it to `RETIRED` with a one-line reason. The tests in
`tools/control/test_control_retired.py` check that every name in `RETIRED` is a real step.
