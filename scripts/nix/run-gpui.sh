#!/usr/bin/env bash
# Launch the native GPUI shell with the runtime stack the native build
# needs on this platform.
#
# GPUI links the X11 client libraries and the Vulkan loader at runtime.
# The NixOS container environment does not ship them in the default
# library path, so this script resolves the runtime packages with nix and
# extends LD_LIBRARY_PATH before running. GPU rendering relies on the
# host driver stack (NVIDIA container runtime); do not set
# VK_ICD_FILENAMES — default device discovery is what the shell has been
# verified against.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

PROVIDERS=(
	nixpkgs#libxcb
	nixpkgs#libx11
	nixpkgs#libxcursor
	nixpkgs#libxrandr
	nixpkgs#libxi
	nixpkgs#libxkbcommon
	nixpkgs#wayland
	nixpkgs#vulkan-loader
)

LIB_PATHS="$(
	nix build --no-link --print-out-paths --impure "${PROVIDERS[@]}" 2>/dev/null |
		while read -r out; do [ -d "$out/lib" ] && echo "$out/lib"; done |
		paste -sd: -
)"
test -n "$LIB_PATHS"

# OBS runtime: stage the shared revo-root layout and export REVO_ROOT so
# StartObs resolves the libobs data/plugin paths; the store's lib dir also
# carries libobs-opengl.so, which obs_startup dlopens as the graphics module.
OBS_STORE="$(bash "$SCRIPT_DIR/stage-obs-root.sh")"
export REVO_ROOT="$PROJECT_DIR/.local/revo-root"
LIB_PATHS="$LIB_PATHS${OBS_STORE:+:$OBS_STORE/lib}"

if [ -z "${DISPLAY:-}" ]; then
	echo "error: \$DISPLAY is not set; launch from a desktop session" >&2
	exit 1
fi

export LD_LIBRARY_PATH="$LIB_PATHS${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

# Real libobs backend: builds and runs inside the shell.nix environment
# (openssl + glib/gobject via pkg-config for the feature build; the shell
# hook prepends the native runtime stack to the inherited LD_LIBRARY_PATH).
# Running the feature build through the outer cargo would rebuild with the
# default feature set and launch the mock binary instead, so the whole
# cargo run stays inside the shell.
if [ "${REVO_GPUI_OBS:-0}" = "1" ]; then
	exec nix-shell "$PROJECT_DIR/shell.nix" --run \
		"cargo run --features obs-backend --manifest-path crates/revo-gpui/Cargo.toml -- --obs"
fi

cd "$PROJECT_DIR"
exec cargo run --manifest-path crates/revo-gpui/Cargo.toml --
