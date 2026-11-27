# Unstoppable Leyline: the tests only the owner can run

**For:** the owner, on real PCs. **Branch:** `feat-unstoppable` (the Passport, connections by key, worlds in three
clicks, characters that survive and the network with no centre, merged and unified). Everything below is built and
passes its tests **inside one process on one machine**. None of it has crossed two real home networks yet. These
five tests are the gates in docs/UNSTOPPABLE.md. Until a test here passes, the website keeps its "Coming" tag on
that part.

**Leyline's data folder** is `%APPDATA%\Leyline` on Windows (`~/.local/share/Leyline` on Linux,
`~/Library/Application Support/Leyline` on a Mac). "Send back" means paste it into the Discord thread or a reply to
the lead. Never send `peer.key`, `passport\device.key`, `passport\phrase.pending`, your 12 or 24 words, a backup
file or its 6 words.

Before you start, on every PC:

1. Install the build from this branch (CI artifact for `feat-unstoppable`).
2. Open Leyline once, then **Friends** and **Your Passport**. If it asks, give a name and press **Make my
   Passport**, write the words down, tick **I wrote these words down**, press **Done**.
3. In **Settings › Network**, check **Reach friends from anywhere** is on (it is by default).
4. Note the Leyline version (Settings, bottom) for every PC.

---

## 1. Two homes and a phone hotspot: join a world from anywhere

**Proves:** connections by key with no relay of ours (ways 1 to 4), the device key from the Passport, invite links
that carry the host's key, and the realm list rewrite through the forwarder.

**You need:** PC **A** (your home, hosts), PC **B** at another home (a friend's), and a phone with a hotspot. A and
B are friends on the Friends list (add with the friend code `LF1-…` first, over Discord). Both have a Wrath game
folder added.

Steps, with timings (a phone stopwatch is fine):

1. **A:** **Host** › **Your own world** › **Start**. Wait for **World running**. Note the seconds.
2. **A:** **Invite a friend**. Check the ways list says **From anywhere: on**. Press **Copy** on the
   `https://leylinelauncher.com/j/#lw1.…` link and send it to B over Discord.
3. **B (at the other home):** click the link in Discord. The browser page opens Leyline (allow it). Leyline shows
   **Join <A's name>'s world?** Press **Join**. Start the stopwatch on the click.
4. **B:** when the game reaches the character screen, stop the watch. Make a character, enter the world, walk
   around for 2 minutes, log out.
5. **B:** move the laptop to the **phone hotspot** (turn off home Wi-Fi). Open the same link again and **Join**
   again. Note whether the same character is there.
6. **A:** turn off **Help friends connect** on a third friend's PC **C** if you have one; repeat step 5 with C on and
   online, and A's router blocking UDP if you can (or B on a mobile network that blocks it), to force "a friend you
   both know".

**Send back** (one block per join):

```
Join N: B on <home Wi-Fi | hotspot>, A on <ISP>, C online <yes/no>
Clicks from the link to the character screen: <n>   Seconds: <s>
Worked: <yes/no>   Same character as last time: <yes/no>
Way used: <B: Settings › Network shows the last way for A>   (same network / direct / hole punching / a friend / community relay)
If it failed: the text under "tried" on B's Join card, word for word
B: %APPDATA%\Leyline\logs\launcher.log, the last 200 lines (no keys are written there above debug level)
```

---

## 2. A fresh PC: three clicks, with timings

**Proves:** worlds in three clicks (Host › template › Start), the prepared database and the background readiness,
on a PC that has never run Leyline.

**You need:** a PC (or a fresh Windows user account) with no `%APPDATA%\Leyline` folder, and a game folder.

1. Install Leyline. Open it. Add the game folder when it asks. Start a stopwatch when the folder is added.
2. Wait on Home without clicking for 2 minutes (the background readiness fetches the engine and reads the maps).
3. Press **Host**, **Your own world**, **Start**. Count every click from Home. Stop the watch at **World running**.
4. Press **Play**; check you reach the character screen.
5. Press **Stop**. Then **Host**, **The original world**, **Start** (the second template should copy the prepared
   database). Time it.
6. Stop it; start **Your own world** again from its card. Time it.

**Send back:** fill the tables in [worlds-three-clicks.md](worlds-three-clicks.md) from
`%APPDATA%\Leyline\world-timings.log` (paste the whole file too), plus:

```
PC: <CPU, RAM, SSD or HDD, Windows version>
Clicks from Home to World running: <n>   Seconds (stopwatch): <s>
Second template seconds: <s>   Started again seconds: <s>
Size of %APPDATA%\Leyline\engine-cache: <GB>
```

---

## 3. Recovery on a new PC: with the phrase, then with two guardians

**Proves:** the Passport brings the same identity back, friends and worlds follow, and characters come back from
friends' sealed copies.

**You need:** your PC **A** with characters saved (play a world and press **Characters** › **Back up now** first),
friends **B** and **C** online on the Friends list, and a **new PC N** (or a fresh Windows account).

### 3a. With the phrase

1. **A:** **Characters** › **Back up now** for your world. Check each character shows "kept by 3 friends" (or 2 if
   you have two). Close Leyline on A (it stays your old PC, untouched).
2. **N:** install, open **Friends** › **Your Passport** › **I already have one** › **With your recovery phrase**.
   Type the words. Restart Leyline when it says so.
3. **N:** check the Friends list shows B and C as friends (not requests), and your friend code is the same as A's.
4. **N:** **Characters** › **Get my characters back from friends**. Wait a minute.
5. **N:** **Your Passport** › **Devices**: check N is listed.

### 3b. With two guardians (do this on another fresh account, or after 3a on N with the phrase "forgotten")

1. **A (before):** **Your Passport** › **Back up** › **Guardians**: tick B and C (and one more if you can), **Save
   guardians**.
2. **N:** **Your Passport** › **Recover** › **Ask 2 guardians**. Copy the `LR1-…` code; send it to B and C.
3. **B and C:** **Your Passport** › **Help a friend**, paste the code, **Make my answer**, **Copy**, send back the
   `LG1-…` answer.
4. **N:** paste both answers (**Add the answer** twice). Write down the new 12 words.
5. **A (still on):** check a **Stop it** warning appears within a few minutes. Do **not** press it.
6. After 3 days: on B, check A's entry is the new key (same name, Friends list shows one entry, not two).

**Send back:**

```
3a: friends back without new requests: <yes/no>; friend code same: <yes/no>
3a: characters back: <n of m>, the Characters page lines for each (name, seq, "kept by")
3a: Devices list: <names>
3b: guardian answers accepted: <yes/no>; "Stop it" shown on A after <minutes>
3b: after 3 days, B sees the new key as the same friend: <yes/no>
Anything that said "couldn't" or "not delivered": the words, as shown
```

---

## 4. Re-home after deleting a world

**Proves:** a character's signed save brings it into another world on the same PC under that world's rules (level
cap, items to the vault, no gold).

1. Play **Your own world** with a character above level 60 that has gold and a few items. Stop the world.
2. **Characters** › **Back up now**.
3. Delete the world: **Host**, the world's card, **Delete** (or remove its folder with Leyline closed). Confirm the
   character is gone from that world.
4. **Characters**: under the character, **Bring to another world** › **The original world** › **See what comes**.
   Screenshot the preview. Press **Bring**.
5. Start **The original world**, **Play**, log in: check the character, its level (60), mail with items, no gold.

**Send back:** the preview screenshot, then

```
Level before / after: <n> / <n>   Gold after: <n>   Mails: <n>   Items in the vault list: <n>
Name kept or renamed at login: <kept/renamed>
Any error text, word for word
```

---

## 5. A community relay on your PC, so hole punching has a coordinator

**Proves:** way 3 (hole punching) with a helper only a player runs, and way 5 (community relay) for strangers.
Leyline runs no helper; until a player runs one, hole punching is skipped (docs/leyline/joining.md).

**You need:** your PC **A** reachable from the internet on one TCP port (forward **3340** on your router to A), and
two friends **B** and **D** on different home networks who are **not** friends with each other.

1. **A:** install the hole-punch helper (iroh's relay server, MIT/Apache-2.0):
   `cargo install iroh-relay --version 1.5.0 --features server` (the same version as the launcher's iroh; or the release binary from the iroh project), then run
   `iroh-relay --dev` (plain http on port 3340; fine for this test). Leave the window open.
2. **A, B and D:** close Leyline, open `%APPDATA%\Leyline\link.json` in Notepad (create it if missing) and set
   `"helpers": ["http://<A's public IP>:3340"]` (keep the other fields). Save, open Leyline. (There is no box for
   this in Settings › Network yet.)
3. **A:** **Host** › **Be a node** on, then **Settings › Network** › **Carry sealed connections for other players**
   on. Check it says **Carrying for other players now.**
4. **B:** host **Your own world**, **Invite a friend**, send D the link. **D:** open it and **Join**.
5. **D:** after joining, **Settings › Network**: note the way shown for B.
6. Stop `iroh-relay` on A, **B** and **D** turn off **Help friends connect**, and D joins again: it should go through
   A as a community relay (way 5) if hole punching can't.

**Send back:**

```
iroh-relay version: <v>   A's port open from outside (https://canyouseeme.org or similar): <yes/no>
Join 1 (helper running): worked <yes/no>, way <...>, seconds <s>
Join 2 (helper stopped): worked <yes/no>, way <...>, seconds <s>
A: Settings › Network "helped today" and "refused" numbers after both joins
The iroh-relay window's last 30 lines (it logs connection counts, not keys)
```

---

## What the results change

| Test | Passes | The lead then |
|---|---|---|
| 1 | joins from both homes and the hotspot | removes "Coming" from "Same network first, then direct, then hole punching" only for the ways that showed up; keeps it for the rest |
| 2 | under a minute on the second template | fills the phase 0 table; "Three clicks" stays as is |
| 3 | friends and characters back on N | removes "Coming" from the recovery phrase, guardians and sealed backups |
| 4 | character arrives with the rules | removes "Coming" from moving a character between worlds on one PC |
| 5 | way 3 or 5 used | removes "Coming" from "a friend relays for you, then any community relay" |
