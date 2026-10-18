#!/usr/bin/env bash
set -euo pipefail

OBS_SRC="${REVO_OBS_SRC:-$RUNNER_TEMP/obs-studio-${REVO_TARGET:?REVO_TARGET is required}}"
rm -rf "$OBS_SRC"
git clone \
  --depth 1 \
  --branch "${OBS_VERSION}" \
  --recursive \
  https://github.com/obsproject/obs-studio.git \
  "$OBS_SRC"
git -C "$OBS_SRC" \
  log -1 --oneline
