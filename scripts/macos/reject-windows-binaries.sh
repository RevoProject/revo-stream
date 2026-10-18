#!/usr/bin/env bash
set -euo pipefail

OBS_BUILD="${REVO_OBS_BUILD:-$RUNNER_TEMP/obs-build-${REVO_TARGET:?REVO_TARGET is required}}"
OBS_INSTALL="${REVO_OBS_INSTALL:-$RUNNER_TEMP/obs-install-${REVO_TARGET:?REVO_TARGET is required}}"
WINDOWS_BINARIES=$(
  {
    find "$OBS_BUILD" \
      -type f \
      \( \
        -name "*.dll" \
        -o -name "*.exe" \
        -o -name "*.pdb" \
      \)
    find "$OBS_INSTALL" \
      -type f \
      \( \
        -name "*.dll" \
        -o -name "*.exe" \
        -o -name "*.pdb" \
      \)
  } |
  sort -u
)
if [ -n "$WINDOWS_BINARIES" ]; then
  echo "ERROR: Windows binaries found in macOS OBS build:"
  echo "$WINDOWS_BINARIES"
  exit 1
fi
echo "No Windows binaries found"
