#!/usr/bin/env bash
set -euo pipefail

# Normalize project architecture labels to canonical names reported by lipo.
case "${REVO_CMAKE_ARCH}" in
  x64|x86-64) REVO_CMAKE_ARCH=x86_64 ;;
  arm64|aarch64) REVO_CMAKE_ARCH=arm64 ;;
esac
export REVO_CMAKE_ARCH

OBS_SRC="${REVO_OBS_SRC:-$RUNNER_TEMP/obs-studio-${REVO_TARGET:?REVO_TARGET is required}}"
OBS_BUILD="${REVO_OBS_BUILD:-$RUNNER_TEMP/obs-build-${REVO_TARGET:?REVO_TARGET is required}}"
rm -rf "$OBS_BUILD"
cmake \
  -S "$OBS_SRC" \
  -B "$OBS_BUILD" \
  --preset macos \
  -DCMAKE_OSX_ARCHITECTURES="${REVO_CMAKE_ARCH}" \
  -DCMAKE_BUILD_TYPE=RelWithDebInfo \
  -DCMAKE_COMPILE_WARNING_AS_ERROR=OFF \
  -DCMAKE_OSX_DEPLOYMENT_TARGET=12.0 \
  -DENABLE_BROWSER=OFF \
  -DENABLE_SCRIPTING=OFF \
  -DENABLE_FRONTEND=OFF
