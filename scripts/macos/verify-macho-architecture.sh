#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:?usage: verify-macho-architecture.sh <root> <arm64|x86_64>}"
EXPECTED_ARCH="${2:?usage: verify-macho-architecture.sh <root> <arm64|x86_64>}"

case "$EXPECTED_ARCH" in
  arm64|x86_64) ;;
  *) echo "ERROR: unsupported expected architecture: $EXPECTED_ARCH"; exit 1 ;;
esac

test -d "$ROOT" || { echo "ERROR: Mach-O validation root does not exist: $ROOT"; exit 1; }
ROOT="$(cd "$ROOT" && pwd -P)"

checked=0
failures=0

contains_bundled_dependency() {
  local dep="$1"
  local rel="${dep#@rpath/}"
  local base="$(basename "$dep")"
  find "$ROOT" -type f \( -path "*/$rel" -o -name "$base" \) -print -quit | grep -q .
}

while IFS= read -r -d '' binary; do
  file "$binary" 2>/dev/null | grep -q "Mach-O" || continue
  checked=$((checked + 1))

  archs="$(lipo -archs "$binary" 2>/dev/null || true)"
  case " $archs " in
    *" $EXPECTED_ARCH "*) ;;
    *)
      echo "ERROR: architecture mismatch"
      echo "  file: $binary"
      echo "  expected: $EXPECTED_ARCH"
      echo "  found: ${archs:-unknown}"
      failures=$((failures + 1))
      ;;
  esac

  while IFS= read -r dep; do
    dep="${dep#        }"
    dep="${dep%% (*}"
    [ -n "$dep" ] || continue
    case "$dep" in
      /System/Library/*|/usr/lib/*)
        ;;
      /opt/homebrew/*|/usr/local/Cellar/*|/usr/local/opt/*|*"/obs-build-"*|*"/obs-install-"*)
        echo "ERROR: non-portable dependency in $binary: $dep"
        failures=$((failures + 1))
        ;;
      @rpath/*)
        if ! contains_bundled_dependency "$dep"; then
          echo "ERROR: missing bundled @rpath dependency in $binary: $dep"
          failures=$((failures + 1))
        fi
        ;;
      @loader_path/*)
        candidate="$(dirname "$binary")/${dep#@loader_path/}"
        if [ ! -e "$candidate" ]; then
          echo "ERROR: missing @loader_path dependency in $binary: $dep"
          failures=$((failures + 1))
        fi
        ;;
      @executable_path/*)
        # Executable-path resolution depends on the final .app layout. The
        # final app verification below resolves it from Contents/MacOS.
        if [[ "$ROOT" == *.app ]]; then
          candidate="$ROOT/Contents/MacOS/${dep#@executable_path/}"
          if [ ! -e "$candidate" ]; then
            echo "ERROR: missing @executable_path dependency in $binary: $dep"
            failures=$((failures + 1))
          fi
        fi
        ;;
      /*)
        if [[ "$dep" != "$ROOT"/* ]]; then
          echo "ERROR: unexpected absolute dependency in $binary: $dep"
          failures=$((failures + 1))
        fi
        ;;
    esac
  done < <(otool -L "$binary" 2>/dev/null | tail -n +2)
done < <(find "$ROOT" -type f -print0)

echo "Validated Mach-O files: $checked (expected arch: $EXPECTED_ARCH)"
if [ "$checked" -eq 0 ]; then
  echo "ERROR: no Mach-O files found under $ROOT"
  exit 1
fi
if [ "$failures" -ne 0 ]; then
  echo "ERROR: Mach-O validation failed with $failures issue(s)"
  exit 1
fi
echo "Mach-O architecture and dependency validation passed"
