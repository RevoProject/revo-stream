#!/usr/bin/env bash
set -euo pipefail

TARGET="${REVO_TARGET}"
APP_DIR="src-tauri/target/$TARGET/release/bundle/macos"
APP="$(
  find "$APP_DIR" \
    -maxdepth 1 \
    -type d \
    -name "*.app" \
    -print -quit
)"
if [ -z "$APP" ]; then
  echo "ERROR: .app was not produced"
  find "src-tauri/target/$TARGET/release/bundle" -maxdepth 3 -print || true
  exit 1
fi

ROOT="$APP/Contents/Resources/resources/revo-root"
# GPUI packages ship revo-gpui (+ revo-engine); the Tauri app ships revo-ui.
EXE="$APP/Contents/MacOS/${REVO_APP_BINARY:-revo-ui}"
test -x "$EXE"
if [ -n "${REVO_APP_EXTRA_BINARIES:-}" ]; then
  for extra in $REVO_APP_EXTRA_BINARIES; do
    test -x "$APP/Contents/MacOS/$extra" || { echo "ERROR: final app is missing $extra"; exit 1; }
  done
fi
test -d "$ROOT/core/lib"
test -d "$ROOT/core/lib/obs-plugins"

for lib in \
  libavcodec.dylib \
  libavformat.dylib \
  libavutil.dylib \
  libswresample.dylib \
  libswscale.dylib
do
  test -f "$ROOT/core/lib/$lib" || {
    echo "ERROR: final app is missing $lib"
    exit 1
  }
done

echo "=== FINAL APP SIZE ==="
du -sh "$APP"
du -sh "$ROOT"

BAD="$(
  find "$APP" -type f -print0 |
    while IFS= read -r -d '' f; do
      file "$f" 2>/dev/null | grep -q "Mach-O" || continue
      otool -L "$f" 2>/dev/null || true
    done |
    grep -E '/opt/homebrew/|/usr/local/opt/' || true
)"

if [ -n "$BAD" ]; then
  echo "$BAD"
  echo "ERROR: final app still contains Homebrew absolute dylib references"
  exit 1
fi

bash scripts/macos/verify-macho-architecture.sh "$APP" "${REVO_CMAKE_ARCH:?REVO_CMAKE_ARCH is required}"

echo "Final macOS .app is self-contained"
