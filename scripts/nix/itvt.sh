#!/usr/bin/env bash

# iTVT — one‑file launcher dla NixOS
# Umieść ten plik obok binarki (np. po bash scripts/nix/compile.sh) i uruchom:
#   chmod +x itvt.sh && ./itvt.sh
# Jeśli binarki nie ma obok, skrypt sam zbuduje aplikację.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BINARY="$SCRIPT_DIR/revo-ui"

if [ ! -f "$BINARY" ]; then
  BINARY="$SCRIPT_DIR/usr/bin/revo-ui"
fi

if [ ! -f "$BINARY" ]; then
  echo "=== Buduję iTVT z kodu źródłowego ==="
  SRC_DIR="$SCRIPT_DIR"
  # Jeśli skrypt jest na zewnątrz, sprawdź sąsiedni katalog
  [ -f "$SRC_DIR/src-tauri/Cargo.toml" ] || SRC_DIR="$(pwd)"
  cd "$SRC_DIR"
  pnpm install --frozen-lockfile
  pnpm tauri build --bundles deb,rpm
  BINARY="src-tauri/target/release/revo-ui"
  echo "=== Zbudowano: $BINARY ==="
fi

# GStreamer — najpierw nix eval (flake), potem fallback do NixOS paths
GST_DIRS=""
for p in gst_all_1.gstreamer gst_all_1.gst-plugins-base gst_all_1.gst-plugins-good \
         gst_all_1.gst-plugins-bad gst_all_1.gst-plugins-ugly gst_all_1.gst-libav; do
  d="$(nix eval --impure "nixpkgs#$p" --raw 2>/dev/null || true)"
  [ -n "$d" ] && [ -d "$d/lib/gstreamer-1.0" ] && GST_DIRS="${GST_DIRS:+$GST_DIRS:}$d/lib/gstreamer-1.0"
done
# Fallback: NixOS system profiles
for d in /run/current-system/sw/lib/gstreamer-1.0 \
         /nix/var/nix/profiles/default/lib/gstreamer-1.0 \
         ~/.nix-profile/lib/gstreamer-1.0; do
  [ -d "$d" ] && GST_DIRS="${GST_DIRS:+$GST_DIRS:}$d"
done

export GST_PLUGIN_SYSTEM_PATH="$GST_DIRS"
export GST_PLUGIN_PATH="$GST_DIRS"
export GST_REGISTRY_REUSE_PLUGIN_SCANNER="no"
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_USE_GL=software

# GPU detection — wybierz odpowiedni wrapper nixGL
GPU="$(lspci 2>/dev/null | grep -iE "vga|3d|display" | grep -ioE "intel|nvidia|amd" | head -1 || echo "auto")"
case "$GPU" in
  nvidia) FLAKE="github:nix-community/nixGL#nixGLNVIDIA" ;;
  amd)    FLAKE="github:nix-community/nixGL#nixGLAMD" ;;
  *)      FLAKE="github:nix-community/nixGL" ;;
esac

exec nix run --impure "$FLAKE" -- "$BINARY"
