# Site shots

Retakes the website's launcher screenshots (`stream/www-files/art/leyline-ui-*.webp`) from the launcher UI as it ships
(`launcher/ui`, the serverless app). It runs in Chromium with a fake Tauri core, the same way `launcher/tests/*.browser.test.cjs` do.

    NODE_PATH=$(npm root -g) node tools/site-shots/shots.cjs            # writes into stream/www-files/art
    NODE_PATH=$(npm root -g) node tools/site-shots/shots.cjs /tmp/out   # or anywhere else
    SHOTS_PNG=/tmp/png SHOTS_ONLY=hero,hosting node tools/site-shots/shots.cjs

| File | Size | Shows |
|---|---|---|
| `leyline-ui-clients.webp`, `-1280.webp` | 3200x1800, 1280x720 (1600x900 window) | Home, the game folder list open, a realm picked, PLAY |
| `leyline-ui-launcher.webp` | 2480x1520 (1240x760 window) | Home, the play bar's realm list open (Single player, Your Den, your realms) |
| `leyline-ui-realms.webp` | 2480x1520 | Realms: other servers |
| `leyline-ui-hosting.webp` | 2480x1520 | Host › More ways to host: the template tiles (Host / Play solo / Fork) |
| `leyline-ui-modes.webp` | 2480x1520 | Modes (Boss Rush) |

The windows are drawn at 2x and saved as WebP at quality 82 (needs python3 with Pillow). The run fails (exit 1) and
prints PROBLEM when a page throws an error or shows a game or publisher name, "Log In", "official realm" or "Wrath Unleashed".
Look at every picture before you commit: the check doesn't catch everything.

The sample data is all made up: the realm names, `.example` addresses and mod authors. The hero slide's picture is the
launcher's own nebula (`launcher/ui/art/leyline-bg.webp`), never a game screenshot. The script refuses every other request.
When the UI changes, change the fake core in `shots.cjs`. A core command it doesn't know gets null, and the run lists
those commands.
