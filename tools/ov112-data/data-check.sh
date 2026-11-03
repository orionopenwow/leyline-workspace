#!/usr/bin/env bash
# Read-only: where the old 1.12 game files are on the server, how big, which container mounts them and which site
# serves them. Owner 2026-10-31: "remove old game files, keep new slice" (Azeroth Survivors' own 1.12 slice is the
# only 1.12 data to stay). This script changes nothing; data-remove.sh does the removal after the owner reads this.
set -u
echo "== containers that mount anything (name, image, state, mounts)"
for c in $(docker ps -a --format '{{.Names}}'); do
  m=$(docker inspect -f '{{range .Mounts}}{{.Source}}->{{.Destination}} {{end}}' "$c" 2>/dev/null)
  [ -n "$m" ] || continue
  printf '  %-34s %-10s %s\n' "$c" "$(docker inspect -f '{{.State.Status}}' "$c")" "$m"
done
echo
echo "== folders that look like 1.12 game data (MPQ archives, or extracted DBFilesClient/World trees), with sizes"
# MPQs and extracted trees; the Wrath stream (/srv/wrath*, wu-*) is listed too but marked KEEP.
find / -xdev \( -path /proc -o -path /sys -o -path /run -o -path /var/lib/docker/overlay2 \) -prune -o \
  \( -iname '*.mpq' -o -iname 'DBFilesClient' -o -iname '__index' -o -iname 'realmlist.wtf' \) -print 2>/dev/null \
  | sed -E 's#/(DBFilesClient|__index|realmlist\.wtf)$##I; s#/[^/]*\.[mM][pP][qQ]$##' | sort -u | while read -r d; do
    [ -d "$d" ] || continue
    sz=$(du -sh "$d" 2>/dev/null | cut -f1)
    tag=CHECK
    case "$d" in *wrath*|*/wu-*|*/leyline*|*/srv/stream*) tag=KEEP-WRATH;; esac
    # a 1.12 tree has no Achievement.dbc (a Wrath one does)
    kind=?
    if find "$d" -maxdepth 3 -iname 'Achievement.dbc' -print -quit 2>/dev/null | grep -q .; then kind=wrath;
    elif find "$d" -maxdepth 3 -iname '*.dbc' -print -quit 2>/dev/null | grep -q .; then kind=1.12;
    elif ls "$d" 2>/dev/null | grep -qi 'expansion\|lichking'; then kind=wrath-mpq;
    elif ls "$d" 2>/dev/null | grep -qi '^dbc.mpq\|^model.mpq\|^texture.mpq'; then kind=1.12-mpq; fi
    printf '  %-6s %-9s %-11s %s\n' "$sz" "$kind" "$tag" "$d"
  done
echo
echo "== Docker volumes and their sizes"
for v in $(docker volume ls -q); do
  mp=$(docker volume inspect -f '{{.Mountpoint}}' "$v"); printf '  %-40s %s\n' "$v" "$(du -sh "$mp" 2>/dev/null | cut -f1)"
done
echo
echo "== which sites answer /data/__index signed out (401/302/403 = gated, 200 = public, 404 = none)"
for h in play.openvanilla.app play.openwow.app wrath.openvanilla.app duels.openvanilla.app bossfights.openvanilla.app; do
  printf '  %-30s %s\n' "$h" "$(curl -s -o /dev/null -w '%{http_code}' --max-time 10 "https://$h/data/__index")"
done
echo
echo "== where else the space is (read-only; 1 Nov: room for a second slice copy)"
docker system df 2>/dev/null | sed 's/^/  /'
for d in /root/fastloop-backups /root/page-backups /srv/wrath/build-realm/backups /srv/wrath/stream/slices /srv/wrath/stream/slices.old* \
         /srv/wrath/backups /root/backups /var/backups /var/log /tmp /root/vr-later /srv/vr; do
  [ -e "$d" ] && printf '  %-8s %s\n' "$(du -sh "$d" 2>/dev/null | cut -f1)" "$d"
done
echo "  the biggest folders two levels under /srv, /opt and /root:"
du -h --max-depth=2 /srv /opt /root 2>/dev/null | sort -rh | head -25 | sed 's/^/    /'
echo
echo "== disk"
df -h / | tail -1
echo
echo "OV112 DATA CHECK OK"
