#!/usr/bin/env bash
# Make room on the server for a second copy of the streamed slice, so a rebuild swaps over with no outage
# (owner 1 Nov 2026 01:05, "All four"): exactly these, nothing else:
#   1. Docker's build cache (no container uses it; rebuilt when an image is built again)
#   2. system logs older than 7 days (the journal, and rotated log files: *.gz, *.1 .. *.9)
#   3. /root/fastloop-backups: the site-update backups, keeping the newest 3
#   4. /srv/vr112/slice: the parked Vanilla Remastered browser slice (its data step makes it again); refused while any
#      container mounts it
# Never touched: /srv/wrath (the Wrath stream, realm, portal data, Builders' Realm), databases, Docker volumes and
# images, the VR world data (volume wenilla-realm_gamedata, its movement maps), every other backup.
#   bash tidy.sh          dry run: what would go, how much, changes nothing
#   bash tidy.sh --go     do it
set -u
GO=""; [ "${1:-}" = "--go" ] && GO=1
say() { echo "  $*"; }
size() { du -sh "$1" 2>/dev/null | cut -f1; }
echo "== disk before"; df -h / | tail -1
echo
echo "== 1. Docker build cache"
docker system df 2>/dev/null | awk 'NR==1 || /Build Cache/' | sed 's/^/  /'
if [ -n "$GO" ]; then docker builder prune -af 2>&1 | tail -1 | sed 's/^/  /'; fi
echo
echo "== 2. system logs older than 7 days"
journalctl --disk-usage 2>/dev/null | sed 's/^/  /'
old=$(find /var/log -type f \( -name '*.gz' -o -regex '.*\.[1-9]$' \) -mtime +7 2>/dev/null)
say "rotated files older than 7 days: $(printf '%s\n' "$old" | grep -c . ) ($(printf '%s\n' "$old" | grep . | xargs -r du -ch 2>/dev/null | tail -1 | cut -f1))"
if [ -n "$GO" ]; then
  journalctl --vacuum-time=7d 2>&1 | tail -1 | sed 's/^/  /'
  printf '%s\n' "$old" | grep . | xargs -r rm -f
fi
echo
echo "== 3. /root/fastloop-backups (keep the newest 3)"
B=/root/fastloop-backups
if [ -d "$B" ]; then
  drop=$(ls -1dt "$B"/*/ 2>/dev/null | tail -n +4)
  say "$(ls -1d "$B"/*/ 2>/dev/null | wc -l) backups, $(size "$B"); going: $(printf '%s\n' "$drop" | grep -c .)"
  printf '%s\n' "$drop" | grep . | sed 's/^/    /'
  if [ -n "$GO" ]; then printf '%s\n' "$drop" | grep . | while read -r d; do case "$d" in "$B"/*) rm -rf -- "$d";; esac; done; fi
fi
echo
echo "== 4. /srv/vr112/slice (the parked Vanilla Remastered browser slice)"
V=/srv/vr112/slice
if [ -d "$V" ]; then
  users=$(for c in $(docker ps --format '{{.Names}}'); do docker inspect -f '{{range .Mounts}}{{.Source}} {{end}}' "$c" | tr ' ' '\n' | grep -q "^$V" && echo "$c"; done)
  say "$(size "$V")"
  if [ -n "$users" ]; then
    say "REFUSED: running containers use it: $users"
  elif [ -n "$GO" ]; then
    find "$V" -mindepth 1 -maxdepth 1 -exec rm -rf -- {} +
    say "emptied (the folder stays)"
  fi
else
  say "not there"
fi
echo
echo "== disk after"; df -h / | tail -1
[ -n "$GO" ] && echo "SERVER TIDY DONE" || echo "SERVER TIDY DRY RUN OK"
