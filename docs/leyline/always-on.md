# Keep your realm up 24/7

This page is for anyone who wants their realm to stay up day and night, so players can come in any time. You don't need to know much about computers. Follow the parts for your system and skip the rest.

There are two ways to do it:

- **On a PC at home.** A spare PC is best. Your everyday PC works too, if you leave it on.
- **On a small server you rent.** Read [When to rent a server instead](#when-to-rent-a-server-instead) first.

Already run a server of your own? Skip to [Put a server you already run on the list](#put-a-server-you-already-run-on-the-list).

## How players find you

You don't open any ports on your router, and nobody sees your home address.

Your realm calls out to the **Leyline network**: a go-between server we run. Players connect to it, and it passes them on to your realm. A public realm is on the realm list within a minute of starting. A private realm is reached only with your invite link.

If your internet drops for a moment, your realm calls back by itself.

## What PC is enough

| Part | Enough | Better |
|---|---|---|
| Memory (RAM) | 8 GB in the PC (the realm uses about 4 GB) | 16 GB, so you can still use the PC |
| Processor | Any 4-core processor from the last ten years | 6 cores or more |
| Disk | 30 GB free | An SSD, not an old spinning disk |
| Network | A cable from the PC to your router | The same. Wi-Fi works, but it drops more often |

A small, quiet "mini PC" is a good choice for a realm that stays on: it uses little power.

You also need your own copy of the game on that PC. The Host pack reads the map data it needs from your game folder once. **We never send game files and the Host pack never downloads them.**

## Step 1: Set up your realm

If your realm already runs, skip this step.

**Docker** is a free program that runs your realm in its own sealed space, so it doesn't touch the rest of your PC. You install it once.

1. Install Docker. Windows or macOS: [Docker Desktop](https://www.docker.com/products/docker-desktop/). Linux: `curl -fsSL https://get.docker.com | sh`.
2. Open the launcher and go to the **Host** page.
3. Get the Host pack there (one button), then pick **Vanilla+** and press **Start**.
4. Make your GM account on your own join link. The launcher shows it.

The first start takes 5 to 20 minutes. After that it takes about a minute.

**Vanilla+ starts "always on" by itself.** That means two things:

- Each part of your realm restarts by itself if it stops.
- On Linux, Docker is set to start when the PC starts.

Other rulesets restart by themselves too. On Windows and macOS you do the steps below for any ruleset.

## Step 2: Keep the PC awake

A sleeping PC is an offline realm. Turn sleep off. You can still let the screen turn off: that's fine.

### Windows 11

1. Open **Start**, then **Settings**.
2. Go to **System**, then **Power & battery** (on a desktop PC it's just **Power**).
3. Open **Screen, sleep & hibernate timeouts** (or **Screen and sleep**).
4. Set **When plugged in, put my device to sleep after** to **Never**.

### Windows 10

1. Open **Start**, then **Settings**.
2. Go to **System**, then **Power & sleep**.
3. Under **Sleep**, set **When plugged in, PC goes to sleep after** to **Never**.

### macOS

1. Open the **Apple menu**, then **System Settings**.
2. Desktop Mac: go to **Energy**. Turn on **Prevent automatic sleeping when the display is off**.
3. MacBook: go to **Battery**, then **Options**. Turn on **Prevent automatic sleeping on power adapter when the display is off**. Keep it plugged in.
4. If you see **Start up automatically after a power failure**, turn it on.

### Linux

On a desktop with GNOME: open **Settings**, then **Power**, and turn off **Automatic Suspend**.

On any Linux, this turns sleep off for good:

```
sudo systemctl mask sleep.target suspend.target hibernate.target hybrid-sleep.target
```

## Step 3: Start Docker when the PC starts

Your realm only runs while Docker runs.

### Windows and macOS (Docker Desktop)

1. Open **Docker Desktop**.
2. Click the **gear** (Settings) at the top.
3. On **General**, turn on **Start Docker Desktop when you sign in to your computer**.
4. Click **Apply**.

Docker Desktop starts when you **sign in**, not when the PC switches on. So after a restart, your realm comes back once someone signs in. To have it come back with nobody there, set the PC to sign in by itself:

- **Windows:** this is hidden in newer versions. Search the web for "Windows automatic sign-in" for your version, or just sign in after a restart.
- **macOS:** **System Settings**, then **Users & Groups**, then **Automatically log in as**, and pick your user. (This is off when FileVault disk encryption is on.)

Only do this on a PC that is in a safe place at home: anyone at the keyboard gets in.

### Linux

The launcher and the Host pack do this for you with Vanilla+. To do it by hand:

```
sudo systemctl enable docker
```

## Step 4: Updates without surprises

System updates sometimes restart the PC. That's fine: your realm comes back by itself after Step 3. You can still pick when it happens.

### Windows

1. Open **Settings**, then **Windows Update**, then **Advanced options**.
2. Open **Active hours**. Set it to the hours your players are on most (for example, 10:00 to 02:00). Windows won't restart in those hours.

Don't turn updates off. They keep your PC safe.

### macOS

1. Open **System Settings**, then **General**, then **Software Update**.
2. Click the **i** next to **Automatic updates**.
3. Turn off **Install macOS updates**. Keep **Install Security Responses and system files** on.
4. Install the big updates yourself, when few players are on.

### Linux

Updates don't restart the PC unless you set them to. Restart after a big update at a quiet time.

### Updating your realm

When there's a new Host pack, update it on the launcher's **Host** page, then press **Start** again. On a server you rent, run the same start command again. Your realm, characters and links are kept.

## Power, heat and cost

- **Power use.** A mini PC uses about 10 to 20 watts. A desktop PC that's mostly idle uses about 30 to 100 watts. A gaming PC uses more.
- **Working out the cost.** Watts x 720 (hours in a month) / 1000 = kWh a month. A 50-watt PC uses about 36 kWh a month. Multiply by the price per kWh on your power bill.
- **Heat.** Give the PC room to breathe. Don't put it in a closed cupboard. Clean the dust out of the fans now and then.
- **Noise.** A PC that runs all night is louder than you think in a bedroom. A hallway or office is better.

## What happens on a power cut

1. Your realm goes offline. Players are dropped and see it as offline on the realm list.
2. Characters are safe. The realm saves as people play. The last few seconds before the cut may be lost.
3. When power is back, the PC needs to switch on again. Many PCs can do this by themselves: in the PC's setup screen (BIOS, opened with a key like Del or F2 as it starts), look for **Restore on AC Power Loss** or **After Power Failure**, and set it to **Power On**. On a Mac it's the setting in Step 2.
4. Once the PC is on and Docker runs (Step 3), your realm starts by itself and is back on the list within a minute.

A small battery backup (a "UPS") keeps the PC on through short cuts. It's optional.

## Check it's back

- **In the launcher:** open the **Host** page. The status shows whether your realm runs and whether it's on the list.
- **On Linux or macOS:** run `leyline status` in the Host pack folder.
- **On Windows:** double-click `Check my realm.bat` in the Host pack folder.

## When to rent a server instead

A small server you rent by the month (often called a "VPS") is better when:

- your power or home internet drops often;
- you want your PC back for games, or to switch it off;
- you want the realm up for months without thinking about it;
- you expect more than a handful of players at once.

**What to rent:** a small Linux server with **4 GB RAM**, **2 cores** and about **30 GB of disk**. Any well-known provider works. Pick one near most of your players.

You still need your own game files. Copy your own game folder to the server first. Then:

```
curl -fsSL https://get.docker.com | sh
curl -fsSL -o host-pack.zip https://github.com/orionopenwow/Leyline/releases/download/host-kit/WrathUnleashed-host-kit.zip
unzip host-pack.zip && cd "Leyline host"
export WU_GAME_DIR=/path/to/your/game
bash leyline host --ruleset vanillaplus --public --always-on
```

It asks for your realm's name and your GM account name. Then make your GM account on the join link it prints (`leyline status` shows it again). A rented server doesn't sleep and starts Docker when it starts, so you're done.

On Windows, you can also double-click `Host a Leyline realm.bat` in the Host pack folder and type `vanillaplus` and `public`.

## Which ruleset

| Ruleset | What it is | Who can join |
|---|---|---|
| **Vanilla+** (`vanillaplus`) | The original world only, up to level 60, 1.5x experience. Made to stay up. | Anyone with the launcher or a browser |
| **Progression** (`progression`) | Normal speed from level 1. Each character moves through the three eras at their own pace. | Anyone with the launcher or a browser |
| **Classic levelling** (`main`) | The full world from level 1 at 3x, with an optional boost. | Anyone with the launcher or a browser |
| **True Vanilla** (pick it on the Host page) | A plain realm for the original client. The Host pack builds it on your PC. | Players with the original client |

## Put a server you already run on the list

For a server you already run, on any server software, here or elsewhere. Only a small helper runs next to it. The helper puts your server on the realm list and passes players from the Leyline network to your login server. **It never changes your server**: not its realm list, accounts, GM rights or files. It has no database login at all.

1. Install Docker and get the Host pack (Step 1).
2. Run the link command. Linux or macOS: `bash leyline link`. Windows: double-click `Link my server.bat`. In the launcher: the **Host** page, **Your own**.
3. Answer four questions:
   1. its name on the realm list;
   2. its login server, as this PC reaches it (usually `127.0.0.1:3724`);
   3. its world server (usually `127.0.0.1:8085`);
   4. the client it's for: `vanilla`, `tbc` or `wrath`.
4. Turn off your login server's wrong-password ban. Every player arrives from the helper's address, so one player's typos would lock everyone out. Set `WrongPass.MaxCount = 0` in your login server's config (`authserver.conf` or `realmd.conf`). The Leyline network limits wrong passwords for each player instead.

It's on the list within a minute. Players make accounts the way they do now.

Later: `leyline link status`, `leyline link stop` (offline at once, off the list after 6 hours), `leyline link remove`. Add `--always-on` to bring it back after a restart.

## Claim it for your account

A listed realm shows as "Unclaimed" until you claim it with your Hero Key. The website gives you a one-time code. Run `leyline claim CODE` (Windows: `Realm tools.bat claim CODE`). Only the PC that runs the realm can claim it. See [Hero Key](hero-key.md).

## If something's wrong

| What you see | What to do |
|---|---|
| "Docker is installed but not running" | Open Docker Desktop (Linux: `sudo systemctl start docker`). Then start your realm again. |
| Not on the list after a few minutes | Check it's back (above). "Not connected" means this PC can't reach the internet. |
| It didn't come back after a restart | Step 3. On Windows and macOS, someone has to sign in, unless the PC signs in by itself. |
| It goes offline at night | Step 2: the PC is going to sleep. |
| Players can't get past the login | (Your own server) check the addresses with `leyline link status`. |
| Players log in but can't enter the world | (Your own server) the realm address your server gives out doesn't reach players. Use the public address players use today. |
| Everyone gets "too many wrong passwords" | (Your own server) step 4 above: turn off the wrong-password ban. |

More: the [Hosting](hosting.md) guide has every command and setting.
