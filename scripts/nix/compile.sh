#!/usr/bin/env bash
# Build RevoStream with Nix.
# Usage:
#   bash scripts/nix/compile.sh          # native GPUI binary (release)
#   bash scripts/nix/compile.sh tauri    # legacy Tauri deb + rpm bundles
set -euo pipefail

MODE="${1:-gpui}"
cd "$(dirname "$0")/../.."

if [ "$MODE" = "gpui" ]; then
  echo "=== Building native GPUI binary with Nix ==="
  nix-shell --run "
    echo '=== Nix shell ready ==='
    cargo build --release --manifest-path crates/revo-gpui/Cargo.toml
    echo '=== Build complete ==='
    ls -la crates/revo-gpui/target/release/revo-gpui
  "
  exit 0
fi

echo "=== Building RevoStream (legacy Tauri bundles) with Nix ==="
nix-shell --run "
  echo '=== Nix shell ready ==='
  pnpm install --frozen-lockfile
  # Skip AppImage — xdg-mime not available on NixOS
  pnpm tauri build --bundles deb,rpm
  echo '=== Build complete ==='
  ls -la src-tauri/target/release/bundle/
"
