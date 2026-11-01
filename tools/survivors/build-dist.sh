#!/usr/bin/env bash
# Azeroth Survivors: the browser build, from the upstream source at the locked commit, unmodified. Linux, run by
# whoever makes the build (not the owner's recorder; docs/SURVIVORS.md "The browser build"). It never reads, fetches
# or writes game data: only upstream's wasm build steps (scripts/web-setup.sh's tools, scripts/web-build.sh's
# commands), never make build / make native / make fetch-data / scripts/setup.sh / scripts/fetch-data.sh.
#
#   bash tools/survivors/build-dist.sh <work dir> [--write-lock]
#
# <work dir>/src      the upstream clone, checked out at stream/survivors/upstream.lock.json's sha (cloned when missing)
# <work dir>/dist     the build: upstream's web/dist, plus licenses/ (upstream's licence files and our small index)
# --write-lock        write every dist file's SHA-256 into the lock (the check on the owner's PC and the server)
#
# The toolchain: upstream pins Rust 1.98.1 with a prebuilt wasm32 std. Where that can't be downloaded (this build box:
# static.rust-lang.org is not reachable) it builds with the installed stable Rust and std from source (RUSTC_BOOTSTRAP=1
# -Z build-std=std,panic_abort, as client/build-browser-client.sh does). SV_TOOLCHAIN=pinned uses the pin instead. The
# lock records which one made the build ("toolchain", "deviations").
set -euo pipefail
WORK="${1:?usage: build-dist.sh <work dir> [--write-lock]}"
WRITE="${2:-}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
LOCK="$ROOT/stream/survivors/upstream.lock.json"
REPO=$(python3 -I -c 'import json,sys;print(json.load(open(sys.argv[1]))["repo"])' "$LOCK")
SHA=$(python3 -I -c 'import json,sys;print(json.load(open(sys.argv[1]))["sha"])' "$LOCK")
SRC="$WORK/src"; DIST="$WORK/dist"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$WORK/target}" CARGO_INCREMENTAL=0
mkdir -p "$WORK"
if [ ! -d "$SRC/.git" ]; then git clone -q "$REPO" "$SRC"; fi
git -C "$SRC" fetch -q origin "$SHA" 2>/dev/null || true
git -C "$SRC" checkout -q --detach "$SHA"
[ -z "$(git -C "$SRC" status --porcelain --untracked-files=no)" ] || { echo "the upstream tree has local changes: refusing" >&2; exit 1; }
echo "== upstream $REPO at $(git -C "$SRC" rev-parse HEAD)"

cd "$SRC"
# scripts/web-setup.sh's tools, by hand where its rustup step can't run: wasi-sdk 27 and binaryen 130 into tools/.
mkdir -p tools/wasi-sdk tools/binaryen
[ -f tools/wasi-sdk/share/wasi-sysroot/lib/wasm32-wasip1/libsetjmp.a ] || curl -sL \
  https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-27/wasi-sdk-27.0-x86_64-linux.tar.gz | tar xz --strip-components=1 -C tools/wasi-sdk
[ -x tools/binaryen/bin/wasm-opt ] || curl -sL \
  https://github.com/WebAssembly/binaryen/releases/download/version_130/binaryen-version_130-x86_64-linux.tar.gz | tar xz --strip-components=1 -C tools/binaryen
want=$(awk '/^name = "wasm-bindgen"$/{getline; sub(/version = /,""); gsub(/"/,""); print; exit}' Cargo.lock)
have=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)
[ "$have" = "$want" ] || cargo install wasm-bindgen-cli --version "$want" --locked --force
export WASI_SDK="$SRC/tools/wasi-sdk"

if [ "${SV_TOOLCHAIN:-}" = pinned ]; then
  TOOLCHAIN="rust $(sed -n 's/^channel = "\(.*\)"/\1/p' rust-toolchain.toml) (the pin), prebuilt wasm32 std"
  rustup target add wasm32-unknown-unknown
  cargo build --profile release --target wasm32-unknown-unknown -p wenilla --no-default-features --features webgpu
else
  export RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-stable}" RUSTC_BOOTSTRAP=1
  TOOLCHAIN="$(rustc --version | cut -d' ' -f1-2) (installed stable), std built from source (-Z build-std=std,panic_abort)"
  cargo build --profile release --target wasm32-unknown-unknown -Z build-std=std,panic_abort \
    -p wenilla --no-default-features --features webgpu
fi
echo "== toolchain: $TOOLCHAIN"

# scripts/web-build.sh's packing steps, as upstream wrote them (its own cargo build line is the one above).
WASM="$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/wenilla.wasm"
rm -rf "$DIST"; mkdir -p "$DIST"
wasm-bindgen --target web --no-typescript --remove-name-section --remove-producers-section --out-dir "$DIST" "$WASM"
cp web/index.html web/favicon.svg web/wasi_stubs.js web/boot.js web/platform.js web/bridge.js "$DIST/"
mkdir -p "$DIST/examples" && cp web/examples/*.js "$DIST/examples/"
for m in web/boot-manifest.json web/world-manifest.json; do if [ -f "$m" ]; then cp "$m" "$DIST/"; fi; done
tools/binaryen/bin/wasm-opt -O3 --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
  --enable-mutable-globals --enable-reference-types --enable-multivalue "$DIST/wenilla_bg.wasm" -o "$DIST/wenilla_bg.wasm.opt"
mv "$DIST/wenilla_bg.wasm.opt" "$DIST/wenilla_bg.wasm"
for f in "$DIST"/*.wasm "$DIST"/*.js; do
  if command -v brotli >/dev/null; then brotli -f -q 5 "$f" -o "$f.br"
  elif python3 -c 'import brotli' 2>/dev/null; then   # the same library at the same quality as upstream's brotli -q 5
    python3 -c 'import brotli,sys;open(sys.argv[2],"wb").write(brotli.compress(open(sys.argv[1],"rb").read(),quality=5))' "$f" "$f.br"
  fi
  gzip -kf -6 -n "$f"
done

# The licences next to the build: upstream's own and its vendored components', and a small index for /licenses/.
mkdir -p "$DIST/licenses"
cp LICENSE-MIT LICENSE-APACHE "$DIST/licenses/"
cp third_party/kira/LICENSE-MIT "$DIST/licenses/kira-LICENSE-MIT"
cp third_party/kira/LICENSE-APACHE "$DIST/licenses/kira-LICENSE-APACHE"
cp third_party/lua-src/LICENSE "$DIST/licenses/lua-LICENSE"
sed "s/__SHA__/$SHA/g" "$HERE/licenses-index.html" > "$DIST/licenses/index.html"

if [ "$WRITE" = --write-lock ]; then
  python3 -I - "$LOCK" "$DIST" "$TOOLCHAIN" "$(git -C "$SRC" log -1 --format=%cI)" <<'PY'
import hashlib, json, os, sys
lock_path, dist, toolchain, committed = sys.argv[1:5]
lock = json.load(open(lock_path))
files, total = {}, 0
for d, _, fs in os.walk(dist):
    for f in sorted(fs):
        p = os.path.join(d, f)
        files[os.path.relpath(p, dist).replace(os.sep, "/")] = hashlib.sha256(open(p, "rb").read()).hexdigest()
        total += os.path.getsize(p)
lock["committed"] = committed
lock["toolchain"] = toolchain
lock["dist"] = {"files": dict(sorted(files.items())), "count": len(files), "bytes": total}
open(lock_path, "w").write(json.dumps(lock, indent=1) + "\n")
print(f"== lock written: {len(files)} files, {total / 1048576:.1f} MB")
PY
fi
du -sh "$DIST"
echo "BUILD DIST OK: $DIST"
