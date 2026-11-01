#!/usr/bin/env bash
# build-browser-client.sh — the browser client, built on this PC and packed for the "client" step.
# Run by "Build browser client.bat" (Leyline Control step "client-build") under Git Bash on Windows;
# it also runs as is on Linux. Nothing here asks a question, and nothing goes system-wide: every
# tool lands in one folder (LEYLINE_BUILD_ROOT, by default %LOCALAPPDATA%\Leyline\client-build).
#
#   1. checks: memory (16 GB, plus a page file that can grow), free disk, the patch list
#   2. tools, once: rustup with the toolchain benilla-wotlk pins (+ rust-src, wasm32 target),
#      wasm-bindgen at the Cargo.lock version, wasi-sdk 27, binaryen (wasm-opt), brotli
#   3. source: benilla-wotlk at b9a49428 with patches/series.txt applied in order
#   4. build: the release profile (fat LTO) as the shipped builds were made: build-std, simd128
#   5. pack: wasm-bindgen, the page files, wasm-opt -O3, brotli -q5, dist-test.tgz in 9,969,155-byte
#      parts, dist-test.parts.txt (CRLF) and wenilla_bg.wasm.br into play\ — the "client" step ships it
#
#   bash build-browser-client.sh [--dry-run | --source-only]
#
# --dry-run: the checks run for real; every download, build and write is only printed.
# --source-only: the source step alone (benilla-wotlk with the patches applied, in the build folder),
#   then stop: what get-source.sh does, and the check that the patch list still applies.
# It never reads or writes game data.
set -euo pipefail

DRY=0
SOURCE_ONLY=0
for a in "$@"; do
  case "$a" in
    --dry-run|/dryrun|-n) DRY=1 ;;
    --source-only) SOURCE_ONLY=1 ;;
    *) echo "unknown option: $a (use --dry-run or --source-only)" >&2; exit 64 ;;
  esac
done

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
PLAY="${LEYLINE_PLAY_DIR:-$REPO/play}"
PATCHES="$HERE/patches"
SERIES="$PATCHES/series.txt"
PIN=b9a49428
UPSTREAM="${LEYLINE_CLIENT_UPSTREAM:-https://github.com/Onixaz/benilla-wotlk}"
PART_BYTES=9969155
NEED_DISK_GB=25
NEED_RAM_GB=15          # a 16 GB PC reports a little under 16
NEED_COMMIT_GB=32       # memory + page file for the fat-LTO link, unless Windows grows the page file itself
WASI_SDK_VERSION=27
BINARYEN_VERSION=130
BROTLI_VERSION=1.1.0
STARTED=$(date +%s)

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) OS=windows; EXE=.exe ;;
  Linux) OS=linux; EXE= ;;
  *) echo "This build runs on Windows (Git Bash) or Linux, not $(uname -s)." >&2; exit 1 ;;
esac

if [ -n "${LEYLINE_BUILD_ROOT:-}" ]; then
  ROOT="$LEYLINE_BUILD_ROOT"
elif [ "$OS" = windows ]; then
  ROOT="$(cygpath -u "${LOCALAPPDATA:-$USERPROFILE/AppData/Local}")/Leyline/client-build"
else
  ROOT="${XDG_CACHE_HOME:-$HOME/.cache}/leyline-client-build"
fi
TOOLS="$ROOT/tools"
SRC="$ROOT/benilla-wotlk"
TARGET="$ROOT/target"
OUT="$ROOT/out"
# Native Windows programs (cargo, rustc, build scripts) read these: C:/... paths, not /c/....
winpath() { if [ "$OS" = windows ]; then cygpath -m "$1"; else printf '%s' "$1"; fi; }
export PATH="$TOOLS/cargo/bin:$PATH"
RUSTUP_HOME="$(winpath "$TOOLS/rustup")"; CARGO_HOME="$(winpath "$TOOLS/cargo")"
export RUSTUP_HOME CARGO_HOME

n=0
step() { n=$((n + 1)); echo; echo "== $n. $* ($(( $(date +%s) - STARTED )) s)"; }
say() { echo "   $*"; }
die() { echo; echo "STOPPED: $*" >&2; exit 1; }
# Anything that downloads, builds or writes goes through here: printed in a dry run, run otherwise.
run() {
  if [ "$DRY" = 1 ]; then echo "   would run: $*"; else "$@"; fi
}
fetch() { # fetch <url> <file>
  if [ "$DRY" = 1 ]; then echo "   would download: $1"; return 0; fi
  say "downloading $1"
  curl -fsSL --retry 3 -o "$2" "$1" || die "could not download $1 (check the internet connection, then run it again)"
}

[ "$DRY" = 1 ] && echo "DRY RUN: checks only; nothing is downloaded, built or written."
echo "Build folder: $ROOT"

# ---------------------------------------------------------------------------------------------
step "Checks: memory, disk, patches"
mem_gb() { # total memory and memory+page file, whole GB; the LEYLINE_TEST_* values stand in for tests
  if [ -n "${LEYLINE_TEST_RAM_GB:-}" ]; then
    echo "$LEYLINE_TEST_RAM_GB ${LEYLINE_TEST_COMMIT_GB:-$LEYLINE_TEST_RAM_GB} ${LEYLINE_TEST_PAGEFILE_AUTO:-1}"
    return
  fi
  if [ "$OS" = windows ]; then
    # shellcheck disable=SC2016  # PowerShell's own $variables
    powershell.exe -NoProfile -NonInteractive -Command \
      '$c=Get-CimInstance Win32_ComputerSystem; $o=Get-CimInstance Win32_OperatingSystem;
       "{0} {1} {2}" -f [math]::Floor($c.TotalPhysicalMemory/1GB), [math]::Floor($o.TotalVirtualMemorySize/1MB), [int][bool]$c.AutomaticManagedPagefile' \
      | tr -d '\r'
  else
    awk '/^MemTotal:/{m=$2} /^SwapTotal:/{s=$2} END{printf "%d %d 0\n", m/1048576, (m+s)/1048576}' /proc/meminfo
  fi
}
read -r RAM COMMIT AUTO <<<"$(mem_gb)"
if [ "$SOURCE_ONLY" = 1 ]; then RAM=$NEED_RAM_GB; COMMIT=$NEED_COMMIT_GB; AUTO=1; NEED_DISK_GB=3; fi  # no build: no memory check
if [ "$SOURCE_ONLY" = 1 ]; then
  say "memory: not needed (source only)"
else
  say "memory: ${RAM} GB; with the page file: ${COMMIT} GB$([ "$AUTO" = 1 ] && echo ' (Windows grows the page file as needed)')"
fi
[ "${RAM:-0}" -ge "$NEED_RAM_GB" ] || die "this computer has ${RAM} GB of memory; the client build needs 16 GB."
if [ "${COMMIT:-0}" -lt "$NEED_COMMIT_GB" ] && [ "$AUTO" != 1 ]; then
  die "memory plus the page file is ${COMMIT} GB; the last build step needs about ${NEED_COMMIT_GB} GB.
Turn on 'Automatically manage paging file size for all drives' (System > About > Advanced system settings >
Performance Settings > Advanced > Virtual memory), restart, and run this again."
fi
run mkdir -p "$ROOT"
probe="$ROOT"; while [ ! -d "$probe" ]; do probe="$(dirname "$probe")"; done
FREE_GB=${LEYLINE_TEST_FREE_GB:-$(( $(df -Pk "$probe" | awk 'NR==2{print $4}') / 1048576 ))}
say "free disk for $ROOT: ${FREE_GB} GB"
have_target=0; [ -d "$TARGET/wasm32-unknown-unknown" ] && have_target=1
[ "$have_target" = 1 ] && say "(an earlier build is there: it is reused)"
if [ "$FREE_GB" -lt "$NEED_DISK_GB" ] && [ "$have_target" = 0 ]; then
  die "only ${FREE_GB} GB free on that drive; the build needs about ${NEED_DISK_GB} GB. Free some space and run it again."
fi
[ "$FREE_GB" -ge 3 ] || die "only ${FREE_GB} GB free on that drive. Free some space and run it again."
[ -f "$SERIES" ] || die "the patch list is missing: $SERIES"
PATCH_LIST=()
while IFS= read -r line || [ -n "$line" ]; do
  line="${line%$'\r'}"
  case "$line" in ''|'#'*) continue ;; esac
  [ -f "$PATCHES/$line" ] || die "the patch list names $line, which is not in client/patches."
  PATCH_LIST+=("$line")
done <"$SERIES"
[ "${#PATCH_LIST[@]}" -gt 0 ] || die "the patch list is empty: $SERIES"
say "patches, in order: ${PATCH_LIST[*]}"
for t in git curl tar split gzip awk; do command -v "$t" >/dev/null || die "$t is missing (Git for Windows brings it)."; done

# ---------------------------------------------------------------------------------------------
step "Source: benilla-wotlk at $PIN with ${#PATCH_LIST[@]} patches"
if [ ! -d "$SRC/.git" ]; then
  run git -c core.autocrlf=false clone -q "$UPSTREAM" "$SRC"
fi
if [ "$DRY" = 0 ]; then
  git -C "$SRC" config core.autocrlf false
  git -C "$SRC" cat-file -e "$PIN^{commit}" 2>/dev/null || git -C "$SRC" fetch -q origin \
    || die "could not fetch benilla-wotlk"
  git -C "$SRC" checkout -q -f -B wrathunleashed "$PIN"
  git -C "$SRC" clean -q -ffdx
  for p in "${PATCH_LIST[@]}"; do
    # LF only: a checkout with autocrlf may have turned the patch's line ends into CRLF.
    tr -d '\r' <"$PATCHES/$p" >"$ROOT/apply.patch"
    git -C "$SRC" apply --whitespace=nowarn "$ROOT/apply.patch" || die "patch $p does not apply on $PIN."
    say "applied $p"
  done
  rm -f "$ROOT/apply.patch"
else
  say "would check out $PIN in $SRC (a clean tree) and apply each patch above with git apply"
fi
if [ -f "$SRC/rust-toolchain.toml" ]; then
  CHANNEL=$(awk -F'"' '/^channel/{print $2; exit}' "$SRC/rust-toolchain.toml")
else
  CHANNEL="${LEYLINE_TEST_CHANNEL:-1.98.1}"   # a dry run before the first fetch: the pin's channel
fi
WB_VERSION=$( [ -f "$SRC/Cargo.lock" ] && awk '/^name = "wasm-bindgen"$/{getline; gsub(/version = |"/,""); print; exit}' "$SRC/Cargo.lock" || true)
WB_VERSION="${WB_VERSION:-${LEYLINE_TEST_WB_VERSION:-0.2.122}}"
say "toolchain $CHANNEL, wasm-bindgen $WB_VERSION"
if [ "$SOURCE_ONLY" = 1 ]; then
  echo; echo "Source ready: $SRC"; exit 0
fi

# ---------------------------------------------------------------------------------------------
step "Rust toolchain $CHANNEL (+ rust-src, wasm32), in $TOOLS"
run mkdir -p "$TOOLS"
if [ ! -x "$TOOLS/cargo/bin/rustup$EXE" ]; then
  if [ "$OS" = windows ]; then
    fetch https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe "$TOOLS/rustup-init.exe"
  else
    fetch https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init "$TOOLS/rustup-init"
    run chmod +x "$TOOLS/rustup-init"
  fi
  run "$TOOLS/rustup-init$EXE" -y -q --no-modify-path --profile minimal --default-toolchain none
else
  say "rustup already there"
fi
# The host half (build scripts, macros) links with Visual Studio's linker when it is installed, else
# with the GNU toolchain's own linker, so no Visual Studio is needed.
HOST=""
if [ "$OS" = windows ]; then
  vswhere="$(cygpath -u "${PROGRAMFILES:-C:/Program Files} (x86)")/Microsoft Visual Studio/Installer/vswhere.exe"
  if [ -x "$vswhere" ] && [ -n "$("$vswhere" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>/dev/null | tr -d '\r')" ]; then
    HOST=x86_64-pc-windows-msvc
  else
    HOST=x86_64-pc-windows-gnu
  fi
  TOOLCHAIN="$CHANNEL-$HOST"
else
  TOOLCHAIN="$CHANNEL"
fi
say "toolchain: $TOOLCHAIN"
run rustup toolchain install "$TOOLCHAIN" --profile minimal --component rust-src --target wasm32-unknown-unknown
export RUSTUP_TOOLCHAIN="$TOOLCHAIN"

# ---------------------------------------------------------------------------------------------
step "wasm-bindgen $WB_VERSION"
WB_DIR="$TOOLS/wasm-bindgen-$WB_VERSION"
WB="$WB_DIR/wasm-bindgen$EXE"
if [ -x "$WB" ] && "$WB" --version >/dev/null 2>&1; then
  say "already there"
else
  wtriple=$([ "$OS" = windows ] && echo x86_64-pc-windows-msvc || echo x86_64-unknown-linux-musl)
  url="https://github.com/wasm-bindgen/wasm-bindgen/releases/download/$WB_VERSION/wasm-bindgen-$WB_VERSION-$wtriple.tar.gz"
  if [ "$DRY" = 1 ]; then
    echo "   would download: $url (else: cargo install wasm-bindgen-cli --version $WB_VERSION)"
  else
    mkdir -p "$WB_DIR"
    if curl -fsSL --retry 3 -o "$TOOLS/wb.tgz" "$url" && tar -xzf "$TOOLS/wb.tgz" -C "$WB_DIR" --strip-components=1 \
       && "$WB" --version >/dev/null 2>&1; then
      say "downloaded"
    else
      say "no ready-made wasm-bindgen; building it (a few minutes)"
      cargo install -q wasm-bindgen-cli --version "$WB_VERSION" --locked --root "$WB_DIR"
      cp "$WB_DIR/bin/wasm-bindgen$EXE" "$WB"
    fi
    rm -f "$TOOLS/wb.tgz"
    "$WB" --version | grep -q "$WB_VERSION" || die "wasm-bindgen is not version $WB_VERSION"
  fi
fi

# ---------------------------------------------------------------------------------------------
step "wasi-sdk $WASI_SDK_VERSION (the C side: the Lua VM)"
WASI="$TOOLS/wasi-sdk-$WASI_SDK_VERSION"
if [ -f "$WASI/share/wasi-sysroot/lib/wasm32-wasip1/libsetjmp.a" ]; then
  say "already there"
else
  wplat=$([ "$OS" = windows ] && echo x86_64-windows || echo x86_64-linux)
  fetch "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-$WASI_SDK_VERSION/wasi-sdk-$WASI_SDK_VERSION.0-$wplat.tar.gz" "$TOOLS/wasi.tgz"
  run mkdir -p "$WASI"
  run tar -xzf "$TOOLS/wasi.tgz" -C "$WASI" --strip-components=1
  run rm -f "$TOOLS/wasi.tgz"
  [ "$DRY" = 1 ] || [ -f "$WASI/share/wasi-sysroot/lib/wasm32-wasip1/libsetjmp.a" ] || die "wasi-sdk did not unpack completely"
fi

# ---------------------------------------------------------------------------------------------
step "binaryen $BINARYEN_VERSION (wasm-opt)"
BIN_DIR="$TOOLS/binaryen-$BINARYEN_VERSION"
WASM_OPT="$BIN_DIR/bin/wasm-opt$EXE"
if [ -x "$WASM_OPT" ]; then
  say "already there"
else
  bplat=$([ "$OS" = windows ] && echo x86_64-windows || echo x86_64-linux)
  fetch "https://github.com/WebAssembly/binaryen/releases/download/version_$BINARYEN_VERSION/binaryen-version_$BINARYEN_VERSION-$bplat.tar.gz" "$TOOLS/binaryen.tgz"
  run mkdir -p "$BIN_DIR"
  run tar -xzf "$TOOLS/binaryen.tgz" -C "$BIN_DIR" --strip-components=1
  run rm -f "$TOOLS/binaryen.tgz"
fi

# ---------------------------------------------------------------------------------------------
step "brotli"
BROTLI=""
if [ -x "$TOOLS/brotli/brotli$EXE" ]; then
  BROTLI="$TOOLS/brotli/brotli$EXE"
elif command -v brotli >/dev/null; then
  BROTLI="$(command -v brotli)"
elif [ "$OS" = windows ]; then
  fetch "https://github.com/google/brotli/releases/download/v$BROTLI_VERSION/brotli-x64-windows-static.zip" "$TOOLS/brotli.zip"
  run mkdir -p "$TOOLS/brotli"
  # Git Bash may have no unzip; Windows' own tar.exe reads zip files.
  if command -v unzip >/dev/null; then run unzip -q -o "$TOOLS/brotli.zip" -d "$TOOLS/brotli"
  else run "$(cygpath -u "${SYSTEMROOT:-C:/Windows}")/System32/tar.exe" -xf "$(winpath "$TOOLS/brotli.zip")" -C "$(winpath "$TOOLS/brotli")"; fi
  run rm -f "$TOOLS/brotli.zip"
  BROTLI="$TOOLS/brotli/brotli$EXE"
  # some releases put it in a subfolder
  if [ "$DRY" = 0 ] && [ ! -x "$BROTLI" ]; then
    found=$(find "$TOOLS/brotli" -iname "brotli$EXE" -type f | head -n 1)
    if [ -n "$found" ]; then BROTLI="$found"; else BROTLI=""; fi
  fi
fi
PY=""
for c in python3 python py; do command -v "$c" >/dev/null && "$c" -c "import brotli" 2>/dev/null && { PY="$c"; break; }; done
if [ -n "$BROTLI" ]; then say "using $BROTLI"
elif [ -n "$PY" ]; then say "using $PY's brotli module"
else die "no brotli tool: install one (or Python's brotli module) and run it again."; fi
compress() { # compress <in> <out>: brotli quality 5, as every shipped build
  if [ -n "$BROTLI" ] && { [ "$DRY" = 1 ] || [ -x "$BROTLI" ]; }; then run "$BROTLI" -f -q 5 "$1" -o "$2"
  else run "$PY" -c "import brotli,sys; open(sys.argv[2],'wb').write(brotli.compress(open(sys.argv[1],'rb').read(), quality=5))" "$1" "$2"; fi
}

# ---------------------------------------------------------------------------------------------
step "Build (release: fat LTO; the last step alone takes 20-40 minutes and most of the memory)"
JOBS=$(( $(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4) ))
[ "$JOBS" -gt 6 ] && JOBS=6
WASI_SDK="$(winpath "$WASI")"; CARGO_TARGET_DIR="$(winpath "$TARGET")"
export WASI_SDK CARGO_TARGET_DIR RUSTC_BOOTSTRAP=1 CARGO_INCREMENTAL=0
export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS="-C target-feature=+simd128"
if [ "$DRY" = 1 ]; then
  say "would run in $SRC: cargo build --release -j $JOBS -Z build-std=std,panic_abort --target wasm32-unknown-unknown -p wenilla --no-default-features --features webgpu"
else
  (cd "$SRC" && cargo build --release -j "$JOBS" -Z build-std=std,panic_abort --target wasm32-unknown-unknown \
     -p wenilla --no-default-features --features webgpu) \
    || die "the build failed (the lines above say why). Out of memory? Close other programs and run it again."
fi
WASM="$TARGET/wasm32-unknown-unknown/release/wenilla.wasm"

# ---------------------------------------------------------------------------------------------
step "Pack: wasm-bindgen, page files, wasm-opt -O3, brotli, parts"
DIST="$OUT/stage/dist-test"
run rm -rf "$OUT"
run mkdir -p "$DIST"
run "$WB" --target web --no-typescript --remove-name-section --remove-producers-section --out-dir "$DIST" "$WASM"
for f in index.html wasi_stubs.js boot.js platform.js bridge.js automate.js warm.html boot-manifest.json world-manifest.json; do
  if [ "$DRY" = 1 ] || [ -f "$SRC/web/$f" ]; then run cp "$SRC/web/$f" "$DIST/"; fi
done
run "$WASM_OPT" -O3 --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
  --enable-mutable-globals --enable-reference-types --enable-multivalue --enable-simd \
  "$DIST/wenilla_bg.wasm" -o "$DIST/wenilla_bg.wasm.opt"
run mv "$DIST/wenilla_bg.wasm.opt" "$DIST/wenilla_bg.wasm"
compress "$DIST/wenilla_bg.wasm" "$OUT/wenilla_bg.wasm.br"
run tar -czf "$OUT/dist-test.tgz" -C "$OUT/stage" dist-test
run split -b "$PART_BYTES" -d -a 2 "$OUT/dist-test.tgz" "$OUT/dist-test.tgz.part"
if [ "$DRY" = 0 ]; then
  PARTS=$(find "$OUT" -maxdepth 1 -name 'dist-test.tgz.part*' | wc -l | tr -d ' ')
  printf '%s\r\n' "$PARTS" >"$OUT/dist-test.parts.txt"
  [ "$PARTS" -le 10 ] || die "the build came out in $PARTS parts; the client step joins at most 10."
  say "$(stat -c %s "$DIST/wenilla_bg.wasm" 2>/dev/null || wc -c <"$DIST/wenilla_bg.wasm") bytes of wasm, $PARTS parts"
fi

# ---------------------------------------------------------------------------------------------
step "Into play: dist-test parts, dist-test.parts.txt, wenilla_bg.wasm.br"
run mkdir -p "$PLAY"
if [ "$DRY" = 1 ]; then
  say "would replace $PLAY/dist-test.tgz.part* with the new parts, and write dist-test.parts.txt and wenilla_bg.wasm.br"
else
  rm -f "$PLAY"/dist-test.tgz.part*
  cp "$OUT"/dist-test.tgz.part* "$OUT/dist-test.parts.txt" "$OUT/wenilla_bg.wasm.br" "$PLAY/"
  got=0; for f in "$PLAY"/dist-test.tgz.part*; do [ -f "$f" ] && got=$((got + 1)); done
  say "done: $got parts"
fi

echo
if [ "$DRY" = 1 ]; then echo "Dry run done: every check passed."; exit 0; fi
echo "Done in $(( ($(date +%s) - STARTED) / 60 )) min. The new client is in play\\; the 'client' step ships it."
