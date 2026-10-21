#!/usr/bin/env bash
set -euo pipefail

TARGET="${REVO_TARGET:?REVO_TARGET is required}"
APP_DIR="src-tauri/target/$TARGET/release/bundle/macos"
APP="$(find "$APP_DIR" -maxdepth 1 -type d -name "*.app" -print -quit)"

if [ -z "$APP" ]; then
  echo "ERROR: .app was not found in $APP_DIR"
  exit 1
fi

is_macho() {
  file "$1" 2>/dev/null | grep -q "Mach-O"
}

# Normalization rewrote load commands and thinned Mach-O files, which
# invalidates their ad-hoc signatures. codesign --deep does not cover flat
# dylibs stored in Resources, and arm64 refuses to load such binaries
# (SIGKILL on dlopen), so sign every Mach-O individually — inner-most first —
# and then the outer bundle.
signed=0
while IFS= read -r -d '' binary; do
  is_macho "$binary" || continue
  case "$binary" in
    "$APP/Contents/MacOS/"*) continue ;;
  esac
  codesign --force --sign - "$binary" 2>/dev/null || {
    echo "ERROR: failed to sign $binary"
    exit 1
  }
  signed=$((signed + 1))
done < <(find "$APP" -type f -print0)

codesign --force --sign - "$APP"
codesign --verify --verbose=1 "$APP" > /dev/null

echo "Ad-hoc signed $signed Mach-O files plus the bundle: $APP"
codesign --verify --verbose=2 "$APP" 2>&1 | tail -3
