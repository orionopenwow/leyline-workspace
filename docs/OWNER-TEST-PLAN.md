# Owner test plan: a new machine, every way in

Owner, 3 Nov: "Fully tested, rounded featuresets, p2p hosting flawless, hero key intuitive, few steps as possible to get
everywhere. Can log in and out, share character, everything makes sense on new machine, then give me what to test to
make sure all kinks are out."

**Before you start:** run the Control steps `signin` (the website), then `ship`, and wait for the launcher build on
GitHub. Then run `first-run-test`. It covers lines 1 to 16 by itself; its log is `tools\first-run\logs\first-run-latest.txt`.

**How to use it:** do each line in order: *Do* this, then *you should see* that. Write the number of any line that
doesn't match and send it with a screenshot. The **Auto** column names the Control step that already checks it.
"(UI test)" means the launcher's own tests in GitHub CI. "—" means only you can check it.

**A new machine:** best is a second PC or a fresh Windows user account. Otherwise close Leyline and open the fresh copy
the test leaves behind:

```
tools\first-run\First run test.bat --keep
set LEYLINE_APP_DIR=<the folder the log names>\data
set WEBVIEW2_USER_DATA_FOLDER=<the folder the log names>\webview
.launcher\first-run\Leyline.exe
```

## A. Install and first open

| # | Do | You should see | Auto |
|---|---|---|---|
| 1 | Download the launcher from leylinelauncher.com/wrath/download and open it | Windows asks once (More info, Run anyway). No other prompt and no error | — |
| 2 | Look at the first screen | The launcher opens on Home at once: no sign-in wall. Log In is top right, and a 4-step tour shows | (UI test) firstrun |
| 3 | Go through the tour with Next | 4 steps; the last is "Find your game files"; Done closes it | (UI test) firstrun |
| 4 | Look at the play bar, bottom left | "GAME FOLDER: No game folder" | (UI test) playbar |
| 5 | Click it, then Find your game files, and pick your Wrath folder | "Added …". The bar shows the folder and READY TO PLAY | first-run-test (6) |
| 6 | Open Settings | The first thing is "Your account". The peer-to-peer part is at the bottom | (UI test) gate |

## B. Log in, each way (log out between each with your picture top right, then Log out)

| # | Do | You should see | Auto |
|---|---|---|---|
| 7 | Log In, then Create a guest account (2 clicks) | You're in at once with a made-up name. A note: "Save your login any time" | (UI test) gate |
| 8 | Settings, then Save your login, then Save as text file | `Leyline Hero Key.txt` saved. It has your name#tag, a password and the key | (UI test) gate |
| 9 | Your picture, then Log out (2 clicks) | Log In comes back. The message says "Logged out" | (UI test) gate |
| 10 | Log In | "Log in as Name#1234" at the top, from the file in Downloads. One click logs you in | (UI test) gate |
| 11 | Log out. Log In, then type your name#tag and the password from the file | Logged in | (UI test) account |
| 12 | Log out. Log In, Log in with your Hero Key, then drop the file on the box | Logged in | (UI test) gate |
| 13 | Log out. Log In, Log in with your browser (2 clicks), then approve in the browser | The launcher goes on by itself. A browser already logged in only asks you to approve | (UI test) gate; `audit` |
| 14 | Log out. Log In, then Continue with Google | Google in your browser, then you're logged in | — |
| 15 | On the website, Log in, then Create a Hero Key, then download it; open the launcher logged out | It logs in by itself (0 clicks): "Logged in as … with the Hero Key you downloaded" | (UI test) gate |
| 16 | Anywhere: look for the words "Sign in" or "Sign out" | There are none. It's always Log in and Log out | (UI test) gate |

## C. Play the official realm

| # | Do | You should see | Auto |
|---|---|---|---|
| 17 | Logged out, press PLAY on Wrath Unleashed | The Log in screen, saying why. Once you're in, the game starts by itself (no second PLAY) | (UI test) gate |
| 18 | Logged in, press PLAY | The game opens and goes past the login screen to character select by itself | first-run-test (7); `client-smoke` |
| 19 | Make a character and enter the world | You're in the world within a minute | first-run-test (7, test character) |
| 20 | Close the game, then look at the game folder | No one-time login left behind | first-run-test (7); `client-smoke` |
| 21 | Realms, Community, then Play on a community realm | That realm's game starts. The play bar shows the community realm | (UI test) gate, realms |
| 22 | Pick Wrath Unleashed again in the play bar's realm list, then PLAY | The official realm, logged in by itself. No old message left in the bar | (UI test) gate, playbar |

## D. Your account on another PC ("share character")

Your characters live on the realm, so they come with your account. To play on another PC you move your **account**:
the Hero Key file or your name and password. You never share it with another person. To play together, add each
other as friends or invite them to your Den.

| # | Do | You should see | Auto |
|---|---|---|---|
| 23 | Your picture, then Hero Key, then "Play on another PC" | 3 short steps: the file, Downloads, open Leyline. It says your characters are on the realm | (UI test) account |
| 24 | Copy `Leyline Hero Key.txt` to the other PC's Downloads, then open Leyline there | Logged in by itself as you, with no clicks | (UI test) gate |
| 25 | On the other PC, PLAY | The same characters as on the first PC | — |
| 26 | Back on the first PC, Hero Key, then Make a new key | It asks first: "Your old key stops working at once". After that the old file no longer logs in | (UI test) account; `audit` |

## E. Your Den

| # | Do | You should see | Auto |
|---|---|---|---|
| 27 | Pick Your Den in the play bar's realm list, then Play (no login needed) | The game opens on the white island, logged in by itself | `den-test`; first-run-test (8) |
| 28 | Walk with W/A/S/D | You walk; nothing pops up | `den-test` |
| 29 | Open the gallery, place a piece, then undo it | The piece shows, then goes | `den-test` |
| 30 | Type `.den save test1`, then look at Saved Dens in the launcher | "test1" is listed | `den-test` |
| 31 | Invite a friend | One click copies an invite link. **Today:** on Den Lite it's "Share with friends", which needs Docker; one-click invite is Docker Den only | `den-test` (test friend) |
| 32 | Your friend opens the link on their PC | They join your Den with their own account. They never type your password | `den-test` (test friend) |

## F. Host a community realm (P2P) and the network

| # | Do | You should see | Auto |
|---|---|---|---|
| 33 | Host, Host a realm: read the top | It says what you need (Docker, your game folder, about 10 GB) and how long it takes | (UI test) hostconsole |
| 34 | Pick a ruleset, then Start | Steps with progress. It ends "Your realm is up" | (UI test) hostconsole; `realm` |
| 35 | Copy invite link, then send it to a friend | The friend's launcher opens the realm from the link. No ports to open, and nobody sees your address | (UI test) realmlink, hostlink |
| 36 | The friend presses Play | They are in your realm through the Leyline network | `check` (relay) |
| 37 | Set your realm to Public | It's on everyone's Community list within a minute | (UI test) hostnetwork |
| 38 | Settings, peer-to-peer (bottom), Check my network | A plain answer, such as "direct connections can work" | (UI test) hostnetwork |
| 39 | Become a node: "Help the network: relay for others" (when the website offers it) | It asks for a public name. Start relaying says it started | (UI test) hostnetwork; `relay` |
| 40 | Stop hosting, then open the Community list | Your realm is gone from the list | — |

## G. Mods, browser game, website, Discord

| # | Do | You should see | Auto |
|---|---|---|---|
| 41 | Mods & Addons, Discover, Addons, then Install on one | "Your first mod or addon is in". It shows under Installed › Addons | (UI test) firstrun, addons |
| 42 | Remove it again | It's gone from Installed | (UI test) addons |
| 43 | The play bar's realm list, then Play in your browser | The game in a browser tab, nothing to install | `check`; `audit` |
| 44 | Home, Azeroth Survivors, then Play | The mode opens at survivors.leylinelauncher.com | `sv-site-check` |
| 45 | Open leylinelauncher.com/wrath/key logged out | Log in (key file, paste or this browser's key), then Google, then Create a Hero Key | `audit`; `signin` self-check |
| 46 | Create a Hero Key there | A made-up name and your key shown once, with Download and Copy | `audit` |
| 47 | Log in on the website with that key | You're logged in. Your profile shows the made-up name | `audit` |
| 48 | Join the Discord (side menu) | The Discord invite opens in your browser | `discord-check` |
| 49 | Look at every page for the game's or publisher's names | None, except the legal line "not affiliated with or endorsed by Blizzard Entertainment" | `brand-check`; (UI test) firstrun |

## H. After the round

| # | Do | You should see | Auto |
|---|---|---|---|
| 50 | Run `audit` | All lines PASS, or the same known lines as last time | `audit` |
| 51 | Run `first-run-test` again | PASS. The test account is closed and the temp folders are gone | `first-run-test` (10, 11) |

**Where "Auto" lives:** UI tests are `launcher/tests/*.browser.test.cjs`, and CI runs them on every ship. The Control
steps are on Leyline Control. `first-run-test` numbers its log lines: 1 is the download, 2 a fresh start, 3 your game
folder, 4 the test account, 5 the test character, 6 the launcher (folder added, logged in), 7 PLAY (by itself, the
world, the screenshots, the one-time login removed), 8 the Den test, 9 your own launcher untouched, 10 the clean-up,
11 no secrets in the log.
