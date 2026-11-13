# Contributing to Leyline

Thanks for wanting to build on Leyline. Start with **[docs/BUILD-ON-LEYLINE.md](docs/BUILD-ON-LEYLINE.md)**: it
picks the right track for you and links everything else.

## The hard lines

A contribution that crosses one of these is closed, however good it is otherwise. The checker
(`python3 mods/wu-mod.py check <folder>`) catches most of them before a person has to.

1. **No game assets.** No files from the game client or made from it: archives, models, textures, maps, sounds,
   database tables, extracted server data, or a client. Not in a mod, not in a pull request, not over P2P. Leyline
   never hosts or ships game data; players bring their own game folder.
2. **No decompiled code.** Nothing decompiled or disassembled from the game client or server binaries, and nothing
   copied from projects that are.
3. **No brand words in front of players.** Text a player reads (UI, mod names and descriptions, docs pages, store
   copy) never names the game or its maker. The only allowed line is the legal one: "not affiliated with or endorsed
   by" the maker. The checker's list is in `mods/wu-mod.py` (`BRAND_WORDS`).
4. **No money.** No donation, tip, support, subscription or shop links, and nothing that sells in-game benefit.
5. **A licence.** Every mod or mode package has a `LICENSE` file and a matching SPDX id in its manifest.

## Three tracks

| Track | What you make | Start here | Checked by |
|---|---|---|---|
| **1. Modes** | a minigame: one Lua file that runs in game | [docs/MODES.md](docs/MODES.md) | `wu-mod.py check`, the mode tests |
| **2. Mod packages** | an addon (`ui/`), realm Lua (`realm/`), or both | `python3 mods/wu-mod.py new my-mod`, then [mods/README-modders.md](mods/README-modders.md) and [mods/SPEC.md](mods/SPEC.md) | `wu-mod.py check`, the fake-ALE tests |
| **3. Realms, relays and forks** | host a realm, run a relay, write your own client of the network, or fork Leyline | [docs/COMMONHOLD-V1.md](docs/COMMONHOLD-V1.md) (the protocol and its conformance vectors), [docs/BUILD-REALM.md](docs/BUILD-REALM.md), [docs/FORKING.md](docs/FORKING.md) | `python3 -m wunet.commonhold check` / `probe`, the hosting tests |

## How to send a change

1. Fork, branch, change. Keep one change per pull request.
2. Run the tests for what you touched (see the table in [docs/BUILD-ON-LEYLINE.md](docs/BUILD-ON-LEYLINE.md#tests)).
3. **Sign off every commit** (`git commit -s`). That adds `Signed-off-by: Your Name <you@example.org>` and means you
   agree to the [Developer Certificate of Origin](DCO.md), **including Leyline's added line**: the contribution
   contains no proprietary game assets or decompiled code. We use a sign-off, not a contributor licence agreement:
   you keep your copyright.
4. Open the pull request and fill in the template. A reviewer answers; mods and modes found by Leyline's own tools
   stay **pending** until a person approves them.

## Proposals

A change to the protocol, a new capability id, or a new record kind goes through a **Leyline Proposal** first (an
issue: the problem, the change, the new vectors). See [docs/COMMONHOLD-V1.md §8](docs/COMMONHOLD-V1.md#8-changing-this-spec).

## Conduct and security

* Be kind: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
* Found a security problem? **Don't open a public issue.** See [SECURITY.md](SECURITY.md).
