# Leyline Launcher without a server (launcher aggregator)

The owner shut down Leyline's only server (Hetzner) in November 2026. The website comes back as a static site at the
same address on GitHub Pages. The launcher stays, as a launcher aggregator: one place for your game folders, other
servers' realms, mods and add-ons from their makers, and the worlds that run on your own PC.

## The one switch

`SERVER_FEATURES` is `false` in two places that must agree:

| Where | What it does |
|---|---|
| `launcher/src-tauri/src/addresses.rs` | `try_both` (every request the core makes to our names) refuses, without touching the network, the browser game (`GAME_HOST`), the relay (`RELAY_HOST`), every old name, and the website's dynamic paths (`/api/`, `/auth/`, `/account…`, `/wrath/status.json`). The static site's read-only files stay. The relay tunnel (`host/litenet.rs`) is never started. |
| `launcher/ui/lite.js` | `html.lite`; every element marked `data-srv` is hidden (`lite.css`), `data-lite` elements only show here. The modules check `window.LeylineLite.lite` and skip what needed the server. |

The code that needed the server is kept behind the switch, so it could come back. The browser tests of that dormant
code turn it on for their own page only (`window.__LEYLINE_SERVER_FEATURES__ = true` in their init script); nothing
in the app sets it.

## Kept

* Realms: the static site's `/wrath/realms.json` (other servers), cached for offline, and any realm address the player
  adds. `realms::aggregate` drops the official realm and anything reached through the relay; a realm's report link,
  relay status file and join link are dropped too. PLAY points the game at the realm's own realmlist.
* Mods and add-ons: the catalog (`/wrath/mods/all.json`) and upstream downloads, cached; the Leyline add-on pack and its
  updates (GitHub releases).
* Single player and the original world (the Built-in engine), Your Den (Den Lite), Boss Rush on this PC with Your best
  times, Characters on this PC.
* Friends on the same network (LAN, `identity/peer.key`): invite links still carry the same-network way. The relay way
  says it is off.
* Game folders, looks and HD packs, themes, news (`/wrath/leyline/news.json`), modes (`/wrath/modes.json`), flags
  (`/wrath/leyline/flags.json`), the docs links, Settings, Agents (local only), Restart to update (GitHub releases).

## Removed or hidden

The sign-in (gate.js never opens; `LeylineGate.need` goes on at once), Hero Key and Google, the account chip, Manage
account and Settings' account box, the official realm everywhere (play bar default, realm lists, Home, the Characters
Online tab, back to the official realm), the browser game, Duels (it ran on the official realm), the official
leaderboards and sharing runs to them, Host (it went through the relay), invite links and Den codes through the relay,
the friends' mailbox, Den sharing on the internet, the Den market and Workshop listing, List your mod, the forum (a
Community page with the Discord instead; Reddit waits for its address, `TODO(owner)` in `lite.js` and `lib.rs`), social
and presence, bug reports to the server (Help: Copy debug info, cleaned by `bugs.rs debug_info`, and Open an issue on
GitHub), install counts, the play bar's queue card, Be a node and the Den re-share at start.

Pages that went away land elsewhere (`app.js LITE_GONE`): Forum and the old community ids open Community; Manage
account and Host open Home; the Dens tab opens Discover.

Single player's Hero Key login: `oneclick::hero_identity` is `None`, so Play uses the world's own login and
`engine::pick_login` goes in on whichever of the player's logins here holds the characters (the Hero Key logins that
played the world are in `state.json`'s `hero_accounts`), so no character is stranded.

## Start

Home is drawn before any network call: the mods list, the realm list, news, the update check and the site's Home
pictures start after it (the core's first update check waits four seconds).

## Tests

`launcher/tests/lite.browser.test.cjs` (screenshots with `LITE_SHOTS=<dir>`), and Rust unit tests in `addresses.rs`
(`the_server_is_off`), `realms.rs` (`an_aggregator_lists_other_servers_only`) and `bugs.rs` (`debug_info_is_cleaned`).
