#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob

BUNDLE="src-tauri/target/release/bundle"
appdirs=("$BUNDLE"/appimage/*.AppDir)
test "${#appdirs[@]}" -eq 1
APPDIR="${appdirs[0]}"
test -x "$APPDIR/AppRun"
VERSION="$(node -p "JSON.parse(require('fs').readFileSync('package.json', 'utf8')).version")"
tar -C "$(dirname "$APPDIR")" -czf \
  "$BUNDLE/revo-ui-${VERSION}-linux-x64-portable.tar.gz" "$(basename "$APPDIR")"
makeself --nox11 "$APPDIR" "$BUNDLE/revo-ui-${VERSION}-linux-x64.run" \
  "RevoStream ${VERSION}" ./AppRun
