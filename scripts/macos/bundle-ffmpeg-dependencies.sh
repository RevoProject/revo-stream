#!/usr/bin/env bash
set -euo pipefail

ROOT="src-tauri/resources/revo-root"
LIB="$ROOT/core/lib"
FFMPEG_PREFIX="$(brew --prefix ffmpeg)"
BREW_PREFIX="$(brew --prefix)"
SEEN_FILE="$RUNNER_TEMP/revo-seen-dylibs.txt"
QUEUE_FILE="$RUNNER_TEMP/revo-dylib-queue.txt"
: > "$SEEN_FILE"
: > "$QUEUE_FILE"
mkdir -p "$LIB"

# Preserve the canonical Homebrew FFmpeg names used by the runtime.
# Homebrew may expose these as symlinks to versioned dylibs.
for lib in             libavcodec.dylib             libavformat.dylib             libavutil.dylib             libswresample.dylib             libswscale.dylib
do
  src="$FFMPEG_PREFIX/lib/$lib"
  if [ ! -e "$src" ]; then
    echo "Missing FFmpeg library: $src"
    exit 1
  fi
  cp -L "$src" "$LIB/$lib"
done

enqueue() {
  local f="$1"
  [ -e "$f" ] || return 0
  local real
  real="$(realpath "$f")"
  [ -f "$real" ] || return 0
  if ! grep -Fqx "$real" "$SEEN_FILE" 2>/dev/null; then
    printf '%s\n' "$real" >> "$SEEN_FILE"
    printf '%s\n' "$f" >> "$QUEUE_FILE"
  fi
}

while IFS= read -r f; do
  [ -n "$f" ] || continue
  enqueue "$f"
done < <(
  find "$FFMPEG_PREFIX/lib" -maxdepth 1 \( -type f -o -type l \) -name "*.dylib" -print | sort
)

while IFS= read -r binary; do
  [ -n "$binary" ] || continue
  while IFS= read -r dep; do
    dep="${dep#        }"
    dep="${dep%% (*}"
    case "$dep" in
      "$BREW_PREFIX"/*) enqueue "$dep" ;;
    esac
  done < <(otool -L "$binary" 2>/dev/null | tail -n +2)
done < <(find "$ROOT" -type f \
  \( -perm -111 -o -name "*.dylib" \) -print)

queue_index=1
while [ "$queue_index" -le "$(wc -l < "$QUEUE_FILE" | tr -d ' ')" ]; do
  src="$(sed -n "${queue_index}p" "$QUEUE_FILE")"
  queue_index=$((queue_index + 1))
  [ -n "$src" ] || continue

  dst="$LIB/$(basename "$src")"
  cp -L "$src" "$dst"

  while IFS= read -r dep; do
    dep="${dep#        }"
    dep="${dep%% (*}"
    case "$dep" in
      "$BREW_PREFIX"/*) enqueue "$dep" ;;
    esac
  done < <(otool -L "$src" 2>/dev/null | tail -n +2)
done

for lib in             libavcodec.dylib             libavformat.dylib             libavutil.dylib             libswresample.dylib             libswscale.dylib
do
  test -f "$LIB/$lib" || {
    echo "ERROR: canonical FFmpeg dylib was not staged: $LIB/$lib"
    exit 1
  }
done

find "$LIB" -maxdepth 1 \( -type f -o -type l \) -name "*.dylib" -print | sort
