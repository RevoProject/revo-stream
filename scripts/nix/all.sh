#!/usr/bin/env bash
# iTVT — full NixOS build pipeline
# 1. Build app (deb + rpm)
# 2. Package self-extracting .run
# 3. Clean build artifacts (keeps .run)

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "=== 1/3: Building app ==="
bash "$SCRIPT_DIR/compile.sh"

echo "=== 2/3: Packaging .run bundle ==="
bash "$SCRIPT_DIR/package-bundle.sh"

echo "=== 3/3: Cleaning build artifacts ==="
# Keep only: .run, deb, rpm, source code
rm -rf "$PROJECT_DIR/src-tauri/target/release/build"
rm -rf "$PROJECT_DIR/src-tauri/target/release/.fingerprint"
rm -rf "$PROJECT_DIR/src-tauri/target/release/deps"
rm -rf "$PROJECT_DIR/src-tauri/target/release/incremental"
rm -rf "$PROJECT_DIR/src-tauri/target/release/*.d"
rm -rf "$PROJECT_DIR/src-tauri/target/release/*.o"
rm -rf "$PROJECT_DIR/node_modules/.cache"
rm -rf "$PROJECT_DIR/dist/assets/"*.br "$PROJECT_DIR/dist/assets/"*.gz 2>/dev/null || true

echo "=== Done ==="
echo "    .run: $(ls -1t "$PROJECT_DIR"/iTVT-*.run 2>/dev/null | head -1)"
echo "    .deb: $(ls -1t "$PROJECT_DIR"/src-tauri/target/release/bundle/deb/*.deb 2>/dev/null | head -1)"
echo "    .rpm: $(ls -1t "$PROJECT_DIR"/src-tauri/target/release/bundle/rpm/*.rpm 2>/dev/null | head -1)"
