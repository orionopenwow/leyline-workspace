# Build on Leyline

Leyline is a launcher, a network of relays and hosts, and a set of game modes and mods that anyone can make. This
page is the front door: pick a track, follow its first steps, run its checks, send it in.

The rules every track shares are in [CONTRIBUTING.md](../CONTRIBUTING.md#the-hard-lines): **no game assets, no
decompiled code, no names of the game or its maker in front of players, no money links, and a licence on everything
you publish.** Sign off your commits ([DCO.md](../DCO.md)).

---

## Track 1: make a mode

A mode is a minigame: one Lua file that runs inside the game. Nothing to install for players.

1. Read [MODES.md](MODES.md): the mode API, the built-in examples, the in-game editor (`/modes edit`).
2. Write it in game or in your editor; try it with **Play solo**, then **Host here** with friends.
3. Package it like a mod (Track 2) so it can be listed, reviewed and featured.

**Featured each week:** one mode at a time can be the week's featured pick on the launcher's Home ("This week") and
the website. See [Weekly](#weekly-featured-picks) below.

## Track 2: make a mod package

A mod package is a folder with a `mod.json`, plus an addon (`ui/`), realm Lua (`realm/`), or both.

```
python3 mods/wu-mod.py new my-mod        # copies the template (Dalaran Dice) with your id
python3 mods/wu-mod.py check my-mod      # every rule below, with a fix for each problem
python3 mods/wu-mod.py pack my-mod       # the zip players install
```

* Format: [mods/SPEC.md](../mods/SPEC.md). Walkthrough: [mods/README-modders.md](../mods/README-modders.md).
  Publishing: [leyline/publishing.md](leyline/publishing.md).
* **What the checker refuses** (errors, so the package can't be listed):
  * game asset files, by extension, by the bytes they start with, or by a folder path copied from a game install;
  * the game's or its maker's names in anything a player reads (the name, description, README, addon titles and
    notes, strings shown in game);
  * donation, tip, support, subscription or shop links;
  * a package with no `LICENSE` file, or a manifest licence that isn't an SPDX id;
  * realm Lua that uses a permission it didn't declare, or touches the operating system.
* Mods that Leyline's own tools find on the web stay **pending** until a person approves them.

`TODO(owner)`: the review promise (for example "an answer within 48 hours for modes and Lua-only mods, longer for
anything with native code").

## Track 3: host a realm, run a relay, build on the protocol, or fork

Everything on the network is a signed record that anyone can check. The protocol is written down in
**[COMMONHOLD-V1.md](COMMONHOLD-V1.md)**, with conformance vectors both of our implementations pass.

* **Host a realm:** [leyline/host-a-realm.md](leyline/host-a-realm.md), [BUILD-REALM.md](BUILD-REALM.md). Your realm
  signs a heartbeat and, now and then, a capability document (what it supports, who to report abuse to). Set
  `admin_peer` in `host.json` to your launcher's peer id to be the abuse contact.
* **Run a relay ("be a node"):** [leyline/be-a-node.md](leyline/be-a-node.md). A relay publishes its capability
  document at `/.well-known/leyline`; set `RELAY_ADMIN_PEER`, `RELAY_RULES_URL` and `RELAY_LANGUAGES`.
* **Write your own relay, host or client:** implement COMMONHOLD-V1.md, then
  ```
  cd hosting/kit/tools
  python3 -m wunet.commonhold check                    # the vectors against the reference code
  python3 -m wunet.commonhold probe https://your.relay  # your live node: info, node record, snapshot
  ```
  Feed `launcher/src-tauri/src/p2p/commonhold-v1-vectors.json` to your implementation: for each case it must give the
  same result (`ok` / `refused` / `ignored`) and keep the same record after each merge.
* **Fork Leyline:** [FORKING.md](FORKING.md) (one branding file, your own keys, build and publish),
  [TOOLKIT.md](TOOLKIT.md) (every tool, how to run it alone), [SUCCESSION.md](SUCCESSION.md).

## Weekly featured picks

The launcher's Home and the website show a **This week** row: one featured item per mode, each until its
`featured_until` time. The data is `stream/weekly.json` (`stream/weekly.py` reads it); the launcher reads the row from
the site's `home.json` (`this_week`) and keeps the last copy for when it is offline, hiding picks whose time has passed.
Picking the week's items is a person's job, not a script's.

## Tests

| You changed | Run |
|---|---|
| a mod or mode package | `python3 mods/wu-mod.py check <folder>`, and the package's own `tests/` (see mods/README-modders.md); the checker's own tests: `python3 mods/tests/test_wu_mod.py` |
| the relay, host kit or protocol | `cd hosting/tests && python3 -m unittest discover -s . -p 'test_*.py'` (includes `test_commonhold.py`) |
| the launcher's Rust core | `cd launcher/src-tauri && cargo test --no-default-features` (CI runs it on `feat-*` branches) |
| the launcher's UI | `cd launcher && node --test tests/<page>.browser.test.cjs` |
| the website | `python3 stream/tests/test_<name>.py stream/wrath_portal.py` |

## Licences

The launcher, host kit, relay and mesh code are MIT (the public Leyline repo's `LICENSE`); Den Lite is MIT OR
Apache-2.0; Forge is GPL-3.0; the realm images are AGPL-3.0. Your own mod or mode can use any SPDX licence you like.

### `TODO(owner)`: tools with no licence yet

These have **no licence stated** ([TOOLKIT.md](TOOLKIT.md)), so nobody else may legally reuse or fork them until one
is chosen. This page doesn't choose for them:

| Tool | Code |
|---|---|
| Release mirror | `tools/release-mirror/` |
| Mod index and daily updater | `tools/hub-index/`, `tools/addon-curate/` |
| Hero Key identity (issuing) | `stream/herokeys.py`, `stream/herokey_sso.py` |
| Website, portal and hub | `stream/` |
| Leyline Control | `tools/control/control.py` |
| Fork kit | `tools/fork-kit/` |
| Discord server setup | `tools/discord-leyline/` |
| The Commonhold v1 spec text | `docs/COMMONHOLD-V1.md` (a documentation licence, and whether it joins the public repo) |
