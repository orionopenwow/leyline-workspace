# Words we use

One name for each thing, everywhere players read: the launcher, the website, in-game messages and the docs. Owner, 31 Oct: "make sure it's simple for users to understand".

## The rules

- Short sentences. One idea each.
- Plain words. If a technical word is needed, explain it the first time, in a few words.
- The same name for the same thing on every page.
- Never the game's own brand names ("World of Warcraft", "WoW", "Warcraft", "Lich King", "Blizzard"). The tests check this.
- No version numbers in sentences. A small version chip or a Details line is fine.
- No money, tips or support links.

## The names

| Say | It means | Don't say |
|---|---|---|
| **the launcher** (first time on a page: **the Leyline launcher**) | The desktop app. | Leyline Launcher, the app, the client, "Leyline Launcher 0.3" in a sentence |
| **Leyline** | The whole project and the network of realms. | Open Vanilla, OpenVanilla, WrathUnleashed |
| **Wrath Unleashed** | Our own realm. | WrathUnleashed, WU, "Wrath" on its own, the official server |
| **your realm** | The realm a person hosts. Any realm is a **realm**. | server, shard, Beacon, instance |
| **the Host pack** | The download a host needs to run a realm. | host kit, kit, Leyline host, the scripts |
| **the realm list** | The list of realms players can join. | the Leyline list, listing, server list |
| **the Leyline network** | How players reach a realm without the host opening ports. Say once: "players reach your realm through the Leyline network: you open no ports and nobody sees your address". | the relay, tunnel, relay agent, realm id (in sentences) |
| **the helper** | The small program that puts a server you already run on the realm list. | relay agent, agent, link agent |
| **Docker** | Say once: "Docker, a free program that runs your realm in its own space". | containers, volumes, compose |
| **server software** | AzerothCore, CMaNGOS and the rest. Their names are fine in a chooser. | core (in a sentence) |
| **ruleset** | Vanilla+, Den, Classic levelling and the rest: how a realm plays. | preset, profile |
| **Den** (or **your Den**) | A private room to build in with friends. The button can say "Make my Den". | My Den (in a sentence), sandbox |
| **Modes** | Game modes: Brawl, golf, duels and the rest. | minigames, playmodes |
| **Mods** | The list of mods and the mods in it: the launcher's Mods page and the website's Mods page. | the Workshop, the catalog, projects |
| **Workshop** | Only the things players build and share in game (builds, courses, modes): Esc > Mods & Workshop > Workshop, and `.workshop`. | (don't use it for the mods list) |
| **addons** | Only the interface add-ons that come with Leyline. | |
| **invite link** | The link a host sends friends. | join code, share URL |
| **your join link** | The host's own link to make their GM account. Keep it to yourself. | |
| **GM account** | The host's own account with game master rights. Say once: "your GM account (it can run the realm from inside the game)". | admin, gm level |
| **Hero Key** | One account for every Leyline realm. | SSO |
| **the original client** / **the Wrath client** | The game clients, by what they play. | client version numbers in sentences |
| **your game folder** | The folder where the player's own game is. | WoW dir, install dir, data path |
| **the website** | play.openvanilla.app. | the portal, the hub, the site |

## Before and after

| Before | After |
|---|---|
| Players reach your realm through the Leyline relay, by its realm id: nobody sees your address and you open no ports. | Players reach your realm through the Leyline network. You open no ports, and nobody sees your address. |
| Any core, here or elsewhere: Link my server runs only the relay agent next to it and never changes your server. | Any server software, here or elsewhere. Link my server runs a small helper next to it. It never changes your server. |
| Can't reach the Workshop. Try again. | Can't load the mods. Try again. |
| Play on community Beacons from the realm list | Play on community realms from the realm list |
| Public: on the Leyline list | Public: on the realm list |
| The relay hasn't seen your realm yet. | The Leyline network hasn't seen your realm yet. |

## Where the words live

- Launcher: `launcher/ui/*.js`, `launcher/ui/index.html`.
- Website: `stream/hub.py`, `stream/hostflow.py`, `stream/profiles.py`, `stream/forum.py`, `stream/listings.py`.
- In game: `realm/custom/lua_scripts/ov_*.lua` (`say()` and `SendBroadcastMessage`) and the WrathUnleashedUI addon.
- Docs: `docs/leyline/*.md`, served at /wrath/docs.

Tests that check the words: `stream/tests/test_brand.py`, `stream/tests/test_brand.lua`, `stream/tests/brand.browser.test.cjs`, `stream/tests/test_docs.py`, `launcher/tests/copy.browser.test.cjs`.
