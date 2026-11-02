# Moving Leyline to leylinelauncher.com

Owner, 1 Nov 2026: the domain was bought at 16:11 (registrar and DNS: Squarespace Domains).

| Part | New name (what players read) | Old name (answers forever) |
|---|---|---|
| The website (its `/` opens the Leyline front page) | `leylinelauncher.com` | `play.openvanilla.app` |
| The browser game | `play.leylinelauncher.com` | `wrath.openvanilla.app` |
| The Leyline network relay | `relay.leylinelauncher.com` | `relay.openvanilla.app` |
| `www.leylinelauncher.com` | 301 to `leylinelauncher.com` | |
| Desktop realmlist | `leylinelauncher.com` | `play.openvanilla.app` |

## Owner guide

1. **Squarespace DNS.** Domains > leylinelauncher.com > DNS > DNS Settings. Delete Squarespace's own default
   records for `@` and `www` (the "Squarespace Defaults" group, if it is there). Under "Custom records", add four
   records, all pointing at the server's IPv4 address. The check step (next) prints that address and these exact lines:

   | Type | Host | Data |
   |---|---|---|
   | A | `@` | the server's IPv4 |
   | A | `play` | the server's IPv4 |
   | A | `relay` | the server's IPv4 |
   | A | `www` | the server's IPv4 |

   AAAA only if the check step says the server has IPv6, and then only for `play` and `www`. Never for `@` or
   `relay`: desktop game clients only speak IPv4.
2. **Check.** In Control, run **domain-check** ("Domain move - check"). It changes nothing. It prints the
   server's IP, whether each of the four names already points at it, whether ports 80 and 443 reach Caddy, and the
   records above. DNS takes 10 to 30 minutes. Run it again until it says **READY**.
3. **Google and Discord sign-in.** Add the new redirect addresses. Keep the old ones.
   - Google Cloud console > APIs & Services > Credentials > the OAuth client > Authorized redirect URIs: add
     `https://leylinelauncher.com/auth/google/callback`. If "Authorized JavaScript origins" lists the old site, add
     `https://leylinelauncher.com` there too. OAuth consent screen > Authorized domains: add `leylinelauncher.com`,
     and change the home page and privacy links there to the new name.
   - Discord developer portal > the application > OAuth2 > Redirects: add
     `https://leylinelauncher.com/auth/discord/callback`. If an Interactions Endpoint URL is set, it can stay as it
     is (it keeps working) or become `https://leylinelauncher.com/wrath/forum/api/discord/interactions`.
4. **Ship today's update** (Claude does this: the sign-in, realm scripts and relay steps), so the server has the code
   that knows both names.
5. **Turn it on.** Run **domain-on** ("Domain move - on"). It backs everything up, adds the new names (Caddy gets
   their certificates by itself), checks each new name over HTTPS, points the website's own sign-in setting, the play
   pages, the realm scripts and the relay at the new names, and only then sends page loads on the old names to the
   new ones (301, path and query kept). Anything fails: everything is put back by itself. If it ends with
   **DOMAIN WAITING**, the new names work but the website's Google sign-in still comes back to the old name; it says
   which setting to fix, then run domain-on again.
6. **Test.** Open `https://leylinelauncher.com`: the front page. Sign in once (sign-ins are per name). Press Play: the
   game opens on `play.leylinelauncher.com`. Open an old link such as `https://play.openvanilla.app/wrath`: it lands on
   the new name. A desktop client with `set realmlist leylinelauncher.com` logs in; one with the old realmlist still does.
7. **Then the rest.** Run the Forever UI, addons and mods steps (the in-game texts name the new site), and release the
   launcher ("Ship the Leyline launcher"). The launcher's update check (GitHub) and its `leyline://` links do not
   change. Launchers already installed keep working on the old names.
8. **By hand, outside the repo:**
   - The Discord server: the welcome and rules texts, pinned posts, channel topics and the server's website link
     (the new texts are in `docs/discord-initial-posts.md` and `stream/discord_template_v2.json`).
   - GitHub: the homepage link and description of `orionopenwow/Leyline` (and `orionopenwow/Leyline-Launcher`):
     Settings > General, or `gh repo edit orionopenwow/Leyline --homepage https://leylinelauncher.com`. There is no
     Control step for repo metadata yet; Claude can add one if you want it.
   - `openvanilla.app` itself (GitHub Pages, the Leyline-Launcher repo's `www/`): optional, its home could send
     visitors to `https://leylinelauncher.com`.
   - Posts already published (Reddit, forums) and anything else that links the old names: they keep working.
   - Squarespace: turn on auto-renew and the domain lock.

Undo at any time: **domain-off** ("Domain move - off"). Every name goes back as it was; the new names stop answering
(the DNS records can stay).

## How it works

- **One list.** `tools/domain/addresses.json` holds the three parts' new and old names. Each component has one copy
  and `tools/domain/tests/test_domain.py` checks them against it: the website (`stream/wrath_portal.py` `ADDRESSES`),
  the launcher (`launcher/src-tauri/src/addresses.rs`, `site!` / `relay!` build every address; `launcher/ui/addresses.js`),
  the host pack and relay (`hosting/kit/tools/wunet/leyline.env`, `domains.py`), the game page
  (`stream/www-files/*.js`), the realm scripts (`ov_modapi.lua` `OVMod.Names`).
- **Caddy** (`/opt/openvanilla-classicplus/Caddyfile`, not in the repo; `tools/domain/domain_caddy.py`). The new
  names are added to the existing site blocks (`play.openvanilla.app, leylinelauncher.com {`), so both names reach the
  same backends and every other script still finds its block; `www` gets its own small block. The 301 is only for
  page loads (GET/HEAD with `Sec-Fetch-Dest: document`, or an old browser asking for text/html), never for what
  launchers, host packs, the game page, iframes or agents call. Never moved even as a page load: the site's
  `/auth/*` (an OAuth round trip finishes on the name it started on), the game's `/__gate`, `/__env.js`, `/__auth`,
  `/data`, `/ws`, the relay's `/agent`, `/r/*`, `/api/*`, `/ws/*`, `/healthz`. `stream-up.sh` and `relay-up.sh` put
  the new names back after they rewrite their blocks (`domain_caddy.py reapply`, from `/srv/wrath/domain/`).
- **The website** (`LEYLINE_DOMAIN_MOVE=1` in portal.env, set by domain-on): pages, links and redirects say the new
  names. Before it, any new name in a page or document is shown as the old one, so the code and the docs can ship
  first. After it, a launcher document fetched on the old site name (the catalog, the Host pack listing) keeps its
  links on the old name: installed launchers only download from the name they fetched from. Origins, the game's
  frame and the game's calls accept both names.
- **Sign-in.** Cookies are per name (none sets a Domain), so players sign in once more on the new name. domain-on
  points the website's own address settings in `/opt/wenilla-realm/.env` at the new name (lists get the new name
  added), restarts its realm service, and checks where Google sign-in comes back to before it turns on the 301. Hero
  Keys are checked by key, not by issuer name, so keys made on either name work. Tested end to end, from an old link
  through the 301 to a signed-in game page, with no loop (`stream/tests/test_domain_move.py`).
- **The launcher** calls the new names first and, when one does not answer at all (no DNS yet), the old one
  (`addresses::try_both`). The official realmlist is `leylinelauncher.com` when the computer can resolve it, else the
  old name; a saved old name still counts as the official realm. Downloads count from the site on either name.
- **The host pack** connects to `wss://relay.leylinelauncher.com/agent` and falls back to the old name; hosts set up
  before keep the relay address they saved (the old name), which answers forever. The relay gives out its new name
  once domain-on restarts it, and always accepts the site and the game on both names.
- **Not changed:** SSH (`root@play.openvanilla.app`), server-side self-checks and fetches on our own box, CI, the
  signed release log's URLs, the release mirror, the GM console's defaults, the Builders' Realm and the 1.12 stack.

## Inventory

Every reference to `openvanilla.app` and its subnames in the repo before the move (commit d0f96dc), with what
happened to it. "Old names now" counts what is left in the file (kept on purpose: fallbacks, both-name lists,
fixtures, server config). Bare `openvanilla.app` is the GitHub Pages site, which stays as it is.

| Class | Files | References | Rule |
|---|---:|---:|---|
| Player-facing text or link | 39 | 146 | the new name |
| Player-facing, chosen at run time (realm scripts: the domain flag; the game page: the name it was opened on) | 10 | 16 | old name until the move is on, then the new one |
| Runtime endpoint of installed things (launcher, host pack, join page) | 48 | 211 | the new name first, the old one as fallback (old installs keep calling the old name) |
| Security-sensitive (origins, CSP, allow-lists, OAuth, signed-link hosts, TLS/site names) | 27 | 84 | both names accepted |
| Server-side, CI or ops endpoint (our own box, scripts, workflows) | 66 | 191 | kept on the old name (it answers forever); Caddy blocks keep it as their first name |
| Operator and design docs about the live server | 13 | 90 | kept: they name the live Caddy blocks, env files and self-checks, which keep the old names as their first name |
| SSH name of the server | 59 | 83 | kept: `root@play.openvanilla.app` reaches the same box before and after |
| Test or fixture | 81 | 391 | kept as fixtures (old names must keep working); updated where a test checks a new default |
| History, smoke log or report | 10 | 35 | unchanged (what happened then) |
| **All** | **353** | **1247** | |

<details><summary>Every file (353)</summary>

| File | Refs before | Old names now | Class |
|---|---:|---:|---|
| `.github/workflows/hub-index.yml` | 1 | 1 | runtime: kept |
| `UPDATES-PENDING.txt` | 4 | 4 | record |
| `client/patches/wotlk-web-port-wip.patch` | 1 | 1 | runtime: kept |
| `docs/ADMIN-CONSOLE.md` | 3 | 3 | ops doc: kept |
| `docs/FORUM.md` | 3 | 0 | text: new |
| `docs/GAME-DATA-ON-OUR-SERVER.md` | 17 | 17 | ops doc: kept |
| `docs/HERO-KEY-SSO.md` | 2 | 0 | both |
| `docs/HOST-PACK.md` | 1 | 1 | both |
| `docs/HYBRID-CLIENT.md` | 1 | 0 | text: new |
| `docs/LEYLINE-MESH.md` | 3 | 3 | ops doc: kept |
| `docs/P2P-AND-TORRENT.md` | 1 | 0 | text: new |
| `docs/RESILIENCE.md` | 2 | 2 | ops doc: kept |
| `docs/SECURITY-HARDENING.md` | 3 | 3 | ops doc: kept |
| `docs/SIGN-IN.md` | 1 | 1 | both |
| `docs/SOCIAL.md` | 1 | 0 | text: new |
| `docs/THEMES.md` | 1 | 1 | both |
| `docs/VANILLA-REMASTERED.md` | 10 | 10 | ops doc: kept |
| `docs/WORDS.md` | 1 | 0 | text: new |
| `docs/WORKSHOP.md` | 1 | 0 | text: new |
| `docs/discord-initial-posts.md` | 9 | 0 | text: new |
| `docs/history/LAUNCH-TONIGHT.md` | 6 | 6 | record |
| `docs/leyline-modes-play.md` | 6 | 0 | text: new |
| `docs/leyline-realm-hopping.md` | 16 | 16 | ops doc: kept |
| `docs/leyline/commands.md` | 2 | 0 | text: new |
| `docs/leyline/cross-server.md` | 2 | 0 | text: new |
| `docs/leyline/getting-started.md` | 5 | 0 | text: new |
| `docs/leyline/hero-key.md` | 3 | 0 | text: new |
| `docs/leyline/hosting.md` | 6 | 0 | text: new |
| `docs/leyline/index.md` | 2 | 0 | text: new |
| `docs/leyline/launcher.md` | 6 | 0 | text: new |
| `docs/leyline/network.md` | 1 | 0 | text: new |
| `docs/leyline/play.md` | 12 | 0 | text: new |
| `docs/leyline/publishing.md` | 3 | 0 | text: new |
| `docs/posts/2026-11-launch/README.md` | 7 | 0 | text: new |
| `docs/posts/2026-11-launch/forum-hosts-wanted.md` | 2 | 0 | text: new |
| `docs/posts/2026-11-launch/forum-welcome.md` | 3 | 0 | text: new |
| `docs/posts/2026-11-launch/reddit-hosts.md` | 2 | 0 | text: new |
| `docs/posts/2026-11-launch/reddit-r-wowservers.md` | 1 | 0 | text: new |
| `docs/smoke/2026-11-01-den-host.md` | 3 | 3 | record |
| `gmconsole/Install GM Console.bat` | 2 | 2 | ssh: kept |
| `gmconsole/Open GM Console.bat` | 3 | 3 | runtime: kept |
| `gmconsole/README.md` | 3 | 3 | ops doc: kept |
| `gmconsole/Uninstall GM Console.bat` | 2 | 2 | ssh: kept |
| `gmconsole/server/gmconsole.py` | 4 | 4 | runtime: kept |
| `hosting/INSTALL-OPERATE.md` | 15 | 15 | ops doc: kept |
| `hosting/README.md` | 6 | 2 | text: new |
| `hosting/STATUS-2026-10-29.md` | 10 | 10 | record |
| `hosting/build-realm/Builders realm - check.bat` | 2 | 2 | ssh: kept |
| `hosting/build-realm/Builders realm - off.bat` | 2 | 2 | ssh: kept |
| `hosting/build-realm/Builders realm - on.bat` | 2 | 2 | ssh: kept |
| `hosting/build-realm/Builders realm - saves.bat` | 2 | 2 | ssh: kept |
| `hosting/build-realm/build-realm.sh` | 1 | 1 | runtime: kept |
| `hosting/discord/Deploy Discord bridge.bat` | 1 | 1 | ssh: kept |
| `hosting/discord/deploy/discord.env.example` | 1 | 1 | runtime: kept |
| `hosting/discord/wudiscord/bridge.py` | 3 | 3 | runtime: kept |
| `hosting/hangout/Deploy Dalaran hangout.bat` | 1 | 1 | ssh: kept |
| `hosting/hangout/hangout-up.sh` | 1 | 1 | runtime: kept |
| `hosting/kit/Host a Wrath realm.bat` | 2 | 0 | fallback |
| `hosting/kit/Link my server.bat` | 2 | 0 | fallback |
| `hosting/kit/Realm tools.bat` | 2 | 0 | fallback |
| `hosting/kit/host-wrath-realm.sh` | 1 | 0 | fallback |
| `hosting/kit/join-network.sh` | 3 | 1 | fallback |
| `hosting/kit/leyline` | 2 | 0 | fallback |
| `hosting/kit/profiles/Host a profile realm.bat` | 1 | 0 | fallback |
| `hosting/kit/tools/wuhost/setup.py` | 2 | 0 | fallback |
| `hosting/kit/tools/wunet/domains.py` | 1 | 5 | fallback |
| `hosting/kit/tools/wunet/herokey.py` | 1 | 0 | both |
| `hosting/kit/tools/wunet/leyline.env` | 1 | 3 | fallback |
| `hosting/mesh-sim/meshsim.py` | 3 | 3 | runtime: kept |
| `hosting/relay/Deploy relay.bat` | 4 | 4 | runtime: kept |
| `hosting/relay/deploy/Caddyfile` | 1 | 1 | both |
| `hosting/relay/deploy/compose.box.yml` | 2 | 2 | runtime: kept |
| `hosting/relay/deploy/deploy-relay.sh` | 1 | 1 | runtime: kept |
| `hosting/relay/deploy/portcheck.py` | 4 | 4 | runtime: kept |
| `hosting/relay/deploy/relay-up.sh` | 7 | 7 | runtime: kept |
| `hosting/relay/deploy/relay.env.example` | 7 | 5 | both |
| `hosting/relay/wurelay/relay.py` | 1 | 0 | both |
| `hosting/relay/wurelay/www/join.html` | 1 | 1 | fallback |
| `hosting/server/set-realm-address.sh` | 1 | 1 | runtime: kept |
| `hosting/site/404-join.js` | 1 | 1 | ops doc: kept |
| `hosting/site/SCHEMA-community.md` | 4 | 4 | ops doc: kept |
| `hosting/site/join/index.html` | 1 | 1 | fallback |
| `hosting/tests/e2e/e2e_client.py` | 1 | 1 | test |
| `hosting/tests/e2e/run-e2e.sh` | 1 | 1 | test |
| `hosting/tests/soak/SOAK-PLAN.md` | 1 | 1 | test |
| `hosting/tests/soak/soak.py` | 1 | 1 | test |
| `hosting/tests/test_build_realm.py` | 3 | 3 | test |
| `hosting/tests/test_discord_bridge.py` | 6 | 6 | test |
| `hosting/tests/test_hangout_deploy.py` | 2 | 2 | test |
| `hosting/tests/test_herokey_sso.py` | 2 | 2 | test |
| `hosting/tests/test_host_windows_e2e.py` | 1 | 1 | test |
| `hosting/tests/test_leyline_e2e.py` | 9 | 10 | test |
| `hosting/tests/test_relay_deploy.py` | 6 | 5 | test |
| `hosting/tests/test_release_log.py` | 4 | 4 | test |
| `hosting/tests/test_setup.py` | 2 | 0 | test |
| `hosting/vanilla-remastered/Put Vanilla Remastered on Leyline.bat` | 1 | 1 | ssh: kept |
| `hosting/vanilla-remastered/Vanilla Remastered browser page - check.bat` | 1 | 1 | ssh: kept |
| `hosting/vanilla-remastered/Vanilla Remastered browser page - off.bat` | 1 | 1 | ssh: kept |
| `hosting/vanilla-remastered/Vanilla Remastered browser page.bat` | 2 | 2 | ssh: kept |
| `hosting/vanilla-remastered/Vanilla Remastered self-test.bat` | 1 | 1 | ssh: kept |
| `hosting/vanilla-remastered/tests/test_vanilla_remastered.py` | 4 | 4 | test |
| `hosting/vanilla-remastered/tests/test_vr_data_site.py` | 15 | 15 | test |
| `hosting/vanilla-remastered/vr-beacon-up.sh` | 1 | 1 | runtime: kept |
| `hosting/vanilla-remastered/vr-data.ps1` | 1 | 1 | runtime: kept |
| `hosting/vanilla-remastered/vr-site-up.sh` | 9 | 9 | runtime: kept |
| `labs/done/ov-rename/leyline_rename.py` | 1 | 1 | record |
| `labs/ov-112-logins/Carry over 1.12 logins.bat` | 1 | 1 | record |
| `labs/ov-112-logins/carry-112-logins.sh` | 1 | 1 | record |
| `labs/players/Play WrathUnleashed.bat` | 1 | 1 | record |
| `labs/players/README.txt` | 2 | 2 | record |
| `launcher/README.md` | 3 | 0 | text: new |
| `launcher/branding.json` | 5 | 3 | fallback |
| `launcher/src-tauri/glue/LeylineHybrid.lua` | 1 | 0 | fallback |
| `launcher/src-tauri/src/agents.rs` | 2 | 2 | fallback |
| `launcher/src-tauri/src/bugs.rs` | 1 | 0 | fallback |
| `launcher/src-tauri/src/catalog.rs` | 11 | 9 | both |
| `launcher/src-tauri/src/clients.rs` | 17 | 17 | fallback |
| `launcher/src-tauri/src/forum.rs` | 5 | 0 | fallback |
| `launcher/src-tauri/src/herokey.rs` | 3 | 3 | fallback |
| `launcher/src-tauri/src/host.rs` | 8 | 7 | fallback |
| `launcher/src-tauri/src/host/console.rs` | 3 | 3 | fallback |
| `launcher/src-tauri/src/host/lite.rs` | 1 | 1 | fallback |
| `launcher/src-tauri/src/host/litenet.rs` | 2 | 2 | fallback |
| `launcher/src-tauri/src/host/pack.rs` | 3 | 3 | both |
| `launcher/src-tauri/src/lib.rs` | 35 | 1 | fallback |
| `launcher/src-tauri/src/listed.rs` | 1 | 1 | fallback |
| `launcher/src-tauri/src/listing.rs` | 2 | 0 | fallback |
| `launcher/src-tauri/src/looks.rs` | 1 | 1 | fallback |
| `launcher/src-tauri/src/modes.rs` | 7 | 0 | fallback |
| `launcher/src-tauri/src/mods.rs` | 1 | 1 | fallback |
| `launcher/src-tauri/src/news.rs` | 3 | 0 | fallback |
| `launcher/src-tauri/src/p2p/flags.rs` | 2 | 0 | fallback |
| `launcher/src-tauri/src/p2p/magnet.rs` | 7 | 7 | both |
| `launcher/src-tauri/src/p2p/mesh-vector.json` | 4 | 4 | fallback |
| `launcher/src-tauri/src/p2p/mesh.rs` | 18 | 18 | fallback |
| `launcher/src-tauri/src/p2p/torrent.rs` | 1 | 1 | fallback |
| `launcher/src-tauri/src/rankings.rs` | 2 | 1 | fallback |
| `launcher/src-tauri/src/realms.rs` | 43 | 38 | fallback |
| `launcher/src-tauri/src/release-log-vector.json` | 2 | 2 | fallback |
| `launcher/src-tauri/src/settings.rs` | 1 | 1 | fallback |
| `launcher/src-tauri/src/social.rs` | 6 | 4 | both |
| `launcher/src-tauri/src/status.rs` | 3 | 2 | fallback |
| `launcher/src-tauri/src/themes.rs` | 3 | 3 | fallback |
| `launcher/src-tauri/src/upstream.rs` | 3 | 3 | both |
| `launcher/src-tauri/tauri.conf.json` | 2 | 2 | both |
| `launcher/tests/account.browser.test.cjs` | 11 | 11 | test |
| `launcher/tests/addons.browser.test.cjs` | 5 | 5 | test |
| `launcher/tests/agents.browser.test.cjs` | 3 | 3 | test |
| `launcher/tests/avatars.test.cjs` | 2 | 2 | test |
| `launcher/tests/bardrop.browser.test.cjs` | 5 | 5 | test |
| `launcher/tests/bugreport.browser.test.cjs` | 3 | 3 | test |
| `launcher/tests/clientdrop.browser.test.cjs` | 5 | 5 | test |
| `launcher/tests/copy.browser.test.cjs` | 6 | 6 | test |
| `launcher/tests/den.browser.test.cjs` | 11 | 11 | test |
| `launcher/tests/forum.browser.test.cjs` | 9 | 9 | test |
| `launcher/tests/gamemods.browser.test.cjs` | 8 | 8 | test |
| `launcher/tests/graphics.browser.test.cjs` | 3 | 3 | test |
| `launcher/tests/guide.browser.test.cjs` | 5 | 5 | test |
| `launcher/tests/home.browser.test.cjs` | 20 | 3 | test |
| `launcher/tests/hostconsole.browser.test.cjs` | 11 | 11 | test |
| `launcher/tests/hostlink.browser.test.cjs` | 3 | 3 | test |
| `launcher/tests/hostmodes.browser.test.cjs` | 10 | 5 | test |
| `launcher/tests/hostnetwork.browser.test.cjs` | 5 | 5 | test |
| `launcher/tests/listmod.browser.test.cjs` | 4 | 4 | test |
| `launcher/tests/looks.browser.test.cjs` | 4 | 4 | test |
| `launcher/tests/market.browser.test.cjs` | 4 | 4 | test |
| `launcher/tests/modes.browser.test.cjs` | 8 | 8 | test |
| `launcher/tests/mods.browser.test.cjs` | 10 | 1 | test |
| `launcher/tests/nav.browser.test.cjs` | 5 | 5 | test |
| `launcher/tests/playbar.browser.test.cjs` | 25 | 25 | test |
| `launcher/tests/realmlink.browser.test.cjs` | 25 | 27 | test |
| `launcher/tests/realms.browser.test.cjs` | 10 | 10 | test |
| `launcher/tests/simple.browser.test.cjs` | 4 | 4 | test |
| `launcher/tests/social.browser.test.cjs` | 4 | 4 | test |
| `launcher/tests/solo.browser.test.cjs` | 14 | 14 | test |
| `launcher/tests/spin.browser.test.cjs` | 3 | 3 | test |
| `launcher/tests/themes.browser.test.cjs` | 3 | 3 | test |
| `launcher/ui/account.js` | 1 | 0 | fallback |
| `launcher/ui/app.js` | 5 | 1 | fallback |
| `launcher/ui/home.js` | 1 | 0 | fallback |
| `launcher/ui/hostmodes.js` | 1 | 0 | fallback |
| `launcher/ui/index.html` | 3 | 0 | fallback |
| `launcher/ui/modes.js` | 2 | 0 | fallback |
| `launcher/ui/realmlink.js` | 1 | 0 | both |
| `launcher/ui/simple.js` | 1 | 0 | fallback |
| `launcher/ui/social.js` | 1 | 0 | both |
| `launcher/ui/style.css` | 1 | 0 | fallback |
| `mods/Put mods on Wrath.bat` | 3 | 2 | runtime: kept |
| `mods/README-modders.md` | 1 | 0 | text: new |
| `mods/all-json.md` | 4 | 0 | text: new |
| `realm/Bots - back on.bat` | 1 | 1 | ssh: kept |
| `realm/Bots - none.bat` | 1 | 1 | ssh: kept |
| `realm/Diagnose character create.bat` | 2 | 2 | ssh: kept |
| `realm/Fix character create.bat` | 2 | 2 | ssh: kept |
| `realm/Lean server.bat` | 1 | 1 | ssh: kept |
| `realm/Leyline brand.bat` | 1 | 1 | ssh: kept |
| `realm/Rollback character fix.bat` | 1 | 1 | ssh: kept |
| `realm/Sample hangout.bat` | 1 | 1 | ssh: kept |
| `realm/Server check.bat` | 1 | 1 | ssh: kept |
| `realm/Server pass - finder 10 and bug reports.bat` | 4 | 4 | runtime: kept |
| `realm/Set levelling rates.bat` | 1 | 1 | ssh: kept |
| `realm/Stop slow-load kicks.bat` | 1 | 1 | ssh: kept |
| `realm/Update realm scripts.bat` | 1 | 1 | ssh: kept |
| `realm/custom/leyline-fetch.py` | 6 | 6 | runtime: kept |
| `realm/custom/lua_scripts/ov_demo.lua` | 1 | 1 | switched |
| `realm/custom/lua_scripts/ov_duels.lua` | 1 | 1 | switched |
| `realm/custom/lua_scripts/ov_go.lua` | 2 | 2 | switched |
| `realm/custom/lua_scripts/ov_leyline.lua` | 1 | 1 | switched |
| `realm/custom/lua_scripts/ov_playmode.lua` | 1 | 1 | switched |
| `realm/custom/lua_scripts/ov_workshop.lua` | 1 | 1 | switched |
| `realm/serverpass.sh` | 3 | 3 | runtime: kept |
| `server/Check my server.bat` | 2 | 2 | ssh: kept |
| `server/Check native clients.bat` | 2 | 2 | runtime: kept |
| `server/Deploy Wrath to my server.bat` | 6 | 6 | runtime: kept |
| `server/Fix bots and finish deploy.bat` | 3 | 3 | runtime: kept |
| `server/Make GM.bat` | 2 | 2 | ssh: kept |
| `server/Make tester accounts.bat` | 2 | 2 | runtime: kept |
| `server/Rollback to vanilla.bat` | 2 | 2 | ssh: kept |
| `server/Speed up downloads.bat` | 1 | 1 | ssh: kept |
| `server/Update sign-in page.bat` | 3 | 3 | runtime: kept |
| `server/deploy.sh` | 2 | 2 | runtime: kept |
| `server/native-client-report.txt` | 6 | 6 | record |
| `server/nettune.sh` | 2 | 2 | runtime: kept |
| `server/portal/wrath_portal.py` | 8 | 8 | runtime: kept |
| `setup/Set up automatic deploys.bat` | 3 | 3 | ssh: kept |
| `stream/Add Forever UI to the stream.bat` | 3 | 3 | runtime: kept |
| `stream/Game data killswitch OFF.bat` | 1 | 1 | ssh: kept |
| `stream/Game data killswitch ON.bat` | 3 | 3 | runtime: kept |
| `stream/Make mini slice.bat` | 3 | 3 | runtime: kept |
| `stream/README-foreverui-stream.md` | 1 | 1 | ops doc: kept |
| `stream/README-stream.md` | 12 | 12 | ops doc: kept |
| `stream/Record a capture.bat` | 3 | 3 | runtime: kept |
| `stream/Remove Forever UI from the stream.bat` | 1 | 1 | ssh: kept |
| `stream/Roll back browser client.bat` | 1 | 1 | ssh: kept |
| `stream/Size report.bat` | 1 | 1 | ssh: kept |
| `stream/Slice health - check.bat` | 1 | 1 | ssh: kept |
| `stream/Turn off the Goldshire guest pass.bat` | 1 | 1 | ssh: kept |
| `stream/Turn on the Goldshire guest pass.bat` | 2 | 2 | ssh: kept |
| `stream/Update browser client.bat` | 3 | 3 | runtime: kept |
| `stream/Update game page.bat` | 2 | 2 | ssh: kept |
| `stream/Update sign-in and warm-up.bat` | 3 | 3 | runtime: kept |
| `stream/Upload a mini slice.bat` | 1 | 1 | ssh: kept |
| `stream/admin.py` | 1 | 1 | both |
| `stream/demo.py` | 1 | 1 | runtime: kept |
| `stream/discord_growth.py` | 1 | 0 | runtime: kept |
| `stream/discord_template_v2.json` | 2 | 0 | text: new |
| `stream/docs.py` | 2 | 0 | runtime: kept |
| `stream/fastloop.sh` | 18 | 18 | runtime: kept |
| `stream/forum.py` | 1 | 1 | both |
| `stream/forum_discord.py` | 1 | 1 | runtime: kept |
| `stream/fui-web/Interface/AddOns/WrathUnleashedUI/GameFolder.lua` | 1 | 0 | text: new |
| `stream/fui-web/Interface/AddOns/WrathUnleashedUI/Host.lua` | 1 | 0 | text: new |
| `stream/fui-web/Interface/AddOns/WrathUnleashedUI/Leyline.lua` | 1 | 0 | text: new |
| `stream/fui-web/Interface/AddOns/WrathUnleashedUI/ModsPanel.lua` | 1 | 0 | text: new |
| `stream/fui-web/Interface/AddOns/WrathUnleashedUI/Workshop.lua` | 1 | 0 | text: new |
| `stream/herokeys.py` | 1 | 1 | both |
| `stream/hostflow.py` | 1 | 0 | both |
| `stream/hq.py` | 1 | 0 | runtime: kept |
| `stream/hub.py` | 3 | 2 | runtime: kept |
| `stream/hub_rank.py` | 2 | 2 | both |
| `stream/leyline_net.py` | 2 | 1 | both |
| `stream/lua/ov_slices.lua` | 2 | 2 | switched |
| `stream/modes.py` | 2 | 2 | runtime: kept |
| `stream/network.py` | 2 | 1 | both |
| `stream/playmodes.py` | 1 | 0 | runtime: kept |
| `stream/profiles.py` | 1 | 1 | both |
| `stream/server/gamedata-caddy.py` | 4 | 5 | both |
| `stream/server/gamedata-switch.sh` | 2 | 2 | runtime: kept |
| `stream/server/stream-lib.sh` | 2 | 2 | runtime: kept |
| `stream/server/stream-up.sh` | 3 | 3 | runtime: kept |
| `stream/tests/browse.browser.test.cjs` | 2 | 2 | test |
| `stream/tests/browse.page.test.cjs` | 2 | 2 | test |
| `stream/tests/hop-frame.browser.test.cjs` | 1 | 1 | test |
| `stream/tests/hop.page.test.cjs` | 5 | 5 | test |
| `stream/tests/host.page.test.cjs` | 3 | 3 | test |
| `stream/tests/ownfiles-layer.browser.test.cjs` | 1 | 1 | test |
| `stream/tests/test_brand.py` | 2 | 2 | test |
| `stream/tests/test_bugreport.py` | 1 | 1 | test |
| `stream/tests/test_demo.py` | 3 | 3 | test |
| `stream/tests/test_gamedata.py` | 8 | 8 | test |
| `stream/tests/test_gamemods.py` | 1 | 1 | test |
| `stream/tests/test_hop.py` | 3 | 3 | test |
| `stream/tests/test_hostflow.py` | 3 | 3 | test |
| `stream/tests/test_hub.py` | 2 | 2 | test |
| `stream/tests/test_layer.py` | 1 | 1 | test |
| `stream/tests/test_network.py` | 1 | 1 | test |
| `stream/tests/test_ov_demo.lua` | 2 | 2 | test |
| `stream/tests/test_ov_duels.lua` | 1 | 1 | test |
| `stream/tests/test_ov_slices.lua` | 2 | 2 | test |
| `stream/tests/test_ov_workshop.lua` | 1 | 1 | test |
| `stream/tests/test_playmodes.py` | 1 | 0 | test |
| `stream/tests/test_realms.py` | 2 | 2 | test |
| `stream/tests/test_server_scripts.sh` | 7 | 7 | test |
| `stream/tests/test_social.py` | 1 | 1 | test |
| `stream/tests/test_stream.py` | 3 | 3 | test |
| `stream/tests/test_upstream_addons.py` | 1 | 1 | test |
| `stream/tests/test_workshop.py` | 1 | 1 | test |
| `stream/tests/test_wuui_gamefolder.lua` | 1 | 0 | test |
| `stream/tests/test_wuui_host.lua` | 3 | 0 | test |
| `stream/tests/test_wuui_leyline.lua` | 9 | 0 | test |
| `stream/tests/test_wuui_workshop.lua` | 1 | 0 | test |
| `stream/workshop.py` | 2 | 1 | runtime: kept |
| `stream/wrath_portal.py` | 15 | 5 | both |
| `stream/www-files/ownfiles.js` | 2 | 3 | switched |
| `stream/www-files/wrath-page.js` | 4 | 4 | switched |
| `stream/www-files/wu-loading.js` | 1 | 3 | switched |
| `tools/art/launcher-shots.cjs` | 8 | 0 | text: new |
| `tools/art/leyline-ui-shots.cjs` | 11 | 0 | text: new |
| `tools/art/wu-card.html` | 1 | 0 | text: new |
| `tools/bugs/Pull bug manifest.bat` | 1 | 1 | ssh: kept |
| `tools/control/control.py` | 9 | 7 | runtime: kept |
| `tools/diag/World server log.bat` | 1 | 1 | ssh: kept |
| `tools/discord-leyline/discord_leyline.py` | 2 | 0 | text: new |
| `tools/front-page/Front page as home - off.bat` | 1 | 1 | ssh: kept |
| `tools/front-page/Front page as home - on.bat` | 1 | 1 | ssh: kept |
| `tools/front-page/front-page.sh` | 3 | 3 | runtime: kept |
| `tools/github-workflows/hub-index.yml` | 1 | 1 | runtime: kept |
| `tools/github-workflows/realm-scripts.yml` | 1 | 1 | runtime: kept |
| `tools/hq/HQ sync.bat` | 1 | 1 | ssh: kept |
| `tools/launcher-site/Land Leyline-Launcher site.bat` | 3 | 3 | runtime: kept |
| `tools/leyline-repo/hostkit.py` | 1 | 1 | runtime: kept |
| `tools/ov112-data/Old 1.12 data - check.bat` | 1 | 1 | ssh: kept |
| `tools/ov112-data/Old 1.12 data - remove (dry run).bat` | 1 | 1 | ssh: kept |
| `tools/ov112-data/Old 1.12 data - remove.bat` | 1 | 1 | ssh: kept |
| `tools/ov112-data/data-check.sh` | 4 | 4 | runtime: kept |
| `tools/ov112-data/data-remove.sh` | 2 | 2 | runtime: kept |
| `tools/ov112-off/Turn 1.12 jobs back on.bat` | 2 | 2 | ssh: kept |
| `tools/ov112-off/Turn off 1.12 jobs - do it.bat` | 2 | 2 | ssh: kept |
| `tools/ov112-off/Turn off 1.12 jobs - dry run.bat` | 2 | 2 | ssh: kept |
| `tools/ov112-off/Turn off the 1.12 realm.bat` | 1 | 1 | ssh: kept |
| `tools/ov112-off/Turn on the 1.12 realm - check.bat` | 2 | 2 | ssh: kept |
| `tools/ov112-off/Turn on the 1.12 realm.bat` | 1 | 1 | ssh: kept |
| `tools/ov112-off/start-112-realm.sh` | 5 | 5 | runtime: kept |
| `tools/ov112-off/stop-112-jobs.sh` | 1 | 1 | runtime: kept |
| `tools/release-mirror/Mirror releases.bat` | 1 | 1 | runtime: kept |
| `tools/release-mirror/install-release-mirror.sh` | 2 | 2 | runtime: kept |
| `tools/release-mirror/release_mirror.py` | 2 | 2 | runtime: kept |
| `tools/release-mirror/test_release_mirror.py` | 1 | 1 | test |
| `tools/security/CI key - restrict (draft).bat` | 2 | 2 | ssh: kept |
| `tools/security/Caddy check.bat` | 2 | 2 | ssh: kept |
| `tools/security/Firewall limits - apply.bat` | 2 | 2 | ssh: kept |
| `tools/security/Firewall limits - dry run.bat` | 2 | 2 | ssh: kept |
| `tools/security/caddy-apply.sh` | 1 | 1 | runtime: kept |
| `tools/security/caddyfile_harden.py` | 5 | 5 | both |
| `tools/security/test_security.py` | 8 | 8 | test |
| `tools/server-tidy/Server tidy - dry run.bat` | 1 | 1 | ssh: kept |
| `tools/server-tidy/Server tidy.bat` | 1 | 1 | ssh: kept |
| `tools/site-art/Publish home screenshots.bat` | 3 | 3 | runtime: kept |
| `tools/site-art/install-home-art.sh` | 2 | 2 | runtime: kept |
| `tools/site-art/publish_home_art.py` | 1 | 1 | runtime: kept |
| `tools/site-art/test_publish_home_art.py` | 1 | 1 | test |
| `tools/video/assemble.py` | 2 | 0 | text: new |
| `tools/video/lib.cjs` | 21 | 0 | text: new |

</details>
