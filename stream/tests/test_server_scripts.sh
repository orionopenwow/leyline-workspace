#!/usr/bin/env bash
# The server scripts against fakes (docker, curl, getent, soap-cmd.sh) in a temp /srv/wrath:
#   bash tests/test_server_scripts.sh [path/to/linux/wenilla-host]     (from the stream folder)
# (no Linux build at hand: any program that answers --help will do, e.g. a two-line shell script.)
# slice-install.sh, slice-health.sh and slice_restage: docs/INCIDENT-2026-11-01-SLICE.md (a fake df where a test
# needs a disk of a given size).
set -uo pipefail
HERE=$(cd "$(dirname "$0")/.." && pwd)
BIN=${1:-$HERE/bin/wenilla-host}
T=$(mktemp -d); [ -n "${KEEP:-}" ] || trap 'rm -rf "$T"' EXIT; echo "tmp: $T"
pass=0; fail=0
check() { if eval "$2"; then pass=$((pass+1)); echo "PASS $1"; else fail=$((fail+1)); echo "FAIL $1"; fi; }

export WU_DIR=$T/srv/wrath WU_CADDYFILE=$T/opt/Caddyfile WU_CADDY_DIR=$T/opt
# slice-install.sh: the disk this runs on decides nothing (2 GB kept free and a 1 GB margin on the server)
export WU_KEEP_FREE_MB=1 WU_KEEP_OLD_MARGIN_MB=0 WU_SLICE_LOCK_WAIT=2
WU=$WU_DIR; ST=$WU/stream
mkdir -p "$ST"/{bin,lua,sql,slices/mini/DBFilesClient,www} "$WU"/{portal,custom/lua_scripts,client-data/dbc} "$T/opt" "$T/fake"
cp "$HERE"/server/*.sh "$HERE"/server/displays.py "$HERE"/server/www-patch.py "$HERE"/server/gamedata-caddy.py "$ST/"
mkdir -p "$ST/www-files"; cp "$HERE"/www-files/* "$ST/www-files/"
cp "$BIN" "$ST/bin/wenilla-host"; chmod 755 "$ST/bin/wenilla-host"
cp "$HERE"/lua/* "$ST/lua/"; cp "$HERE"/sql/*.sql "$ST/sql/"; cp "$HERE/slices/mini.json" "$ST/slices/"
cp "$HERE/wrath_portal.py" "$ST/"
echo "old portal" > "$WU/portal/wrath_portal.py"
printf 'SOAP_USER=WUSOAP\nSOAP_PASS=X\n' > "$WU/portal.env"
cat > "$ST/www/index.html" <<'H'
<!doctype html>
<script>
  window.__wenilla_env = { dev_query_creds: '1' };
</script>
<script type="module">
  import init from './wenilla.js';
</script>
H
echo br > "$ST/www/index.html.br"; echo wasm > "$ST/www/wenilla_bg.wasm"
# a real client page and boot.js (the anchors www-patch.py edits by), from the kit's own copies minus the edits
python3 - "$HERE/www-files" "$ST/www" <<'PY'
import re, sys
src, dst = sys.argv[1:3]
s = open(f"{src}/index.html").read()
s = s.replace("  import { ensureDataSource } from './ownfiles.js';\n  import { installSections, mountSectionsUi } from './sections.js';\n", "")
s = re.sub(r"  // \"Download this section\" \(web/sections\.js\).*?const w = await boot\(init, \{ dataReady \}\);\n", "  const w = await boot(init);\n", s, flags=re.S)
s = re.sub(r"  /\* \"Download this section\".*?#dl\[hidden\] \{ display: none; \}\n", "", s, flags=re.S)
s = re.sub(r'<button type="button" id="dl".*?</button>\n', "", s)
s = s.replace("window.__wenilla_env = {};", "window.__wenilla_env = { dev_query_creds: '1' };")
open(f"{dst}/index.html", "w").write(s)
b = open(f"{src}/boot.js").read()
b = re.sub(r"  // `opts\.dataReady`.*?\n  \);\n", """  const prefetch = skipPrefetch
    ? Promise.resolve()
    : prefetchManifest('./boot-manifest.json', (done, total) => ui.setData(done, total)).catch(
        () => ui.setData(0, 0)
      );
""", b, flags=re.S)
open(f"{dst}/boot.js", "w").write(b)
PY
echo br > "$ST/www/boot.js.br"
echo dbc > "$ST/slices/mini/DBFilesClient/Map.dbc"
python3 - "$WU/client-data/dbc/LFGDungeons.dbc" <<'PY'
import struct, sys
fields = 49; rows = [(241, 575, 1), (261, -1, 6)]
body = b"".join(struct.pack(f"<{fields}i", *([r[0]] + [0]*22 + [r[1], 0, 0, r[2]] + [0]*22)) for r in rows)
open(sys.argv[1], "wb").write(b"WDBC" + struct.pack("<4I", len(rows), fields, fields*4, 1) + body + b"\0")
PY
cat > "$WU_CADDYFILE" <<'C'
play.openvanilla.app {
	handle {
		reverse_proxy ovlive-realm:8090
	}
}
C
cp "$WU_CADDYFILE" "$T/Caddyfile.orig"
cat > "$WU/soap-cmd.sh" <<'S'
#!/usr/bin/env bash
echo "$*" >> "$WU_DIR/soap.log"
case "$1" in ovstream*) echo "ovstream ok: WUSOAP cleared" ;; *) echo ok ;; esac
S
cat > "$T/fake/docker" <<'D'
#!/usr/bin/env bash
echo "docker $*" >> "$WU_DIR/docker.log"
[ "$1 $2 $3" = "exec wu-play stat" ] && { cat "$WU_DIR/fake-play-inode" 2>/dev/null; exit 0; }
[ "$1 $2" = "exec wu-portal" ] && [ -n "${FAKE_PLAY_CODE:-}" ] && { echo "$FAKE_PLAY_CODE"; exit 0; }
case "$1 $2" in
  "ps --format"|"ps -a") printf 'ov-ac-worldserver\nov-ac-database\nwu-portal\nwu-play\n' ;;
  "exec -i") cat >> "$WU_DIR/sql.log" ;;
  "compose exec") [ -n "${FAKE_CADDY_FAIL:-}" ] && exit 1; exit 0 ;;
  "stats --no-stream") echo "12MiB / 256MiB" ;;
  "run -d") echo deadbeef ;;
esac
exit 0
D
printf '#!/bin/sh\ncase "$*" in *ipify*) printf 1.2.3.4 ;; *) printf 302 ;; esac\n' > "$T/fake/curl"
printf '#!/bin/sh\necho "1.2.3.4 STREAM wrath.openvanilla.app"\n' > "$T/fake/getent"
# df: a disk of FAKE_DF_CAP_KB when a test sets it (what slices/ holds is used), the real one otherwise
cat > "$T/fake/df" <<D
#!/bin/sh
if [ -n "\${FAKE_DF_CAP_KB:-}" ] && [ "\$1" = "--output=avail" ]; then
  echo Avail; echo \$(( FAKE_DF_CAP_KB - \$(du -sk "\$WU_DIR/stream/slices" | cut -f1) )); exit 0
fi
exec $(command -v df) "\$@"
D
chmod +x "$T"/fake/* "$WU/soap-cmd.sh"
export PATH="$T/fake:$PATH"

# --- stream-up
bash "$ST/stream-up.sh" > "$T/up1.log" 2>&1; rc=$?
check "stream-up succeeds" "[ $rc = 0 ] || { tail -20 $T/up1.log; false; }"
check "stream-up warns that this slice (no Spell.dbc, no GlueXML.toc) is not whole" "grep -q 'WARNING: PROBLEM: missing DBFilesClient.Spell.dbc' $T/up1.log && grep -q 'the slice is not whole' $T/up1.log"
check "index.html loads /__env.js once" "[ \$(grep -c 'src=\"/__env.js\"' $ST/www/index.html) = 1 ]"
check "index.html: no dev_query_creds" "! grep -q dev_query_creds $ST/www/index.html"
check "index.html: stale precompressed copy removed" "[ ! -e $ST/www/index.html.br ]"
check "table SQL applied" "grep -q 'CREATE TABLE IF NOT EXISTS ov_stream_accounts' $WU/sql.log"
check "ov_slices.lua installed" "[ -f $WU/custom/lua_scripts/ov_slices.lua ]"
check "Dungeon Finder ids generated (random left out)" "grep -q '\[241\] = 575' $WU/custom/lua_scripts/ov_slices_lfg.lua && ! grep -q 261 $WU/custom/lua_scripts/ov_slices_lfg.lua"
check "Dungeon Finder levels generated" "grep -q 'OV_STREAM_LFG_LEVEL' $WU/custom/lua_scripts/ov_slices_lfg.lua"
check "tiers generated from mini.json" "grep -q 'OV_STREAM_TIERS = {' $WU/custom/lua_scripts/ov_slices_tiers.lua && grep -q 'key = \"northshire\"' $WU/custom/lua_scripts/ov_slices_tiers.lua"
check "www: the six scripts copied, page and boot.js edited" "[ -f $ST/www/sections.js ] && [ -f $ST/www/ownfiles-worker.js ] && grep -q \"from './sections.js'\" $ST/www/index.html && grep -q 'id=\"dl\"' $ST/www/index.html && grep -q 'opts.dataReady' $ST/www/boot.js && [ ! -e $ST/www/boot.js.br ]"
check "www: the page still loads /__env.js and keeps its own lines" "grep -q 'src=\"/__env.js\"' $ST/www/index.html && grep -qF \"import('./platform.js')\" $ST/www/index.html"
check "reload ale + probe" "grep -q 'reload ale' $WU/soap.log && grep -q 'ovstream clear WUSOAP' $WU/soap.log"
check "portal installed, old kept" "grep -q stream_play $WU/portal/wrath_portal.py && grep -q 'old portal' $WU/portal/wrath_portal.py.before-stream"
check "portal slice description" "[ -f $WU/portal/slices/mini.json ]"
check "portal.env WU_STREAM=1" "grep -qx WU_STREAM=1 $WU/portal.env && grep -qx STREAM_HOST=wrath.openvanilla.app $WU/portal.env"
check "portal.env WU_OWNFILES=1" "grep -qx WU_OWNFILES=1 $WU/portal.env"
check "wu-portal recreated like deploy.sh" "grep -q 'run -d --name wu-portal .*--network wenilla-realm_default --network-alias mangosd' $WU/docker.log && grep -q 'network connect ov-wotlk_edge wu-portal' $WU/docker.log"
run=$(grep 'run -d --name wu-play' "$WU/docker.log")
check "wu-play: bookworm-slim, 256m, read-only, only-listed" "[[ '$run' == *'--memory 256m'* && '$run' == *'--read-only'* && '$run' == *'debian:bookworm-slim'* && '$run' == *'--only-listed'* && '$run' == *'--upstream play.openvanilla.app'* ]]"
check "wu-play: slice and www mounted read-only" "[[ '$run' == *'/slices/mini:/slice:ro'* && '$run' == *'/www:/www:ro'* ]]"
check "Caddy block added once" "[ \$(grep -c '^wrath.openvanilla.app {' $WU_CADDYFILE) = 1 ]"
check "Caddy: forward_auth /__auth, gate to wu-portal, rest to wu-play" "grep -q 'uri /__auth' $WU_CADDYFILE && grep -q '@gate path /__gate /__env.js' $WU_CADDYFILE && grep -q 'reverse_proxy wu-play:8090' $WU_CADDYFILE"
check "Caddy: original site untouched" "grep -q 'reverse_proxy ovlive-realm:8090' $WU_CADDYFILE"
check "Caddyfile backup" "cmp -s $WU/Caddyfile.before-stream $T/Caddyfile.orig"

# --- again: idempotent
bash "$ST/stream-up.sh" > "$T/up2.log" 2>&1; rc=$?
check "second run succeeds" "[ $rc = 0 ]"
check "www edits not doubled" "[ \$(grep -c \"from './sections.js'\" $ST/www/index.html) = 1 ] && [ \$(grep -c 'const dataReady = Promise.resolve' $ST/www/boot.js) = 1 ]"
check "still one Caddy block, one env.js tag" "[ \$(grep -c '^wrath.openvanilla.app {' $WU_CADDYFILE) = 1 ] && [ \$(grep -c '__env.js\"' $ST/www/index.html) = 1 ]"
check "the first backup is kept" "cmp -s $WU/Caddyfile.before-stream $T/Caddyfile.orig && grep -q 'old portal' $WU/portal/wrath_portal.py.before-stream"

# --- Caddy refuses: restored
cp "$WU_CADDYFILE" "$T/cf.before-fail"
FAKE_CADDY_FAIL=1 bash "$ST/stream-up.sh" > "$T/up3.log" 2>&1; rc=$?
check "Caddy refusing: stream-up fails" "[ $rc != 0 ] && grep -q 'Caddy refused' $T/up3.log"
check "Caddy refusing: Caddyfile restored" "cmp -s $WU_CADDYFILE $T/cf.before-fail"

# --- a broken tier list is refused before anything changes
cp "$ST/slices/mini.json" "$T/mini.good"
python3 - "$ST/slices/mini.json" <<'PY'
import json, sys; d = json.load(open(sys.argv[1])); d["order"].append("nowhere"); json.dump(d, open(sys.argv[1], "w"))
PY
bash "$ST/stream-up.sh" > "$T/up5.log" 2>&1; rc=$?
check "broken tier list: refused" "[ $rc != 0 ] && grep -q 'tier list is broken' $T/up5.log"
cp "$T/mini.good" "$ST/slices/mini.json"

# --- missing pieces are refused with a message
mv "$ST/www/wenilla_bg.wasm" "$T/wasm"
bash "$ST/stream-up.sh" > "$T/up4.log" 2>&1; rc=$?
check "no client build: refused" "[ $rc != 0 ] && grep -q 'no browser client' $T/up4.log"
mv "$T/wasm" "$ST/www/wenilla_bg.wasm"

# --- the off switch
: > "$WU/sql.log"
bash "$ST/stream-switch.sh" off > "$T/off.log" 2>&1; rc=$?
check "switch off" "[ $rc = 0 ] && grep -qx WU_STREAM=0 $WU/portal.env && grep -q 'DELETE FROM ov_stream_accounts' $WU/sql.log"
bash "$ST/stream-switch.sh" on > "$T/on.log" 2>&1; rc=$?
check "switch on" "[ $rc = 0 ] && grep -qx WU_STREAM=1 $WU/portal.env"
bash "$ST/stream-switch.sh" sideways > /dev/null 2>&1; rc=$?
check "switch: bad argument refused" "[ $rc != 0 ]"

# --- slice-install from a tar stream
mkdir -p "$T/work/mini/DBFilesClient" "$T/work/mini/World/Maps"
# a whole slice: the key files slice-health.sh --verify asks for, and its own tier lists (.tiers, as --export writes)
mkdir -p "$T/work/mini/Interface/GlueXML" "$T/work/mini/.tiers"
echo new > "$T/work/mini/DBFilesClient/Map.dbc"; echo adt > "$T/work/mini/World/Maps/x.adt"
echo spell > "$T/work/mini/DBFilesClient/Spell.dbc"; echo area > "$T/work/mini/DBFilesClient/AreaTable.dbc"
printf 'GlueParent.xml\nGlueLocalizationPost.xml\n' > "$T/work/mini/Interface/GlueXML/GlueXML.toc"
printf '# tier 0\tbase\t4\t20\n4\tDBFilesClient\\Map.dbc\n6\tDBFILESCLIENT\\spell.dbc\n5\tDBFilesClient\\AreaTable.dbc\n9\tInterface\\GlueXML\\GlueXML.toc\n' > "$T/work/mini/.tiers/0.txt"
printf '4\tWorld\\Maps\\x.adt\n' > "$T/work/mini/.tiers/1.txt"
echo '[{"tier": 0, "files": 4, "bytes": 20}, {"tier": 1, "files": 1, "bytes": 4}]' > "$T/work/mini/.tiers/index.json"
cp -a "$ST/slices/mini" "$T/first-slice"
tar -cf - -C "$T/work" mini | bash "$ST/slice-install.sh" mini 1000 > "$T/inst.log" 2>&1; rc=$?
check "slice-install swaps in the new slice" "[ $rc = 0 ] && grep -qx new $ST/slices/mini/DBFilesClient/Map.dbc && [ -f $ST/slices/mini/World/Maps/x.adt ] && [ ! -e $ST/slices/mini.new ] && [ ! -e $ST/slices/mini.stage ] || { cat $T/inst.log; false; }"
check "slice-install checks the new slice before the swap" "grep -q 'checking the new slice' $T/inst.log && grep -q 'WHOLE' $T/inst.log && grep -q '5/5 listed files on disk' $T/inst.log"
check "slice-install keeps the previous slice as .old (room on the disk)" "[ -d $ST/slices/mini.old ] && cmp -s $ST/slices/mini.old/DBFilesClient/Map.dbc $T/first-slice/DBFilesClient/Map.dbc && grep -q 'previous slice kept as' $T/inst.log"
check "slice-install restarts wu-play" "grep -q 'restart wu-play' $WU/docker.log"
python3 - "$ST/slices/mini.json" "$T/mini2.json" <<'PY'
import json, sys; d = json.load(open(sys.argv[1])); o = d["order"]; o.remove("icc-toc"); o.insert(2, "icc-toc"); json.dump(d, open(sys.argv[2], "w"))
PY
cp "$T/mini2.json" "$ST/slices/mini.json"; : > "$WU/soap.log"
tar -cf - -C "$T/work" mini | bash "$ST/slice-install.sh" mini 1000 > "$T/inst3.log" 2>&1; rc=$?
check "slice-install with a reordered mini.json: realm tiers remade + reload ale" "[ $rc = 0 ] && grep -q '\[2\] = { key = \"icc-toc\"' $WU/custom/lua_scripts/ov_slices_tiers.lua && grep -q 'reload ale' $WU/soap.log"
cp "$T/mini.good" "$ST/slices/mini.json"
mkdir -p "$T/bad/mini"; echo x > "$T/bad/mini/readme.txt"
tar -cf - -C "$T/bad" mini | bash "$ST/slice-install.sh" mini 10 > "$T/inst2.log" 2>&1; rc=$?
check "slice-install refuses a non-slice and keeps the old one" "[ $rc != 0 ] && grep -qx new $ST/slices/mini/DBFilesClient/Map.dbc"
echo | bash "$ST/slice-install.sh" '../x' > /dev/null 2>&1; rc=$?
check "slice-install refuses a bad name" "[ $rc != 0 ]"

# --- slice-install: never a half slice at the served path (incident 1 Nov 2026, docs/INCIDENT-2026-11-01-SLICE.md)
live_ino() { stat -c %i "$ST/slices/mini"; }
echo good > "$ST/slices/mini/DBFilesClient/Spell.dbc"; ino=$(live_ino)
# an upload without Interface/GlueXML/GlueXML.toc
rm -rf "$T/nog"; cp -a "$T/work" "$T/nog"; rm -rf "$T/nog/mini/Interface"
tar -cf - -C "$T/nog" mini | bash "$ST/slice-install.sh" mini 1000 > "$T/nog.log" 2>&1; rc=$?
check "upload without GlueXML.toc: refused, the old slice keeps serving untouched" "[ $rc != 0 ] && grep -q 'missing Interface' $T/nog.log && grep -q 'not whole' $T/nog.log && [ \$(live_ino) = $ino ] && grep -qx good $ST/slices/mini/DBFilesClient/Spell.dbc && [ ! -e $ST/slices/mini.stage ] && [ ! -e $ST/slices/mini.new ]"
# an upload whose own tier lists name files it does not hold (an export cut short)
rm -rf "$T/short"; cp -a "$T/work" "$T/short"
for i in 1 2 3 4 5 6 7 8 9; do printf '4\tWorld\\Maps\\gone%s.adt\n' "$i" >> "$T/short/mini/.tiers/1.txt"; done
tar -cf - -C "$T/short" mini | bash "$ST/slice-install.sh" mini 1000 > "$T/short.log" 2>&1; rc=$?
check "upload missing listed files: refused, the old slice untouched" "[ $rc != 0 ] && grep -q 'listed files are missing' $T/short.log && [ \$(live_ino) = $ino ] && grep -qx good $ST/slices/mini/DBFilesClient/Spell.dbc"
# a slice description with the upload goes in only together with the slice
python3 - "$ST/slices/mini.json" "$T/short/mini.json" <<'PY2'
import json, sys; d = json.load(open(sys.argv[1])); d["maxTier"] = 1; json.dump(d, open(sys.argv[2], "w"))
PY2
cp "$ST/slices/mini.json" "$T/json.before"
tar -cf - -C "$T/short" mini.json mini | bash "$ST/slice-install.sh" mini 1000 > /dev/null 2>&1
check "refused upload: its mini.json is not installed either" "cmp -s $ST/slices/mini.json $T/json.before && [ ! -e $ST/slices/mini.json.new ]"
# wu-play answers 404 for a key file on the new slice: the previous slice goes back
rm -rf "$T/next"; cp -a "$T/work" "$T/next"; echo next > "$T/next/mini/DBFilesClient/Spell.dbc"
FAKE_PLAY_CODE=404 WU_PLAY_WAIT=2 bash -c 'tar -cf - -C "$1" mini | bash "$2/slice-install.sh" mini 1000' _ "$T/next" "$ST" > "$T/p404.log" 2>&1; rc=$?
check "wu-play answers 404 after the swap: the previous slice is back" "grep -q 'answers 404' $T/p404.log && grep -q 'SLICE ROLLBACK OK' $T/p404.log && grep -qx good $ST/slices/mini/DBFilesClient/Spell.dbc && grep -qx next $ST/slices/mini.old/DBFilesClient/Spell.dbc"
FAKE_PLAY_CODE=200 bash "$ST/slice-install.sh" --rollback mini > "$T/rb.log" 2>&1; rc=$?
check "--rollback swaps the two again" "[ $rc = 0 ] && grep -qx next $ST/slices/mini/DBFilesClient/Spell.dbc && grep -qx good $ST/slices/mini.old/DBFilesClient/Spell.dbc"
bash "$ST/slice-install.sh" --rollback mini > /dev/null 2>&1
rm -rf "$ST/slices/mini.old"
bash "$ST/slice-install.sh" --rollback mini > "$T/rb2.log" 2>&1; rc=$?
check "--rollback without a previous slice: refused" "[ $rc != 0 ] && grep -q 'no previous slice' $T/rb2.log && grep -qx good $ST/slices/mini/DBFilesClient/Spell.dbc"
# too little disk even if the old slice went: nothing is removed
ino=$(live_ino); : > "$WU/docker.log"; cp -a "$ST/slices/mini" "$ST/slices/mini.old"
tar -cf - -C "$T/work" mini | WU_KEEP_FREE_MB=99999999 bash "$ST/slice-install.sh" mini 1000 > "$T/disk.log" 2>&1; rc=$?
check "too little disk even without the old slice: refused, the old slice keeps serving" "[ $rc != 0 ] && grep -q 'Nothing changed' $T/disk.log && [ \$(live_ino) = $ino ] && [ -d $ST/slices/mini.old ] && ! grep -q 'rm -f wu-play' $WU/docker.log"
# room for one copy only (the 00:58 case): the old slice is renamed away, then deleted, then the new one goes in
rm -rf "$ST/slices/mini.old"; : > "$WU/docker.log"
cap=$(( $(du -sk "$ST/slices" | cut -f1) + 1024 + 1 - 8 ))   # need = 1 MB kept free + 1000 bytes + 5%: 8 KB short
tar -cf - -C "$T/next" mini | FAKE_DF_CAP_KB=$cap bash "$ST/slice-install.sh" mini 1000 > "$T/low.log" 2>&1; rc=$?
check "room for one copy: old slice removed first, the new one installed, wu-play back" "[ $rc = 0 ] && grep -q 'removing the old slice first' $T/low.log && grep -q 'rm -f wu-play' $WU/docker.log && grep -qx next $ST/slices/mini/DBFilesClient/Spell.dbc && ! ls -d $ST/slices/*.going >/dev/null 2>&1 || { cat $T/low.log; false; }"
# a leftover copy that wu-play still serves (it was renamed under the running container): wu-play moves first
mkdir -p "$ST/slices/mini.stage/DBFilesClient"; stat -c %i "$ST/slices/mini.stage" > "$WU/fake-play-inode"; : > "$WU/docker.log"
tar -cf - -C "$T/work" mini | bash "$ST/slice-install.sh" mini 1000 > "$T/left.log" 2>&1; rc=$?
check "leftover copy served by wu-play: wu-play restarted before it is removed" "[ $rc = 0 ] && grep -q 'it still served mini.stage' $T/left.log && [ \$(grep -c 'restart wu-play' $WU/docker.log) -ge 2 ]"
rm -f "$WU/fake-play-inode"
# one slice change at a time
if command -v flock >/dev/null; then
  ino=$(live_ino)
  ( exec 8>>"$ST/slices/.lock"; flock 8; sleep 6 ) & lockpid=$!; sleep 1
  tar -cf - -C "$T/work" mini | bash "$ST/slice-install.sh" mini 1000 > "$T/lock.log" 2>&1; rc=$?
  kill $lockpid 2>/dev/null; wait $lockpid 2>/dev/null
  check "a second slice change while one runs: refused, nothing changed" "[ $rc != 0 ] && grep -q 'another slice change' $T/lock.log && [ \$(live_ino) = $ino ]"
fi

# --- slice-health.sh: the read-only check
bash "$ST/slice-health.sh" > "$T/health.log" 2>&1; rc=$?
check "slice-health: whole slice, counts, key files, tier lists" "[ $rc = 0 ] && grep -q '^SLICE HEALTH DONE: OK' $T/health.log && grep -q 'ok  DBFilesClient.Spell.dbc' $T/health.log && grep -q 'tier lists: 2 tiers, 5/5' $T/health.log && grep -q '== mini (SERVED)' $T/health.log || { cat $T/health.log; false; }"
check "slice-health: lists the rollback copy apart" "grep -q '== mini.old (a copy, not served: old)' $T/health.log"
mv "$ST/slices/mini/DBFilesClient/Spell.dbc" "$T/spell.keep"
bash "$ST/slice-health.sh" > "$T/health2.log" 2>&1; rc=$?
check "slice-health: a missing key file is a PROBLEM on the served slice" "[ $rc = 0 ] && grep -q 'PROBLEM: missing DBFilesClient.Spell.dbc' $T/health2.log && grep -q '^SLICE HEALTH DONE: PROBLEMS' $T/health2.log"
bash "$ST/slice-health.sh" --verify "$ST/slices/mini" > /dev/null 2>&1; rc=$?
check "slice-health --verify: exit 1 for a slice that is not whole" "[ $rc = 1 ]"
mv "$T/spell.keep" "$ST/slices/mini/DBFilesClient/Spell.dbc"
echo 1234 > "$WU/fake-play-inode"
bash "$ST/slice-health.sh" > "$T/health3.log" 2>&1
check "slice-health: wu-play serving a replaced folder is a PROBLEM" "grep -q 'PROBLEM: wu-play serves another folder' $T/health3.log"
rm -f "$WU/fake-play-inode"

# --- slice_restage (stream-lib.sh): a change to a live slice goes into a hard-link copy, then a rename
cat > "$T/restage.sh" <<'R'
set -euo pipefail
. "$1/stream-lib.sh"
grow() { printf 'new\n' > "$1/added.txt.tmp" && mv "$1/added.txt.tmp" "$1/Interface/GlueXML/GlueXML.toc"; echo x > "$1/World/Maps/y.adt"; }
fail() { rm -rf "$1/DBFilesClient"; return 1; }
breakit() { rm -rf "$1/Interface"; }
case "$2" in
  grow) slice_restage "$1/slices/mini" grow; echo "swapped:$SLICE_SWAPPED"; slice_swapped_done ;;
  fail) slice_restage "$1/slices/mini" fail ;;
  break) slice_restage "$1/slices/mini" breakit ;;
esac
R
toc_before=$(cat "$ST/slices/mini/Interface/GlueXML/GlueXML.toc"); ino=$(live_ino)
bash "$T/restage.sh" "$ST" fail > "$T/rs1.log" 2>&1; rc=$?
check "restage: a failed change leaves the live slice exactly as it was" "[ $rc != 0 ] && [ \$(live_ino) = $ino ] && [ -f $ST/slices/mini/DBFilesClient/Spell.dbc ] && [ ! -e $ST/slices/mini.stage ]"
bash "$T/restage.sh" "$ST" break > "$T/rs2.log" 2>&1; rc=$?
check "restage: a change that leaves no whole slice is not swapped in" "[ $rc != 0 ] && grep -q 'not a whole slice' $T/rs2.log && [ \$(live_ino) = $ino ] && [ -d $ST/slices/mini/Interface/GlueXML ]"
cp -p "$ST/slices/mini/Interface/GlueXML/GlueXML.toc" "$T/toc.linkcheck"; ln -f "$ST/slices/mini/DBFilesClient/Map.dbc" "$T/map.link"
bash "$T/restage.sh" "$ST" grow > "$T/rs3.log" 2>&1; rc=$?
check "restage: a good change is renamed into place, the replaced copy removed" "[ $rc = 0 ] && [ \$(live_ino) != $ino ] && grep -qx new $ST/slices/mini/Interface/GlueXML/GlueXML.toc && [ -f $ST/slices/mini/World/Maps/y.adt ] && grep -q 'swapped: .*mini.prev' $T/rs3.log && [ ! -e $ST/slices/mini.prev ]"
check "restage: the copy shares the data (hard links), the old files were never written in place" "[ \$(stat -c %i $ST/slices/mini/DBFilesClient/Map.dbc) = \$(stat -c %i $T/map.link) ] && [ \"\$(cat $T/toc.linkcheck)\" = \"$toc_before\" ]"
rm -f "$T/map.link"

# --- Forever UI laid over the live slice (fui-install.sh reapply-slice): through a copy, never in place
OVF="$ST/overlays/foreverui"
mkdir -p "$OVF/Interface/AddOns/ForeverUI" "$OVF/Interface/GlueXML"
echo '## Title: ForeverUI' > "$OVF/Interface/AddOns/ForeverUI/ForeverUI.toc"; echo '<Ui/>' > "$OVF/Interface/GlueXML/ForeverUIGlue.xml"
cp "$HERE/server/fui-overlay.py" "$ST/"
toc_ino=$(stat -c %i "$ST/slices/mini/Interface/GlueXML/GlueXML.toc"); cp "$ST/slices/mini/Interface/GlueXML/GlueXML.toc" "$T/toc.stock"
ln -f "$ST/slices/mini/Interface/GlueXML/GlueXML.toc" "$T/toc.link"; ino=$(live_ino)
bash "$ST/fui-install.sh" reapply-slice mini > "$T/fui.log" 2>&1; rc=$?
check "fui reapply on the live slice: laid over a copy, renamed in" "[ $rc = 0 ] && [ \$(live_ino) != $ino ] && [ -f $ST/slices/mini/Interface/AddOns/ForeverUI/ForeverUI.toc ] && grep -qx ForeverUIGlue.xml $ST/slices/mini/Interface/GlueXML/GlueXML.toc && [ ! -e $ST/slices/mini.prev ] && [ ! -e $ST/slices/mini.stage ] || { cat $T/fui.log; false; }"
check "fui reapply: the stock GlueXML.toc the old slice served was not edited in place" "cmp -s $T/toc.link $T/toc.stock"
bash "$ST/fui-install.sh" remove > "$T/fui2.log" 2>&1; rc=$?
check "fui remove: the live slice as uploaded again (through a copy)" "[ $rc = 0 ] && [ ! -e $ST/slices/mini/Interface/AddOns/ForeverUI ] && cmp -s $ST/slices/mini/Interface/GlueXML/GlueXML.toc $T/toc.stock && [ ! -e $ST/slices/mini/.overlay-foreverui ]"
rm -rf "$ST/overlays" "$T/toc.link"

# --- the game data killswitch (gamedata-switch.sh): fakes for Caddy's answers (410 once its block is in)
cat > "$T/fake/curl" <<'C'
#!/bin/sh
url=""; for a in "$@"; do case "$a" in http*) url=$a ;; esac; done
case "$url" in *ipify*) printf 1.2.3.4; exit 0 ;; esac
if grep -q "game data killswitch" "$WU_CADDYFILE" 2>/dev/null; then
  case "$url" in */data*|*manifest.json) printf 410; exit 0 ;; esac
fi
case "$url" in *.wasm|*.webp) printf 200 ;; *) printf 302 ;; esac
C
chmod +x "$T/fake/curl"
sed -i '2i [ "$1 $2" = "exec wu-portal" ] \&\& [ -n "${FAKE_EXEC_FAIL:-}" ] \&\& exit 1' "$T/fake/docker"
export WU_BACKUPS=$T/backups WU_SETTLE=0 WU_PLAY_WAIT=1
mkdir -p "$ST/overlays/foreverui/Interface/ForeverUI"; echo art > "$ST/overlays/foreverui/Interface/ForeverUI/x.blp"
echo '{"names":["Interface\\GlueXML\\GlueParent.lua"]}' > "$ST/www/boot-manifest.json"
GD=$WU/portal/gamedata-off
cp "$WU_CADDYFILE" "$T/cf.before-gd"
: > "$WU/sql.log"
bash "$ST/gamedata-switch.sh" off > "$T/gd1.log" 2>&1; rc=$?
check "gamedata off: succeeds" "[ $rc = 0 ] && grep -q '^GAMEDATA OFF OK' $T/gd1.log || { tail -30 $T/gd1.log; false; }"
check "gamedata off: the flag with since" "grep -q '\"gamedata_streaming\": false, \"since\": \"20' $GD"
check "gamedata off: one 410 block, inside the stream site" "[ \$(grep -c '# >>> game data killswitch' $WU_CADDYFILE) = 1 ] && python3 -I $ST/gamedata-caddy.py status $WU_CADDYFILE | grep -qx on"
check "gamedata off: Caddy backup kept" "ls $T/backups/*/Caddyfile >/dev/null 2>&1"
check "gamedata off: stream accounts released" "grep -q 'DELETE FROM ov_stream_accounts' $WU/sql.log"
check "gamedata off: before 302 (gated), after 410" "grep -A12 -- '-- before --' $T/gd1.log | grep -q 'index.json  302' && grep -A14 -- '-- after --' $T/gd1.log | grep -q 'index.json  410'"
check "gamedata off: the checks name the tier index and a slice file" "grep -q 'CHECK /data/__tier/index.json -> 410' $T/gd1.log && grep -q 'CHECK /data/DBFilesClient%5CMap.dbc -> 410' $T/gd1.log"
check "gamedata off: logged" "grep -q ' off by=.* checks=1' $WU/gamedata-switch.log && grep -q '\"state\": \"off\"' $WU/portal/gamedata-last.json"
check "gamedata off: nothing moved without --park" "[ -f $ST/slices/mini/DBFilesClient/Map.dbc ] && [ ! -e $WU/offline-gamedata/CURRENT ]"
bash "$ST/gamedata-switch.sh" off > "$T/gd2.log" 2>&1; rc=$?
check "gamedata off twice: still one block" "[ $rc = 0 ] && [ \$(grep -c '# >>> game data killswitch' $WU_CADDYFILE) = 1 ] && grep -q 'already off' $T/gd2.log"
tar -cf - -C "$T/work" mini | bash "$ST/slice-install.sh" mini 1000 > "$T/gd-inst.log" 2>&1; rc=$?
check "gamedata off: a slice upload is refused" "[ $rc != 0 ] && grep -q 'game data is switched off' $T/gd-inst.log && grep -qx new $ST/slices/mini/DBFilesClient/Map.dbc"
bash "$ST/stream-up.sh" > "$T/gd-up.log" 2>&1; rc=$?
check "gamedata off: stream-up keeps the 410 block" "[ $rc = 0 ] && [ \$(grep -c '# >>> game data killswitch' $WU_CADDYFILE) = 1 ] && grep -q 'game data is switched off here' $T/gd-up.log"
bash "$ST/gamedata-switch.sh" status > "$T/gd-st.log" 2>&1; rc=$?
check "gamedata status" "[ $rc = 0 ] && grep -q 'game data streaming: OFF since' $T/gd-st.log && grep -q '^GAMEDATA STATUS OK' $T/gd-st.log"
bash "$ST/gamedata-switch.sh" on > "$T/gd3.log" 2>&1; rc=$?
check "gamedata on: succeeds, the Caddyfile as before, no flag" "[ $rc = 0 ] && grep -q '^GAMEDATA ON OK' $T/gd3.log && cmp -s $WU_CADDYFILE $T/cf.before-gd && [ ! -e $GD ]"
check "gamedata on: logged, since" "grep -q ' on by=.* checks=1' $WU/gamedata-switch.log && grep -q '\"state\": \"on\"' $WU/portal/gamedata-last.json"
# --park: off the served folders, never deleted, and back
: > "$WU/docker.log"
bash "$ST/gamedata-switch.sh" off --park > "$T/gd4.log" 2>&1; rc=$?
P=$(cat "$WU/offline-gamedata/CURRENT" 2>/dev/null)
check "gamedata off --park: succeeds" "[ $rc = 0 ] && grep -q '^GAMEDATA OFF OK' $T/gd4.log || { tail -20 $T/gd4.log; false; }"
check "park: the slice moved, a placeholder served" "[ -n '$P' ] && grep -qx new $P/slices/mini/DBFilesClient/Map.dbc && [ -f $ST/slices/mini/.gamedata-parked ] && [ ! -e $ST/slices/mini/DBFilesClient/Map.dbc ]"
check "park: the Forever UI overlay and the manifest moved" "[ -f $P/overlays/foreverui/Interface/ForeverUI/x.blp ] && [ ! -e $ST/overlays/foreverui ] && [ -f $P/www/boot-manifest.json ] && [ ! -e $ST/www/boot-manifest.json ]"
check "park: wu-play restarted" "grep -q 'restart wu-play' $WU/docker.log"
bash "$ST/gamedata-switch.sh" on > "$T/gd5.log" 2>&1; rc=$?
check "on after park: everything back" "[ $rc = 0 ] && grep -qx new $ST/slices/mini/DBFilesClient/Map.dbc && [ ! -e $ST/slices/mini/.gamedata-parked ] && [ -f $ST/overlays/foreverui/Interface/ForeverUI/x.blp ] && [ -f $ST/www/boot-manifest.json ] && [ ! -e $WU/offline-gamedata/CURRENT ]"
check "on after park: the record kept" "ls $P/restored-*.txt >/dev/null 2>&1"
# park, but wu-play no longer serves the page on the placeholders: everything goes back, the flag stays (fail safe)
FAKE_EXEC_FAIL=1 bash "$ST/gamedata-switch.sh" off --park > "$T/gd6.log" 2>&1; rc=$?
check "park refused by wu-play: fails, files back, still off" "[ $rc != 0 ] && grep -qx new $ST/slices/mini/DBFilesClient/Map.dbc && [ ! -e $WU/offline-gamedata/CURRENT ] && [ -f $GD ]"
bash "$ST/gamedata-switch.sh" on > /dev/null 2>&1
# Caddy refusing the 410 block: its old file back, the flag stays (the gate still answers 410)
FAKE_CADDY_FAIL=1 bash "$ST/gamedata-switch.sh" off > "$T/gd7.log" 2>&1; rc=$?
check "gamedata off, Caddy refusing: fails, Caddyfile restored, flag kept" "[ $rc != 0 ] && cmp -s $WU_CADDYFILE $T/cf.before-gd && [ -f $GD ]"
FAKE_CADDY_FAIL=1 bash "$ST/gamedata-switch.sh" on > "$T/gd8.log" 2>&1; rc=$?
check "gamedata on with no block to remove: no Caddy reload needed, flag gone" "[ $rc = 0 ] && [ ! -e $GD ]"
bash "$ST/gamedata-switch.sh" sideways > /dev/null 2>&1; rc=$?
check "gamedata: bad argument refused" "[ $rc != 0 ]"
rm -rf "$ST/overlays"

# --- stream-down
: > "$WU/sql.log"
bash "$ST/stream-down.sh" > "$T/down.log" 2>&1; rc=$?
check "stream-down succeeds" "[ $rc = 0 ]"
check "Caddy block gone, the rest as before" "! grep -q 'wrath.openvanilla.app' $WU_CADDYFILE && grep -q 'reverse_proxy ovlive-realm:8090' $WU_CADDYFILE"
check "wu-play removed, old portal back, WU_STREAM=0" "grep -q 'rm -f wu-play' $WU/docker.log && grep -q 'old portal' $WU/portal/wrath_portal.py && grep -qx WU_STREAM=0 $WU/portal.env"
check "realm rule removed, marks released" "[ ! -f $WU/custom/lua_scripts/ov_slices.lua ] && [ ! -f $WU/custom/lua_scripts/ov_slices_tiers.lua ] && grep -q 'DELETE FROM ov_stream_accounts' $WU/sql.log"
check "own-files button off again" "grep -qx WU_OWNFILES=0 $WU/portal.env"

echo; echo "$pass/$((pass+fail)) passed"
[ "$fail" = 0 ]
