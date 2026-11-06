#!/usr/bin/env bash
# WrathUnleashed: fix character creation on the live main realm. Runs on the server from /srv/wrath/charfix
# ("Fix character create.bat" uploads it and starts it). Safe to run again.
#   1. backs up the world tables it changes (mysqldump, /root/charfix-backups/<time>/) and the profile file
#   2. applies charfix_world.sql to acore_world (all 83 race/class pairs: start rows, gear, bars, skills); with the
#      "all" preset (docs/RACE-CLASS-COMBOS.md) also mod-arac's own data/sql/db-world/arac.sql at the pinned commit
#      (every race with every class: 100 pairs), checked against its sha256 before it runs
#   3. makes sure the profile allows Death Knights from the first character (AC_* settings)
#   4. restarts only the world server and waits until it is ready
#   5. prints a short verification
# Undo: bash /srv/wrath/charfix/charfix_rollback.sh
# Preset: CHARFIX_COMBOS=modern|all (the .bat's first word), else the profile's OV_RACE_CLASS_COMBOS, else modern.
# The choice is written into the profile (OV_RACE_CLASS_COMBOS) and into the website's data folder
# (/srv/wrath/portal-data/leyline/race-class-combos.txt), so the browser page and the launcher offer the same pairs.
# Never prints a password: MySQL runs inside its container with the container's own MYSQL_ROOT_PASSWORD.
set -euo pipefail
cd /srv/wrath
KIT=/srv/wrath/charfix
STAMP=$(date +%Y%m%d-%H%M%S)
BK=/root/charfix-backups/$STAMP
log() { printf '\n== %s\n' "$*"; }
die() { printf '\nCHARFIX FAILED: %s\n' "$*" >&2; exit 1; }
trap 'echo "CHARFIX FAILED at line $LINENO (nothing after this step ran). Backups, if made: '"$BK"'" >&2' ERR
sqlw() { docker exec -i ov-ac-database sh -c 'MYSQL_PWD="$MYSQL_ROOT_PASSWORD" exec mysql -uroot --table acore_world'; }
sqlq() { docker exec -i ov-ac-database sh -c 'MYSQL_PWD="$MYSQL_ROOT_PASSWORD" exec mysql -uroot -N -B acore_world' <<<"$1"; }

log "0/5 Checks"
for c in ov-ac-database ov-ac-worldserver; do
  docker ps --format '{{.Names}}' | grep -x "$c" >/dev/null || die "container $c is not running"
done
[ -f "$KIT/charfix_world.sql" ] || die "$KIT/charfix_world.sql is missing"
PROFILE=$(grep -E '^OV_PROFILE=' .env 2>/dev/null | tail -1 | cut -d= -f2 | tr -d '\r"' || true)
PROFILE=${PROFILE:-brawl}
echo "  realm profile: $PROFILE"
if [ "$PROFILE" != main ] && [ "${CHARFIX_ANY_PROFILE:-0}" != 1 ]; then
  die "the realm runs the '$PROFILE' profile, not 'main'. This fix puts every start back in the race's own zone,
which would undo Brawl's Dalaran start. Run it with CHARFIX_ANY_PROFILE=1 only if that is really wanted."
fi
PF=profiles/$PROFILE.env
[ -f "$PF" ] || die "no $PF"
before=$(sqlq "SELECT COUNT(*) FROM playercreateinfo")
echo "  playercreateinfo before: $before pairs"
sed -i 's/\r$//' "$PF"
COMBOS=${CHARFIX_COMBOS:-$(grep -E '^OV_RACE_CLASS_COMBOS=' "$PF" | tail -1 | cut -d= -f2 | tr -d '"' || true)}
COMBOS=$(printf '%s' "${COMBOS:-modern}" | tr 'A-Z' 'a-z')
case "$COMBOS" in
  modern) WANT_PAIRS=83 ;;
  all) WANT_PAIRS=100 ;;
  *) die "race/class preset '$COMBOS' is not modern or all" ;;
esac
echo "  race/class preset: $COMBOS ($WANT_PAIRS pairs)"
# mod-arac's own SQL for "all": the copy fetch.sh put beside the realm's sources, else the module's file at the pinned
# commit (realm/custom/sources.lock). Either way it must be exactly that commit's file.
ARAC_COMMIT=3f605e09c656eb3b620ef2d70c7ef61808c1cfb0
ARAC_SHA256=101caecc8dd73ca4ec50461d2670473c4e57be520ea5285f1f04a9f786a39cd0
ARAC_SQL=""
if [ "$COMBOS" = all ]; then
  ARAC_SQL="$KIT/arac.sql"
  local_copy=/srv/wrath/custom/acore/modules/mod-arac/data/sql/db-world/arac.sql
  if [ -f "$local_copy" ]; then cp -f "$local_copy" "$ARAC_SQL"
  else curl -fsSL --max-time 60 -o "$ARAC_SQL" "https://raw.githubusercontent.com/heyitsbench/mod-arac/$ARAC_COMMIT/data/sql/db-world/arac.sql" \
    || die "could not fetch mod-arac's arac.sql ($ARAC_COMMIT); nothing was changed"
  fi
  [ "$(sha256sum "$ARAC_SQL" | cut -c1-64)" = "$ARAC_SHA256" ] || die "arac.sql is not mod-arac $ARAC_COMMIT's file (sha256 differs); nothing was changed"
  echo "  mod-arac's arac.sql ($ARAC_COMMIT) ready"
fi

log "1/5 Backup -> $BK"
mkdir -p "$BK"; chmod 700 /root/charfix-backups "$BK"
TABLES="playercreateinfo playercreateinfo_action playercreateinfo_skills charstartoutfit_dbc skillraceclassinfo_dbc quest_template"
# arac.sql also writes these two
[ "$COMBOS" = all ] && TABLES="$TABLES playercreateinfo_spell_custom player_totem_model"
NTABLES=$(echo $TABLES | wc -w)
docker exec ov-ac-database sh -c "MYSQL_PWD=\"\$MYSQL_ROOT_PASSWORD\" exec mysqldump -uroot --single-transaction --skip-lock-tables acore_world $TABLES" \
  | gzip > "$BK/acore_world_tables.sql.gz"
[ "$(gzip -dc "$BK/acore_world_tables.sql.gz" | grep -c '^CREATE TABLE')" = "$NTABLES" ] || die "the backup does not hold the $NTABLES tables; nothing was changed"
had_q=$(sqlq "SELECT COUNT(*) FROM information_schema.TABLES WHERE TABLE_SCHEMA='acore_world' AND TABLE_NAME='ov_charfix_quest_races'")
echo "$had_q" > "$BK/had_quest_backup_table"
cp -p "$PF" "$BK/profile.env"
echo "$PF" > "$BK/profile.path"
ln -sfn "$BK" /root/charfix-backups/latest
# the state before the FIRST run: what the rollback goes back to (a re-run never moves it)
[ -e /root/charfix-backups/original ] || ln -s "$BK" /root/charfix-backups/original
echo "  $(du -h "$BK/acore_world_tables.sql.gz" | cut -f1) dump + $PF"

log "2/5 World database (charfix_world.sql)"
sqlw < "$KIT/charfix_world.sql" >/dev/null
mkdir -p sql && cp -f "$KIT/charfix_world.sql" sql/charfix_world.sql
echo "  applied; also copied to /srv/wrath/sql/charfix_world.sql"
if [ -n "$ARAC_SQL" ]; then
  # after charfix (which keeps the 83 and saves the class quests' original races first): the other 17 pairs' start
  # rows, bars, spells, totems for the new shaman races, and the class quests for every race (INSERT IGNORE: the
  # 83 keep charfix's rows)
  sqlw < "$ARAC_SQL" >/dev/null
  cp -f "$ARAC_SQL" sql/arac.sql
  echo "  applied mod-arac's arac.sql (all races, all classes); also copied to /srv/wrath/sql/arac.sql"
else
  rm -f sql/arac.sql
fi

log "3/5 Settings ($PF)"
changed=0
kv() {   # set KEY=VALUE in the profile, keep everything else
  if grep -qE "^$1=$2\$" "$PF"; then echo "  $1=$2 (already)"; return; fi
  if grep -qE "^$1=" "$PF"; then sed -i "s#^$1=.*#$1=$2#" "$PF"; else printf '%s=%s\n' "$1" "$2" >> "$PF"; fi
  echo "  $1=$2 (set)"; changed=1
}
sed -i 's/\r$//' "$PF"
# Death Knights from the first character: no level-55 character needed, up to 10 per realm (the core's max).
kv AC_CHARACTER_CREATING_MIN_LEVEL_FOR_HEROIC_CHARACTER 0
kv AC_HEROIC_CHARACTERS_PER_REALM 10
# No race, class or faction is switched off for creation (the core defaults, made explicit).
kv AC_CHARACTER_CREATING_DISABLED 0
kv AC_CHARACTER_CREATING_DISABLED_RACE_MASK 0
kv AC_CHARACTER_CREATING_DISABLED_CLASS_MASK 0
# mod-individual-progression would refuse Death Knights (DeathKnightUnlockProgression 13); main keeps it off.
kv AC_INDIVIDUAL_PROGRESSION_ENABLE 0
# The preset, remembered for the next run (no restart needed for it alone: only this script reads it).
was_changed=$changed
kv OV_RACE_CLASS_COMBOS "$COMBOS"
changed=$was_changed
# The website's copy: the browser page's env (stream/wrath_portal.py combos_preset) and the launcher (the network
# listing's sample block) read it; no portal restart needed (read when the file changes).
install -d -m 755 /srv/wrath/portal-data/leyline
printf '%s\n' "$COMBOS" > /srv/wrath/portal-data/leyline/race-class-combos.txt
echo "  website: race-class-combos.txt = $COMBOS"
for k in CHARACTER_CREATING HEROIC_CHARACTERS INDIVIDUAL_PROGRESSION_ENABLE; do
  if grep -E "^\s+AC_${k}" docker-compose.prod.yml >/dev/null 2>&1; then
    echo "  WARNING: docker-compose.prod.yml sets AC_${k}* itself, and that wins over $PF"
  fi
done

log "4/5 Restart the world server"
started() { docker inspect -f '{{.State.StartedAt}}' ov-ac-worldserver; }
old=$(started)
if [ "$changed" = 1 ]; then
  docker compose up -d --no-deps ac-worldserver      # recreate: env_file changes only reach a new container
fi
if [ "$(started)" = "$old" ]; then docker compose restart ac-worldserver; fi
sleep 3
ok=0
for i in $(seq 1 120); do
  if docker logs --since "$(started)" ov-ac-worldserver 2>&1 | grep -F 'ready...' >/dev/null; then ok=1; break; fi
  if (( i % 6 == 0 )); then echo "  still starting ($((i * 10 / 60)) min)..."; fi
  sleep 10
done
[ "$ok" = 1 ] || die "the world server did not report ready in 20 min: docker logs --tail 80 ov-ac-worldserver. Undo: bash $KIT/charfix_rollback.sh"
echo "  world server ready"

log "5/5 Verification"
sqlw <<'SQL'
SELECT COUNT(*) AS pairs_with_start_row,
       SUM(class = 6) AS death_knight_pairs,
       SUM(class = 6 AND map = 609) AS dks_in_acherus,
       SUM(class <> 6 AND map IN (0, 1, 530)) AS others_on_home_continents
FROM playercreateinfo;
SELECT map, zone, COUNT(*) AS pairs, GROUP_CONCAT(CONCAT(race, '/', class) ORDER BY race, class SEPARATOR ' ') AS race_class
FROM playercreateinfo GROUP BY map, zone ORDER BY map, zone;
SELECT COUNT(*) AS non_dk_pairs,
  SUM(EXISTS (SELECT 1 FROM charstartoutfit_dbc o WHERE o.RaceID = p.race AND o.ClassID = p.class AND o.SexID = 0)
      AND EXISTS (SELECT 1 FROM charstartoutfit_dbc o WHERE o.RaceID = p.race AND o.ClassID = p.class AND o.SexID = 1)) AS with_gear_both_sexes,
  SUM(EXISTS (SELECT 1 FROM playercreateinfo_action a WHERE a.race = p.race AND a.class = p.class)) AS with_action_bar
FROM playercreateinfo p WHERE p.class <> 6;
SELECT o.RaceID AS race, o.ClassID AS class, p.map, p.zone,
  (o.ItemID_1>0)+(o.ItemID_2>0)+(o.ItemID_3>0)+(o.ItemID_4>0)+(o.ItemID_5>0)+(o.ItemID_6>0)+(o.ItemID_7>0)+(o.ItemID_8>0)
   +(o.ItemID_9>0)+(o.ItemID_10>0)+(o.ItemID_11>0)+(o.ItemID_12>0) AS items,
  (SELECT COUNT(*) FROM playercreateinfo_action a WHERE a.race = o.RaceID AND a.class = o.ClassID) AS buttons,
  (SELECT GROUP_CONCAT(s.skill) FROM playercreateinfo_skills s WHERE s.comment = CONCAT('charfix ', o.RaceID, '/', o.ClassID, ' from ', SUBSTRING_INDEX(s.comment, ' ', -1))) AS extra_skills
FROM charstartoutfit_dbc o JOIN playercreateinfo p ON p.race = o.RaceID AND p.class = o.ClassID
WHERE o.ID BETWEEN 9001 AND 9100 AND o.SexID = 0 ORDER BY o.RaceID, o.ClassID;
SQL
echo "  (the table lists the 21 new pairs; the 62 stock pairs keep their own gear, and Death Knights get the"
echo "   dbc's Acherus outfit as stock)"
since=$(started)
loaded=$(docker logs --since "$since" ov-ac-worldserver 2>&1 | grep -oE 'Loaded [0-9]+ Player Create Definitions' | tail -1 || true)
echo "  world server: ${loaded:-no 'Player Create Definitions' line found}"
errs=$(docker logs --since "$since" ov-ac-worldserver 2>&1 | grep -iE 'playercreateinfo|charstartoutfit|skillraceclassinfo|CharStartOutfit' | grep -viE '^.*>> Loaded' || true)
[ -n "$errs" ] && { echo "  world server complained:"; echo "$errs" | tail -20 | sed 's/^/    /'; } || echo "  no playercreateinfo/outfit errors in the world server log"
echo "  settings the running world server has:"
docker exec ov-ac-worldserver env | grep -E '^AC_(CHARACTER_CREATING|HEROIC_CHARACTERS_PER_REALM|START_HEROIC_PLAYER_LEVEL|START_PLAYER_LEVEL|INDIVIDUAL_PROGRESSION_ENABLE)' | sort | sed 's/^/    /' || true
n=$(sqlq "SELECT COUNT(*) FROM playercreateinfo")
case "$loaded" in "Loaded $n Player Create Definitions") ;; *) echo "  WARNING: the server's count differs from the table ($n)";; esac
[ "$n" = "$WANT_PAIRS" ] || die "playercreateinfo has $n pairs, not $WANT_PAIRS ($COMBOS)"
echo
echo "Backup: $BK (before the first run: $(readlink -f /root/charfix-backups/original))   Undo: bash $KIT/charfix_rollback.sh"
echo "CHARFIX OK: $WANT_PAIRS race/class pairs open ($COMBOS); new characters start in their race's zone (Death Knights in Acherus)."
