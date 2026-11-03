# The blank slate: Leyline as a Commons anyone can run

Owner, 3 Nov 2026: "eventually we will break apart everything and have a blank slate repo with all the features and
main directory of mods and capabilities, so if ever Leyline goes down the movement continues."

Whiteboard, same day: "Seed server working: host own w/o Leyline, every feature separate, guide"; "Host in browser
kit + guide"; "Directory: living list, wiki in forum".

This is the plan. Some of it is built (marked **built**); the rest is phased below. Why it matters and the three ways
to keep the network up are in [MOVEMENT.md](MOVEMENT.md). The single points of failure and who decides are in
[SUCCESSION.md](SUCCESSION.md). Each tool in more depth: [TOOLKIT.md](TOOLKIT.md).

The hard lines hold for every piece and every copy:
- no game data and no game client on any server or over P2P; players bring their own;
- no money, tips, tokens or mining;
- no decompiled clients;
- the game's and its maker's names stay out of what players read.

---

## 1. The shape: one "Commons" repo, every piece on its own

A blank-slate repo holds every piece as its own folder, each with its own README ("run it alone"), its own licence
file and its own tests. Nothing in it names our servers except as defaults in one file.

```
leyline-commons/
  README.md            the mission paragraph (MOVEMENT.md) and the three ways to keep it alive
  brand.json           the one file every piece reads for name, colours, logo and links (section 3)
  directory.json       the living list of pieces and mods lists (section 4)
  launcher/            the desktop app
  site/                the website, forum and Log in page
  browser-kit/         the game page around the browser client (own files only)
  host-kit/            host a realm from your own client; the one-click wrappers
  relay/               relay and mesh node
  den-lite/  forge/    the Den's server and editor
  mod-index/           the daily mod finder and the approved lists
  release/             release mirror, release log, fork check
  discord/             the bridge and the server setup
  commons/             Make my own Leyline (tools/commons today)
  docs/                the public guides
```

Today these live in two repos: the public `orionopenwow/Leyline` (what `tools/leyline-repo/export.py` `ALLOW` lists)
and the private workspace. The Commons repo is the public one grown to hold everything, under licences the owner
picks.

## 2. Every piece, where it is, and how it runs alone

The machine-readable copy of this table is [tools/commons/directory.json](../tools/commons/directory.json)
(**built**). `python3 -I tools/commons/commons.py check-directory` checks that every path exists.

"Alone today" means it already runs without the rest. "Cut out" is when it can move into the Commons repo: **now**
(already public, or ours to publish with no other work), **D1** (waits only for the owner's licence decision),
**work** (needs building first).

| Piece | Where it is now | Licence | Needs | Run it alone | Alone today | Cut out |
|---|---|---|---|---|---|---|
| Launcher | `launcher/` | MIT | updater key, release host, a relay; a website for Log in and the list (optional) | `npx tauri build` after `rebrand.py` | yes | **now** (public) |
| Updates and release log | `launcher/src-tauri/src/update.rs`, `wunet/releaselog.py` | MIT | updater key; release key (not made) | `wunet.releaselog keygen/append/verify` | yes | **now** (public) |
| Host kit and Host pack | `hosting/kit/`, `hosting/pack/` | MIT; realm images AGPL-3.0, other cores GPL-2.0 | Docker, your own client, a relay | `Start my own server.bat`, `bash leyline host` | yes | **now** (public) |
| Relay and mesh node | `hosting/relay/`, `wunet/mesh.py` | MIT | public name, TLS, open ports | `Be a node.bat`, Docker | yes | **now** (public) |
| Hero Key for realms | `wunet/herokey.py` | MIT | a pinned site key | `setup.py herokey` | yes | **now** (public) |
| Agents (MCP) | `launcher/src-tauri/src/agents.rs` | MIT | the launcher | `Leyline mcp` | yes | **now** (public) |
| In-game addons | `stream/fui-web/.../WrathUnleashedUI`, `addons/` | MIT (ours) | your own client | copy into AddOns | yes | **now** (partly public) |
| Discord bridge | `hosting/discord/` | MIT | your own bot | `wudiscord.bridge` | yes | **now** (public) |
| Den Lite | `labs/den-lite/` | MIT OR Apache-2.0 | nothing online | `den-lite --dens DIR` | yes | **now** (licence already stated) |
| Forge | `forge/` | GPL-3.0 | Windows build | CI `forge.yml` | yes | **now** (source already published per build) |
| Website, portal, hub | `stream/` | none stated | Python 3, Caddy; the sign-in service for Log in | `site/run-site.sh` from Make my own Leyline (**built**) | mostly | **D1** |
| Log in page (Hero Key, web side) | `stream/herokeys.py`, `herokey_sso.py` | none stated | the sign-in service; the key table and pepper for the same keys | inside the website | no | **D1** + **work** (publish the sign-in service) |
| Forum | `stream/forum.py` | none stated | the website | inside the website | inside it | **D1** |
| Mod index and daily updater | `tools/hub-index/`, `tools/addon-curate/` | none stated | git, network | `hub_index.py build`, `curate.py run` | yes | **D1** |
| Release mirror | `tools/release-mirror/` | none stated | GitHub, SSH | `release_mirror.py build` | yes | **D1** |
| Fork kit | `tools/fork-kit/`, `rebrand.py`, `branding.json` | MIT (rebrand), none (fork_kit) | Python 3 | `fork_kit.py check` | yes | **D1** |
| Commons kit | `tools/commons/` (**built**) | none stated | Python 3 | `Make my own Leyline.bat` | yes | **D1** |
| Discord setup | `tools/discord-leyline/` | none stated | your own bot token | its preview and do-it .bat files | yes | **D1** |
| Browser kit | `stream/www-files/` | none stated | the browser client build and its server (separate source) | packed by Make my own Leyline (**built**, page only) | no | **D1** + **work** (publish the client source) |
| Leyline Control | `tools/control/` | none stated | Windows, SSH, GitHub | `control.py` | yes | **work** (steps into CI with a deploy key) |
| Fresh computer test | `tools/first-run/` | none stated | Windows | `Fresh computer test.bat` | yes | **D1** |
| Azeroth Survivors | `stream/survivors.py` + upstream pin | upstream MIT or Apache-2.0; permission TODO(owner) | the author's permission statement | local mode planned | no | **work** |
| Seeded realm | `docs/SEEDED-REALM-PLAN.md`, `cmangos-1.12.1` profile | MIT (kit), GPL-2.0 (core) | Keepers, a relay | Seed v0 (not built) | no | **work** |
| Kill switch | being built separately ([KILL-SWITCH.md](KILL-SWITCH.md)) | none stated | the owner's PC | see its doc | no | **work** |

### "Every feature separate": what running alone means

Each piece's README in the Commons repo answers four questions, the same four as `directory.json`:
1. **What it does,** in one line.
2. **What it needs** (other pieces, keys, accounts, hardware).
3. **How to run it alone,** one command or one click.
4. **What breaks without the rest,** honestly. For example, the website without the sign-in service shows its pages
   but Log in says it isn't answering; a realm without our relay needs any other relay.

## 3. One brand file

Today the look and name are spread over several places: `launcher/branding.json` (read at build time),
`LOOK_VARS` and the `--lk-*` tokens in the website, `leyline-look.css` on the game page, and words in each page. A
copy of Leyline needs one file it can change. That file is `brand.json` (format in
[tools/commons/README.md](../tools/commons/README.md)).

| Piece | Reads brand.json | How |
|---|---|---|
| Website and Log in page | **built** | `WU_BRAND_FILE=/path/brand.json` (`stream/brandfile.py`): name in what people read, colours after the site's own, logo as the mark and tab icon, Discord link. Unset: the site is exactly as before |
| Browser kit | **built** (at pack time) | `brand.css` with the colours and logo; the page title |
| Launcher | **built** (at pack time) | converted to `launcher/branding.json` for `rebrand.py` |
| Host kit, in-game windows | later | the bat titles, the `leyline` command's words and the addon's strings still say Leyline |
| Launcher at run time | later | `branding.rs` reads branding.json at build time; a reskin needs its own build |

## 4. The directory: a living list

**The file (built):** `tools/commons/directory.json`. One entry per piece (id, name, area, what, code paths, public,
licence, needs, run alone, alone today/later/plan, phase), plus `mod_directories`: where the mods lists live
(`stream/hub/community-index.json` for everything found, `stream/upstream-addons.json` and `stream/looks.json` for
what a person approved, the mode and template catalogs). The mods themselves stay where they are: the directory
points at the lists, it does not copy them, and found mods stay pending until approved.

**The forum page (format built, posting not):** `python3 -I tools/commons/commons.py wiki` prints the directory as
plain-text forum posts, each within the forum's 5,000-character limit, numbered (1/3, 2/3...). The forum shows plain
text and links only, so the format is:

```
THE DIRECTORY (updated 2026-11-03)
...
== PLAYER ==
* Launcher (runs on its own today; in the public repo; licence: MIT)
  The desktop app: ...
  Needs: ...
  Run it alone: ...
  Code: launcher
```

How it stays living:
1. The file in git is the truth. Anyone proposes a change as a reply in the pinned thread or a pull request.
2. A staff member edits `directory.json`, runs `check-directory`, and re-posts with `wiki`.
3. **Not built:** the forum has no editable "wiki" posts (FORUM.md: plain text, no rich editing). A small "pinned
   post the staff can replace" is the next step: one HQ button that runs `wiki` and swaps the thread's posts.

## 5. Phases

| Phase | What | Needs | State |
|---|---|---|---|
| **0. The kit** | MOVEMENT.md; `directory.json` and its forum format; the site reads `brand.json`; Make my own Leyline; Start my own server; Be a node; Control step `commons-check` | nothing | **built** (branch `agent/commons`) |
| **1. Cut out what's public** | Make the Commons repo from the public pieces (section 2, "now"): a second export target beside `tools/leyline-repo/export.py`, with the same game data, secret and brand checks, laid out as section 1. Each piece gets its README with the four answers | the repo's name and home (C1) | ready to build |
| **2. Publish the private pieces** | Everything marked **D1**: the website, forum, Log in page, mod index, release mirror, fork kit, commons kit, Discord setup, browser page | the licence (C2 = SUCCESSION D1) | waits on the owner |
| **3. Close the gaps** | Publish the sign-in service (or a small open one that reads the same key table); publish the browser client's source and its server; host kit and in-game windows read `brand.json`; Survivors local mode; Seed v0 | work, and C3, C4 | later |
| **4. Run without one PC** | Control's steps as CI jobs with a deploy key; stewards; spare keys; the successor feed (SUCCESSION.md 2.2 to 2.5) | people (SUCCESSION D3, D4) | later |

### What can be cut out today, and what can't

- **Today:** launcher, updates and release log, host kit, relay and mesh, Hero Key checking on realms, agents, the
  in-game addons, the Discord bridge, Den Lite, Forge. All but Den Lite and Forge are already in the public repo.
- **After one decision (licence):** website, forum, mod index, release mirror, fork kit, this commons kit, Discord
  setup, the browser page.
- **After work:** Log in for other sites (the sign-in service), the browser client, Control without the owner's
  PC, Survivors as a local mode, the seeded realm, the kill switch.

## 6. Owner decisions

| | Decision | Recommendation |
|---|---|---|
| **C1** | The Commons repo: its name, and whether it lives under a GitHub organization (SUCCESSION D3) | `leyline-commons` under an organization with two owners |
| **C2** | Licence for the private pieces (SUCCESSION D1) | MIT, like the rest; CC0 for the directory and index data |
| **C3** | Publish the sign-in service, or build a small open one that checks the same key table | publish it |
| **C4** | Publish the browser client's source and its small server | yes, with the same "no game data" checks |
| **C5** | Who keeps the directory: staff only, or reviewers too; and the HQ "replace the pinned post" button | reviewers propose, staff post |
| **C6** | The mission paragraph in MOVEMENT.md (`TODO(owner)`) | your words |
| **C7** | Whether copies made with the kit must say "based on Leyline" somewhere (a credit line), and its words (`TODO(owner)`) | a short credit line in the site footer, optional |
