#!/usr/bin/env bash
set -euo pipefail

TARGET="${REVO_TARGET}"
APP_DIR="src-tauri/target/$TARGET/release/bundle/macos"
APP="$(find "$APP_DIR" -maxdepth 1 -type d -name "*.app" -print -quit)"

if [ -z "$APP" ]; then
  echo "ERROR: .app was not produced before final normalization"
  find "src-tauri/target/$TARGET/release/bundle" -maxdepth 3 -print || true
  exit 1
fi

ROOT="$APP/Contents/Resources/resources/revo-root"
APP_LIB="$ROOT/core/lib"
BREW_PREFIX="$(brew --prefix)"
BREW_CELLAR="$BREW_PREFIX/Cellar"

test -d "$APP_LIB"

is_macho() {
  file "$1" 2>/dev/null | grep -q "Mach-O"
}

add_rpath() {
  local binary="$1"
  local rpath="$2"

  if ! otool -l "$binary" 2>/dev/null | grep -Fq "path $rpath "; then
    install_name_tool -add_rpath "$rpath" "$binary"
  fi
}

normalize_dep() {
  local dep="$1"
  local base
  local src=""

  case "$dep" in
    /opt/homebrew/Cellar/*|/usr/local/Cellar/*|/opt/homebrew/*|/usr/local/opt/*)
      ;;
    *)
      return 1
      ;;
  esac

  base="$(basename "$dep")"

  if [ -f "$APP_LIB/$base" ]; then
    return 0
  fi

  if [ -f "$dep" ]; then
    src="$dep"
  elif [ -d "$BREW_CELLAR" ]; then
    src="$(
      find "$BREW_CELLAR" \
        -type f \
        -name "$base" \
        -print \
        -quit 2>/dev/null || true
    )"
  fi

  if [ -z "$src" ] || [ ! -f "$src" ]; then
    echo "ERROR: bundled dependency not found for $dep"
    echo "       expected: $APP_LIB/$base"
    echo "       Homebrew prefix: $BREW_PREFIX"
    return 2
  fi

  echo "Bundling missing Homebrew dependency: $base"
  cp -L "$src" "$APP_LIB/$base"
  return 0
}

normalize_binary() {
  local binary="$1"
  local dep
  local base
  local id

  is_macho "$binary" || return 0

  case "$binary" in
    "$APP/Contents/MacOS/"*)
      add_rpath \
        "$binary" \
        "@loader_path/../Resources/resources/revo-root/core/lib"
      ;;

    "$APP/Contents/Resources/resources/revo-root/core/lib/obs-plugins/"*.plugin/Contents/MacOS/*)
      add_rpath "$binary" "@loader_path/../../../.."
      ;;

    "$APP/Contents/Resources/resources/revo-root/core/lib/"*.framework/Versions/*/*)
      add_rpath "$binary" "@loader_path/../../../"
      ;;

    "$APP/Contents/Resources/resources/revo-root/core/lib/"*)
      add_rpath "$binary" "@loader_path"
      ;;
  esac

  # Every dylib/framework binary gets an app-local install name.
  # This is important for dylibs such as libmpg123 whose own
  # LC_ID_DYLIB can still point at Homebrew.
  if [[ "$binary" == *.dylib ||
        "$binary" == *.framework/Versions/*/* ]]; then
    base="$(basename "$binary")"
    id="$(otool -D "$binary" 2>/dev/null | tail -n +2 | head -1 | sed 's/^[[:space:]]*//')"

    case "$id" in
      /opt/homebrew/Cellar/*|/usr/local/Cellar/*|/opt/homebrew/*|/usr/local/opt/*)
        echo "Relinking install name $(basename "$binary"): $id -> @rpath/$base"
        install_name_tool -id "@rpath/$base" "$binary"
        ;;
    esac
  fi

  # Relink every Homebrew load command. Re-read otool output after
  # changing LC_ID_DYLIB so a dylib's own old Homebrew install name
  # cannot survive into the final gate.
  while IFS= read -r dep; do
    dep="$(printf '%s\n' "$dep" |
      sed -E 's/^[[:space:]]+//; s/[[:space:]]+\(.+$//')"
    [ -n "$dep" ] || continue

    case "$dep" in
      /opt/homebrew/Cellar/*|/usr/local/Cellar/*|/opt/homebrew/*|/usr/local/opt/*)
        base="$(basename "$dep")"

        if [ ! -f "$APP_LIB/$base" ]; then
          normalize_dep "$dep"
        fi

        if [ ! -f "$APP_LIB/$base" ]; then
          echo "ERROR: dependency was not staged: $dep"
          echo "       expected: $APP_LIB/$base"
          exit 1
        fi

        echo "Relinking $(basename "$binary"): $dep -> @rpath/$base"
        install_name_tool \
          -change "$dep" \
          "@rpath/$base" \
          "$binary"
        ;;
    esac
  done < <(
    otool -L "$binary" 2>/dev/null |
      tail -n +2
  )
}

# ------------------------------------------------------------
# Pass 1: discover and stage all Homebrew dependencies.
# ------------------------------------------------------------
while IFS= read -r -d '' binary; do
  is_macho "$binary" || continue

  while IFS= read -r dep; do
    dep="$(printf '%s\n' "$dep" |
      sed -E 's/^[[:space:]]+//; s/[[:space:]]+\(.+$//')"
    [ -n "$dep" ] || continue

    case "$dep" in
                /opt/homebrew/Cellar/*|/usr/local/Cellar/*|/opt/homebrew/*|/usr/local/opt/*)
        normalize_dep "$dep"
        ;;
    esac
  done < <(
    otool -L "$binary" 2>/dev/null |
      tail -n +2
  )
done < <(find "$APP" -type f -print0)

# ------------------------------------------------------------
# Pass 2: normalize every Mach-O currently in the final .app.
# ------------------------------------------------------------
while IFS= read -r -d '' binary; do
  normalize_binary "$binary"
done < <(find "$APP" -type f -print0)

# ------------------------------------------------------------
# Pass 3: re-scan the final app. This catches dependencies
# introduced by dylibs that were bundled during Pass 1/2.
# ------------------------------------------------------------
while IFS= read -r -d '' binary; do
  normalize_binary "$binary"
done < <(find "$APP" -type f -print0)

# ------------------------------------------------------------
# Final hard gate: no absolute Homebrew dylib references may
# remain anywhere in the shipped .app.
# ------------------------------------------------------------
echo "=== Exact remaining Homebrew references ==="

find "$APP" -type f -print0 |
  while IFS= read -r -d '' f; do
    is_macho "$f" || continue

    remaining="$(
      otool -L "$f" 2>/dev/null |
        grep -E \
          '/opt/homebrew/|/usr/local/opt/|/opt/homebrew/Cellar/|/usr/local/Cellar/' \
        || true
    )"

    if [ -n "$remaining" ]; then
      echo "--- $f"
      echo "$remaining"
    fi
  done

echo "=== Remaining Homebrew references after normalization ==="

REMAINING="$(
  find "$APP" -type f -print0 |
    while IFS= read -r -d '' f; do
      is_macho "$f" || continue
      otool -L "$f" 2>/dev/null || true
    done |
    grep -E \
      '/opt/homebrew/|/usr/local/opt/|/opt/homebrew/Cellar/|/usr/local/Cellar/' \
      || true
)"

if [ -n "$REMAINING" ]; then
  echo "$REMAINING"
  echo "ERROR: Homebrew absolute dylib references remain after normalization"
  exit 1
fi

echo "Final macOS app Mach-O normalization complete"
