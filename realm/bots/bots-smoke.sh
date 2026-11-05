#!/usr/bin/env bash
# The bot smoke test (docs/BOTS-UPDATES.md): a throwaway realm from the images under test, a headless player (no game
# client), the bot checks, then everything removed again. All of it is bots_smoke.py's `smoke` (the same on the
# owner's PC: realm\Bots smoke test.bat, Control step bots-smoke); this is the runner's way in.
#
#   realm/bots/bots-smoke.sh [--image ghcr.io/orionopenwow/wu-ac-wotlk] [--tag canary] [--playerbots-src DIR]
#                            [--ale-src DIR] [--out report.json] [--keep] [--data-volume ov-client-data]
#
# Needs Docker, Python 3 and the dbc + maps volume extracted from YOUR OWN game folder (realm\1-extract.bat makes
# ov-client-data). It is only read; nothing downloads or ships game data. No password is printed: the database's and
# the console's are random, kept in a 600 temp folder that goes at the end. Exit 0 = every check passed, 3 = no data.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
exec "${PYTHON:-python3}" -I "$HERE/bots_smoke.py" smoke "$@"
