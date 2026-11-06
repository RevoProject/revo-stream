#!/usr/bin/env bash
# Package the native GPUI binary into a portable Linux bundle.
#
# Usage:
#   bash scripts/linux/package-gpui.sh [OBS_INSTALL]
#
# The resulting layout is the GPUI packaging contract:
#
#   revo-gpui-<version>-linux-x64/
#   ├── revo-gpui          native binary
#   ├── revo-root/         staged OBS runtime (core/lib, core/share, core/bin)
#   ├── launch.sh          sets REVO_ROOT and private library paths, execs binary
#   └── README.txt         host requirements
#
# Outputs (under crates/revo-gpui/target/release/bundle/):
#   revo-gpui-<version>-linux-x64-portable.tar.gz
#   revo-gpui-<version>-linux-x64.run       (self-extracting, makeself)
set -euo pipefail
shopt -s nullglob

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
OBS_INSTALL="${1:-}"

BIN="crates/revo-gpui/target/release/revo-gpui"
BUNDLE_DIR="crates/revo-gpui/target/release/bundle"
VERSION="$(awk '/^\[package\]/{f=1} f && /^version = /{gsub(/[",]/,""); print $3; exit}' \
  crates/revo-gpui/Cargo.toml)"
test -n "$VERSION"
PKG="$BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64"

test -x "$BIN" || {
  echo "GPUI binary missing: $BIN" >&2
  echo "Build it first: cargo build --release --manifest-path crates/revo-gpui/Cargo.toml" >&2
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

rm -rf "$PKG" "$BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64-portable.tar.gz" \
  "$BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64.run"
mkdir -p "$PKG"

echo "=== Staging binary ==="
cp "$BIN" "$PKG/revo-gpui"

echo "=== Staging OBS runtime (revo-root) ==="
if [ "${STAGE_MODE:-}" = "reuse" ]; then
  cp -a "$STAGE_SRC" "$PKG/revo-root"
else
  bash "$SCRIPT_DIR/prepare-runtime.sh" "$OBS_INSTALL" "$PKG/revo-root"
fi
test -f "$PKG/revo-root/core/lib/libobs.so"
test -f "$PKG/revo-root/data/share/obs/libobs/default.effect"

echo "=== Writing launcher ==="
cat > "$PKG/launch.sh" << 'EOF'
#!/usr/bin/env bash
# RevoStream GPUI launcher: wires the staged OBS runtime and private libs.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"

export REVO_ROOT="$HERE/revo-root"
export LD_LIBRARY_PATH="$HERE/revo-root/core/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
# GL/Vulkan/Wayland/X11 and desktop frameworks stay on the host.
exec "$HERE/revo-gpui" "$@"
EOF
chmod +x "$PKG/launch.sh"

cat > "$PKG/README.txt" << 'EOF'
RevoStream (native GPUI build)

Run:      ./launch.sh
Requires: X11 or Wayland session, GPU/OpenGL (or Vulkan) drivers, glibc,
          fontconfig/freetype. The OBS runtime and its private libraries are
          bundled under revo-root/ and located via REVO_ROOT.
EOF

echo "=== Checking binary dependencies ==="
missing="$(ldd "$PKG/revo-gpui" | awk '/not found/ { print $1 }' || true)"
if [ -n "$missing" ]; then
  echo "Missing runtime dependencies for the GPUI binary:" >&2
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
echo "    tar.gz: $BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64-portable.tar.gz"
[ -f "$BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64.run" ] && \
  echo "    .run:   $BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64.run"
exit 0
