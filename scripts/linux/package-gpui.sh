#!/usr/bin/env bash
# Package the native GPUI application into a portable Linux bundle.
#
# Usage:
#   bash scripts/linux/package-gpui.sh [OBS_INSTALL]
#
# The resulting directory is the GPUI packaging contract shared by every Linux
# artifact (portable tar.gz, .run, DEB, RPM, AppImage):
#
#   revo-gpui-<version>-linux-x64/
#   ├── revo-gpui          native UI binary
#   ├── revo-engine        libobs engine process (UI sibling, see services/engine.rs)
#   ├── revo-root/         staged OBS runtime (core/lib, core/share, core/bin)
#   ├── share/             desktop entry + hicolor icons
#   ├── launch.sh          sets REVO_ROOT and private library paths, execs binary
#   └── README.txt         host requirements
#
# Outputs (under crates/revo-gpui/target/release/bundle/):
#   revo-gpui-<version>-linux-x64-portable.tar.gz
#   revo-gpui-<version>-linux-x64.run       (self-extracting, makeself)
#
# Build the binaries first:
#   cargo build --release --features obs-backend --manifest-path crates/revo-gpui/Cargo.toml
#   cargo build --release --bin revo-engine   --manifest-path crates/revo-backend/Cargo.toml
set -euo pipefail
shopt -s nullglob

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
OBS_INSTALL="${1:-}"

BIN="crates/revo-gpui/target/release/revo-gpui"
ENGINE_BIN="crates/revo-backend/target/release/revo-engine"
BUNDLE_DIR="crates/revo-gpui/target/release/bundle"
APP_ID="com.klubuntuprojects.revo"
VERSION="$(awk '/^\[package\]/{f=1} f && /^version = /{gsub(/[",]/,""); print $3; exit}' \
  "$PROJECT_DIR/crates/revo-gpui/Cargo.toml")"
test -n "$VERSION"
PKG="$BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64"

test -x "$BIN" || {
  echo "GPUI binary missing: $BIN" >&2
  echo "Build it first: cargo build --release --features obs-backend --manifest-path crates/revo-gpui/Cargo.toml" >&2
  exit 1
}
test -x "$ENGINE_BIN" || {
  echo "engine binary missing: $ENGINE_BIN" >&2
  echo "Build it first: cargo build --release --bin revo-engine --manifest-path crates/revo-backend/Cargo.toml" >&2
  exit 1
}

# Reuse an already-staged revo-root when OBS_INSTALL is not provided.
if [ -z "$OBS_INSTALL" ]; then
  if [ -f "src-tauri/resources/revo-root/core/lib/libobs.so" ]; then
    echo "=== Reusing staged OBS runtime from src-tauri/resources/revo-root ==="
    STAGE_SRC="src-tauri/resources/revo-root"
    STAGE_MODE="reuse"
  else
    echo "Usage: package-gpui.sh [OBS_INSTALL]" >&2
    echo "Provide an OBS install dir, or stage one first:" >&2
    echo "  bash scripts/linux/build-obs.sh && bash scripts/linux/prepare-runtime.sh <obs-install>" >&2
    exit 1
  fi
fi

# A previously staged tree may carry read-only files (an OBS runtime staged
# from a store-backed directory); make it removable before re-staging.
chmod -R u+rwX "$PKG" 2>/dev/null || true
rm -rf "$PKG" "$BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64-portable.tar.gz" \
  "$BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64.run"
mkdir -p "$PKG"

echo "=== Staging binaries ==="
cp "$BIN" "$PKG/revo-gpui"
cp "$ENGINE_BIN" "$PKG/revo-engine"

echo "=== Staging OBS runtime (revo-root) ==="
if [ "${STAGE_MODE:-}" = "reuse" ]; then
  cp -a "$STAGE_SRC" "$PKG/revo-root"
else
  bash "$SCRIPT_DIR/prepare-runtime.sh" "$OBS_INSTALL" "$PKG/revo-root"
fi
chmod -R u+rwX "$PKG/revo-root"
test -f "$PKG/revo-root/core/lib/libobs.so"
test -f "$PKG/revo-root/data/share/obs/libobs/default.effect"

echo "=== Staging desktop entry and icons ==="
mkdir -p "$PKG/share/applications" "$PKG/share/icons/hicolor"
ICON_SIZES=()
for source in "$PROJECT_DIR/src-tauri/icons"/*.png; do
  case "$(basename "$source")" in
    32x32.png) size=32x32 ;;
    64x64.png) size=64x64 ;;
    128x128.png) size=128x128 ;;
    128x128@2x.png) size=256x256 ;;
    *) continue ;;
  esac
  install -d "$PKG/share/icons/hicolor/$size/apps"
  cp "$source" "$PKG/share/icons/hicolor/$size/apps/revo-gpui.png"
  ICON_SIZES+=("$size")
done
test "${#ICON_SIZES[@]}" -gt 0 || { echo "no icons staged" >&2; exit 1; }

cat > "$PKG/share/applications/$APP_ID.desktop" << EOF
[Desktop Entry]
Name=RevoStream
Comment=Native GPUI streaming and recording studio
Exec=launch.sh
Icon=revo-gpui
Terminal=false
Type=Application
Categories=AudioVideo;Network;
StartupWMClass=RevoStream
EOF

echo "=== Writing launcher ==="
cat > "$PKG/launch.sh" << 'EOF'
#!/usr/bin/env bash
# RevoStream GPUI launcher: wires the staged OBS runtime and private libs.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"

export REVO_ROOT="$HERE/revo-root"
export LD_LIBRARY_PATH="$HERE/revo-root/core/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
# The engine is looked up as a sibling of the UI binary (services/engine.rs).
export REVO_ENGINE_BIN="${REVO_ENGINE_BIN:-$HERE/revo-engine}"
# GL/Vulkan/Wayland/X11 and desktop frameworks stay on the host.
exec "$HERE/revo-gpui" "$@"
EOF
chmod +x "$PKG/launch.sh"

cat > "$PKG/README.txt" << EOF
RevoStream (native GPUI build) $VERSION

Run:      ./launch.sh
Engine:   revo-engine runs the libobs runtime in its own process; the UI
          spawns it automatically (REVO_ENGINE_BIN overrides the path).
Requires: X11 or Wayland session, GPU/OpenGL (or Vulkan) drivers, glibc,
          fontconfig/freetype, xkbcommon. The OBS runtime and its private
          libraries are bundled under revo-root/ and located via REVO_ROOT.
EOF

echo "=== Checking binary dependencies ==="
missing="$(ldd "$PKG/revo-gpui" | awk '/not found/ { print $1 }' || true)"
if [ -n "$missing" ]; then
  echo "Missing runtime dependencies for the GPUI binary:" >&2
  printf '  %s\n' "$missing" >&2
  exit 1
fi
missing="$(ldd "$PKG/revo-engine" | awk '/not found/ { print $1 }' || true)"
if [ -n "$missing" ]; then
  echo "Missing runtime dependencies for the engine binary:" >&2
  printf '  %s\n' "$missing" >&2
  exit 1
fi

echo "=== Building archives ==="
tar -C "$BUNDLE_DIR" -czf \
  "$BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64-portable.tar.gz" \
  "$(basename "$PKG")"

if command -v makeself >/dev/null 2>&1; then
  makeself --nox11 "$PKG" \
    "$BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64.run" \
    "RevoStream GPUI ${VERSION}" ./launch.sh
else
  echo "makeself not found; skipping .run archive (tar.gz is complete)." >&2
fi

echo "=== Done ==="
echo "    staged: $PKG"
echo "    tar.gz: $BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64-portable.tar.gz"
[ -f "$BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64.run" ] && \
  echo "    .run:   $BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64.run"
exit 0
