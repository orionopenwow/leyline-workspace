# Launcher version policy

Owner, 3 Nov: "a suggested time window for pushing updates constantly so the launchers everyone has stay consistent
and connected."

The server side is built (stream/staffdesk.py). The launcher side is not yet: this page says what it should do.

## What the website publishes

`GET https://leylinelauncher.com/wrath/leyline/version.json` (public, CORS `*`, cached a minute):

```json
{
 "format": 1,
 "updated_at": "2026-11-03T18:00:00Z",
 "current": "0.4.2",
 "beta": "0.5.0",
 "minimum": "0.4.0",
 "notice": "Please update to keep playing together",
 "hold": false,
 "window": {"text": "Tuesday 10:00-12:00 UTC", "next_start": "2026-11-10T10:00:00Z"},
 "releases": "https://github.com/orionopenwow/Leyline/releases"
}
```

- `current`: the stable version (HQ > Releases sets the channel). `null` until one is set.
- `minimum`: the oldest version that still works with the network (HQ > Review > Upgrades, owner only, typed
  `set`). `null` = no minimum. It can't be set newer than `current`.
- `notice`: one line the launcher may show next to the update prompt (no brand words; checked when saved).
- `hold`: the owner's hold on updates (HQ > Releases). While it is true, don't nag.
- `window`: when the team ships updates (the quietest two hours of the week). For information: the launcher can
  say "Updates usually arrive Tuesdays around 10:00 UTC".

## What the launcher should do (to build)

The launcher already finds and installs its own updates from GitHub releases (`launcher/src-tauri/src/update.rs`,
every 6 hours and at start). Add one small check beside it:

1. Fetch `version.json` with the same timeouts and offline cache as the releases check. Any error: do nothing new.
2. Compare the running version with `minimum` (semver, as `update.rs` already compares).
   - Older than `minimum`: before Play on an Official or Community realm, show "This launcher is too old to join the
     network. Update now" with the update button the title bar already has (and `notice` under it). Your Den and
     offline play stay open.
   - Older than `current` but not below `minimum`: nothing new; the existing "Restart to update" is enough.
3. While `hold` is true, skip step 2's prompt.
4. Never block on this file: a missing or broken `version.json` means "no minimum".

Tests to add: a fake `version.json` in the launcher's update tests (old, current, missing, broken, hold).

## How the team uses it

1. Ship launcher releases inside the window (HQ > Review > Upgrades shows it; it moves as player numbers change).
2. After a release that changes how the launcher talks to the realms, relay or website, raise `minimum` to it a few
   days later, once most launchers have updated themselves (HQ > Numbers shows launcher versions when they are logged).
3. Lowering or clearing `minimum` is always safe.
