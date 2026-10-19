#!/usr/bin/env bash
set -euo pipefail

TARGET="${REVO_TARGET:?REVO_TARGET is required}"
APP_DIR="src-tauri/target/$TARGET/release/bundle/macos"
APP="$(find "$APP_DIR" -maxdepth 1 -type d -name "*.app" -print -quit)"

if [ -z "$APP" ]; then
  echo "ERROR: .app was not found in $APP_DIR"
  exit 1
fi

SRC="src-tauri/resources/revo-root"
DST="$APP/Contents/Resources/resources/revo-root"

test -d "$SRC" || { echo "ERROR: staged runtime root is missing: $SRC"; exit 1; }

# The Tauri bundler has been observed to drop staged subdirectories (for
# example core/share) from the bundled app. Copy the complete runtime root
# over the bundled copy instead of trusting the bundler.
mkdir -p "$DST"
ditto "$SRC" "$DST"

failures=0
check() {
  if [ -e "$DST/$1" ]; then
    echo "ok: $1"
  else
    echo "ERROR: bundled runtime is missing $1"
    failures=$((failures + 1))
  fi
}

check "core/share/obs/libobs/default.effect"
check "core/lib/libobs.framework/Versions/A/libobs"
check "core/lib/obs-plugins/obs-ffmpeg.plugin/Contents/MacOS/obs-ffmpeg"
check "core/lib/obs-plugins/rtmp-services.plugin/Contents/MacOS/rtmp-services"
check "core/lib/libavcodec.dylib"
check "core/lib/libavformat.dylib"
check "core/lib/libavutil.dylib"
check "core/lib/libswresample.dylib"
check "core/lib/libswscale.dylib"

plugin_count="$(find "$DST/core/lib/obs-plugins" -maxdepth 1 -type d -name "*.plugin" 2>/dev/null | wc -l | tr -d ' ')"
if [ "$plugin_count" -ge 1 ]; then
  echo "ok: $plugin_count OBS plugin bundles"
else
  echo "ERROR: bundled runtime has no OBS plugin bundles"
  failures=$((failures + 1))
fi

if [ "$failures" -gt 0 ]; then
  echo "ERROR: bundled app resources validation failed with $failures issue(s)"
  exit 1
fi

echo "Bundled app runtime resources are complete in $APP"
