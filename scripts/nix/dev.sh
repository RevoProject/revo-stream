#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Uzycie:
#   bash scripts/nix/dev.sh                # cargo run (tylko Rust)
#   bash scripts/nix/dev.sh tauri          # pnpm tauri dev (frontend + Rust)

MODE="${1:-cargo}"

# --- Prepare REVO_ROOT with OBS share data from Nix store ---
# Umieszczone poza src-tauri/, by Tauri nie watchowalo tych plikow.
OBS_STORE="$(nix eval --impure nixpkgs#obs-studio --raw 2>/dev/null || true)"
REVO_ROOT="$PROJECT_DIR/.local/revo-root"
EFFECT="$REVO_ROOT/share/obs/libobs/default.effect"

if [ -n "$OBS_STORE" ]; then
  # OBS libobs shader effects — do data_dir (REVO_ROOT/share/obs/libobs/)
  if [ ! -f "$EFFECT" ]; then
    mkdir -p "$REVO_ROOT/share/obs/libobs"
    for f in "$OBS_STORE/share/obs/libobs/"*; do
      [ -e "$f" ] && ln -sf "$f" "$REVO_ROOT/share/obs/libobs/"
    done
  fi
  # OBS plugins — oczekiwane w ROOT/core/lib/obs-plugins/
  if [ ! -L "$REVO_ROOT/core/lib/obs-plugins" ]; then
    mkdir -p "$REVO_ROOT/core/lib"
    ln -sfT "$OBS_STORE/lib/obs-plugins" "$REVO_ROOT/core/lib/obs-plugins"
  fi
  # OBS share data (frontend, locale, locale, etc.) dla core_data_dir
  if [ ! -L "$REVO_ROOT/core/share/obs" ]; then
    mkdir -p "$REVO_ROOT/core/share"
    ln -sfT "$OBS_STORE/share/obs" "$REVO_ROOT/core/share/obs"
  fi
fi

export REVO_ROOT
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_USE_GL=software
# OBS runtime needs libobs-opengl.so via dlopen — dodajemy sciezke lib obs-studio
export LD_LIBRARY_PATH="${OBS_STORE}/lib:${LD_LIBRARY_PATH:-}"

run_inside_shell() {
  nix-shell "$PROJECT_DIR/shell.nix" --run \
    "nix run --impure github:nix-community/nixGL -- $1"
}

if [ "$MODE" = "tauri" ]; then
  run_inside_shell "pnpm run tauri dev"
else
  cd "$PROJECT_DIR/src-tauri"
  run_inside_shell "cargo run --no-default-features"
fi
