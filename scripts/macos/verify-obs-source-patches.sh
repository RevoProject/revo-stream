#!/usr/bin/env bash
set -euo pipefail

OBS_SRC="$RUNNER_TEMP/obs-studio"
test -f "$OBS_SRC/CMakePresets.json"
test -f "$OBS_SRC/libobs-metal/CMakeLists.txt"
test -f "$OBS_SRC/plugins/mac-virtualcam/src/dal-plugin/OBSDALStream.mm"
echo "=== OBS source ==="
git -C "$OBS_SRC" \
  log -1 --oneline
echo
echo "=== libobs-metal ==="
head -n 8 \
  "$OBS_SRC/libobs-metal/CMakeLists.txt"
echo
echo "=== OBSDALStream diagnostics ==="
grep -n -C 2 \
  "clang diagnostic" \
  "$OBS_SRC/plugins/mac-virtualcam/src/dal-plugin/OBSDALStream.mm"
