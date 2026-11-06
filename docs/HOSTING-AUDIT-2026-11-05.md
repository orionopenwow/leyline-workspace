# Hosting and peer-to-peer audit, 5 Nov 2026

The owner's ask: "do a full server hosting p2p audit on everything smoke test it with virtual machines if you want
but people should be able to host from anything". Branch `agent/hostaudit`, on top of tonight's two relay fixes
(4d2c212 the join page's knock, bc6cc9a private realms in the browser). Nothing was deployed or pushed.

## 1. Findings, most important first

| # | Sev | Finding | State |
|---|---|---|---|
| 1 | P1 live | Every invite link stopped at step 1 since 2 Nov: the relay refused its own join page's knock and account posts (origin check). | Fixed tonight (4d2c212), **not on main or the relay yet**: `ship` then `relay`. |
| 2 | P1 | **The Host pack and host kit could not be built at all**: the boss fights setting `LEYLINE_BOSSFIGHT_BOTS` in `realm/profiles/main.env` is not `AC_/OV_/WU_`, so the public-kit check refused every pack (`test_hostpack` 3 errors, `test_bots_updates` pins). The `hostkit` publish uses the same check. The hosting CI (`wu-hosting`) had been red on main for the last 3 pushes. | Fixed: `LEYLINE_` is allowed (`tools/leyline-repo/hostkit.py`); the secret-word filter still applies. |
| 3 | P1 | **The Built-in engine (Windows, no Docker, shown as "Recommended" in Single player) cannot be downloaded**: `engine-latest` was never published because `engine-windows.yml` is not on GitHub main (also missing there: `bots-canary.yml`, `lua-check.yml`). Players got "Couldn't get the engine: the author's site answered 404". | Message fixed (engine.rs `get_error`: "isn't published yet ... Choose Docker on this page for now"). **Owner**: double-click `setup\Turn on GitHub builds.bat` (it pushes the workflow files; needs your browser). |
| 4 | P2 | Hosting a server you already run needed Docker even though the agent is plain Python (`leyline link`). | Fixed: no Docker (or `LEYLINE_NO_DOCKER=1`) runs the agent on python3 3.10+, with status/stop/remove and an `@reboot` line. Tested end to end. |
| 5 | P2 | No warning before an hour of building on a computer that can't run the realm well: ARM Linux (images are x86-64 only), Apple chips (emulation), too little memory for the profile, too little disk. | Fixed: `setup.py preflight`, run by `host-wrath-realm.sh` and `Host a Wrath realm.bat` before the images are pulled. Warnings only. |
| 6 | P2 | A full realm for friends without Docker exists nowhere: the Built-in engine is single player only, Den Lite is the building room. | Open (feature). The pieces exist: litenet.rs already tunnels Den Lite through the relay with the agent's protocol; pointing it at the engine's 127.0.0.1:3724/8085 plus a realm-list write would make "Host with the Built-in engine" (needs cargo, a Windows test). |
| 7 | P3 | The peer-to-peer refusal list (p2p/checker.rs) and the export list (export.py) didn't name the server data a host makes from its client (`<7 digits>.map`, `.vmtree`, `.vmtile`, `.mmap`, `.mmtile`). Only catalog-hashed packages travel, so this is the floor under the floor. | Fixed in both (and the `MAPS` header); the Built-in engine already refused them. |
| 8 | P3 | The relay's one "request failed" log line printed the request path, which can carry an invite (`/r/<id>/i/<secret>/ws/<port>`). | Fixed: redacted. |
| 9 | P3 | `.bat` files are stored with LF in git (the PC commits with autocrlf), so CI and the public repos' zip downloads have LF batch files. The packs fix this (text_fix), clones and zips don't; cmd.exe can misread labels in LF files. | Test made LF-tolerant. **Owner decision**: add `.gitattributes` with `*.bat text eol=crlf`. |
| 10 | P3 | The relay shows a realm online a moment before its agent has written the realm list address; a player in that moment gets the old address. My smoke hit it on the first try. | Open, low: the realm list keeps the address across restarts, so only a realm's very first start is affected. Fix later: the relay holds players until the agent's first status. |
| 11 | P3 | `GET /api/realms/<id>` answers a private realm's name and ports to anyone with its id. The id is only in the invite link and the ports still need a knock or the invite. | Accepted (the browser's private hop uses it). |
| 12 | P3 | The knock table is bounded only by expiry (24 h). The relay is IPv4-only, so an address flood is limited. | Open, low: a global cap (`RELAY_MAX_KNOCK_IPS`) if the relay ever answers on IPv6. |
| 13 | info | A private realm that picks a community relay (`relay-pick --volunteers`) shows that relay's operator its invite (knocks pass through it) and its game traffic. Passwords stay safe (SRP6, verifier made in the browser). | The relay picker should keep private realms on Leyline's own relays; worth a line in the picker. |
| 14 | P3 | `test_network` used fixed ports 18193/18194 and failed whenever another test held them. | Fixed (free ports). |

## 2. Security checks

| Check | Result |
|---|---|
| Origin checks on knock, accounts, browser WebSocket | The relay's own origins, the site and the play origins pass; other sites get 403. No Origin (launcher, raw tools) is allowed by design: a knock still needs the invite for a private realm. |
| Invite secrets never sent to servers | The secret rides in the `#fragment` (join page, browser hop `#inv=`), goes to the relay only in the knock body or the WebSocket URL, which it must check. The relay stores only sha256. Not in relay logs (smoke check), now not in the error log either. Caddy has no access log. |
| Invite strength | 20 base32 characters (100 bits), `secrets.choice`. Realm ids 80 bits. |
| Rate limits | Knocks 10/min/IP, accounts 3/IP/h and 50/realm/day, browser upgrades 30/min/IP, connections 40/min/IP, streams per IP and per realm, wrong passwords per IP, new realms 3/IP/h, Hero Key 20/min/IP. All exercised by test_relay.py. |
| Private realms need the invite or a knock | Proven between separate addresses (section 3): a stranger with a real account is refused on the per-realm port, the shared port and the WebSocket while the invited friend is knocked. |
| The host's address never reaches players | Proven: the realm list and the relay's answers carry only the relay's address and ports. |
| No game files over peer-to-peer | Allow-list of catalog packages with sha256, plus the refusal list (names, magic bytes, inside zips and tarballs), now with server data too. Den Lite friends use their own game folders. |
| Brand words | None in the new player-facing text (the link test checks the no-Docker output). |

## 3. Smoke tests

### Separate computers (new: `hosting/tests/smoke/machines.py`, `test_machines_smoke.py`, about 20 s)

Docker isn't available in this sandbox, so the "virtual machines" are separate OS processes, each leaving from
its own address on the loopback /8. The in-process rigs put every player on 127.0.0.1, so per-address rules were
never tested between computers before.

- 127.0.0.1 the relay (`python -m wurelay.relay`, configured by env as on the server);
- 127.0.0.2 the host's PC: the fake 3.3.5a realm listens only there, and the real agent reaches the relay outbound only (as behind a home router or CGNAT);
- 127.0.0.3 the invited friend, 127.0.0.4 a fresh computer, 127.0.0.9 a stranger: the docker e2e's own player script (`e2e/e2e_client.py`).

```
PASS host's PC (127.0.0.2) is online at the relay through its outbound tunnel only
  [127.0.0.3] unknocked refused / wrong invite refused / websocket without invite refused / invite lets this IP in /
              account from the join API / login + char create + world + logout on the per-realm port, the shared
              port and the browser WebSocket path / status.json          (10/10)
PASS a stranger (127.0.0.9) with a real account is refused on the per-realm port, the shared port, the websocket
PASS the private realm is not in the public listing
PASS the relay never hands out the host's own address or ports
  [127.0.0.4] relay sees it public / fresh IP plays with no invite / listed by signed heartbeat / bots counted   (4/4)
PASS relay log has no invite secret and no player address in clear
{"ok": true, "passed": 9, "failed": []}
```

### No-Docker link (new: `test_link.py LinkWithoutDocker`)
The real `leyline link` with Docker installed but not running: the agent starts on python3, the relay sees it,
a player logs in through the relay into the world, `link status` says "running on python3", `link stop` ends it.

### Suites

| Suite | Result |
|---|---|
| `python3 -m pytest -q hosting/tests` | 404 collected: 4 failed before (hostpack x3, bots pins); now all pass, plus the new tests. CI's own form (`python -m unittest discover -s hosting/tests`) OK. |
| `stream/tests/test_hop.py stream/wrath_portal.py` | 88/88 |
| `test_network.py` | 89/89 (after the port fix) |
| `test_hostflow.py` | 175/175 |
| `test_herokeys.py`, `test_hq_network.py`, `test_portal_hardening.py` | 142/142, 32/32, 24/24 |
| `tools/leyline-repo` (`test_hostkit`, `test_export.py` run directly) | 13 + 68/68 |
| Launcher UI: denhost, hostflow, hostlink, hostmodes, hostnetwork, hostconsole, single | 53/53 |
| `hosting/mesh-sim/meshsim.py --quick` | DHT always finds the closest live node (10% liars, 30% offline); relay choice serves everyone with seeds or scale-out |
| Hero Key SSO, hop bridge, Den Lite tunnel | in-process: test_herokey_sso, test_hop_bridge, test_den_lite_tunnel pass |

## 4. Host from anything: the platform matrix

| Host computer | Full realm for friends | Your Den (Den Lite, no Docker) | Single player | Today |
|---|---|---|---|---|
| Windows x64 + Docker Desktop | Yes: Host page / `Host a Wrath realm.bat` | Yes | Docker engine | The main path. |
| Windows x64, no Docker | No (gap 6) | Yes (`den-lite-windows-x64.exe`, built 6 Nov) | Built-in engine: **not published** (finding 3) | Now says to use Docker; owner press needed. |
| Linux x64 + Docker | Yes: `bash leyline host` | Yes (`den-lite-linux-x64`) | Docker engine | Works. |
| Linux x64, no Docker, server already running | Yes: `leyline link` on python3 (new) | Yes | n/a | Fixed tonight. |
| macOS Intel + Docker Desktop | Yes: `bash leyline host` | No build ("isn't made for this kind of PC yet") | Docker engine | Works. |
| macOS Apple chip + Docker Desktop | Yes, through x86 emulation (slower) | No build | Docker engine | Preflight now says so. |
| Linux ARM (Raspberry Pi, ARM VPS) | Only with qemu/binfmt or a local image build (hours, untested) | No build | n/a | Preflight now says so. Next cheap step: arm64 in `wu-images.yml` (`platforms: linux/amd64,linux/arm64`) and an ARM Den Lite build. |
| Windows on ARM | Docker Desktop's emulation (untested) | No build | No | Not offered. |
| Low memory (under 4 GB for Docker) | `dalaran`/`den` profiles (about 2 GB) | Yes | | Preflight names the lighter profile. |
| Android | No hosting (the APK plan is a player and launcher) | No | | Out of scope today. |
| Any machine running its own 3.3.5a / 2.4.3 / 1.12.1 server | Yes: `leyline link` / `Link my server.bat`, Docker or (now) python3 | | | Works. |

## 5. Not verified here

- A real AzerothCore realm (`tests/e2e/run-e2e.sh --real`) and a real game client through the relay: no Docker
  daemon and no game data here.
- `run-e2e.sh --fake` (containers): no Docker daemon. The same player script passed against processes instead.
- Whether the GHCR images (`wu-ac-wotlk-*:stable`, `wu-host-tools:1`) are public: ghcr.io is blocked from this
  sandbox. `wu-images` last succeeded 6 Nov 00:41.
- The Rust changes (engine.rs `get_error`, p2p/checker.rs) are not compiled here (no cargo builds): rustfmt parses
  both; `launcher-check` CI builds them with their new tests.
- The `.bat` preflight lines were not run on Windows.
- The 1-hour soak on the relay VM.

## 6. Leyline Control presses (after this branch is on the PC)

1. `ship`: the fixes to GitHub (the hosting CI goes green; the launcher rebuild carries the engine message and the
   refusal list).
2. `relay`: puts tonight's knock fix (every invite link) and the log redaction on the relay. It is a deploy.
3. `signin`: tonight's private-realm hop in the browser game (portal, bc6cc9a).
4. `hostkit`: republish the host kit, now that it can be built again.
5. By hand, once: `setup\Turn on GitHub builds.bat` (the Built-in engine's workflow, the bots canary, the Lua check).
6. Owner decisions: `.gitattributes` for CRLF batch files (finding 9); arm64 images (matrix); private realms only on
   Leyline's own relays (finding 13).
