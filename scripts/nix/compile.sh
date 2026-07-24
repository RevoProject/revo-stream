#!/usr/bin/env bash
set -euo pipefail

echo "=== Building iTVT with Nix ==="
cd "$(dirname "$0")/../.."

# Enter nix shell and build
nix-shell --run "
  echo '=== Nix shell ready ==='
  pnpm install --frozen-lockfile
  # Skip AppImage — xdg-mime not available on NixOS
  pnpm tauri build --bundles deb,rpm
  echo '=== Build complete ==='
  ls -la src-tauri/target/release/bundle/
"
