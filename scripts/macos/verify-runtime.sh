#!/usr/bin/env bash
set -euo pipefail

ROOT="src-tauri/resources/revo-root"
if [ ! -d "$ROOT/core/lib" ]; then
  echo "ERROR: core/lib does not exist"
  exit 1
fi
if [ ! -d "$ROOT/core/lib/obs-plugins" ]; then
  echo "ERROR: core/lib/obs-plugins does not exist"
  exit 1
fi
DYLIB_COUNT=$(
  find "$ROOT/core/lib" \
    -type f \
    -name "*.dylib" |
  wc -l |
  tr -d ' '
)
echo "macOS dylibs: $DYLIB_COUNT"
if [ "$DYLIB_COUNT" -eq 0 ]; then
  echo "ERROR: no macOS dylibs found"
  exit 1
fi
for lib in \
  libavcodec.dylib \
  libavformat.dylib \
  libavutil.dylib \
  libswresample.dylib \
  libswscale.dylib
do
  test -f "$ROOT/core/lib/$lib" || {
    echo "ERROR: required FFmpeg library missing: $ROOT/core/lib/$lib"
    exit 1
  }
done
echo "Required FFmpeg dylibs are staged"
PLUGIN_COUNT=$(
  find "$ROOT/core/lib/obs-plugins" \
    -type d \
    -name "*.plugin" |
  wc -l |
  tr -d ' '
)
echo "macOS OBS plugins: $PLUGIN_COUNT"
if [ "$PLUGIN_COUNT" -eq 0 ]; then
  echo "ERROR: no OBS .plugin bundles found"
  exit 1
fi
WINDOWS_BINARIES=$(
  find "$ROOT" \
    -type f \
    \( \
      -name "*.dll" \
      -o -name "*.exe" \
      -o -name "*.pdb" \
    \) \
    -print
)
if [ -n "$WINDOWS_BINARIES" ]; then
  echo "ERROR: Windows binaries leaked into macOS runtime:"
  echo "$WINDOWS_BINARIES"
  exit 1
fi
echo
echo "============================================================"
echo "macOS dylibs"
echo "============================================================"
find "$ROOT/core/lib" \
  -type f \
  -name "*.dylib" \
  -print |
sort
echo
echo "============================================================"
echo "macOS OBS plugins"
echo "============================================================"
find "$ROOT/core/lib/obs-plugins" \
  -type d \
  -name "*.plugin" \
  -print |
sort
echo
echo "============================================================"
echo "macOS frameworks"
echo "============================================================"
find "$ROOT/core/lib" \
  -type d \
  -name "*.framework" \
  -print |
sort
echo
echo "macOS runtime layout looks valid"
bash scripts/macos/verify-macho-architecture.sh "$ROOT" "${REVO_CMAKE_ARCH:?REVO_CMAKE_ARCH is required}"
