#!/usr/bin/env bash
# Assemble the native GPUI application into a macOS .app bundle.
#
# Inputs (release build, plus the runtime staged by populate-revo-root.sh):
#   crates/revo-gpui/target/$REVO_TARGET/release/revo-gpui
#   crates/revo-backend/target/$REVO_TARGET/release/revo-engine
#   src-tauri/resources/revo-root
#
# Output:
#   src-tauri/target/$REVO_TARGET/release/bundle/macos/RevoStream.app
#
# The bundle is written where the Tauri bundler used to put its app, so the
# shared post-processing scripts (ensure-app-resources, normalize-app-dylib-paths,
# codesign-app, verify-app, build-dmg) keep working unchanged.
set -euo pipefail

TARGET="${REVO_TARGET:?REVO_TARGET is required}"
ARCH="${REVO_ARCH:?REVO_ARCH is required}"
ROOT="${REVO_ROOT_STAGED:-src-tauri/resources/revo-root}"
BUNDLE_ID="com.klubuntuprojects.revo"
APP_NAME="RevoStream"

UI_BIN="crates/revo-gpui/target/$TARGET/release/revo-gpui"
ENGINE_BIN="crates/revo-backend/target/$TARGET/release/revo-engine"
MACOS_DIR="src-tauri/target/$TARGET/release/bundle/macos"
APP="$MACOS_DIR/$APP_NAME.app"
PLIST_TOOL="/usr/libexec/PlistBuddy"

test -x "$UI_BIN" || { echo "missing $UI_BIN" >&2; exit 1; }
test -x "$ENGINE_BIN" || { echo "missing $ENGINE_BIN" >&2; exit 1; }
test -d "$ROOT" || { echo "missing staged runtime root: $ROOT" >&2; exit 1; }

VERSION="$(awk '/^\[package\]/{f=1} f && /^version = /{gsub(/[",]/,""); print $3; exit}' \
  crates/revo-gpui/Cargo.toml)"
test -n "$VERSION"

if [[ "$VERSION" =~ ^([0-9]+\.[0-9]+\.[0-9]+)(-pre([0-9]+))?(\+.*)?$ ]]; then
  MACOS_SHORT_VERSION="${BASH_REMATCH[1]}"
  MACOS_BUILD_VERSION="${BASH_REMATCH[3]:-0}"
else
  echo "Unsupported Cargo version: $VERSION" >&2
  exit 1
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources/resources"

echo "=== Staging binaries ==="
install -m755 "$UI_BIN" "$APP/Contents/MacOS/revo-gpui"
install -m755 "$ENGINE_BIN" "$APP/Contents/MacOS/revo-engine"

echo "=== Staging icons ==="
if [ -f "src-tauri/icons/icon.icns" ]; then
  cp "src-tauri/icons/icon.icns" "$APP/Contents/Resources/icon.icns"
fi

echo "=== Staging OBS runtime ==="
ditto "$ROOT" "$APP/Contents/Resources/resources/revo-root"

echo "=== Writing Info.plist ==="
camera_usage="RevoStream uses your camera as a video source for recording and streaming."
microphone_usage="RevoStream uses your microphone as an audio source for recording and streaming."
if [ -f "src-tauri/Info.plist" ] && [ -x "$PLIST_TOOL" ]; then
  camera_usage="$("$PLIST_TOOL" -c "Print :NSCameraUsageDescription" src-tauri/Info.plist 2>/dev/null || echo "$camera_usage")"
  microphone_usage="$("$PLIST_TOOL" -c "Print :NSMicrophoneUsageDescription" src-tauri/Info.plist 2>/dev/null || echo "$microphone_usage")"
fi

cat > "$APP/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>revo-gpui</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundleDisplayName</key>
    <string>$APP_NAME</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$MACOS_SHORT_VERSION</string>
    <key>CFBundleVersion</key>
    <string>$MACOS_BUILD_VERSION</string>
    <key>LSMinimumSystemVersion</key>
    <string>${MACOSX_DEPLOYMENT_TARGET:-12.0}</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSCameraUsageDescription</key>
    <string>$camera_usage</string>
    <key>NSMicrophoneUsageDescription</key>
    <string>$microphone_usage</string>
</dict>
</plist>
EOF
printf 'APPL????' > "$APP/Contents/PkgInfo"

# The engine is looked up as a sibling of the UI binary, which Contents/MacOS
# satisfies inside the bundle. The runtime root is found through the
# exe-relative candidate list in revo-backend::settings::core.
echo "Built macOS GPUI app: $APP ($VERSION, $ARCH)"
test -x "$APP/Contents/MacOS/revo-gpui"
test -x "$APP/Contents/MacOS/revo-engine"
test -e "$APP/Contents/Resources/resources/revo-root/core/share/obs/libobs/default.effect" ||
  test -e "$APP/Contents/Resources/resources/revo-root/data/share/obs/libobs/default.effect"
