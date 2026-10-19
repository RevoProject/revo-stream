#!/usr/bin/env bash
set -euo pipefail

ROOT="src-tauri/resources/revo-root"
LIB="$ROOT/core/lib"
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
while IFS= read -r dylib; do
  [ -n "$dylib" ] || continue
  install_name_tool -id "@rpath/$(basename "$dylib")" "$dylib"
  add_rpath "$dylib" "@loader_path"
done < <(
  find "$LIB" -maxdepth 1 -type f -name "*.dylib" -print
)
while IFS= read -r binary; do
  [ -n "$binary" ] || continue
  file "$binary" 2>/dev/null | grep -q "Mach-O" || continue
  remove_external_rpaths "$binary"
  id="$(otool -D "$binary" 2>/dev/null | tail -n +2 | head -n 1 | sed 's/^[[:space:]]*//' || true)"
  while IFS= read -r dep; do
    dep="${dep#        }"
    dep="${dep%% (*}"
    [ "$dep" = "$id" ] && continue
    case "$dep" in
      /System/Library/*|/usr/lib/*|@rpath/*|@loader_path/*|@executable_path/*) continue ;;
      /*)
        base="$(basename "$dep")"
        replacement="@rpath/$base"
        if [ ! -f "$LIB/$base" ]; then
          framework_rel="$(framework_relative_path "$dep")"
          if [ "$framework_rel" = "$dep" ] || [ ! -f "$LIB/$framework_rel" ]; then
            echo "ERROR: absolute dependency was not staged: $binary -> $dep"
            exit 1
          fi
          replacement="@rpath/$framework_rel"
        fi
        if [ -z "$replacement" ]; then
          echo "ERROR: absolute dependency was not staged: $binary -> $dep"
          exit 1
        fi
        install_name_tool -change "$dep" "$replacement" "$binary"
        ;;
    esac
  done < <(otool -L "$binary" 2>/dev/null | tail -n +2)
  case "$binary" in
    */obs-plugins/*.plugin/Contents/MacOS/*)
      add_rpath "$binary" "@loader_path/../../../.."
      ;;
    */*.framework/Versions/*/*)
      add_rpath "$binary" "@loader_path/../../../"
      ;;
    */*.framework/*)
      add_rpath "$binary" "@loader_path/.."
      ;;
    */core/bin/*)
      add_rpath "$binary" "@loader_path/../lib"
      ;;
    */core/lib/*)
      add_rpath "$binary" "@loader_path"
      ;;
  esac
done < <(find "$ROOT" -type f -print)
