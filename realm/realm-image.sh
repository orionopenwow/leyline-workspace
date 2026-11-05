#!/usr/bin/env bash
# Leyline: put the newest realm image live (the C++ modules: mod-leyline-bossfights, mod-sandbox-core, the pinned
# AzerothCore fork and its modules). The images are built by GitHub (the wu-images workflow, after a "ship" that
# changed realm/custom/modules or sources.lock) and published as ghcr.io/orionopenwow/wu-ac-wotlk-*:stable.
# Usage on the server, in /srv/wrath:
#   bash realm-image.sh            pull :stable, back up, swap, restart, check, roll back by itself on failure
#   REALM_IMAGE_TAG=sha-1234567 bash realm-image.sh    a given build instead of :stable
#   REALM_IMAGE_WARN_MIN=0         no in-game warning (default 5 minutes)
# Steps: 1. pull the three images (nothing changes if that fails); 2. the nightly backup job run now (wu-backup);
# 3. the running images kept as :rollback; 4. a warning in game, saveall; 5. docker compose up -d (db-import adds
# the modules' tables, then the login and world servers); 6. self-check: the world server says "ready..." and
# "mod-leyline-bossfights: on"; else the :rollback images go back and the realm restarts on them.
set -euo pipefail
cd /srv/wrath
die() { printf '\nREALM IMAGE FAILED: %s\n' "$*" >&2; exit 1; }
TAG=${REALM_IMAGE_TAG:-stable}
[[ "$TAG" =~ ^[A-Za-z0-9._-]{1,64}$ ]] || die "odd image tag"
REG=ghcr.io/orionopenwow/wu-ac-wotlk
LOCAL=ov/ac-wotlk
PARTS="db-import authserver worldserver"
grep -qE '^OV_IMAGE=ov/ac-wotlk$' .env && grep -qE '^OV_TAG=custom$' .env || die "the realm doesn't run the ov/ac-wotlk-*:custom images (.env)"

echo "== 1. pulling $REG-*:$TAG"
for s in $PARTS; do docker pull -q "$REG-$s:$TAG" >/dev/null || die "could not pull $REG-$s:$TAG (is the build finished and the package public?). Nothing changed."; done
new=$(docker image inspect -f '{{.Id}}' "$REG-worldserver:$TAG")
old=$(docker image inspect -f '{{.Id}}' "$LOCAL-worldserver:custom" 2>/dev/null || true)
if [ "$new" = "$old" ]; then echo "  the realm already runs this image"; echo "REALM IMAGE OK"; exit 0; fi

echo "== 2. backup (wu-backup)"
systemctl start wu-backup || die "the backup failed; nothing changed"

echo "== 3. keeping the running images as :rollback"
for s in $PARTS; do
  docker image inspect "$LOCAL-$s:custom" >/dev/null 2>&1 && docker tag "$LOCAL-$s:custom" "$LOCAL-$s:rollback"
done
rollback() {
  echo "  putting the previous images back"
  for s in $PARTS; do docker image inspect "$LOCAL-$s:rollback" >/dev/null 2>&1 && docker tag "$LOCAL-$s:rollback" "$LOCAL-$s:custom"; done
  docker compose up -d || true
}
for s in $PARTS; do docker tag "$REG-$s:$TAG" "$LOCAL-$s:custom"; done

MIN=${REALM_IMAGE_WARN_MIN:-5}
if [ "$MIN" -gt 0 ]; then
  echo "== 4. announcing a restart in $MIN minute(s)"
  bash soap-cmd.sh "announce Server restart in $MIN minutes for an update. Back in a few minutes." >/dev/null 2>&1 || true
  [ "$MIN" -gt 1 ] && sleep $(( MIN * 60 - 60 ))
  bash soap-cmd.sh "announce Server restart in 1 minute." >/dev/null 2>&1 || true
  sleep 60
fi
bash soap-cmd.sh "saveall" >/dev/null 2>&1 || true

echo "== 5. restarting on the new images (the database update can take a few minutes)"
docker compose up -d || { rollback; die "docker compose up failed; rolled back"; }
sleep 5
started() { docker inspect -f '{{.State.StartedAt}}' ov-ac-worldserver; }
ok=0
for i in $(seq 1 180); do
  if docker logs --since "$(started)" ov-ac-worldserver 2>&1 | grep -F 'ready...' >/dev/null; then ok=1; break; fi
  if (( i % 6 == 0 )); then echo "  still starting ($((i * 10 / 60)) min)..."; fi
  sleep 10
done
if [ "$ok" != 1 ]; then rollback; die "the world server did not report ready in 30 minutes; rolled back (docker logs --tail 80 ov-ac-worldserver)"; fi

echo "== 6. self-check"
line=$(docker logs --since "$(started)" ov-ac-worldserver 2>&1 | grep -F 'mod-leyline-bossfights:' | tail -1 || true)
if [ -z "$line" ]; then rollback; die "the new image has no mod-leyline-bossfights (old build?); rolled back"; fi
echo "  mod-leyline-bossfights:${line##*mod-leyline-bossfights:}"
echo "REALM IMAGE OK"
