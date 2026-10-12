#!/usr/bin/env bash
set -euo pipefail

DMG_DIR="src-tauri/target/${REVO_TARGET}/release/bundle/dmg"
DMG="$(
  find "$DMG_DIR" \
    -maxdepth 1 \
    -type f \
    -name "*.dmg" \
    -print -quit
)"
if [ -z "$DMG" ]; then
  echo "ERROR: DMG was not produced"
  echo "Expected directory: $DMG_DIR"
  exit 1
fi

echo "macOS DMG:"
ls -lh "$DMG"

if command -v hdiutil >/dev/null 2>&1; then
  hdiutil imageinfo "$DMG" >/dev/null
fi
