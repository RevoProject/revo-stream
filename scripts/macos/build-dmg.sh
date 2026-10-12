#!/usr/bin/env bash
set -euo pipefail

TARGET="${REVO_TARGET}"
ARCH="${REVO_ARCH}"
APP_DIR="src-tauri/target/$TARGET/release/bundle/macos"
DMG_DIR="src-tauri/target/$TARGET/release/bundle/dmg"
APP="$(find "$APP_DIR" -maxdepth 1 -type d -name "*.app" -print -quit)"
if [ -z "$APP" ]; then
  echo "ERROR: normalized .app not found"
  exit 1
fi

VERSION="$(node -p "JSON.parse(require('fs').readFileSync('package.json', 'utf8')).version")"
if [ "$ARCH" = "arm64" ]; then
  DMG_ARCH="arm64"
else
  DMG_ARCH="x64"
fi

rm -rf "$RUNNER_TEMP/revo-dmg-stage"
mkdir -p "$RUNNER_TEMP/revo-dmg-stage" "$DMG_DIR"
cp -R "$APP" "$RUNNER_TEMP/revo-dmg-stage/"

rm -f "$DMG_DIR"/*.dmg
DMG="$DMG_DIR/revo-ui_${VERSION}_${DMG_ARCH}.dmg"

hdiutil create \
  -volname "RevoStream" \
  -srcfolder "$RUNNER_TEMP/revo-dmg-stage" \
  -ov \
  -format UDZO \
  "$DMG"

test -s "$DMG"
echo "Created normalized macOS DMG:"
ls -lh "$DMG"
