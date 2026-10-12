#!/usr/bin/env bash
set -euo pipefail

: "${RUNNER_TEMP:?RUNNER_TEMP must point to the build workspace}"
: "${OBS_VERSION:?OBS_VERSION is required}"
: "${GITHUB_ENV:?GITHUB_ENV is required for subsequent build steps}"
OBS_SRC="$RUNNER_TEMP/obs-studio"
OBS_BUILD="$RUNNER_TEMP/obs-build"
OBS_INSTALL="$RUNNER_TEMP/obs-install"
git clone --depth 1 --branch "$OBS_VERSION" --recursive \
  https://github.com/obsproject/obs-studio.git "$OBS_SRC"
cmake -S "$OBS_SRC" -B "$OBS_BUILD" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="$OBS_INSTALL" \
  -DCMAKE_INSTALL_LIBDIR=lib \
  -DCMAKE_COMPILE_WARNING_AS_ERROR=OFF \
  -DENABLE_RELOCATABLE=ON \
  -DENABLE_FRONTEND=OFF -DENABLE_BROWSER=OFF -DENABLE_SCRIPTING=OFF \
  -DENABLE_AJA=OFF -DENABLE_DECKLINK=OFF -DENABLE_VLC=OFF \
  -DENABLE_VST=OFF -DENABLE_WEBRTC=OFF -DENABLE_WEBSOCKET=OFF \
  -DENABLE_NVENC=OFF -DENABLE_QSV11=OFF
cmake --build "$OBS_BUILD" --parallel 2
cmake --install "$OBS_BUILD"
test -f "$OBS_INSTALL/lib/libobs.so"
{
  echo "LIBOBS_PATH=$OBS_INSTALL/lib"
  echo "REVO_OBS_LIB_DIR=$OBS_INSTALL/lib"
  echo "LD_LIBRARY_PATH=$OBS_INSTALL/lib"
  echo "PKG_CONFIG_PATH=$OBS_INSTALL/lib/pkgconfig"
  # Tauri uses the same usr/bin -> usr/lib layout for AppDir and native packages.
  # shellcheck disable=SC2016 # $ORIGIN is expanded by the ELF loader.
  echo 'RUSTFLAGS=-C link-arg=-Wl,-rpath,$ORIGIN/../lib/revo-ui/resources/revo-root/core/lib'
} >> "$GITHUB_ENV"
