#!/usr/bin/env bash
set -euo pipefail

brew install ffmpeg mpg123
FFMPEG_PREFIX="$(brew --prefix ffmpeg)"
BREW_PREFIX="$(brew --prefix)"
echo "FFMPEG_PREFIX=$FFMPEG_PREFIX" >> "$GITHUB_ENV"
echo "BREW_PREFIX=$BREW_PREFIX" >> "$GITHUB_ENV"
test -d "$FFMPEG_PREFIX/lib"
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
done
find "$FFMPEG_PREFIX/lib" -maxdepth 1 \( -type f -o -type l \) -name "*.dylib" -print | sort
