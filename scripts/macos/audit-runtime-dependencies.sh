#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:?usage: audit-runtime-dependencies.sh <root> <report-directory> <label>}"
REPORT_DIR="${2:?usage: audit-runtime-dependencies.sh <root> <report-directory> <label>}"
LABEL="${3:?usage: audit-runtime-dependencies.sh <root> <report-directory> <label>}"
EXPECTED_ARCH="${REVO_CMAKE_ARCH:?REVO_CMAKE_ARCH is required}"

# Normalize project architecture labels to canonical names reported by lipo.
case "${REVO_CMAKE_ARCH}" in
  x64|x86-64) REVO_CMAKE_ARCH=x86_64 ;;
  arm64|aarch64) REVO_CMAKE_ARCH=arm64 ;;
esac
export REVO_CMAKE_ARCH

test -d "$ROOT" || { echo "ERROR: audit root does not exist: $ROOT"; exit 1; }
mkdir -p "$REPORT_DIR"

MANIFEST="$REPORT_DIR/${LABEL}-macho.tsv"
DETAILS="$REPORT_DIR/${LABEL}-macho-details.txt"
COLLISIONS="$REPORT_DIR/${LABEL}-dylib-collisions.txt"
SYMBOLS="$REPORT_DIR/${LABEL}-ffmpeg-symbols.txt"
FAILURES="$REPORT_DIR/${LABEL}-failures.txt"
MIXED_ARCHS="$REPORT_DIR/${LABEL}-multi-architecture.txt"
ARCH_MISMATCHES="$REPORT_DIR/${LABEL}-architecture-mismatches.txt"

printf 'path\tsha256\tarchitectures\tinstall_name\n' > "$MANIFEST"
: > "$DETAILS"
: > "$COLLISIONS"
: > "$SYMBOLS"
: > "$FAILURES"
: > "$MIXED_ARCHS"
: > "$ARCH_MISMATCHES"

MACHO_LIST="$REPORT_DIR/${LABEL}-macho-files.txt"
: > "$MACHO_LIST"
while IFS= read -r -d '' candidate; do
  if file "$candidate" 2>/dev/null | grep -q "Mach-O"; then
    printf '%s\n' "$candidate" >> "$MACHO_LIST"
  fi
done < <(find "$ROOT" -type f -print0)

if [ ! -s "$MACHO_LIST" ]; then
  echo "ERROR: no Mach-O files found under $ROOT" | tee -a "$FAILURES"
  exit 1
fi

while IFS= read -r binary; do
  archs="$(lipo -archs "$binary" 2>/dev/null || true)"
  sha="$(shasum -a 256 "$binary" | cut -d ' ' -f 1)"
  install_name="$(otool -D "$binary" 2>/dev/null | tail -n +2 | head -n 1 | sed 's/^[[:space:]]*//' || true)"
  printf '%s\t%s\t%s\t%s\n' "$binary" "$sha" "$archs" "$install_name" >> "$MANIFEST"

  case " $archs " in
    *" $EXPECTED_ARCH "*) ;;
    *)
      mismatch="architecture mismatch: $binary (expected $EXPECTED_ARCH, found ${archs:-unknown})"
      echo "$mismatch" | tee -a "$ARCH_MISMATCHES"
      case "$LABEL" in
        obs-build) ;;
        *) echo "$mismatch" >> "$FAILURES" ;;
      esac
      ;;
  esac
  if [ "$(printf '%s\n' "$archs" | wc -w | tr -d ' ')" -gt 1 ]; then
    printf '%s\t%s\n' "$binary" "$archs" >> "$MIXED_ARCHS"
    case "$LABEL" in
      staged-final|final-app)
        printf 'unexpected multi-architecture final binary: %s (%s)\n' \
          "$binary" "$archs" | tee -a "$FAILURES"
        ;;
    esac
  fi

  {
    echo "===== $binary"
    file "$binary"
    lipo -info "$binary" 2>&1 || true
    echo "--- LC_ID_DYLIB"
    otool -D "$binary" 2>&1 || true
    echo "--- linked libraries"
    otool -L "$binary" 2>&1 || true
    echo "--- rpaths"
    otool -l "$binary" 2>/dev/null |
      awk '/cmd LC_RPATH/{show=1; next} show && /path /{print; show=0}' || true
    echo "--- FFmpeg undefined symbols"
    nm -arch "$EXPECTED_ARCH" -u "$binary" 2>/dev/null |
      grep -E '^.*_(av_|avcodec_|avformat_|avfilter_|avdevice_|avio_|avpriv_|ff_|swr_|sws_)' || true
    echo
  } >> "$DETAILS"
done < "$MACHO_LIST"

resolve_rpath_dependency() {
  local binary="$1"
  local dependency="$2"
  local relative="${dependency#@rpath/}"
  local executable_dir=""
  local raw_rpath resolved

  case "$ROOT" in
    *.app) executable_dir="$ROOT/Contents/MacOS" ;;
  esac

  while IFS= read -r raw_rpath; do
    raw_rpath="$(printf '%s\n' "$raw_rpath" | sed -E 's/^[[:space:]]*path ([^ ]+).*/\1/')"
    case "$raw_rpath" in
      @loader_path*) resolved="$(dirname "$binary")${raw_rpath#@loader_path}" ;;
      @executable_path*)
        [ -n "$executable_dir" ] || continue
        resolved="$executable_dir${raw_rpath#@executable_path}"
        ;;
      /*) resolved="$raw_rpath" ;;
      *) continue ;;
    esac
    if [ -e "$resolved/$relative" ]; then
      printf '%s\n' "$resolved/$relative"
      return 0
    fi
  done < <(
    otool -l "$binary" 2>/dev/null |
      awk '/cmd LC_RPATH/{show=1; next} show && /path /{print; show=0}'
  )
  return 1
}

case "$LABEL" in
  staged-final|final-app)
    while IFS= read -r binary; do
      id="$(otool -D "$binary" 2>/dev/null | tail -n +2 | head -n 1 | sed 's/^[[:space:]]*//' || true)"
      while IFS= read -r dependency; do
        dependency="$(printf '%s\n' "$dependency" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+\(.+$//')"
        [ -n "$dependency" ] || continue
        [ "$dependency" = "$id" ] && continue
        case "$dependency" in
          /System/Library/*|/usr/lib/*) ;;
          @loader_path/*)
            candidate="$(dirname "$binary")/${dependency#@loader_path/}"
            [ -e "$candidate" ] || echo "unresolved dependency: $binary -> $dependency" | tee -a "$FAILURES"
            ;;
          @executable_path/*)
            case "$ROOT" in
              *.app) candidate="$ROOT/Contents/MacOS/${dependency#@executable_path/}" ;;
              *) candidate="" ;;
            esac
            [ -n "$candidate" ] && [ -e "$candidate" ] || echo "unresolved dependency: $binary -> $dependency" | tee -a "$FAILURES"
            ;;
          @rpath/*)
            resolve_rpath_dependency "$binary" "$dependency" > /dev/null || \
              echo "unresolved consumer rpath: $binary -> $dependency" | tee -a "$FAILURES"
            ;;
          /*)
            case "$dependency" in
              "$ROOT"/*) [ -e "$dependency" ] || echo "unresolved dependency: $binary -> $dependency" | tee -a "$FAILURES" ;;
              *) echo "unexpected absolute dependency: $binary -> $dependency" | tee -a "$FAILURES" ;;
            esac
            ;;
        esac
      done < <(otool -L "$binary" 2>/dev/null | tail -n +2)
    done < "$MACHO_LIST"
    ;;
esac

# Flattening build trees by basename is unsafe when two different dylibs share
# a name. Report and reject those collisions before packaging can hide them.
DYLIB_HASHES="$REPORT_DIR/${LABEL}-dylib-hashes.tsv"
: > "$DYLIB_HASHES"
while IFS= read -r binary; do
  case "$binary" in
    *.dylib|*.framework/Versions/*/*)
      printf '%s\t%s\t%s\n' \
        "$(basename "$binary")" \
        "$(shasum -a 256 "$binary" | cut -d ' ' -f 1)" \
        "$binary" >> "$DYLIB_HASHES"
      ;;
  esac
done < "$MACHO_LIST"

cut -f 1 "$DYLIB_HASHES" | sort -u | while IFS= read -r base; do
  [ -n "$base" ] || continue
  hashes="$(awk -F '\t' -v base="$base" '$1 == base { print $2 }' "$DYLIB_HASHES" | sort -u | wc -l | tr -d ' ')"
  if [ "$hashes" -gt 1 ]; then
    echo "Conflicting Mach-O providers named $base:" | tee -a "$COLLISIONS"
    awk -F '\t' -v base="$base" '$1 == base { print "  " $2 "  " $3 }' "$DYLIB_HASHES" | tee -a "$COLLISIONS"
  fi
done
if [ -s "$COLLISIONS" ]; then
  cat "$COLLISIONS" >> "$FAILURES"
fi

LIBAV_LIST="$REPORT_DIR/${LABEL}-libav-files.txt"
find "$ROOT" -type f \
  \( -name 'libav*.dylib' -o -name 'libsw*.dylib' \) \
  -print | sort > "$LIBAV_LIST"

{
  echo "===== FFmpeg providers"
  while IFS= read -r provider; do
    [ -n "$provider" ] || continue
    echo "--- $provider"
    file "$provider"
    shasum -a 256 "$provider"
    otool -D "$provider" 2>&1 || true
    otool -L "$provider" 2>&1 || true
  done < "$LIBAV_LIST"
  echo
  echo "===== av_dict_copy consumers"
} >> "$SYMBOLS"

AV_DICT_CONSUMERS=0
while IFS= read -r binary; do
  if nm -arch "$EXPECTED_ARCH" -u "$binary" 2>/dev/null | grep -Eq '(^|[[:space:]])_av_dict_copy$'; then
    echo "$binary" | tee -a "$SYMBOLS"
    AV_DICT_CONSUMERS=$((AV_DICT_CONSUMERS + 1))
  fi
done < "$MACHO_LIST"

AV_DICT_PROVIDERS=0
{
  echo
  echo "===== av_dict_copy providers"
} >> "$SYMBOLS"
while IFS= read -r provider; do
  [ -n "$provider" ] || continue
  if nm -arch "$EXPECTED_ARCH" -gU "$provider" 2>/dev/null | grep -Eq '(^|[[:space:]])_av_dict_copy$'; then
    echo "$provider" | tee -a "$SYMBOLS"
    AV_DICT_PROVIDERS=$((AV_DICT_PROVIDERS + 1))
  fi
done < "$LIBAV_LIST"

printf '\nConsumers: %s\nProviders: %s\n' "$AV_DICT_CONSUMERS" "$AV_DICT_PROVIDERS" >> "$SYMBOLS"
if [ "$AV_DICT_CONSUMERS" -gt 0 ] && [ "$AV_DICT_PROVIDERS" -eq 0 ]; then
  echo "av_dict_copy is imported but no scanned FFmpeg library exports it" | tee -a "$SYMBOLS"
fi

# Architecture-compatible libraries can still be ABI-incompatible. Require
# every FFmpeg symbol imported by the scanned runtime to be exported by the
# bundled FFmpeg provider set.
FFMPEG_IMPORTS="$REPORT_DIR/${LABEL}-ffmpeg-imports.txt"
FFMPEG_EXPORTS="$REPORT_DIR/${LABEL}-ffmpeg-exports.txt"
FFMPEG_UNRESOLVED="$REPORT_DIR/${LABEL}-ffmpeg-unresolved.txt"
: > "$FFMPEG_IMPORTS"
: > "$FFMPEG_EXPORTS"
while IFS= read -r binary; do
  nm -arch "$EXPECTED_ARCH" -u "$binary" 2>/dev/null |
    awk '{ print $NF }' |
    grep -E '^_(av_|avcodec_|avformat_|avfilter_|avdevice_|avio_|avpriv_|ff_|swr_|sws_)[A-Za-z0-9_]*$' >> "$FFMPEG_IMPORTS" || true
done < "$MACHO_LIST"
while IFS= read -r provider; do
  [ -n "$provider" ] || continue
  nm -arch "$EXPECTED_ARCH" -gU "$provider" 2>/dev/null |
    awk '{ print $NF }' |
    grep -E '^_(av_|avcodec_|avformat_|avfilter_|avdevice_|avio_|avpriv_|ff_|swr_|sws_)[A-Za-z0-9_]*$' >> "$FFMPEG_EXPORTS" || true
done < "$LIBAV_LIST"
sort -u "$FFMPEG_IMPORTS" -o "$FFMPEG_IMPORTS"
sort -u "$FFMPEG_EXPORTS" -o "$FFMPEG_EXPORTS"
comm -23 "$FFMPEG_IMPORTS" "$FFMPEG_EXPORTS" > "$FFMPEG_UNRESOLVED"
if [ -s "$FFMPEG_UNRESOLVED" ]; then
  echo "FFmpeg symbols imported without a provider inside this scan root:" | tee -a "$SYMBOLS"
  cat "$FFMPEG_UNRESOLVED" | tee -a "$SYMBOLS"
fi

case "$LABEL" in
  staged-final|final-app)
    # Validate symbols against the FFmpeg dylibs that each consumer actually
    # loads, rather than against a root-wide union of unrelated providers.
    while IFS= read -r binary; do
      CONSUMER_IMPORTS="$REPORT_DIR/${LABEL}-consumer-imports.tmp"
      CONSUMER_EXPORTS="$REPORT_DIR/${LABEL}-consumer-exports.tmp"
      : > "$CONSUMER_IMPORTS"
      : > "$CONSUMER_EXPORTS"
      nm -arch "$EXPECTED_ARCH" -u "$binary" 2>/dev/null |
        awk '{ print $NF }' |
        grep -E '^_(av_|avcodec_|avformat_|avfilter_|avdevice_|avio_|avpriv_|ff_|swr_|sws_)[A-Za-z0-9_]*$' |
        sort -u > "$CONSUMER_IMPORTS" || true
      [ -s "$CONSUMER_IMPORTS" ] || continue
      id="$(otool -D "$binary" 2>/dev/null | tail -n +2 | head -n 1 | sed 's/^[[:space:]]*//' || true)"

      while IFS= read -r dependency; do
        dependency="$(printf '%s\n' "$dependency" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+\(.+$//')"
        [ "$dependency" = "$id" ] && continue
        case "$(basename "$dependency")" in
          libav*.dylib|libsw*.dylib) ;;
          *) continue ;;
        esac

        provider=""
        case "$dependency" in
          @rpath/*) provider="$(resolve_rpath_dependency "$binary" "$dependency" || true)" ;;
          @loader_path/*) provider="$(dirname "$binary")/${dependency#@loader_path/}" ;;
          @executable_path/*)
            case "$ROOT" in
              *.app) provider="$ROOT/Contents/MacOS/${dependency#@executable_path/}" ;;
            esac
            ;;
          /*) provider="$dependency" ;;
        esac
        [ -n "$provider" ] && [ -f "$provider" ] || continue
        nm -arch "$EXPECTED_ARCH" -gU "$provider" 2>/dev/null |
          awk '{ print $NF }' |
          grep -E '^_(av_|avcodec_|avformat_|avfilter_|avdevice_|avio_|avpriv_|ff_|swr_|sws_)[A-Za-z0-9_]*$' \
          >> "$CONSUMER_EXPORTS" || true
      done < <(otool -L "$binary" 2>/dev/null | tail -n +2)

      sort -u "$CONSUMER_EXPORTS" -o "$CONSUMER_EXPORTS"
      CONSUMER_UNRESOLVED="$REPORT_DIR/${LABEL}-consumer-unresolved.tmp"
      comm -23 "$CONSUMER_IMPORTS" "$CONSUMER_EXPORTS" > "$CONSUMER_UNRESOLVED"
      if [ -s "$CONSUMER_UNRESOLVED" ]; then
        echo "FFmpeg ABI mismatch for $binary:" | tee -a "$FAILURES"
        cat "$CONSUMER_UNRESOLVED" | tee -a "$FAILURES"
      fi
    done < "$MACHO_LIST"
    rm -f \
      "$REPORT_DIR/${LABEL}-consumer-imports.tmp" \
      "$REPORT_DIR/${LABEL}-consumer-exports.tmp" \
      "$REPORT_DIR/${LABEL}-consumer-unresolved.tmp"
    ;;
esac

echo "Audited $(wc -l < "$MACHO_LIST" | tr -d ' ') Mach-O files for $LABEL"
if [ -s "$MIXED_ARCHS" ]; then
  echo "Universal or mixed-architecture Mach-O files were recorded in $MIXED_ARCHS"
fi
if [ -s "$FAILURES" ]; then
  echo "ERROR: dependency audit failed:"
  cat "$FAILURES"
  exit 1
fi
echo "Dependency audit passed for $LABEL"
