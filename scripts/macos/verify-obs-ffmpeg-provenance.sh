#!/usr/bin/env bash
set -euo pipefail

OBS_BUILD="${REVO_OBS_BUILD:-$RUNNER_TEMP/obs-build-${REVO_TARGET:?REVO_TARGET is required}}"
REPORT_DIR="${1:?usage: verify-obs-ffmpeg-provenance.sh <report-directory>}"
CACHE="$OBS_BUILD/CMakeCache.txt"
BREW_PREFIX="$(brew --prefix)"
FFMPEG_PREFIX="$(brew --prefix ffmpeg)"
FFMPEG_PREFIX_REAL="$(cd "$FFMPEG_PREFIX" && pwd -P)"

test -f "$CACHE" || { echo "ERROR: OBS CMake cache not found: $CACHE"; exit 1; }
mkdir -p "$REPORT_DIR"

SELECTION="$REPORT_DIR/obs-ffmpeg-cmake-paths.txt"
FAILURES="$REPORT_DIR/obs-ffmpeg-provenance-failures.txt"
: > "$SELECTION"
: > "$FAILURES"

# Capture absolute paths from FFmpeg-related CMake cache entries. OBS' macOS
# preset can inject its own dependency archive, so merely having Homebrew on
# PATH does not prove which headers and libraries were selected.
awk -F= '
  tolower($1) ~ /(ffmpeg|avcodec|avformat|avutil|avfilter|avdevice|swresample|swscale)/ && $2 ~ /^\// {
    count = split($2, paths, ";")
    for (i = 1; i <= count; i++) {
      if (paths[i] ~ /^\//) print paths[i]
    }
  }
' "$CACHE" | sort -u > "$SELECTION"

if [ ! -s "$SELECTION" ]; then
  echo "ERROR: CMakeCache.txt does not expose absolute FFmpeg selection paths" | tee -a "$FAILURES"
else
  while IFS= read -r path; do
    case "$path" in
      "$BREW_PREFIX"/*) ;;
      *) echo "OBS selected FFmpeg outside Homebrew prefix: $path" | tee -a "$FAILURES" ;;
    esac
  done < "$SELECTION"
fi

for component in avcodec avformat avutil swresample swscale; do
  COMPONENT_PATHS="$REPORT_DIR/obs-ffmpeg-${component}-libraries.txt"
  awk -F= -v component="$component" '
    tolower($1) ~ component && toupper($1) ~ /(LIBRARY|LIBRARIES)/ && $2 ~ /^\// {
      count = split($2, paths, ";")
      for (i = 1; i <= count; i++) {
        if (paths[i] ~ /^\//) print paths[i]
      }
    }
  ' "$CACHE" | sort -u > "$COMPONENT_PATHS"

  if [ ! -s "$COMPONENT_PATHS" ]; then
    echo "No absolute CMake library selection found for $component" | tee -a "$FAILURES"
    continue
  fi

  while IFS= read -r path; do
    if [ ! -e "$path" ]; then
      echo "Selected $component library does not exist: $path" | tee -a "$FAILURES"
      continue
    fi
    real="$(realpath "$path")"
    case "$real" in
      "$FFMPEG_PREFIX_REAL"/*) ;;
      *) echo "Selected $component library is not from $FFMPEG_PREFIX_REAL: $real" | tee -a "$FAILURES" ;;
    esac
  done < "$COMPONENT_PATHS"
done

echo "Homebrew prefix: $BREW_PREFIX"
echo "FFmpeg formula prefix: $FFMPEG_PREFIX_REAL"
echo "OBS FFmpeg CMake selections:"
cat "$SELECTION"
if [ -s "$FAILURES" ]; then
  echo "ERROR: OBS and staged Homebrew FFmpeg do not have one proven provenance"
  cat "$FAILURES"
  exit 1
fi
echo "OBS CMake FFmpeg provenance matches the staged Homebrew dependency set"
