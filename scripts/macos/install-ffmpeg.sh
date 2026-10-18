#!/usr/bin/env bash
set -euo pipefail

: "${REVO_CMAKE_ARCH:?REVO_CMAKE_ARCH is required}"
case "$REVO_CMAKE_ARCH" in
  arm64) EXPECTED_HOST="arm64" ;;
  x86_64) EXPECTED_HOST="x86_64" ;;
  *) echo "ERROR: unsupported REVO_CMAKE_ARCH=$REVO_CMAKE_ARCH"; exit 1 ;;
esac

HOST_ARCH="$(uname -m)"
if [ "$HOST_ARCH" != "$EXPECTED_HOST" ]; then
  echo "ERROR: Homebrew dependencies would use host architecture $HOST_ARCH, but target is $REVO_CMAKE_ARCH"
  echo "Use an Intel runner for x86_64 and an Apple Silicon runner for arm64."
  exit 1
fi

brew install ffmpeg mpg123
FFMPEG_PREFIX="$(brew --prefix ffmpeg)"
BREW_PREFIX="$(brew --prefix)"
echo "FFMPEG_PREFIX=$FFMPEG_PREFIX" >> "$GITHUB_ENV"
echo "BREW_PREFIX=$BREW_PREFIX" >> "$GITHUB_ENV"
test -d "$FFMPEG_PREFIX/lib"

verify_arch() {
  local file="$1"
  local archs
  archs="$(lipo -archs "$(realpath "$file")" 2>/dev/null || true)"
  case " $archs " in
    *" $REVO_CMAKE_ARCH "*) ;;
    *)
      echo "ERROR: wrong architecture in $file"
      echo "Expected: $REVO_CMAKE_ARCH"
      echo "Found: ${archs:-not a Mach-O dylib}"
      file "$file" || true
      exit 1
      ;;
  esac
}

for lib in \
  libavcodec.dylib \
  libavformat.dylib \
  libavutil.dylib \
  libswresample.dylib \
  libswscale.dylib
do
  test -f "$FFMPEG_PREFIX/lib/$lib" || {
    echo "Missing FFmpeg library: $FFMPEG_PREFIX/lib/$lib"
    exit 1
  }
  verify_arch "$FFMPEG_PREFIX/lib/$lib"
done

while IFS= read -r dylib; do
  verify_arch "$dylib"
done < <(find "$FFMPEG_PREFIX/lib" -maxdepth 1 \( -type f -o -type l \) -name "*.dylib" -print | sort)
find "$FFMPEG_PREFIX/lib" -maxdepth 1 \( -type f -o -type l \) -name "*.dylib" -print | sort
