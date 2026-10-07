#!/usr/bin/env bash
set -euo pipefail

OBS_INSTALL="${1:?Usage: prepare-linux-runtime.sh OBS_INSTALL [RESOURCE_ROOT]}"
ROOT="${2:-src-tauri/resources/revo-root}"

test -f "$OBS_INSTALL/lib/libobs.so"
test -f "$OBS_INSTALL/share/obs/libobs/default.effect"
test -d "$OBS_INSTALL/lib/obs-plugins"
mkdir -p "$ROOT/core/lib" "$ROOT/core/bin" "$ROOT/core/share" "$ROOT/data"
cp -a "$OBS_INSTALL/lib/." "$ROOT/core/lib/"
cp -a "$OBS_INSTALL/share/." "$ROOT/core/share/"
if [[ -d "$OBS_INSTALL/bin" ]]; then
  cp -a "$OBS_INSTALL/bin/." "$ROOT/core/bin/"
fi
if [[ -d data ]]; then
  cp -a data/. "$ROOT/data/"
fi
mkdir -p "$ROOT/data/share/obs/libobs"
cp -a "$OBS_INSTALL/share/obs/libobs/." "$ROOT/data/share/obs/libobs/"

mux="$(find "$OBS_INSTALL" -type f -name obs-ffmpeg-mux -print -quit)"
test -n "$mux"
cp -a "$mux" "$ROOT/core/bin/obs-ffmpeg-mux"

# Keep glibc, desktop/media frameworks and GPU dispatch on the host. The
# package manifests declare them; private OBS/FFmpeg dependencies travel together.
is_host_library() {
  case "$1" in
    ld-linux*.so*|libc.so*|libm.so*|libpthread.so*|libdl.so*|librt.so*|libresolv.so*|libutil.so*|libnss_*.so*) return 0 ;;
    libglib-2.0.so*|libgobject-2.0.so*|libgio-2.0.so*|libgmodule-2.0.so*) return 0 ;;
    libgtk-3.so*|libgdk-3.so*|libgdk_pixbuf-2.0.so*|libpango*.so*|libcairo*.so*|libatk*.so*|libatspi.so*) return 0 ;;
    libwebkit2gtk*.so*|libjavascriptcoregtk*.so*|libsoup*.so*|libgstreamer*.so*|libgst*.so*) return 0 ;;
    libGL.so*|libEGL.so*|libGLX.so*|libOpenGL.so*|libGLdispatch.so*) return 0 ;;
    *) return 1 ;;
  esac
}

export LD_LIBRARY_PATH="$OBS_INSTALL/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
while IFS= read -r -d '' binary; do
  if ! file -b "$binary" | grep -q '^ELF '; then
    continue
  fi
  dependencies="$(ldd "$binary")"
  if [[ "$dependencies" == *"not found"* ]]; then
    printf 'Missing runtime dependency for %s:\n%s\n' "$binary" "$dependencies" >&2
    exit 1
  fi
  while IFS= read -r dependency; do
    name="$(basename "$dependency")"
    if is_host_library "$name"; then
      continue
    fi
    if [[ ! -e "$ROOT/core/lib/$name" ]]; then
      cp -aL "$dependency" "$ROOT/core/lib/$name"
    fi
  done < <(printf '%s\n' "$dependencies" | awk '/=> \/[^ ]+/ { print $3 }')
done < <(find "$ROOT/core" -type f -print0)

# DT_RUNPATH is not transitive: each private library needs its own relative path.
while IFS= read -r -d '' binary; do
  if ! patchelf --print-rpath "$binary" >/dev/null 2>&1; then
    continue
  fi
  # shellcheck disable=SC2016 # $ORIGIN is expanded by the ELF loader, not the shell.
  case "$binary" in
    "$ROOT/core/lib/obs-plugins/"*) rpath='$ORIGIN:$ORIGIN/..' ;;
    "$ROOT/core/bin/"*) rpath='$ORIGIN/../lib' ;;
    *) rpath='$ORIGIN' ;;
  esac
  patchelf --set-rpath "$rpath" "$binary"
done < <(find "$ROOT/core" -type f -print0)

LD_LIBRARY_PATH="$(realpath "$ROOT/core/lib")"
export LD_LIBRARY_PATH
while IFS= read -r -d '' binary; do
  if ! file -b "$binary" | grep -q '^ELF '; then
    continue
  fi
  dependencies="$(ldd "$binary")"
  if [[ "$dependencies" == *"not found"* ]]; then
    printf 'Incomplete staged runtime for %s:\n%s\n' "$binary" "$dependencies" >&2
    exit 1
  fi
done < <(find "$ROOT/core" -type f -print0)

test -f "$ROOT/core/lib/obs-plugins/obs-ffmpeg.so"
test -f "$ROOT/core/lib/obs-plugins/obs-x264.so"
test -f "$ROOT/core/lib/obs-plugins/rtmp-services.so"
test -x "$ROOT/core/bin/obs-ffmpeg-mux"
