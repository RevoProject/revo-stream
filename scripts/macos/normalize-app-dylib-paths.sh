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

test -d "$APP_LIB"

is_macho() {
  file "$1" 2>/dev/null | grep -q "Mach-O"
}

framework_relative_path() {
  printf '%s\n' "$1" | sed -E 's#^.*/([^/]+\.framework/.*)$#\1#'
}

remove_external_rpaths() {
  local binary="$1"
  local rpath
  while IFS= read -r rpath; do
    rpath="$(printf '%s\n' "$rpath" | sed -E 's/^[[:space:]]*path ([^ ]+).*/\1/')"
    case "$rpath" in
      /usr/lib/*|/System/Library/*) ;;
      /*) install_name_tool -delete_rpath "$rpath" "$binary" ;;
    esac
  done < <(otool -l "$binary" 2>/dev/null | awk '/cmd LC_RPATH/{show=1; next} show && /path /{print; show=0}')
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

  case "$dep" in
    /System/Library/*|/usr/lib/*)
      return 0
      ;;
    /*)
      ;;
    *)
      return 1
      ;;
  esac

  base="$(basename "$dep")"

  if [ ! -f "$APP_LIB/$base" ]; then
    framework_rel="$(framework_relative_path "$dep")"
    if [ "$framework_rel" = "$dep" ] || [ ! -f "$APP_LIB/$framework_rel" ]; then
      echo "ERROR: dependency closure was incomplete before app packaging: $dep"
      echo "       expected under: $APP_LIB"
      return 2
    fi
  fi
}

normalize_binary() {
  local binary="$1"
  local dep
  local base
  local id=""

  is_macho "$binary" || return 0
  remove_external_rpaths "$binary"

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

    "$APP/Contents/Resources/resources/revo-root/core/lib/"*.framework/*)
      add_rpath "$binary" "@loader_path/.."
      ;;

    "$APP/Contents/Resources/resources/revo-root/core/bin/"*)
      add_rpath "$binary" "@loader_path/../lib"
      ;;

    "$APP/Contents/Resources/resources/revo-root/core/lib/"*)
      add_rpath "$binary" "@loader_path"
      ;;
  esac

  # Every dylib/framework binary with an absolute install name gets an
  # app-local install name. Staging must already have provided its closure.
  if [[ "$binary" == *.dylib ||
        "$binary" == *.framework/Versions/*/* ]]; then
    base="$(basename "$binary")"
    id="$(otool -D "$binary" 2>/dev/null | tail -n +2 | head -1 | sed 's/^[[:space:]]*//')"

    case "$id" in
      /*)
        framework_rel="$(framework_relative_path "$id")"
        if [ "$framework_rel" != "$id" ] && [ -f "$APP_LIB/$framework_rel" ]; then
          replacement="@rpath/$framework_rel"
        else
          replacement="@rpath/$base"
        fi
        echo "Relinking install name $(basename "$binary"): $id -> $replacement"
        install_name_tool -id "$replacement" "$binary"
        ;;
    esac
  fi

  # Relink every non-system absolute load command to its staged provider.
  while IFS= read -r dep; do
    dep="$(printf '%s\n' "$dep" |
      sed -E 's/^[[:space:]]+//; s/[[:space:]]+\(.+$//')"
    [ -n "$dep" ] || continue
    [ "$dep" = "$id" ] && continue

    case "$dep" in
      /System/Library/*|/usr/lib/*) ;;
      /*)
        base="$(basename "$dep")"

        if [ ! -f "$APP_LIB/$base" ]; then
          normalize_dep "$dep"
        fi

        framework_rel="$(framework_relative_path "$dep")"
        if [ -f "$APP_LIB/$base" ]; then
          replacement="@rpath/$base"
        elif [ "$framework_rel" != "$dep" ] && [ -f "$APP_LIB/$framework_rel" ]; then
          replacement="@rpath/$framework_rel"
        else
          echo "ERROR: dependency was not staged under $APP_LIB: $dep"
          exit 1
        fi

        echo "Relinking $(basename "$binary"): $dep -> $replacement"
        install_name_tool \
          -change "$dep" \
          "$replacement" \
          "$binary"
        ;;
    esac
  done < <(
    otool -L "$binary" 2>/dev/null |
      tail -n +2
  )
}

# ------------------------------------------------------------
# Pass 1: prove all absolute non-system dependencies were already staged.
# ------------------------------------------------------------
while IFS= read -r -d '' binary; do
  is_macho "$binary" || continue
  id="$(otool -D "$binary" 2>/dev/null | tail -n +2 | head -n 1 | sed 's/^[[:space:]]*//' || true)"

  while IFS= read -r dep; do
    dep="$(printf '%s\n' "$dep" |
      sed -E 's/^[[:space:]]+//; s/[[:space:]]+\(.+$//')"
    [ -n "$dep" ] || continue
    [ "$dep" = "$id" ] && continue

    case "$dep" in
      /System/Library/*|/usr/lib/*) ;;
      /*) normalize_dep "$dep" ;;
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
# Pass 3: re-scan the final app after load-command changes.
# ------------------------------------------------------------
while IFS= read -r -d '' binary; do
  normalize_binary "$binary"
done < <(find "$APP" -type f -print0)

# ------------------------------------------------------------
# Final hard gate: no non-system absolute dylib references may remain.
# ------------------------------------------------------------
echo "=== Exact remaining non-system absolute references ==="

find "$APP" -type f -print0 |
  while IFS= read -r -d '' f; do
    is_macho "$f" || continue

    remaining="$(
      otool -L "$f" 2>/dev/null |
        grep -E '^[[:space:]]*/' |
        grep -Ev '^[[:space:]]*(/System/Library/|/usr/lib/)' \
        || true
    )"

    if [ -n "$remaining" ]; then
      echo "--- $f"
      echo "$remaining"
    fi
  done

echo "=== Remaining non-system absolute references after normalization ==="

REMAINING="$(
  find "$APP" -type f -print0 |
    while IFS= read -r -d '' f; do
      is_macho "$f" || continue
      otool -L "$f" 2>/dev/null || true
    done |
    grep -E '^[[:space:]]*/' |
    grep -Ev '^[[:space:]]*(/System/Library/|/usr/lib/)' \
      || true
)"

if [ -n "$REMAINING" ]; then
  echo "$REMAINING"
  echo "ERROR: non-system absolute dylib references remain after normalization"
  exit 1
fi

REMAINING_RPATHS="$(
  find "$APP" -type f -print0 |
    while IFS= read -r -d '' f; do
      is_macho "$f" || continue
      otool -l "$f" 2>/dev/null |
        awk -v file="$f" '
          /cmd LC_RPATH/ { show=1; next }
          show && /path \// {
            if ($2 !~ "^/usr/lib(/|$)" && $2 !~ "^/System/Library(/|$)") print file ": " $0
            show=0
          }
        '
    done
)"
if [ -n "$REMAINING_RPATHS" ]; then
  echo "$REMAINING_RPATHS"
  echo "ERROR: external absolute LC_RPATH entries remain after normalization"
  exit 1
fi

echo "Final macOS app Mach-O normalization complete"
