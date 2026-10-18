#!/usr/bin/env bash
set -euo pipefail

OBS_BUILD="${REVO_OBS_BUILD:-$RUNNER_TEMP/obs-build-${REVO_TARGET:?REVO_TARGET is required}}"
OBS_INSTALL="${REVO_OBS_INSTALL:-$RUNNER_TEMP/obs-install-${REVO_TARGET:?REVO_TARGET is required}}"
rm -rf "$OBS_INSTALL"
mkdir -p "$OBS_INSTALL"
cmake \
  --install "$OBS_BUILD" \
  --config RelWithDebInfo \
  --prefix "$OBS_INSTALL"
