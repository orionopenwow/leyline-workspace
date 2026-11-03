# The movement continues without us

Owner, 3 Nov 2026: "explain a way to keep a server alive with bare minimum software so people only need one thing to
click and start their own local server or be a node for a mass public server, or teach people how to upload a server
to Hetzner to keep the movement alive, containing all of what Leyline is and our movement in a mission paragraph."

This page is for anyone, not only us. It says what Leyline is, what it promises, and three ways to keep it alive:
on your own PC, as a node, or on a server you rent. How Leyline splits into pieces anyone can run is in
[BLANK-SLATE.md](BLANK-SLATE.md). Making your own copy under your own name is in
[tools/commons/README.md](../tools/commons/README.md) and [FORKING.md](FORKING.md).

---

## What Leyline is

> **TODO(owner): finalize this paragraph in your own words.** Draft:
>
> Leyline is a free launcher, network and toolkit for community realms and private dens, run by the players who use
> it. No one owns it: not us, and not any one server. You bring your own game files. We never ship any, and nothing
> on the network ever carries them. What we make is open source, so if any part of Leyline goes quiet, anyone can
> pick it up, give it a new name if they like, and carry it on. The realms, the relays, the guides and the mods
> directory belong to the people who keep them running.

### The promises

| Promise | What it means | Where we are today |
|---|---|---|
| **Free** | No shop, no tips, no tokens, no paid tiers. Nobody pays to play or to host | Kept |
| **No game data shipped** | We never host, send or share game files or game clients, and P2P never carries them | Kept for the launcher, host kit, relay and this kit. The official site still serves the approved browser slices until the game data switch is turned off (GAME-DATA-ON-OUR-SERVER.md). A copy made with this kit never serves any |
| **Your own files** | Everything built from game files is read from your own client, on your own PC | Kept |
| **Open source** | Anyone can read, run and change the code | The launcher, host kit, relay and guides are MIT. The website, mod index and Control are **not published yet** (SUCCESSION.md, D1) |
| **Anyone can carry it on** | No piece needs us to exist | Mostly: see "What still depends on us" below |

---

## Three ways to keep it alive

| | 1. Your own server, one click | 2. Be a node | 3. Your own always-on server |
|---|---|---|---|
| **Who it's for** | you and your friends | the whole network | a realm for many players, day and night |
| **Runs on** | your PC | your PC or a rented server | a rented server (Hetzner below) |
| **What you click** | `Start my own server.bat` | `Be a node.bat` | one command over SSH |
| **What it costs** | nothing but power | nothing but power and upload | about €6 to €9 a month in Europe (below) |

### 1. One click: your own server on your PC

Your own realm, from your own game client, on your own PC. Friends join with your invite link. No router settings:
your realm calls out to a relay, players reach it through the relay, and nobody sees your home address.

**What you need:**
- A PC with 8 GB of memory (16 GB is better), any 4-core processor from the last ten years, 30 GB free.
- **Docker Desktop** (free, docker.com).
- Your own game folder. The kit reads the map data it needs from it, once, on your PC. Nothing is downloaded but the
  open-source server software.

**What to click:** `Start my own server.bat`. It is inside the host kit zip (and in `tools/commons` in the repo). It
asks for a name, a template and your game folder, then starts. It starts **private**. The first start takes 5 to 20
minutes. The launcher's **Host** page does the same thing with buttons.

**What it costs:** nothing. Your PC uses more power while the realm runs. To keep it up day and night, see
[always-on.md](leyline/always-on.md).

### 2. Be a node for a big public realm

A node carries other players' game connections, so a popular realm isn't limited by one relay. It never stores or
passes game files or game clients. Players are sent only to nodes that answer with their own key, and only to the
closest one with room.

**What you need:**
- Docker.
- A **public name** (like `node.example.org`) that points at the machine.
- TCP **80, 443 and 41000 to 41051** open to the internet (port forwarding at home, or a rented server).

**What to click:** `Be a node.bat` (inside the host kit zip), or in the launcher: **Host**, **Network**, **Help carry
the network**. Stop it with `Be a node.bat off`.

On a rented Linux server, in the unzipped host kit folder:

    docker build -q -t wu-host-tools:1 tools
    docker run --rm -v "$PWD:/kit" -e WU_KIT=/kit wu-host-tools:1 python -m wuhost.setup volunteer-relay --host node.example.org --region eu
    docker compose -f docker-compose.volunteer-relay.yml up -d

**What it costs:** upload. You set caps (upload in KB/s, players at most). Defaults per realm: 256 KB/s and 5 GB a
day. A small rented server works well (way 3, "Sizes").

**Honest note:** until the network opens community relays to everyone (the `relay_volunteer` and `mesh_relays`
switches, LEYLINE-MESH.md), only hosts who choose community relays send players to a node.

### 3. Your own always-on server on Hetzner

Hetzner is one of many hosts; any provider that rents a small Linux server works the same way. No affiliate links,
and nobody here makes the account for you.

**Step 1: the account.** Go to hetzner.com and sign up yourself. It is your account, in your name, with your own
payment details. Nobody from Leyline ever asks for your password.

**Step 2: pick a size and a place.** In the Cloud Console: **New project**, then **Add server**.
- **Location:** the one nearest most of your players. Germany and Finland are the cheapest; the US and Singapore cost
  more and include less traffic (prices below).
- **Image:** Ubuntu (the newest LTS).
- **Type:** see "Sizes". Pick the **Intel/AMD** line (CX or CPX). The Arm line (CAX) is cheap but our realm
  images may not run on Arm: we have not tested it.

**Step 3: an SSH key** (how you log in without a password). On your own PC, in a terminal (Windows 10 and 11 have it):

    ssh-keygen -t ed25519

Press Enter for the defaults. Open the `.pub` file it made (in your user folder, `.ssh\id_ed25519.pub`), copy all of
it, and paste it under **SSH keys** when you add the server. Never share the other file (without `.pub`): that one is
your private key.

**Step 4: a firewall.** In the Cloud Console, **Firewalls**, make one and attach it to your server. Allow in:

| What you run | Open (TCP, inbound) |
|---|---|
| Always | 22 (SSH). Better: only from your own address |
| A realm | nothing else. The realm calls out to the relay |
| A node | 80, 443, 41000-41051 |
| Your own website (`Make my own Leyline`) | 80, 443 |

**Step 5: copy your own game folder up** (a realm needs it; a node or a website doesn't). From your PC:

    scp -r "C:\Games\MyGame" root@YOUR-SERVER-IP:/root/game

**Step 6: one command to install the server kit.** Log in (`ssh root@YOUR-SERVER-IP`), then:

    curl -fsSL https://get.docker.com | sh && apt-get install -y unzip && curl -fsSL -o host-kit.zip https://github.com/orionopenwow/Leyline/releases/download/host-kit/WrathUnleashed-host-kit.zip && unzip -o host-kit.zip && cd "Leyline host" && WU_GAME_DIR=/root/game bash leyline host --ruleset vanillaplus --public --always-on

It asks for your realm's name and your GM account name, then prints your join link. Make your GM account on that
link (`bash leyline status` shows it again). `--always-on` makes the realm start again by itself after a reboot.

If that download address is ever gone, use any copy of the host kit: one a friend shares, the release mirror, or the
`host-kit` zip that `Make my own Leyline` packs. Upload it with `scp` and start from `unzip`.

**Step 7: backups.**
- Stop the realm (`bash leyline stop`), then take a **snapshot** in the Cloud Console. A snapshot is the whole
  server, characters included. Hetzner charges for snapshots and for its automatic backup option: see their pricing
  page.
- Also keep your own copy of the kit's `state` folder (your realm's key and links): `scp -r root@YOUR-SERVER-IP:"/root/Leyline host/state" .`
  It holds your realm's key: keep it private.

**Step 8: updates.**
- The server: `apt-get update && apt-get upgrade -y` now and then, or turn on automatic security updates with
  `apt-get install -y unattended-upgrades`.
- The realm: when there's a new host kit, unzip it over the old one and run the same `bash leyline host ...` command
  again. Your realm, characters and links are kept.

**Sizes (and what they cost).** Prices as listed on Hetzner's own pages when read on 3 Nov 2026, per month, without
VAT, with an IPv4 address included. They changed twice in 2026 (April and June), so check the page before you
order. When we read the pages, the order buttons said the plans were currently unavailable: check stock too.

| For | Plan (Germany/Finland) | Price | Elsewhere |
|---|---|---|---|
| A node, or your own website | CX23: 2 vCPU, 4 GB, 40 GB, 20 TB traffic | €5.99 | US: CPX11, 2 vCPU, 2 GB, 1 TB, €17.99 |
| A small realm (the minimum the guides ask: 4 GB) | CX23 | €5.99 | US: CPX21, 3 vCPU, 4 GB, 2 TB, €32.49. Singapore: CPX22, 2 vCPU, 4 GB, 1 TB, €26.99 |
| A realm with room (recommended) | CX33: 4 vCPU, 8 GB, 80 GB, 20 TB | €8.99 | US: CPX31, 4 vCPU, 8 GB, 3 TB, €62.99 |

Traffic beyond what's included costs extra (about €1 per TB in Europe and the US, more in Singapore). A realm with a
few dozen players uses far less than 20 TB.

---

## What still depends on us (and the plan)

Honestly, today:
- **Log in with a Hero Key** needs our website and the sign-in service. Realms keep working with their own join pages
  without it.
- **The realm list** and **relays** start from ours. The mesh lets others carry both, and a host can point a realm at
  any relay.
- **Launcher updates** need our updater key. There is no spare yet (SUCCESSION.md, D4).
- **The website, mod index and Control** are not published yet (SUCCESSION.md, D1).

The plan to close each gap, piece by piece, is [BLANK-SLATE.md](BLANK-SLATE.md). Who decides what is in
[SUCCESSION.md](SUCCESSION.md).

## Sources

- Hetzner, Cost-Optimized cloud plans (CX23, CX33; prices per month excl. VAT, incl. IPv4; read 3 Nov 2026):
  https://www.hetzner.com/cloud/cost-optimized/
- Hetzner, Regular Performance cloud plans (CPX, with the US and Singapore prices and traffic; read 3 Nov 2026):
  https://www.hetzner.com/cloud/regular-performance/
- Hetzner, price adjustment from 1 April 2026: https://www.hetzner.com/pressroom/statement-price-adjustment/
- Hetzner, standardization and price adjustment from 15 June 2026:
  https://www.hetzner.com/pressroom/standardization-and-price-adjustment-of-our-server-products/
- Our own guides: [always-on.md](leyline/always-on.md) (sizes, the rented-server command),
  [be-a-node.md](leyline/be-a-node.md), [LEYLINE-MESH.md](LEYLINE-MESH.md) (caps, switches).
