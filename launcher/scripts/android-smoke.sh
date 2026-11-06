#!/usr/bin/env bash
# The phone app's smoke test on an Android emulator (tools/github-workflows/leyline-android.yml, docs/ANDROID.md):
# install the APK, start it, wait, take a screenshot upright and one sideways, keep the app's log, and fail when the
# app crashed (a Java FATAL EXCEPTION or a native fatal signal in its process) or is no longer running.
#
#   bash launcher/scripts/android-smoke.sh APK [OUT_DIR] [PACKAGE]
set -uo pipefail
APK="${1:?usage: android-smoke.sh APK [OUT_DIR] [PACKAGE]}"
OUT="${2:-android-smoke}"
PKG="${3:-com.leylinelauncher.app}"
WAIT="${SMOKE_WAIT:-30}"
mkdir -p "$OUT"
fail() { echo "SMOKE FAILED: $*"; adb logcat -d > "$OUT/logcat.txt" 2>/dev/null; exit 1; }

adb wait-for-device
adb shell getprop ro.build.version.release | tr -d '\r' | sed 's/^/Android /'
adb install -r -g "$APK" || fail "the APK didn't install"
adb logcat -c
adb shell monkey -p "$PKG" -c android.intent.category.LAUNCHER 1 >/dev/null || fail "the app didn't start"
sleep "$WAIT"
pid=$(adb shell pidof "$PKG" | tr -d '\r')
adb exec-out screencap -p > "$OUT/portrait.png"
[ -n "$pid" ] || fail "the app is not running $WAIT s after it started"
echo "running as pid $pid"

# sideways (the browser game plays in landscape)
adb shell settings put system accelerometer_rotation 0
adb shell settings put system user_rotation 1
sleep 10
adb exec-out screencap -p > "$OUT/landscape.png"
adb shell uiautomator dump /sdcard/leyline-ui.xml >/dev/null 2>&1 && adb pull /sdcard/leyline-ui.xml "$OUT/ui.xml" >/dev/null 2>&1
pid2=$(adb shell pidof "$PKG" | tr -d '\r')

adb logcat -d > "$OUT/logcat.txt"
grep -E "$PKG|RustStdoutStderr|Tauri|chromium" "$OUT/logcat.txt" > "$OUT/app-log.txt" || true
crash=$(grep -A4 "FATAL EXCEPTION" "$OUT/logcat.txt" | grep -F "Process: $PKG" || true)
native=$(grep -E "Fatal signal" "$OUT/logcat.txt" | grep -F "$PKG" || true)
[ -z "$crash" ] || fail "the app crashed (Java): $crash"
[ -z "$native" ] || fail "the app crashed (native): $native"
[ -n "$pid2" ] || fail "the app stopped after turning sideways"
[ "$pid" = "$pid2" ] || echo "note: the app restarted when it turned sideways ($pid -> $pid2)"
echo "SMOKE OK: started, ran $WAIT s upright and 10 s sideways, no crash"
