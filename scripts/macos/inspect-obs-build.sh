#!/usr/bin/env bash
set -euo pipefail

OBS_BUILD="$RUNNER_TEMP/obs-build"
OBS_INSTALL="$RUNNER_TEMP/obs-install"
echo "============================================================"
echo "OBS BUILD: dylibs"
echo "============================================================"
find "$OBS_BUILD" \
  -type f \
  -name "*.dylib" \
  -print |
sort
echo
echo "============================================================"
echo "OBS BUILD: plugins"
echo "============================================================"
find "$OBS_BUILD" \
  -type d \
  -name "*.plugin" \
  -print |
sort
echo
echo "============================================================"
echo "OBS BUILD: frameworks"
echo "============================================================"
find "$OBS_BUILD" \
  -type d \
  -name "*.framework" \
  -print |
sort
echo
echo "============================================================"
echo "OBS INSTALL: files"
echo "============================================================"
find "$OBS_INSTALL" \
  -type f \
  -print |
sort |
head -500
