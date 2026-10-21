#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-src-tauri/resources/revo-root}"
EXPECTED_ARCH="${REVO_CMAKE_ARCH:?REVO_CMAKE_ARCH is required}"

# Normalize project architecture labels to canonical names reported by lipo.
case "${REVO_CMAKE_ARCH}" in
  x64|x86-64) REVO_CMAKE_ARCH=x86_64 ;;
  arm64|aarch64) REVO_CMAKE_ARCH=arm64 ;;
esac
export REVO_CMAKE_ARCH
REPORT_DIR="${REVO_DIAGNOSTICS:-$RUNNER_TEMP}"
REPORT="$REPORT_DIR/thinned-runtime.tsv"

test -d "$ROOT" || { echo "ERROR: runtime root does not exist: $ROOT"; exit 1; }
mkdir -p "$REPORT_DIR"
printf 'path\tsource_architectures\tresult_architecture\n' > "$REPORT"

while IFS= read -r -d '' binary; do
  file "$binary" 2>/dev/null | grep -q "Mach-O" || continue
  archs="$(lipo -archs "$binary" 2>/dev/null || true)"
  case " $archs " in
    *" $EXPECTED_ARCH "*) ;;
    *)
      echo "ERROR: staged Mach-O lacks $EXPECTED_ARCH: $binary ($archs)"
      exit 1
      ;;
  esac

  if [ "$archs" != "$EXPECTED_ARCH" ]; then
    temp="$binary.revo-thin"
    mode="$(stat -f '%Lp' "$binary")"
    lipo "$binary" -thin "$EXPECTED_ARCH" -output "$temp"
    chmod "$mode" "$temp"
    mv "$temp" "$binary"
  fi
  result="$(lipo -archs "$binary")"
  test "$result" = "$EXPECTED_ARCH"
  printf '%s\t%s\t%s\n' "$binary" "$archs" "$result" >> "$REPORT"
done < <(find "$ROOT" -type f -print0)

echo "Thinned staged runtime Mach-O files to $EXPECTED_ARCH"
