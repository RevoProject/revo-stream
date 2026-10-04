#!/usr/bin/env bash
# Run RevoStream in DEBUG/dev mode.
#
# Usage:
#   bash scripts/nix/debug.sh            # dev mode: vite dev server + debug binary
#   bash scripts/nix/debug.sh embedded    # standalone: binary with embedded build/
#
# dev mode (default):
#   - starts the frontend dev server via `pnpm tauri dev`
#     (beforeDevCommand -> `pnpm dev` -> vite dev on http://localhost:1420)
#   - the debug binary loads the frontend from devUrl, so frontend edits are
#     picked up live (HMR) without rebuilding the binary
#
# embedded mode:
#   - builds with `--features custom-protocol` so the binary serves the
#     compiled build/ instead of trying to reach localhost:1420
#   - NOTE: run `pnpm build` first, otherwise you get the STALE frontend
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
MODE="${1:-dev}"

# Runtime shared libraries and GStreamer plugin dirs from the Nix store.
# Same resolution as run.sh — lib.getLib picks the library output of
# multi-output packages (e.g. glib, pango default to -bin outputs).
eval_out=$(nix eval --impure --raw --expr '
  let
    pkgs = import <nixpkgs> {};
    lib = pkgs.lib;
    getLib = name: lib.getLib (builtins.getAttr name pkgs);
    gst = name: "${lib.getLib pkgs.gst_all_1.${name}}/lib/gstreamer-1.0";
    libNames = [ "obs-studio" "gtk3" "webkitgtk_4_1" "libsoup_3" "glib" "cairo" "pango" "gdk-pixbuf" "atk" "libxkbcommon" "dbus" "gsettings-desktop-schemas" ];
    gstNames = [ "gstreamer" "gst-plugins-base" "gst-plugins-good" "gst-plugins-bad" "gst-plugins-ugly" "gst-libav" ];
  in
    (lib.concatStringsSep ":" (map (name: "${getLib name}/lib") libNames))
    + "\n" + (lib.concatStringsSep ":" (map gst gstNames))
' 2>/dev/null) || eval_out=""

LIB_PATH=""
GST_LIB_PATH=""
if [ -n "$eval_out" ]; then
  LIB_PATH="${eval_out%%$'\n'*}"
  GST_LIB_PATH="${eval_out#*$'\n'}"
fi

# Fallback: per-package lookup (slower, handles missing flake expr)
if [ -z "$LIB_PATH" ]; then
  for pkg in obs-studio gtk3 webkitgtk_4_1 libsoup_3 glib cairo pango gdk-pixbuf atk libxkbcommon dbus; do
    path=$(nix eval --impure "nixpkgs#$pkg.lib" --raw 2>/dev/null || \
           nix eval --impure "nixpkgs#$pkg.out" --raw 2>/dev/null || true)
    if [ -n "$path" ] && [ -d "$path/lib" ]; then
      LIB_PATH="${LIB_PATH:+$LIB_PATH:}$path/lib"
    fi
  done
  for pkg in gst_all_1.gstreamer gst_all_1.gst-plugins-base gst_all_1.gst-plugins-good gst_all_1.gst-plugins-bad gst_all_1.gst-plugins-ugly gst_all_1.gst-libav; do
    path=$(nix eval --impure "nixpkgs#$pkg" --raw 2>/dev/null || true)
    if [ -n "$path" ] && [ -d "$path/lib/gstreamer-1.0" ]; then
      GST_LIB_PATH="${GST_LIB_PATH:+$GST_LIB_PATH:}$path/lib/gstreamer-1.0"
    fi
  done
fi

export GST_PLUGIN_SYSTEM_PATH="$GST_LIB_PATH"
export GST_PLUGIN_PATH="$GST_LIB_PATH"
export GST_REGISTRY_REUSE_PLUGIN_SCANNER="no"
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_USE_GL=software

# OBS runtime — REVO_ROOT z efektami libobs (libobs-opengl.so ładowane przez dlopen)
export REVO_ROOT="${REVO_ROOT:-$PROJECT_DIR/.local/revo-root}"
export LD_LIBRARY_PATH="$LIB_PATH${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

if [ "$MODE" = "embedded" ]; then
  # Standalone binary with build/ compiled in. Requires a fresh `pnpm build`,
  # otherwise the binary serves the STALE embedded frontend.
  nix-shell "$PROJECT_DIR/shell.nix" --run \
    "cargo build --manifest-path src-tauri/Cargo.toml --features custom-protocol"
  exec nix run --impure github:nix-community/nixGL -- "$PROJECT_DIR/src-tauri/target/debug/revo-ui"
fi

# dev mode (default): Tauri CLI starts the vite dev server (beforeDevCommand
# -> `pnpm dev`) and runs the debug binary pointed at devUrl localhost:1420.
# Frontend edits hot-reload; no stale embedded assets possible.
cd "$PROJECT_DIR"
exec nix-shell "$PROJECT_DIR/shell.nix" --run \
  "nix run --impure github:nix-community/nixGL -- pnpm tauri dev"
