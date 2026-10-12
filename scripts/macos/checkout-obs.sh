#!/usr/bin/env bash
set -euo pipefail

rm -rf "$RUNNER_TEMP/obs-studio"
git clone \
  --depth 1 \
  --branch "${OBS_VERSION}" \
  --recursive \
  https://github.com/obsproject/obs-studio.git \
  "$RUNNER_TEMP/obs-studio"
git -C "$RUNNER_TEMP/obs-studio" \
  log -1 --oneline
