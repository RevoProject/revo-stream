#!/usr/bin/env bash
set -euo pipefail

: "${REVO_CMAKE_ARCH:?REVO_CMAKE_ARCH is required}"

# Normalize project architecture labels to canonical names reported by lipo.
case "${REVO_CMAKE_ARCH}" in
  x64|x86-64) REVO_CMAKE_ARCH=x86_64 ;;
  arm64|aarch64) REVO_CMAKE_ARCH=arm64 ;;
esac
export REVO_CMAKE_ARCH
: "${REVO_TARGET:?REVO_TARGET is required}"
ROOT="src-tauri/resources/revo-root"
LIB="$ROOT/core/lib"
ROOT_REAL="$(cd "$ROOT" && pwd -P)"
OBS_BUILD="${REVO_OBS_BUILD:-$RUNNER_TEMP/obs-build-$REVO_TARGET}"
CACHE="$OBS_BUILD/CMakeCache.txt"
PREFIX_FILE="$OBS_BUILD/revo-obs-deps-prefix.txt"
QUEUE_FILE="$RUNNER_TEMP/revo-dylib-queue-${REVO_TARGET}.tsv"
SEEN_FILE="$RUNNER_TEMP/revo-seen-dylibs-${REVO_TARGET}.tsv"
REPORT_DIR="${REVO_DIAGNOSTICS:-$RUNNER_TEMP}"
PROVENANCE="$REPORT_DIR/staged-obs-deps-provenance.tsv"

test -f "$CACHE" || { echo "ERROR: OBS CMake cache not found: $CACHE"; exit 1; }
test -f "$PREFIX_FILE" || { echo "ERROR: validated OBS dependency prefix is missing: $PREFIX_FILE"; exit 1; }
DEPS_PREFIX="$(cat "$PREFIX_FILE")"
DEPS_PARENT="$(cd "$DEPS_PREFIX/.." && pwd -P)"
test -d "$DEPS_PREFIX/lib" || { echo "ERROR: OBS dependency lib directory is missing: $DEPS_PREFIX/lib"; exit 1; }
mkdir -p "$LIB" "$REPORT_DIR"
: > "$QUEUE_FILE"
: > "$SEEN_FILE"
printf 'load_name\tsource\tsource_realpath\tdestination\tsource_architectures\tdestination_architecture\tsource_slice_sha256\tdestination_sha256\n' > "$PROVENANCE"

thin_to() {
  local source="$1"
  local destination="$2"
  local archs
  mkdir -p "$(dirname "$destination")"
  archs="$(lipo -archs "$source" 2>/dev/null || true)"
  case " $archs " in
    *" $REVO_CMAKE_ARCH "*) ;;
    *) echo "ERROR: $source does not contain $REVO_CMAKE_ARCH (found: ${archs:-not Mach-O})"; exit 1 ;;
  esac
  if [ "$archs" = "$REVO_CMAKE_ARCH" ]; then
    cp "$source" "$destination"
  else
    lipo "$source" -thin "$REVO_CMAKE_ARCH" -output "$destination"
  fi
  test "$(lipo -archs "$destination")" = "$REVO_CMAKE_ARCH"
}

is_valid_dependency_source() {
  case "$1" in
    "$DEPS_PARENT"/obs-deps-*-universal/*) return 0 ;;
    *) return 1 ;;
  esac
}

slice_hash() {
  local source="$1"
  local temp="$RUNNER_TEMP/revo-slice-${REVO_TARGET}-$$"
  thin_to "$source" "$temp"
  shasum -a 256 "$temp" | cut -d ' ' -f 1
  rm -f "$temp"
}

enqueue() {
  local source="$1"
  local destination_name="$2"
  local real existing
  [ -e "$source" ] || { echo "ERROR: dependency source is missing: $source"; exit 1; }
  real="$(realpath "$source")"
  is_valid_dependency_source "$real" || {
    echo "ERROR: refusing dependency outside validated OBS archives: $real"
    exit 1
  }
  existing="$(awk -F '\t' -v name="$destination_name" '$1 == name { print $2; exit }' "$SEEN_FILE")"
  if [ -n "$existing" ]; then
    if [ "$existing" != "$real" ] && [ "$(slice_hash "$existing")" != "$(slice_hash "$real")" ]; then
      echo "ERROR: conflicting providers for $destination_name"
      echo "  $existing"
      echo "  $real"
      exit 1
    fi
    return 0
  fi
  printf '%s\t%s\n' "$destination_name" "$real" >> "$SEEN_FILE"
  printf '%s\t%s\n' "$source" "$destination_name" >> "$QUEUE_FILE"
}

resolve_dependency() {
  local consumer="$1"
  local dependency="$2"
  local relative candidate candidate_real dependency_prefix raw_rpath
  case "$dependency" in
    /System/Library/*|/usr/lib/*) return 1 ;;
    "$DEPS_PARENT"/obs-deps-*-universal/*)
      printf '%s\n' "$dependency"
      return 0
      ;;
    @loader_path/*)
      candidate="$(dirname "$consumer")/${dependency#@loader_path/}"
      candidate_real="$(realpath "$candidate" 2>/dev/null || true)"
      is_valid_dependency_source "$candidate_real" && { printf '%s\n' "$candidate"; return 0; }
      ;;
    @rpath/*)
      relative="${dependency#@rpath/}"
      while IFS= read -r raw_rpath; do
        raw_rpath="$(printf '%s\n' "$raw_rpath" | sed -E 's/^[[:space:]]*path ([^ ]+).*/\1/')"
        case "$raw_rpath" in
          @loader_path*) candidate="$(dirname "$consumer")${raw_rpath#@loader_path}/$relative" ;;
          /*) candidate="$raw_rpath/$relative" ;;
          *) continue ;;
        esac
        candidate_real="$(realpath "$candidate" 2>/dev/null || true)"
        is_valid_dependency_source "$candidate_real" && { printf '%s\n' "$candidate"; return 0; }
      done < <(otool -l "$consumer" 2>/dev/null | awk '/cmd LC_RPATH/{show=1; next} show && /path /{print; show=0}')
      for dependency_prefix in "$DEPS_PARENT"/obs-deps-*-universal; do
        [ -d "$dependency_prefix" ] || continue
        candidate="$dependency_prefix/lib/$relative"
        [ -e "$candidate" ] && { printf '%s\n' "$candidate"; return 0; }
      done
      ;;
  esac
  return 1
}

staged_dependency_exists() {
  local consumer="$1"
  local dependency="$2"
  local relative candidate raw_rpath
  case "$dependency" in
    @loader_path/*)
      candidate="$(dirname "$consumer")/${dependency#@loader_path/}"
      [ -e "$candidate" ]
      return
      ;;
    @rpath/*)
      relative="${dependency#@rpath/}"
      [ -e "$LIB/$relative" ] && return 0
      while IFS= read -r raw_rpath; do
        raw_rpath="$(printf '%s\n' "$raw_rpath" | sed -E 's/^[[:space:]]*path ([^ ]+).*/\1/')"
        case "$raw_rpath" in
          @loader_path*) candidate="$(dirname "$consumer")${raw_rpath#@loader_path}/$relative" ;;
          /*) candidate="$raw_rpath/$relative" ;;
          *) continue ;;
        esac
        case "$(realpath "$candidate" 2>/dev/null || true)" in
          "$ROOT_REAL"/*) return 0 ;;
        esac
      done < <(otool -l "$consumer" 2>/dev/null | awk '/cmd LC_RPATH/{show=1; next} show && /path /{print; show=0}')
      ;;
  esac
  return 1
}

enqueue_consumer_dependencies() {
  local consumer="$1"
  local dependency source id destination_name
  id="$(otool -D "$consumer" 2>/dev/null | tail -n +2 | head -n 1 | sed 's/^[[:space:]]*//' || true)"
  while IFS= read -r dependency; do
    dependency="$(printf '%s\n' "$dependency" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+\(.+$//')"
    [ -n "$dependency" ] || continue
    [ "$dependency" = "$id" ] && continue
    source="$(resolve_dependency "$consumer" "$dependency" || true)"
    if [ -n "$source" ]; then
      destination_name="$(basename "$dependency")"
      case "$dependency" in
        @rpath/*) destination_name="${dependency#@rpath/}" ;;
        *.framework/*) destination_name="$(printf '%s\n' "$dependency" | sed -E 's#^.*/([^/]+\.framework/.*)$#\1#')" ;;
      esac
      enqueue "$source" "$destination_name"
      continue
    fi
    case "$dependency" in
      /System/Library/*|/usr/lib/*) ;;
      @rpath/*|@loader_path/*)
        staged_dependency_exists "$consumer" "$dependency" || {
          echo "ERROR: unresolved dependency for $consumer: $dependency"
          exit 1
        }
        ;;
      /*) echo "ERROR: dependency escapes validated OBS set: $consumer -> $dependency"; exit 1 ;;
    esac
  done < <(otool -L "$consumer" 2>/dev/null | tail -n +2)
}

while IFS='=' read -r key source; do
  [ -n "$source" ] || continue
  component="${key#FFmpeg_}"
  component="${component%%_LIBRARY*}"
  enqueue "$source" "lib${component}.dylib"
done < <(
  awk -F= '$1 ~ /^FFmpeg_(avcodec|avformat|avutil|avfilter|avdevice|postproc|swresample|swscale)_LIBRARY(:|_(RELEASE|RELWITHDEBINFO|DEBUG|MINSIZEREL):)/ && $2 ~ /^\// { print $1 "=" $2 }' "$CACHE" |
    sort -u
)

while IFS= read -r -d '' consumer; do
  file "$consumer" 2>/dev/null | grep -q "Mach-O" || continue
  enqueue_consumer_dependencies "$consumer"
done < <(find "$ROOT" -type f -print0)

queue_index=1
while [ "$queue_index" -le "$(wc -l < "$QUEUE_FILE" | tr -d ' ')" ]; do
  entry="$(sed -n "${queue_index}p" "$QUEUE_FILE")"
  queue_index=$((queue_index + 1))
  source="${entry%%	*}"
  destination_name="${entry#*	}"
  real="$(realpath "$source")"
  destination="$LIB/$destination_name"
  source_archs="$(lipo -archs "$real")"
  source_slice_sha="$(slice_hash "$real")"

  if [ -f "$destination" ] && [ "$(slice_hash "$destination")" != "$source_slice_sha" ]; then
    echo "ERROR: staged $destination_name conflicts with the CMake-selected OBS dependency"
    exit 1
  fi
  thin_to "$real" "$destination"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$destination_name" "$source" "$real" "$destination" "$source_archs" "$REVO_CMAKE_ARCH" \
    "$source_slice_sha" "$(shasum -a 256 "$destination" | cut -d ' ' -f 1)" >> "$PROVENANCE"

  install_name="$(otool -D "$real" 2>/dev/null | tail -n +2 | head -n 1 | sed 's/^[[:space:]]*//' || true)"
  case "$install_name" in
    @rpath/*)
      alias_name="${install_name#@rpath/}"
      [ "$alias_name" = "$destination_name" ] || enqueue "$real" "$alias_name"
      ;;
    @loader_path/*|"$DEPS_PARENT"/obs-deps-*-universal/*)
      alias_name="$(basename "$install_name")"
      [ "$alias_name" = "$destination_name" ] || enqueue "$real" "$alias_name"
      ;;
  esac
  enqueue_consumer_dependencies "$real"
done

for lib in libavcodec.dylib libavformat.dylib libavutil.dylib libswresample.dylib libswscale.dylib; do
  test -f "$LIB/$lib" || { echo "ERROR: canonical FFmpeg dylib was not staged: $LIB/$lib"; exit 1; }
done

echo "Staged CMake-selected OBS dependency closure from $DEPS_PARENT"
find "$LIB" -maxdepth 1 -type f -name "*.dylib" -print | sort
