#!/bin/sh
# The server half of the Control step "release-mirror" (tools/release-mirror/release_mirror.py upload): put the new
# release files into /srv/wrath/portal-data/leyline/releases (wu-portal's /data/leyline/releases, served at
# https://play.openvanilla.app/wrath/leyline/releases/ by stream/leyline_net.py; read on every request: no restart).
# New files go in first (their names carry the version, nothing is overwritten), then index.json, SHA256SUMS,
# latest.json and chain.json, each checked against the upload's index first. A backup of those four is kept
# (release-backups/<time>, the last five) and put back if the site does not answer with the new latest.json.
# Launcher folders the new index no longer lists are removed afterwards. Run from the unpacked upload.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
U="$HERE/releases"
R=/srv/wrath/portal-data/leyline/releases
B=/srv/wrath/portal-data/leyline/release-backups
S=$(date +%Y%m%d-%H%M%S)
URL=https://play.openvanilla.app/wrath/leyline/releases
[ -f "$U/index.json" ] || { echo "ERROR: no index.json in the upload" >&2; exit 1; }
# every uploaded file must be in the upload's index with its sha256 (nothing else is copied)
python3 - "$U" <<'PY' || { echo "ERROR: the upload does not match its index.json; nothing was changed" >&2; exit 1; }
import hashlib, json, os, re, sys
u = sys.argv[1]
idx = json.load(open(os.path.join(u, "index.json")))["files"]
ok = re.compile(r"^(?:(?:launcher-v[0-9A-Za-z.+-]{1,48}|host-pack|addons|sources)/)?[A-Za-z0-9][A-Za-z0-9._+-]{0,119}$")
for d, _, fs in os.walk(u):
    for f in fs:
        rel = os.path.relpath(os.path.join(d, f), u).replace(os.sep, "/")
        if rel in ("index.json", "SHA256SUMS", "chain.json"):
            continue
        assert ok.match(rel) and ".." not in rel, rel
        assert not rel.lower().endswith((".mpq", ".blp", ".m2", ".wmo", ".adt", ".dbc", ".db2", ".skin", ".anim")), rel
        h = hashlib.sha256(open(os.path.join(d, f), "rb").read()).hexdigest()
        assert idx.get(rel, {}).get("sha256") == h, rel
PY
mkdir -p "$R" "$B/$S"
for f in index.json SHA256SUMS latest.json chain.json; do [ -f "$R/$f" ] && cp -p "$R/$f" "$B/$S/$f"; done
# 1. the new files (folders by version), readable by the portal
( cd "$U" && find . -type f ! -name index.json ! -name SHA256SUMS ! -name latest.json ! -name chain.json ) | while read -r f; do
  f=${f#./}
  install -D -m 644 "$U/$f" "$R/$f"
done
# 2. the lists, each replaced in one step
for f in latest.json chain.json SHA256SUMS index.json; do
  [ -f "$U/$f" ] || continue
  install -m 644 "$U/$f" "$R/.$f.new" && mv -f "$R/.$f.new" "$R/$f"
done
find "$R" -type d -exec chmod 755 {} +
sleep 1
want=$(sha256sum "$U/latest.json" 2>/dev/null | cut -d' ' -f1 || true)
got=$(curl -fs -m 30 "$URL/latest.json" | sha256sum | cut -d' ' -f1 || true)
if [ -n "$want" ] && [ "$want" != "$got" ]; then
  for f in index.json SHA256SUMS latest.json chain.json; do [ -f "$B/$S/$f" ] && cp -p "$B/$S/$f" "$R/$f"; done
  echo "ERROR: $URL/latest.json did not answer with the new manifest; the old lists are back." >&2
  exit 1
fi
# 3. launcher folders the index no longer lists (older than the kept releases)
for d in "$R"/launcher-v*; do
  [ -d "$d" ] || continue
  n=$(basename "$d")
  grep -q "\"$n/" "$R/index.json" || rm -rf "$d"
done
ls -1t "$B" | tail -n +6 | while read -r d; do rm -rf "$B/$d"; done
echo "  $(grep -c sha256 "$R/index.json" || true) files listed; $(du -sh "$R" | cut -f1) on disk (backup of the lists: $B/$S)"
echo "RELEASE MIRROR OK"
