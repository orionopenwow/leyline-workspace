# Incident 1 Nov 2026: the streamed slice lost half its files (black screen)

## What happened

All times are local (UTC-7). Server times are UTC.

| Time | What | Evidence |
|---|---|---|
| 00:58 | Autopilot `1234a slice`, step 5: "not enough disk for both copies (7784 MB free, 9834 MB needed): removing the old slice first". The new slice went in whole ("slice mini: 7.8G, 131283 files"), but wu-play stayed down: 502. | `logs/1234a-slice.log` |
| 01:02 | `1239a client` ran `stream-up.sh`. wu-play started at 08:02:19 UTC on `slices/mini` (7.8G, 131283 files). | `logs/1239a-client.log`, server check |
| 01:06 to ~01:25 | `1241b tidy`: build cache, logs, 89 old site backups, `/srv/vr112/slice` (1.5G). Disk went from 29G used / 7.5G free to 26G used / 11G free. | `tools/server-tidy/live/tidy-20261101-0106.txt` (last written 01:25) |
| 01:25 to 01:27 | `1241c slice` step 1 (the kit, `patch-index.sh`: boot list +270 Forever UI, +10 mod files) and step 2 (`displays.py`). At 01:35 it was still in step 3 on the PC, so its step 5 (`slice-install.sh`) never ran. | `logs/1241c-slice.log` |
| ~01:27 | Something deleted about 80,000 files (3 GB) from the live `slices/mini`, including `DBFilesClient/` and `Interface/GlueXML/`. | `slices/` and `slices/mini` mtime 08:27 UTC |
| 01:34 | Server check: `slices/mini` had 51,219 files (4.8G). Disk: 23G used, 14G free, so the files really were deleted (11G + 3G). wu-play was still the 08:02 container, so it answered 404 instead of 502. | `logs/server-check-latest.txt` |

## Cause

We couldn't reproduce the deletion from the scripts in the repo. Here is what is certain, then the candidates in order.

**Certain:**
- None of the server-side parts of `1241c` delete or move slice files:
  - the kit unpack writes only the scripts, `slices/mini.json` and folder metadata;
  - `fui-install.sh reapply-www` and `mods-install.sh reapply-www` edit only `www/boot-manifest.json` (the log shows both);
  - `displays.py` only reads.
- The tidy touched only `/srv/vr112/slice`. Its `find`/`rm` follow no symlinks. Its final `df` came before the 3 GB drop.
- `slices/mini.json` on the server is 126,742 bytes with mtime 08:18 UTC. That is the PowerShell-written `work\mini.json` from a step 4. The kit's copy is about 48.7 KB, and the kit unpack at 01:25 replaced it. The only thing that puts an uploaded description in `slices/` is `slice-install.sh`. So at about 01:27 a slice upload was being installed that no Autopilot log records: a second "Make mini slice.bat" or "Upload a mini slice.bat" window, step 5.
- That install never reached its end. Every finished install restarts wu-play, and wu-play was still the 08:02 container.

**Candidates, most likely first:**
1. **An unlogged second slice upload, stopped part-way** (window closed or SSH dropped) while the old `slice-install.sh` was renaming and deleting whole slice folders. It ran alongside Autopilot's tidy and slice steps. The old script had these weaknesses:
   - no lock;
   - it deleted the `.old`/`.new`/`.stage` folders of every slice name at start;
   - it deleted the replaced folder while wu-play still served it;
   - it checked nothing but "a `DBFilesClient` folder exists".
   - GNU tar also accepts an upload cut at a file boundary as complete (exit 0; tested).

   A half-done `rm -rf` leaves exactly this picture: some folders gone in directory order, the rest intact.
2. **A command typed on the server**, by hand or by another session (3 users were logged in at 08:34), for example to make room for "both copies".

The kit unpack also did two wrong things, though it deleted nothing:
- It overwrote `slices/mini.json` (the description that matches the installed slice: maxTier 54) with the PC's full one (maxTier 58).
- It made `/srv/wrath/stream` and `slices/` writable by everyone (a Windows tar records folders as 0777).

**To settle it on the server** (read-only):
```
stat /srv/wrath/stream/slices/mini                                    # Birth: 07:58 = the 00:58 slice, 08:18 = a later upload
docker exec wu-play stat -c '%i links=%h' /slice; stat -c %i /srv/wrath/stream/slices/mini   # links=0: wu-play serves a deleted folder
grep -E 'Accepted|session opened' /var/log/auth.log | grep 'Nov  1 08:[0-2]'   # who was logged in from 08:00 to 08:29
tail -50 /root/.bash_history
```

## The fix (branch `agent/slice-safe`)

- **`slice-install.sh`**:
  - One slice change at a time (`flock` on `slices/.lock`).
  - The upload is unpacked to `.new`, the overlays are laid over `.stage`, and then `slice-health.sh --verify` checks it: the key files are there and readable by everyone, and at least 99% of the names in its own `.tiers` lists are on disk. Only then does it go into place, by renaming.
  - The upload's `mini.json` goes in only together with its slice.
  - The previous slice is kept as `<name>.old` when the disk has room. After the swap, wu-play is asked for `DBFilesClient\Spell.dbc`, and a wrong answer puts the previous slice back. `--rollback <name>` does the same by hand.
  - It refuses to delete anything unless that would make enough room.
  - In the low-disk case the old slice is renamed away before it is deleted, so the served path never shows a half slice.
  - A folder wu-play still serves (checked by inode) is removed only after wu-play has been restarted.
  - The folder permissions are reset.
- **Live slice changes go through a copy.** `fui-install.sh` (install, remove, brand, reapply) and `mods-install.sh` (install, rollback, brand, reapply) change a hard-link copy (`slice_restage` in `stream-lib.sh`, so no second copy of the data on disk), check it, and rename it into place. A failure leaves the live slice exactly as it was. `gamedata-switch.sh --park` takes the same lock.
- **"Make mini slice.bat"**: the kit sends slice descriptions to `slice-defs/`, never into the live `slices/`. It unpacks with the server's own owner and permissions. "Update browser client.bat" unpacks the same way.
- **`stream-up.sh`** warns when the slice it is about to serve is not whole.
- **`tidy.sh`**:
  - `rm --one-file-system`;
  - it refuses when `/srv/vr112/slice` is a symlink, resolves elsewhere, or has something mounted under it;
  - it removes old log files one name per line.

## How to check

- **"stream/Slice health - check.bat"** (Autopilot step `slice-health`, read-only). For every slice folder it shows:
  - file counts per folder;
  - the key files;
  - how many of the names in its tier lists are on disk;
  - whether wu-play serves `slices/<name>` itself (inode) and what it answers for key files.

  The last line is `SLICE HEALTH DONE: OK` or `... PROBLEMS`. It saves to `logs/slice-health-latest.txt`.
- **On the server:** `bash /srv/wrath/stream/slice-health.sh`, or `--verify <dir>` for one folder.
- **Tests:** `bash stream/tests/test_server_scripts.sh <a wenilla-host, or any program that answers --help>`.
