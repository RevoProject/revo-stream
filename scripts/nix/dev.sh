#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Uzycie:
#   bash scripts/nix/dev.sh                # natywna powloka GPUI (crates/revo-gpui)
#   bash scripts/nix/dev.sh tauri          # backend Tauri (glowne okno bez UI — parity)
#   bash scripts/nix/dev.sh tauri-frontend # pnpm tauri dev (legacy frontend)

MODE="${1:-gpui}"

# --- Prepare REVO_ROOT with OBS share data from Nix store ---
# Umieszczone poza src-tauri/, by Tauri nie watchowalo tych plikow.
OBS_STORE="$(bash "$SCRIPT_DIR/stage-obs-root.sh")"
REVO_ROOT="$PROJECT_DIR/.local/revo-root"

export REVO_ROOT
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_USE_GL=software
# OBS runtime needs libobs-opengl.so via dlopen — dodajemy sciezke lib obs-studio
APPINDICATOR_STORE="$(nix eval --impure nixpkgs#libayatana-appindicator --raw 2>/dev/null || true)"
export LD_LIBRARY_PATH="${OBS_STORE}/lib${APPINDICATOR_STORE:+:$APPINDICATOR_STORE/lib}:${LD_LIBRARY_PATH:-}"

# Same targeted repair used by debug.sh. It only cleans the dev profile when
# Cargo reports missing target/debug/build/*/out/build_script_* artifacts.
nix-shell "$PROJECT_DIR/shell.nix" --run \
  "bash scripts/nix/repair-debug-profile.sh"

run_inside_shell() {
  nix-shell "$PROJECT_DIR/shell.nix" --run \
    "nix run --impure github:nix-community/nixGL -- $1"
}

if [ "$MODE" = "tauri-frontend" ]; then
  run_inside_shell "pnpm run tauri dev"
elif [ "$MODE" = "gpui" ]; then
  # Native GPUI shell: resolves the X11/Vulkan runtime stack with nix and
  # runs the binary with the host NVIDIA driver stack (see run-gpui.sh).
  exec bash "$SCRIPT_DIR/run-gpui.sh"
else
  cd "$PROJECT_DIR/src-tauri"
  run_inside_shell "cargo run --no-default-features"
fi
