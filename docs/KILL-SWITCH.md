# The kill switch

Owner, 3 Nov 2026: "Kill switch — even Discord and git source code"; "upgrade Leyline Control to have the kill
switch and be able to delete everything and send a message to the community".

This is a card in Leyline Control (`tools/control/control.py`). The code is `tools/killswitch/killswitch.py`, its
server half is `tools/killswitch/server/ks_server.py`, and the website half is a small hook in
`stream/wrath_portal.py` (`shutdown.json`).

## Rules it keeps

- **Owner only.** The card's buttons are the only way in. It is not a step: it is never in `UPDATES-PENDING.txt`,
  never run by Autopilot, Launch all or a Waiting on you button. `/api/do` refuses it.
- **Preview first.** Every stage needs a preview from the last hour, with the same message, parts and choices. The
  preview only reads. It lists exactly what will happen, part by part, and anything that blocks it.
- **Typed phrases.** Pause needs `SHUT DOWN LEYLINE`. Delete needs `DELETE LEYLINE FOREVER`. Undo pause needs
  `BRING LEYLINE BACK`.
- **Pause before delete.** Delete works only on parts that a finished Pause took down, and only 24 hours after that
  Pause (`DELETE_WAIT`), so a bad day can't become a permanent loss.
- **Backup before delete.** Delete starts with a full backup to the Desktop
  (`Leyline-killswitch-backup-<time>`), checks every file, and stops, deleting nothing, if any part of it fails.
- **No secrets in logs.** The Discord bot token is the one the Discord card saved (`tools/discord-leyline/token.txt`).
  It is hidden in every line. The GitHub sign-in stays inside `gh`; the tool never reads or prints it.
- **Never Chumpton.** GitHub work runs only when `gh` is signed in as `orionopenwow`. Any other account stops it.
- **Dry run.** A tick box. The run goes through every check and logs what it would do, and changes nothing.
- **Log.** `logs\killswitch-<time>.log`, one per preview or run.
- **No DNS, no money, no accounts.** DNS stays the owner's. Nothing here creates accounts or types passwords.
- **Server.** Only our own files, SOAP, and existing Control steps. Never `rollback.sh` or `docker compose up` in
  `/opt/wenilla-realm`.

## The card

1. **Farewell message**: one text box. It starts as `TODO(owner)` text, and Pause refuses while `TODO(owner)` is
   still in it, or while it has a brand word. At most 1800 characters (Discord's limit, with room).
2. **Parts** (tick boxes): Community message, Realms and network, Website, Discord, GitHub. Choices: make the
   GitHub repos private on Pause (off by default); Dry run.
3. **Preview**: runs the read-only checks and shows the list.
4. **Pause**: type `SHUT DOWN LEYLINE`, press Pause.
5. **Delete** (shows only after a Pause): its own preview, type `DELETE LEYLINE FOREVER`, press Delete.
6. **Undo pause** (shows only while paused, before any Delete): type `BRING LEYLINE BACK`.

## What each part does

| Part | Pause (recoverable) | Delete (only after Pause) | Manual (the card lists it) |
|---|---|---|---|
| **Community message** | Discord: posted in the announcements channel and pinned. Website: a banner on every page. Launcher: the top post in News (`news.json`) and the update notice (`version-policy.json`). Realm: `announce` and the message of the day over SOAP | Nothing: the message stays | |
| **Realms and network** | "Stop every node" on (`nodes_off`), a relay staff block on every realm in `realms.json`, then the existing off steps: `hangout-off`, `vr-beacon-off`, `build-realm-off`, `ov112-realm-off`, `sv-site-off`, `gamedata-off` | Nothing more by itself | Turn the main realm's machine off at the host, if wanted. Seeded realms: the retire switch in `docs/SEEDED-REALM-PLAN.md` section 6 is not built yet; `nodes_off` covers them |
| **Website** | Every page answers the farewell page (503). The feeds the launcher reads (`/wrath/leyline/*.json`), `/wrath/status.json`, the art and HQ (`/wrath/admin`) keep working | Portal data backed up, then wiped on the server. The farewell page stays | DNS: remove or point the names away (Squarespace). Cancel the server when done |
| **Discord** | Every channel read-only for `@everyone` (posting, threads, reactions, voice). The old permissions are saved for Undo | Every channel the bot can delete, except the one with the farewell post | **Delete the server**: only its owner can, in the Discord app (Server Settings > Delete Server). A bot can't |
| **GitHub** | `orionopenwow/leyline-workspace`, `orionopenwow/Leyline`, `orionopenwow/Leyline-Launcher` archived (read-only), and made private if ticked | Those repos deleted. Needs the `delete_repo` scope: the owner runs `gh auth refresh -h github.com -s delete_repo` himself | The openvanilla.app Pages site goes with Leyline-Launcher |

Pause order: the message first (Discord, launcher news, website banner, realm), then the realms, the website, Discord,
GitHub. A failed existing step in Pause is logged and the rest goes on (in a takedown, more down is better). Any
failure in Delete stops it.

Undo pause: the website banner and farewell page off, the news post and update notice taken back, `nodes_off` and the
staff blocks the Pause added removed, Discord's saved permissions put back, the repos unarchived (and public again if
the Pause made them private). The realm steps it turned off are listed for the owner to turn back on (their own
`... on` steps); the Undo doesn't guess which ones were on.

## The backup (before any Delete)

`Desktop\Leyline-killswitch-backup-<time>\`:

- `github\<repo>.bundle`: `gh repo clone <repo> -- --mirror`, then `git bundle create --all`, checked with
  `git bundle verify` and its head count against the mirror's refs. Plus `<repo>-releases.json` (every release and
  asset: name, size, download link) and `<repo>-issues.json` (issues and pull requests).
- `discord\`: the server, roles, channels with permissions, and every message the bot can read (up to 20 000 a
  channel), when Discord is ticked or a token is saved.
- `portal\portal-data-<time>.tar.gz`: made on the server, copied down, its sha256 checked against the server's, and
  opened to list it. Only when Website is ticked.
- `MANIFEST.json`: every file, its size and sha256, and what was checked.

The GitHub bundles are always taken before any Delete. If any check fails, Delete stops and deletes nothing.

## Server half

`ks_server.py` is copied to `/srv/wrath/killswitch/` over SSH and run with `python3`. It reads and writes only:

- `/srv/wrath/portal-data/leyline/shutdown.json`, `news.json`, `version-policy.json`;
- `/srv/wrath/portal-data/admin/relay-block.json` (the relay polls it);
- `/srv/wrath/killswitch/` (its state for Undo, and the backups under `backups/`);
- SOAP through `/srv/wrath/soap-cmd.sh` (`announce`, `server set motd`).

Commands: `status`, `pause`, `resume`, `backup`, `wipe` (`--dry-run` on all that write). `wipe` refuses without a
backup from the last hour whose sha256 it is given.

## Readiness (review 4 Nov)

Before launch week, and again after any change to the card:

1. **A dry run of Pause.** Tick every part and Dry run, write a test message, press Preview, then Pause. Save the
   list it shows (the log in `logs\`). Nothing changes.
2. **A second copy of the backup.** The backup folder is on the Desktop. Copy it to a drive that isn't this PC
   after any Delete backup; it holds the website's player data, so keep that copy private.
3. **Game data only.** If only the streamed game data must go, the `gamedata-off` step is enough (fast, undone by
   `gamedata-on`). The Website's `/data` must then answer 410: the `signin` step's self-check now fails when it doesn't.

## If the PC is gone (pause by hand)

The card needs Leyline Control on the owner's PC. Without it, the same Pause by hand, in this order:

1. **Discord** (the app on any device): post the farewell in the announcements channel and pin it. Server
   Settings > Roles > @everyone: turn off Send Messages, Create Threads, Add Reactions and Connect.
2. **Website** (SSH from any computer with the server key): write
   `{"mode":"closed","message":"<the farewell>"}` to `/srv/wrath/portal-data/leyline/shutdown.json`. Every page
   shows the farewell; the launcher's feeds, status.json and HQ keep working. Deleting the file undoes it.
3. **Game data**: `bash /srv/wrath/stream/gamedata-switch.sh off` on the server (410 for every /data read).
4. **Realms and network**: in HQ (`/wrath/admin`, signed in as the owner), the network's "Stop every node now" button.
5. **GitHub** (github.com, signed in as orionopenwow): each repo's Settings > Archive this repository. Never the
   other account.

Nothing here deletes anything: everything can be undone the same way.

## Owner decisions

- The farewell text (`TODO(owner)`).
- The 24-hour wait between Pause and Delete.
- Which realm steps Pause runs (the list above), and whether to stop the main realm's machine too.
- Make the repos private on Pause, or leave them public and archived.
- Delete wipes the website's player data (accounts, Hero Keys, profiles). Keep the backup somewhere safe and private:
  it holds that data.
