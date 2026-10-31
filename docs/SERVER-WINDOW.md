# One server window

Spec for the next phase: one "Server" window for every place a realm is run from, with one set of tabs, one action
catalog and one look. Not built yet. Workstream 6 of the 31 Oct research; the owner's calls of 31 Oct are in.

## Owner decisions (31 Oct)

- **A ban covers the whole account, everywhere.** Player realms ban the character's account (AzerothCore
  `.ban account`), like HQ. One button name everywhere: **Ban account** / **Unban account**, with a reason and how long
  (1 to 8760 hours, or 0 for permanent). Built: `ov_hostctl.lua`, the launcher's host console, the in-game Your realm
  window, HQ.
- **The host window, the GM window and everything like them look alike, from one place.** Built: the tokens below.

## Where it shows

| Surface | For | Code today |
|---|---|---|
| Launcher host console | the host's own realm on this PC | `launcher/ui/hostconsole.js`, `hostconsole.css`, `launcher/src-tauri/src/host/console.rs` |
| In game, Your realm | the host, as GM, in the game | `WrathUnleashedUI/Host.lua`, `Style.lua`; realm half `realm/custom/lua_scripts/ov_hostctl.lua` |
| Leyline HQ `/wrath/admin` | staff, the official realm and the network | `stream/admin.py`, `stream/hq.py`, `docs/ADMIN-CONSOLE.md` |
| GM console | the owner, the live server box | `gmconsole/` (stays apart, behind the SSH tunnel; the window only links to it) |

## Roles

| Role | Who | Sees |
|---|---|---|
| host | the owner of a kit realm on this PC | every tab for that realm |
| linked host | `leyline link` | Overview, Realm, Guide; GM tabs only with a SOAP adapter (owner decision, later) |
| realm GM | GM level 3+ on that realm, in game | Players, Messages, Server (restart) |
| mod / admin / owner | HQ roles (`GET /wrath/admin/api/me` -> `can`) | the official realm's tabs, by `can` |

A tab the role can't use is hidden; a button the role can't use is not drawn. The server checks again every time.

## The seven tabs

Same order everywhere. A tab with nothing for this role or realm is hidden.

1. **Overview**: name, Online badge, six tiles (Database, Login server, World server, Relay agent, Players, Uptime),
   Start / Stop, the health check (five checks), the invite link (Copy, New link).
2. **Players**: who's online (search); per player **Kick**, **Mute** (minutes + reason) / Unmute,
   **Send to Dalaran**, **Ban account** (how long + reason) / **Unban account**; the allow list (on/off, add, remove).
3. **Messages**: Announce, On-screen notice, Message of the day.
4. **Realm**: public or private, who may sign up, browser players, max players, house rules, listing tags, invite
   links, Hero Key claim. Official realm: HQ's hide / flag / official / featured / verified / block.
5. **Reports**: `.bug` reports, GM tickets, player reports (official now; own realms later via `ov_reports.lua`).
6. **Server**: logs, restart with a five-minute warning (and cancel), updates (Host pack; official: a link to the GM
   console's Updates).
7. **Guide**: short how-tos, each linking its page on the website.

Network-wide HQ work (catalog, Workshop, forum, staff, flags, releases) stays in HQ; the window links to it.

## Wireframe

```
+--------------------------------------------------------------------------------------+
| (crest)  YOUR REALM CONSOLE                         [ Frost Haven (this PC)    v ]   |
|          Frost Haven                ● Online · listed        [Stop] [Refresh]        |
|--------------------------------------------------------------------------------------|
| [Overview] [Players] [Messages] [Realm] [Reports] [Server] [Guide]                    |
|======================================================================================|
| +-----------+ +-----------+ +-----------+ +-----------+ +-----------+ +-----------+  |
| | Database  | | Login     | | World     | | Relay     | | Players   | | Uptime    |  |
| | ● ready   | | ● ready   | | ● ready   | | ● listed  | | 12 online | | 3 days    |  |
| +-----------+ +-----------+ +-----------+ +-----------+ +-----------+ +-----------+  |
| Health check                                              [Run check]                |
| Invite link  https://.../join/abc...#KEY            [Copy] [New link]                |
|--------------------------------------------------------------------------------------|
| status line: "Announcement sent."                                                    |
+--------------------------------------------------------------------------------------+

PLAYERS
| Search [ Grumpy           ]                                                          |
| Grumpy   lvl 80 · Dalaran   [Kick] [Mute v] [Send to Dalaran] [Ban account v]        |
| Allow list: off   [Turn on]   [ name ] [Add] [Remove]                                |

Realm switcher (v):   Frost Haven (this PC)  |  My Server (linked)  |  Wrath Unleashed (staff)
```

The in-game window is the small version: the same tab names, fewer tabs (Players, Messages, Server), dark only.

## One action catalog

`realm/control/actions.json`, read by the launcher (JS), HQ (Python) and generated into Lua. Every button comes from
it, so names, limits, confirms and words are the same everywhere.

| id | Label | Args | Limits | Confirm | own realm (`ov_hostctl.lua`) | official (HQ `api/act`) |
|---|---|---|---|---|---|---|
| player.kick | Kick | char | | ask | `kick` | `game.kick` (mod) |
| player.mute | Mute | char, minutes, reason | 1-10080 min, reason 3-160 | none | `mute` | `game.mute` (mod) |
| player.unmute | Unmute | char | | none | `unmute` | `game.unmute` (mod) |
| player.rescue | Send to Dalaran | char | | none | `rescue` | `game.rescue` (mod) |
| player.ban | Ban account | char (own) / account (HQ), hours, reason | 0-8760 h (0 = permanent), reason 3-160 | ask (own) / typed (HQ) | `ban` -> `.ban account` | `game.ban` (admin) |
| player.unban | Unban account | char / account | | none / typed | `unban` -> `.unban account` | `game.unban` (admin) |
| allow.* | Allow-list | on/off, add/del char | 500 names | ask (on) | `allow` | - |
| msg.announce | Announce | text | 200 chars, 1 per 10 s | none | `announce` | `game.announce` (admin) |
| msg.notify | On-screen notice | text | 200 chars, shares announce's limit | none | `notify` | `game.notify` (admin) |
| msg.motd | Message of the day | text | 200 chars | none | `motd` | - (owner decision) |
| realm.restart | Restart | - | always 5 min, 1 per 10 min | ask | `restart` | - (GM console) |
| realm.restart.cancel | Cancel restart | - | | none | `cancel` | - |

Arg kinds carry the checks that exist twice today (`console.rs clean_name / clean_text / clean_reason`,
`ov_hostctl.lua CleanName / CleanText / CleanReason`, `hq.py char_name / say_text / game_reason`). One test file runs
all three against the same cases.

## One call shape, three adapters

```
realm_act(realm, action_id, args) -> {ok, message}
realm_read(realm, what)           -> {...}   what: status | players | gm_status | logs | reports
```

| Realm | Adapter | Exists |
|---|---|---|
| own realm, this PC | launcher Rust: a row in `ov_host_queue` via `docker exec` (`console.rs gm_with`) | yes |
| own realm, in game | OVMOD mod `wuhost` -> `ov_hostctl.lua` | yes |
| official | HQ `POST /wrath/admin/api/act` (ids mapped `player.kick -> game.kick`) | yes (CSRF from the launcher's webview: open question) |
| linked AC/TC | SOAP from the launcher, opt-in, GM login in the OS keychain | no (owner decision) |

Still missing for the Players tab: a who's-online list for own realms (`status` gives a count only). Add a `players`
reply to `ov_hostctl.lua` (name, level, zone; bots left out).

## One look

- **Tokens:** `design/server-window.tokens.json` (Dark, Light, shape, and the in-game palette). The neutrals are the
  launcher's own `--t-*` (`launcher/ui/style.css`); the accent is frost blue, after the PLAY button.
- **Generator:** `python3 design/make_tokens.py` rewrites the marked `sw-tokens` blocks in
  `launcher/ui/hostconsole.css` (`--sw-*`; neutrals point at `--t-*` so theme packs still move them),
  `stream/admin.py` (`SW_TOKENS_CSS` for HQ; Light by the system's `prefers-color-scheme`) and `Style.lua` (`S.C`,
  `S.HEX`, dark only). `--check` fails when a block is stale; `stream/tests/test_server_window_tokens.py` runs it.
- **Components** (from `hostconsole.css`): the crest header, the rune rule, tiles (`hc-tile`), inset boxes
  (`hc-box`), action rows with their form below (`hc-act`), the confirm dialog, pills. HQ already draws its panels,
  boxes, buttons and tabs from the same tokens; the tab build should share the class names too. Lua mirrors tiles and
  rows with `S.Box` / `S.Button`.

## Build order

| Step | What | Size |
|---|---|---|
| 1 | `realm/control/actions.json` + the shared arg-check test (Rust, Python, Lua) | S |
| 2 | Tabs in `hostconsole.js`: split today's one-page console into the seven tabs (Start tab stays the guided setup) | M |
| 3 | Players tab for own realms: `players` reply in `ov_hostctl.lua`, per-row buttons | M |
| 4 | HQ realm pages in the same seven tabs (Game becomes Players + Messages) | M |
| 5 | Realm switcher with the official realm for staff, via HQ's API | M (needs the CSRF answer) |
| 6 | In-game window: the same tab names, buttons from the catalog | M |

## Open questions

1. Message of the day on the official realm from HQ: yes or no?
2. Should staff see the official realm inside the launcher's window, or keep HQ web-only and only share the look?
3. SOAP adapter for linked servers (a GM login typed into the launcher, kept in the OS keychain): yes, no, or later?
4. The rest of the launcher still uses its own accent (#5b8def); the server windows use frost blue. Move the whole
   launcher to frost blue (one token in `style.css`)?
