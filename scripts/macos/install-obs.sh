#!/usr/bin/env bash
set -euo pipefail

OBS_INSTALL="$RUNNER_TEMP/obs-install"
rm -rf "$OBS_INSTALL"
mkdir -p "$OBS_INSTALL"
cmake \
  --install "$RUNNER_TEMP/obs-build" \
  --config RelWithDebInfo \
  --prefix "$OBS_INSTALL"
