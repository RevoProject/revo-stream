#!/usr/bin/env bash
set -euo pipefail

: "${REVO_CMAKE_ARCH:?REVO_CMAKE_ARCH is required}"
OBS_SRC="${REVO_OBS_SRC:-$RUNNER_TEMP/obs-studio-${REVO_TARGET:?REVO_TARGET is required}}"
OBS_BUILD="${REVO_OBS_BUILD:-$RUNNER_TEMP/obs-build-${REVO_TARGET:?REVO_TARGET is required}}"
REPORT_DIR="${1:?usage: verify-obs-ffmpeg-provenance.sh <report-directory>}"
CACHE="$OBS_BUILD/CMakeCache.txt"
EXPECTED_DEPS_PARENT="$(cd "$OBS_SRC/.deps" && pwd -P)"
EXPECTED_ARCHIVE_SHA256="9403bb43fb0a9bb215739a5659ca274fe884dbbbcd22bd9ca781c961fb041c42"
EXPECTED_QT_ARCHIVE_SHA256="990f11638b80a4509e14e8c315f6e4caa0861e37fcd3113a256fbff835ffca29"

test -f "$CACHE" || { echo "ERROR: OBS CMake cache not found: $CACHE"; exit 1; }
mkdir -p "$REPORT_DIR"

MANIFEST="$REPORT_DIR/obs-ffmpeg-provenance.tsv"
FAILURES="$REPORT_DIR/obs-ffmpeg-provenance-failures.txt"
PREFIX_FILE="$OBS_BUILD/revo-obs-deps-prefix.txt"
printf 'component\tcache_path\treal_path\tinstall_name\tarchitectures\tsha256\n' > "$MANIFEST"
: > "$FAILURES"

extract_component_library() {
  local component="$1"
  awk -F= -v key="FFmpeg_${component}_LIBRARY" '
    $1 == key ":FILEPATH" || $1 == key ":STRING" ||
    $1 ~ ("^" key "_(RELEASE|RELWITHDEBINFO|DEBUG|MINSIZEREL):") {
      if ($2 ~ /^\//) { print $2; exit }
    }
  ' "$CACHE"
}

DEPS_PREFIX=""
for component in avcodec avformat avutil swresample swscale; do
  selected="$(extract_component_library "$component")"
  if [ -z "$selected" ]; then
    echo "Missing CMake selection for FFmpeg $component" | tee -a "$FAILURES"
    continue
  fi
  if [ ! -e "$selected" ]; then
    echo "Selected FFmpeg $component library does not exist: $selected" | tee -a "$FAILURES"
    continue
  fi

  real="$(realpath "$selected")"
  prefix="$(cd "$(dirname "$real")/.." && pwd -P)"
  case "$prefix" in
    "$EXPECTED_DEPS_PARENT"/obs-deps-*-universal) ;;
    *) echo "FFmpeg $component is outside the pinned OBS dependency set: $real" | tee -a "$FAILURES" ;;
  esac
  if [ -z "$DEPS_PREFIX" ]; then
    DEPS_PREFIX="$prefix"
  elif [ "$prefix" != "$DEPS_PREFIX" ]; then
    echo "FFmpeg components use different dependency prefixes: $DEPS_PREFIX and $prefix" | tee -a "$FAILURES"
  fi

  archs="$(lipo -archs "$real" 2>/dev/null || true)"
  case " $archs " in
    *" $REVO_CMAKE_ARCH "*) ;;
    *) echo "FFmpeg $component lacks $REVO_CMAKE_ARCH: $real ($archs)" | tee -a "$FAILURES" ;;
  esac
  install_name="$(otool -D "$real" 2>/dev/null | tail -n +2 | head -n 1 | sed 's/^[[:space:]]*//' || true)"
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$component" "$selected" "$real" "$install_name" "$archs" \
    "$(shasum -a 256 "$real" | cut -d ' ' -f 1)" >> "$MANIFEST"
done

if [ -n "$DEPS_PREFIX" ]; then
  prefix_path="$(awk -F= '$1 ~ /^CMAKE_PREFIX_PATH:/ { print $2; exit }' "$CACHE")"
  case ";$prefix_path;" in
    *";$DEPS_PREFIX;"*) ;;
    *) echo "CMAKE_PREFIX_PATH does not contain the selected OBS dependency prefix: $DEPS_PREFIX" | tee -a "$FAILURES" ;;
  esac

  while IFS= read -r selected; do
    [ -n "$selected" ] || continue
    real="$(realpath "$selected")"
    case "$real" in
      "$DEPS_PREFIX"/*) ;;
      *) echo "Configured FFmpeg library escapes $DEPS_PREFIX: $real" | tee -a "$FAILURES" ;;
    esac
  done < <(
    awk -F= '$1 ~ /^FFmpeg_(avcodec|avformat|avutil|avfilter|avdevice|postproc|swresample|swscale)_LIBRARY(:|_(RELEASE|RELWITHDEBINFO|DEBUG|MINSIZEREL):)/ && $2 ~ /^\// { print $2 }' "$CACHE" |
      sort -u
  )
fi

if ! grep -Fq "\"macos-universal\": \"$EXPECTED_ARCHIVE_SHA256\"" "$OBS_SRC/CMakePresets.json"; then
  echo "OBS CMake preset does not contain the expected obs-deps archive hash" | tee -a "$FAILURES"
fi
if ! grep -Fq "\"macos-universal\": \"$EXPECTED_QT_ARCHIVE_SHA256\"" "$OBS_SRC/CMakePresets.json"; then
  echo "OBS CMake preset does not contain the expected Qt dependency archive hash" | tee -a "$FAILURES"
fi

version="$(awk -F= '$1 ~ /^FFmpeg_VERSION:/ { print $2; exit }' "$CACHE")"
{
  echo "OBS source: $OBS_SRC"
  echo "OBS dependency prefix: $DEPS_PREFIX"
  echo "OBS dependency archive SHA-256: $EXPECTED_ARCHIVE_SHA256"
  echo "OBS Qt dependency archive SHA-256: $EXPECTED_QT_ARCHIVE_SHA256"
  echo "CMake FFmpeg version: ${version:-unknown}"
} | tee "$REPORT_DIR/obs-ffmpeg-selection.txt"

if [ -s "$FAILURES" ]; then
  echo "ERROR: OBS FFmpeg provenance validation failed"
  cat "$FAILURES"
  exit 1
fi
printf '%s\n' "$DEPS_PREFIX" > "$PREFIX_FILE"
echo "OBS CMake FFmpeg provenance is coherent and pinned"
