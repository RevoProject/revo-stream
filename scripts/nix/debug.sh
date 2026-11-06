#!/usr/bin/env bash
# Run RevoStream in DEBUG/dev mode.
#
# Usage:
#   bash scripts/nix/debug.sh            # native GPUI shell (debug build)
#   bash scripts/nix/debug.sh tauri      # legacy Tauri backend (main window has no UI)
#   bash scripts/nix/debug.sh embedded   # legacy Tauri standalone binary (embedded build/)
#
# gpui mode (default):
#   - builds and runs the native GPUI application from crates/revo-gpui
#
# tauri / embedded modes:
#   - legacy Tauri adapter kept as the ObsRuntime parity reference; the main
#     window has no UI since the Svelte components were removed
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
MODE="${1:-gpui}"

# Runtime shared libraries and GStreamer plugin dirs from the Nix store.
# Same resolution as run.sh — lib.getLib picks the library output of
# multi-output packages (e.g. glib, pango default to -bin outputs).
eval_out=$(nix eval --impure --raw --expr '
  let
    pkgs = import <nixpkgs> {};
    lib = pkgs.lib;
    getLib = name: lib.getLib (builtins.getAttr name pkgs);
    gst = name: "${lib.getLib pkgs.gst_all_1.${name}}/lib/gstreamer-1.0";
    libNames = [ "obs-studio" "gtk3" "webkitgtk_4_1" "libsoup_3" "glib" "cairo" "pango" "gdk-pixbuf" "atk" "libxkbcommon" "dbus" "gsettings-desktop-schemas" "libayatana-appindicator" ];
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
  for pkg in obs-studio gtk3 webkitgtk_4_1 libsoup_3 glib cairo pango gdk-pixbuf atk libxkbcommon dbus libayatana-appindicator; do
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

# A killed Cargo build may leave fresh fingerprints but missing generated
# build_script executables. Repair that exact state before Cargo runs;
# healthy no-op checks are incremental and finish quickly.
nix-shell "$PROJECT_DIR/shell.nix" --run \
  "bash scripts/nix/repair-debug-profile.sh"

if [ "$MODE" = "gpui" ]; then
  # Native GPUI shell wrapped by nixGL, same GPU handling as run.sh.
  cd "$PROJECT_DIR"
  exec nix-shell "$PROJECT_DIR/shell.nix" --run \
    "nix run --impure github:nix-community/nixGL -- cargo run --manifest-path crates/revo-gpui/Cargo.toml"
fi

if [ "$MODE" = "embedded" ]; then
  # Standalone binary with build/ compiled in. Requires a fresh `pnpm build`,
  # otherwise the binary serves the STALE embedded frontend.
  nix-shell "$PROJECT_DIR/shell.nix" --run \
    "cargo build --manifest-path src-tauri/Cargo.toml --features custom-protocol"
  exec nix run --impure github:nix-community/nixGL -- "$PROJECT_DIR/src-tauri/target/debug/revo-ui"
fi

# tauri modes: run the legacy adapter through the Tauri CLI (embedded) or
# pnpm tauri dev. The main window has no UI; used for backend parity runs.
cd "$PROJECT_DIR"
exec nix-shell "$PROJECT_DIR/shell.nix" --run \
  "nix run --impure github:nix-community/nixGL -- pnpm tauri dev"
