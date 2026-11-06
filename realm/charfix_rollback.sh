#!/usr/bin/env bash
# WrathUnleashed: undo charfix.sh. Restores the world tables (six; eight after the "all" preset) and the profile file from a charfix backup
# (default: the one made before the first run, /root/charfix-backups/original), then restarts only the world server.
#   bash /srv/wrath/charfix/charfix_rollback.sh [/root/charfix-backups/<time>]
# Characters already made with one of the 21 new race/class pairs can't log in after this (the core has no
# start data for their pair any more); they come back if charfix.sh is run again.
set -euo pipefail
cd /srv/wrath
BK=${1:-/root/charfix-backups/original}
BK=$(readlink -f "$BK")
die() { printf '\nROLLBACK FAILED: %s\n' "$*" >&2; exit 1; }
[ -f "$BK/acore_world_tables.sql.gz" ] || die "no backup in $BK"
docker ps --format '{{.Names}}' | grep -x ov-ac-database >/dev/null || die "ov-ac-database is not running"
echo "== Restoring the world tables from $BK"
gzip -dc "$BK/acore_world_tables.sql.gz" | docker exec -i ov-ac-database sh -c 'MYSQL_PWD="$MYSQL_ROOT_PASSWORD" exec mysql -uroot acore_world'
if [ "$(cat "$BK/had_quest_backup_table" 2>/dev/null || echo 0)" = 0 ]; then
  docker exec -i ov-ac-database sh -c 'MYSQL_PWD="$MYSQL_ROOT_PASSWORD" exec mysql -uroot acore_world' <<<"DROP TABLE IF EXISTS ov_charfix_quest_races;"
fi
rm -f sql/charfix_world.sql sql/arac.sql
PF=$(cat "$BK/profile.path")
changed=0
if ! cmp -s "$BK/profile.env" "$PF"; then cp -p "$BK/profile.env" "$PF"; changed=1; echo "== $PF restored"; fi
# the website's race/class preset follows the restored profile (modern when it names none)
COMBOS=$(grep -E '^OV_RACE_CLASS_COMBOS=' "$PF" | tail -1 | cut -d= -f2 | tr -d '"\r' || true)
if [ -d /srv/wrath/portal-data ]; then
  install -d -m 755 /srv/wrath/portal-data/leyline
  printf '%s\n' "${COMBOS:-modern}" > /srv/wrath/portal-data/leyline/race-class-combos.txt
  echo "== website: race-class-combos.txt = ${COMBOS:-modern}"
fi
echo "== Restarting the world server"
started() { docker inspect -f '{{.State.StartedAt}}' ov-ac-worldserver; }
old=$(started)
[ "$changed" = 1 ] && docker compose up -d --no-deps ac-worldserver
[ "$(started)" = "$old" ] && docker compose restart ac-worldserver
sleep 3
for i in $(seq 1 120); do
  docker logs --since "$(started)" ov-ac-worldserver 2>&1 | grep -F 'ready...' >/dev/null && break
  [ "$i" = 120 ] && die "the world server did not report ready in 20 min"
  sleep 10
done
n=$(docker exec -i ov-ac-database sh -c 'MYSQL_PWD="$MYSQL_ROOT_PASSWORD" exec mysql -uroot -N -B acore_world' <<<"SELECT COUNT(*) FROM playercreateinfo")
echo "ROLLBACK OK: world tables back to $BK (playercreateinfo: $n pairs)."
