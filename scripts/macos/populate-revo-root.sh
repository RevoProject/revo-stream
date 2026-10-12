#!/usr/bin/env bash
set -euo pipefail

ROOT="src-tauri/resources/revo-root"
OBS_BUILD="$RUNNER_TEMP/obs-build"
OBS_INSTALL="$RUNNER_TEMP/obs-install"
rm -rf "$ROOT"
mkdir -p \
  "$ROOT/data" \
  "$ROOT/core/bin" \
  "$ROOT/core/lib" \
  "$ROOT/core/lib/obs-plugins" \
  "$ROOT/core/share"
# --------------------------------------------------------
# RevoStream data
# --------------------------------------------------------
if [ -d "data" ]; then
  rsync -a \
    "data/" \
    "$ROOT/data/"
fi
# --------------------------------------------------------
# OBS dylibs
# --------------------------------------------------------
DYLIB_COUNT=0
while IFS= read -r dylib; do
  [ -n "$dylib" ] || continue
  echo "Copying dylib: $dylib"
  cp -L \
    "$dylib" \
    "$ROOT/core/lib/$(basename "$dylib")"
  DYLIB_COUNT=$((DYLIB_COUNT + 1))
done < <(
  find "$OBS_BUILD" \
    -type f \
    -name "*.dylib" \
    -print |
  sort
)
if [ "$DYLIB_COUNT" -eq 0 ]; then
  echo "ERROR: no OBS dylibs found"
  exit 1
fi
echo "Collected dylibs: $DYLIB_COUNT"
# --------------------------------------------------------
# Frameworks
# --------------------------------------------------------
FRAMEWORK_COUNT=0
while IFS= read -r framework; do
  [ -n "$framework" ] || continue
  name="$(basename "$framework")"
  echo "Copying framework: $name"
  rm -rf \
    "$ROOT/core/lib/$name"
  cp -R \
    "$framework" \
    "$ROOT/core/lib/$name"
  FRAMEWORK_COUNT=$((FRAMEWORK_COUNT + 1))
done < <(
  find "$OBS_BUILD" \
    -type d \
    -name "*.framework" \
    ! -path "*/EagerLinkingTBDs/*" \
    -print |
  sort -u
)
echo "Collected frameworks: $FRAMEWORK_COUNT"
# --------------------------------------------------------
# OBS plugins
# --------------------------------------------------------
PLUGIN_COUNT=0
while IFS= read -r plugin; do
  [ -n "$plugin" ] || continue
  name="$(basename "$plugin")"
  echo "Copying plugin: $name"
  rm -rf \
    "$ROOT/core/lib/obs-plugins/$name"
  cp -R \
    "$plugin" \
    "$ROOT/core/lib/obs-plugins/$name"
  PLUGIN_COUNT=$((PLUGIN_COUNT + 1))
done < <(
  find "$OBS_BUILD" \
    -type d \
    -name "*.plugin" \
    -print |
  sort -u
)
if [ "$PLUGIN_COUNT" -eq 0 ]; then
  echo "ERROR: no OBS .plugin bundles found"
  exit 1
fi
echo "Collected plugins: $PLUGIN_COUNT"
# --------------------------------------------------------
# Installed resources
# --------------------------------------------------------
if [ -d "$OBS_INSTALL/share" ]; then
  rsync -a \
    "$OBS_INSTALL/share/" \
    "$ROOT/core/share/"
fi
if [ -d "$OBS_INSTALL/bin" ]; then
  rsync -a \
    "$OBS_INSTALL/bin/" \
    "$ROOT/core/bin/"
fi
# --------------------------------------------------------
# Final tree
# --------------------------------------------------------
echo
echo "============================================================"
echo "FINAL macOS revo-root"
echo "============================================================"
find "$ROOT" \
  -maxdepth 8 \
  -print |
sort |
head -2000
