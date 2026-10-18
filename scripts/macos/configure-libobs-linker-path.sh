#!/usr/bin/env bash
set -euo pipefail

OBS_BUILD="${REVO_OBS_BUILD:-$RUNNER_TEMP/obs-build-${REVO_TARGET:?REVO_TARGET is required}}"
echo "============================================================"
echo "Locate REAL OBS libobs Mach-O binary"
echo "============================================================"
LIBOBS_BINARY=""
while IFS= read -r candidate; do
  [ -n "$candidate" ] || continue
  case "$candidate" in
    *.tbd)
      continue
      ;;
  esac
  if file "$candidate" | grep -q "Mach-O"; then
    LIBOBS_BINARY="$candidate"
    break
  fi
done < <(
  find "$OBS_BUILD" \
    -type f \
    -name "libobs" \
    ! -path "*/EagerLinkingTBDs/*" \
    -print |
  sort
)
# Also check symlinked framework binaries.
if [ -z "$LIBOBS_BINARY" ]; then
  while IFS= read -r candidate; do
    [ -n "$candidate" ] || continue
    case "$candidate" in
      *.tbd)
        continue
        ;;
    esac
    if file "$candidate" | grep -q "Mach-O"; then
      LIBOBS_BINARY="$candidate"
      break
    fi
  done < <(
    find "$OBS_BUILD" \
      -name "libobs" \
      ! -path "*/EagerLinkingTBDs/*" \
      -print |
    sort
  )
fi
if [ -z "$LIBOBS_BINARY" ]; then
  echo
  echo "ERROR: REAL libobs Mach-O binary not found"
  echo
  echo "All libobs paths found:"
  find "$OBS_BUILD" \
    -name "libobs" \
    -print |
  sort || true
  echo
  echo "All libobs.framework paths:"
  find "$OBS_BUILD" \
    -type d \
    -name "libobs.framework" \
    -print |
  sort || true
  echo
  echo "EagerLinkingTBD files:"
  find "$OBS_BUILD" \
    -path "*/EagerLinkingTBDs/*" \
    -print |
  sort || true
  exit 1
fi
echo
echo "============================================================"
echo "REAL libobs binary"
echo "============================================================"
echo "$LIBOBS_BINARY"
echo
echo "file:"
file "$LIBOBS_BINARY"
echo
echo "architecture:"
lipo -info "$LIBOBS_BINARY" || true
LIBOBS_DIR="$(dirname "$LIBOBS_BINARY")"
LIBOBS_LINK="$LIBOBS_DIR/libobs.dylib"
echo
echo "Linker directory:"
echo "$LIBOBS_DIR"
# libobs-rs uses:
#
#   -L "$LIBOBS_PATH"
#   -lobs
#
# Expose the framework binary under the name expected
# by the linker.
if [ -e "$LIBOBS_LINK" ] || [ -L "$LIBOBS_LINK" ]; then
  rm -f "$LIBOBS_LINK"
fi
ln -s \
  "$(basename "$LIBOBS_BINARY")" \
  "$LIBOBS_LINK"
echo
echo "Created linker alias:"
ls -lah "$LIBOBS_LINK"
echo
echo "Checking OBS symbols:"
nm -gU "$LIBOBS_BINARY" |
  grep -E \
    '(_base_set_log_handler|_obs_add_data_path|_gs_flush|_gs_ortho)' |
  head -20 ||
  true
echo
echo "============================================================"
echo "LIBOBS_PATH"
echo "============================================================"
echo "$LIBOBS_DIR"
printf 'LIBOBS_PATH=%s\n' "$LIBOBS_DIR" >> "$GITHUB_ENV"
