#!/usr/bin/env bash
set -euo pipefail

ROOT="src-tauri/resources/revo-root"
LIB="$ROOT/core/lib"
BREW_PREFIX="$(brew --prefix)"
add_rpath() {
  local binary="$1"
  local rpath="$2"
  if ! otool -l "$binary" 2>/dev/null | grep -Fq "path $rpath "; then
    install_name_tool -add_rpath "$rpath" "$binary" 2>/dev/null || true
  fi
}
while IFS= read -r dylib; do
  [ -n "$dylib" ] || continue
  install_name_tool -id "@rpath/$(basename "$dylib")" "$dylib" 2>/dev/null || true
  add_rpath "$dylib" "@loader_path"
done < <(
  find "$LIB" -maxdepth 1 -type f -name "*.dylib" -print
)
while IFS= read -r binary; do
  [ -n "$binary" ] || continue
  while IFS= read -r dep; do
    dep="${dep#        }"
    dep="${dep%% (*}"
    case "$dep" in
      "$BREW_PREFIX"/*)
        base="$(basename "$dep")"
        if [ -f "$LIB/$base" ]; then
          install_name_tool -change "$dep" "@rpath/$base" "$binary" 2>/dev/null || true
        fi
        ;;
    esac
  done < <(otool -L "$binary" 2>/dev/null | tail -n +2)
  case "$binary" in
    */obs-plugins/*.plugin/Contents/MacOS/*)
      add_rpath "$binary" "@loader_path/../../../.."
      ;;
    */libobs.framework/Versions/*/*)
      add_rpath "$binary" "@loader_path/../../../"
      ;;
    */core/lib/*)
      add_rpath "$binary" "@loader_path"
      ;;
  esac
done < <(find "$ROOT" -type f -print)
