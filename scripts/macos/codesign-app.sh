#!/usr/bin/env bash
set -euo pipefail

TARGET="${REVO_TARGET:?REVO_TARGET is required}"
APP_DIR="src-tauri/target/$TARGET/release/bundle/macos"
APP="$(find "$APP_DIR" -maxdepth 1 -type d -name "*.app" -print -quit)"

if [ -z "$APP" ]; then
  echo "ERROR: .app was not found in $APP_DIR"
  exit 1
fi

# Normalization rewrote load commands and thinned Mach-O files, which
# invalidates their ad-hoc signatures. arm64 refuses to load such binaries
# (SIGKILL on dlopen), so re-sign everything ad-hoc after normalization.
codesign --force --deep --sign - "$APP"

codesign --verify --deep --verbose=2 "$APP" > /dev/null
echo "Ad-hoc re-signed and verified: $APP"
