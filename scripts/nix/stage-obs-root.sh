#!/usr/bin/env bash
# Stages the shared OBS runtime layout (.local/revo-root) from the Nix store
# and prints the resolved obs-studio store path (empty when Nix could not
# resolve it). Shared by the dev and gpui launchers so every native path
# boots the real libobs engine against the same REVO_ROOT layout.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

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
	# OBS share data (frontend, locale, etc.) dla core_data_dir
	if [ ! -L "$REVO_ROOT/core/share/obs" ]; then
		mkdir -p "$REVO_ROOT/core/share"
		ln -sfT "$OBS_STORE/share/obs" "$REVO_ROOT/core/share/obs"
	fi
fi

printf '%s' "$OBS_STORE"
