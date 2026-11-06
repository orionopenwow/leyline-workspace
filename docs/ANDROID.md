# Leyline on Android

The phone app is the same Leyline launcher (`launcher/`, Tauri 2) built for Android. It plays the browser game inside
the app, and shares the launcher's sign-in, realm list and mod browsing. Everything that needs a computer stays on the
desktop launcher. The plan is the Claude Doc "Leyline on Android".

## What the app does

- **Play** (the first tab). The big PLAY button opens the website's Play page (`https://leylinelauncher.com/wrath`: the
  usual sign-in, then Play, which goes on to `play.leylinelauncher.com`) inside the app, full screen. The touch controls
  come from the page itself. The phone's **Back** button returns to the launcher.
  - The game draws with WebGPU. If the phone's web view has no `navigator.gpu`, PLAY doesn't open a black page. It
    shows "This phone's web view can't draw the game yet: open in Chrome" and an **Open in Chrome** button, which opens
    the same Play page in the phone's browser.
- **Play solo on this phone** (under PLAY; docs/PHONE-SOLO.md). The same game page with `go=solo:phone`, through the
  site's Play: the page runs its own server (Den Lite in a worker), so the game plays with no realm. It needs a web view
  that allows shared memory on the cross-origin-isolated game page; one that doesn't gets one line and plays on the
  realm. No WebGPU: the same Open in Chrome line as PLAY.
- **Join a friend** (on Play): a friend's invite link to the realm they host on their PC plays it in the app.
  **Paste invite link and join** reads the clipboard (one tap); the box and **Join** take a typed link. The app opens
  the site's browser hop, `/wrath?go=r:<id>#inv=<invite>`: the invite stays in the fragment, never sent to a server,
  and the realm's own join page makes the friend's account. A friend's Den link, a `DEN-…` code, a link on another
  site or an empty clipboard each get one line, and nothing opens (docs/MOBILE-RELEASE.md).
- **Realms.** The Official and Community lists, as on the desktop.
  - The official realm's Play is the browser game (the same as PLAY).
  - A community realm for the browser game's client plays in the app too (`/wrath?go=r:<id>`, as the website's Play
    in your browser). Any other says it plays with your own game on a computer.
  - "Got an invite link?" takes a friend's invite link (the same as Join a friend). A Den link or code says Dens run
    in the game on a computer.
- **Mods.** Mods & Addons › Discover: Mods, Addons and Looks, with search and the sort tabs, for browsing only.
  - A line at the top says addons work on the phone (the game page's Your addons, from the addon's .zip), and that
    mods, loadouts and looks install into a game folder, on a computer.
  - Each card says whether it works on a phone: an addon yes, a realm mod on a realm that runs it, a mod with both
    halves its realm half only, a client mod no.
  - The Install, Update and Use buttons are hidden. Looks are shown for their information.
- **Account.** Log in with your Hero Key, Google, your name and password, or a new guest account (`ui/gate.js`,
  `ui/account.js`). Logging in with the browser opens the approval page in Chrome. The app sees the approval by itself.
- **News, Forum and Join the Discord** are linked from the Play tab.

## What is hidden on a phone, and why

These need the game on a computer, a computer's files or a local server, so the phone build leaves them out:

- game folders;
- launching the native game;
- hosting realms, Your Den and Den Lite, Docker;
- writing patches;
- installing mods, addons, looks, themes and graphics settings;
- Project Reforged;
- Single player;
- Agents;
- the desktop's Settings page;
- the first-run tour.

**Be a node** is hidden too. The node code has not been built or tested for Android, and a phone would also need
"only on Wi-Fi and while charging" checks. The Play tab's "On your computer" card says all this in one line.

Native play on the phone (a translation layer like Winlator) is out of scope: that is phase 3 of the plan.

## How it is put together

| Part | Where |
|---|---|
| Phone entry point and commands | `launcher/src-tauri/src/mobile.rs`. `#[tauri::mobile_entry_point] pub fn run()`, and only the shared commands: `platform`, `get_state` (no game folders), the realm list and status, the mods catalog and ranking, links, the Hero Key sign-in (`social::commands`), forum, news and game modes. The app keeps its files in its own data folder (`LEYLINE_APP_DIR` is set to Tauri's `app_data_dir`). |
| Desktop app | `launcher/src-tauri/src/app.rs`, compiled only for desktop targets (`cfg(desktop)`). The desktop-only command modules (client import, graphics, addons, loadouts, listing, bugs, themes, Dens, agents, looks, p2p) are gated the same way. |
| cfgs | `launcher/src-tauri/build.rs` sets `desktop` or `mobile` for the whole crate, and `torrent_engine` only on desktop. `librqbit` and `tokio` are desktop-only dependencies in `Cargo.toml`. |
| The platform flag | `lib.rs` `platform()` returns `{ os, mobile, node }` through the `platform` command (desktop answers it too). `ui/platform.js` reads it first and sets `html.phone` and `data-platform="android"`. An older core counts as a computer. |
| Phone layout | `ui/phone.css` (everything under `html.phone`) and `ui/phone.js`: the tab bar, PLAY, the WebGPU check, Realms, Mods, and sending desktop-only pages back to Play. |
| Android config | `launcher/src-tauri/tauri.android.conf.json`: identifier `com.leylinelauncher.app`, minSdk 26. `capabilities/mobile.json` holds the phone's permissions (`core:default` only; internet comes with the app, and no storage permission is asked for). |
| Full-screen activity | `launcher/src-tauri/mobile/android/MainActivity.kt`. CI puts it over the generated one: the system bars are hidden, and a swipe shows them. |
| Smoke test | `launcher/scripts/android-smoke.sh` |
| Workflow | `tools/github-workflows/leyline-android.yml`. Its expectations are in `tools/github-workflows/test_workflows.py`. |

The Android project (`launcher/src-tauri/gen/android`) is never committed. CI makes it fresh with
`tauri android init` each time.

## How CI builds it

The workflow `leyline android` runs in the public Leyline repo:

- **When:** on every push to main that changes `launcher/`, on pull requests that change it, and by hand
  (Actions › leyline android › Run workflow).

**Job `apk`:**

1. Sets up Java 17, the Android SDK, platform 34, build-tools 34 and NDK r27 (via `sdkmanager`), Rust with the Android
   targets, and the Tauri CLI from `launcher/package-lock.json`.
2. Runs `tauri android init`, swaps in the full-screen activity, and makes the icon from
   `stream/www-files/art/leyline-crystal-512.png`.
3. Runs `tauri android build --debug --apk --target aarch64 --target x86_64`. It checks the APK holds no game data.
4. Uploads the artifact **leyline-android-debug** (`Leyline-debug.apk` and its sha256). It is debug-signed: it installs
   on any phone that allows unknown apps.
5. **Release** runs only when the release key is in the repo's secrets: `ANDROID_KEYSTORE_B64` (the keystore file,
   base64), `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` and `ANDROID_KEY_PASSWORD`. It builds for arm64 and armv7,
   then aligns, signs and verifies the APK, and uploads the artifact **leyline-android-release** (`Leyline.apk`). The
   keystore is decoded only into the runner's temp folder and deleted. Without the secrets this part is skipped.

**Job `smoke`:**

1. Boots an Android 14 (API 34) x86_64 emulator, installs the debug APK and starts it.
2. Waits 30 s, takes a screenshot upright and one sideways, and keeps the log.
3. Fails if the app crashed (a Java `FATAL EXCEPTION` or a native `Fatal signal` in its process) or stopped running.
4. Uploads the artifact **android-smoke** (`portrait.png`, `landscape.png`, `logcat.txt`, `app-log.txt`, `ui.xml`),
   even when it fails.

**Job `publish`** (pushes to main and Run workflow on main only, never pull requests; only after `smoke` passed):
puts `Leyline-debug.apk` (and `Leyline.apk` with the release key) with their `.sha256` on the rolling pre-release
**android-latest** of the repo it runs in. The only job with write access (the built-in token).

## Turning it on (the owner)

1. **Ship** (Leyline Control), so the `leyline` step exports the launcher with the Android changes into the Leyline
   repo.
2. Run **`setup\Turn on GitHub builds.bat`**, the same one as for the other builds. It now also pushes
   `leyline-android.yml` to the Leyline repo (`tools/github-builds/github_builds.py`, `LY_FILES`). Running it again is
   safe.
3. The first run starts by itself with the next launcher change. To start one now: github.com/orionopenwow/Leyline ›
   Actions › **leyline android** › Run workflow.
4. *Optional, later:* a release key, so updates install over each other without reinstalling.
   - On your PC, run `keytool -genkeypair -v -keystore leyline-release.jks -alias leyline -keyalg RSA -keysize 4096 -validity 10000`
     (keytool comes with Java). Keep the file and its passwords safe and never put them in the repo.
   - Add the four secrets to the Leyline repo (Settings › Secrets and variables › Actions). For example, run
     `gh secret set ANDROID_KEYSTORE_B64 -R orionopenwow/Leyline` and paste the base64 of the file.

## Getting the APK onto a phone

0. Once the `publish` job has run (main only, after the emulator passed), the APK is on the rolling pre-release
   **android-latest**: `https://github.com/orionopenwow/Leyline/releases/download/android-latest/Leyline-debug.apk`
   (and on Leyline-Mobile's releases once that repo exists: docs/MOBILE-RELEASE.md). No GitHub login needed. Or:
1. On github.com/orionopenwow/Leyline › Actions › **leyline android**, open the newest green run. Under **Artifacts**,
   download **leyline-android-debug** (or **leyline-android-release** once the release key is set up). It is a zip with
   `Leyline-debug.apk` inside. You need to be logged in to GitHub to download artifacts.
2. Put the APK on the phone (USB, a cloud drive, or download it on the phone itself) and tap it.
3. Android asks to allow installing from that app (the Files app or the browser): **Settings › Allow from this source**
   ("Install unknown apps"). Then **Install**. Play Protect may warn about an unknown developer: **Install anyway**.
4. A debug APK and a release APK are signed with different keys. To switch from one to the other, uninstall first.

## Phone test checklist

- [ ] The app opens on the Play tab, with no desktop menus, window buttons or play bar.
- [ ] The tab bar has Play, Realms, Mods and Account, and each opens its page.
- [ ] It works upright and sideways. Nothing is cut off at the right, and nothing scrolls sideways.
- [ ] Every button is easy to hit with a thumb.
- [ ] PLAY opens the website inside the app, full screen. Logging in on the page works.
- [ ] Play reaches character select, and the touch controls show.
- [ ] Back returns to the launcher's Play tab, and PLAY again goes back to the game.
- [ ] On a phone whose web view can't draw the game: the line and **Open in Chrome**, which opens Chrome on the Play
      page.
- [ ] Realms: both lists load. The official realm's Play opens the game. A community realm's Play gives the "on a
      computer" line.
- [ ] Realms › Got an invite link?: a `DEN-…` code and Join say the Den needs a computer.
- [ ] Join a friend: a friend presses Invite a friend on their PC (a full realm); copy the link on the phone, tap
      Paste invite link and join: their realm's join page opens in the game, make the account, play.
- [ ] Realms: a community realm for the browser game plays in the app.
- [ ] Mods: Discover lists mods, addons and looks. There are no Install buttons, and the line at the top says why.
- [ ] Account: log in with the Hero Key, then Google. Logging in with the browser opens Chrome and comes back logged in.
- [ ] Play solo on this phone: the game opens, logs in and enters the world by itself, with no realm (docs/PHONE-SOLO.md
      "Owner presses" first). Close the app and open solo again: the world is as you left it.
- [ ] News, Forum and Join the Discord open.
- [ ] Close the app and open it again: you are still logged in.
- [ ] No banned words anywhere, and no game client version numbers in sentences.

## Not checked yet

- **CI has not built the app yet.** The Rust and Android build has not been compiled anywhere. Nothing in this cloud
  can run cargo for Android, the Android SDK or an emulator. The first CI run is the first build. If it fails, the log
  of the step that failed says why.
- **Orientation is not locked.** The app turns with the phone, and the PLAY tab asks you to turn the phone sideways.
  Locking the game to landscape needs a small Android plugin.
- **Back while playing.** Back goes back through the website's own pages first, then to the launcher.
