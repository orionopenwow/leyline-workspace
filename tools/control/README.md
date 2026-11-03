# Leyline Control

`control.py` is the owner's control panel on his PC (`Leyline Control.bat`, http://127.0.0.1:8097). It runs the
steps in `UPDATES-PENDING.txt` (the `STEPS` dict names each one's `.bat`), streams their output, keeps logs in
`logs\<id>-<step>.log`, and has Autopilot, the Discord panel, Waiting on you and the cards below. The module's own
text at the top of `control.py` lists everything on the page.

## Discord & GM (read-only)

One card for running the live side at a glance. It only reads files this PC already has; nothing on it changes
anything until a button is pressed, and each button runs an existing step through the same `/api/do` checks as
Waiting on you (`discord-apply` is never one of them: it stays on the Discord panel, after a preview and a typed YES).

| Line | From |
|---|---|
| server | the Live checks (every 30 s) and the last `check` (`control/last-check.json`) |
| bridge | the Discord bridge on the server (`wu-discord`, `/opt/wu-discord`), as the last `check` saw it: `realm/diag.sh` prints its state and last lines under "== Discord bridge" |
| bot | the Discord panel's token (saved or not; never shown) and its last preview and bot settings |
| bugs | the open bug reports (new, triaged, planned) from the `bugs` step's `tools/bugs/live/backlog.json`, newest first |
| mods | the curated addons (`mods/upstream.json`) and looks (`stream/looks.json`) with no reviewer yet: they wait for the owner on HQ |
| quick | the last quick edits (below) |

Buttons: Check server (`check`), Pull bug reports (`bugs`), Discord bot settings (`discord-check`), Discord preview
(`discord-preview`), World server log (`worldlog`), Find new mods (`mods-daily`, approves nothing), GM console, Open HQ.
Every line shown goes through `scrub()` (no tokens or passwords).

## Kill switch (owner only)

The last card, folded. It takes Leyline down with the owner's farewell message: **Pause** (the message everywhere,
then each ticked part stopped or hidden; **Undo** brings it back) and **Delete** (only after a Pause and a day's wait,
a checked backup to the Desktop first). Each stage needs its own fresh preview and a typed phrase: `SHUT DOWN LEYLINE`,
`BRING LEYLINE BACK`, `DELETE LEYLINE FOREVER`. It is not a step: never in `UPDATES-PENDING.txt`, Autopilot or
Launch all. Logs: `logs\killswitch-<time>-<stage>.log`. Design and the manual steps: `docs/KILL-SWITCH.md`.
Tests: `python3 -I tools/control/test_control_killswitch.py`, `python3 -I tools/killswitch/test_killswitch.py`.

## Quick edits (`apply-check` / `apply`)

For a small change, Claude doesn't write a new `.bat`. After writing the changed files onto the PC (checked against
git first, as always), Claude drops a manifest in `control\quick\<id>.json`:

```json
{"files": [{"path": "stream/forum.py", "sha256": "<64 lowercase hex>"}], "deploy": "signin", "note": "Forum: a typo"}
```

and queues it (ids as usual):

```
1311a apply-check  Forum: a typo            (read-only; Autopilot runs it even with Ask before deploys on)
1311b apply  quick:1311a  Forum: a typo     (checks again, then deploys)
```

`tools/control/quick.py` finds the manifest by the line's id, or by a `quick:<name>` first word in its note. It
refuses the whole manifest, and deploys nothing, unless every file is:

* a relative path with forward slashes inside the Leyline folder (no `..`, drive or absolute path, no link that
  leads out), and not in `.git`, `control/`, `logs/`, `.repos/` or `_local/`;
* not a secret or settings file (`.env` files, `token.txt`, keys, certificates, credentials, anything named
  password or secret), and not game data or a program (`.mpq`, `.exe`, `.dll`);
* on the PC, at most 5 MB, with exactly the sha256 the manifest gives;
* under what its deploy step ships: `signin` (the website: `stream/`), `addons` (`addons/`, `stream/fui-web/`),
  `scripts` (the realm scripts: `realm/custom/`), or `none` (anywhere; checked and logged only, the next `ship`
  carries it).

Then `apply` runs that step's own `.bat` (the same one Control runs for `signin`, `addons` or `scripts`, with its
backup, self-check and rollback) and appends a line to `control\quick\applied.jsonl` (paths and short hashes only;
the Discord & GM card shows the last few). A manifest with any other key, deploy name or shape is refused. Anything
bigger than one of these deploys still gets its own step.

Tests: `python3 -I tools/control/test_control_quick.py`, `python3 -I tools/control/test_control_gm.py`.
