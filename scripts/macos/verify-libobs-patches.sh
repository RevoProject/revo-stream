#!/usr/bin/env bash
set -euo pipefail

echo "============================================================"
echo "VERIFY libobs bindgen"
echo "============================================================"
BUILD_RS=$(find "$HOME/.cargo/git/checkouts" \
  -path "*/7e76c91/libobs/build.rs" \
  -print -quit)
if [ -z "$BUILD_RS" ]; then
  echo "libobs build.rs not found"
  exit 1
fi
grep -n -C 4 \
  "layout_tests" \
  "$BUILD_RS"
echo
echo "============================================================"
echo "VERIFY libobs-wrapper"
echo "============================================================"
WRAPPER_SRC=$(find "$HOME/.cargo/git/checkouts" \
  -path "*/7e76c91/libobs-wrapper/src" \
  -type d \
  -print -quit)
if [ -z "$WRAPPER_SRC" ]; then
  echo "libobs-wrapper source not found"
  exit 1
fi
OTHER_RS="$WRAPPER_SRC/utils/initialization/other.rs"
echo
echo "--- Rc import ---"
grep -n -C 3 \
  "use std::rc::Rc;" \
  "$OTHER_RS"
if grep -B1 \
  'use std::rc::Rc;' \
  "$OTHER_RS" |
  grep -q 'cfg(target_os = "linux")'; then
  echo "ERROR: Rc is still Linux-only"
  exit 1
fi
if ! grep -q \
  '^use std::rc::Rc;$' \
  "$OTHER_RS"; then
  echo "ERROR: Rc import is missing"
  exit 1
fi
echo "OK: Rc is available on macOS"
echo
echo "--- PlatformSpecificGuard ---"
grep -n -C 5 \
  "PlatformSpecificGuard" \
  "$OTHER_RS"
echo
echo "--- platform_specific_setup ---"
grep -n -C 5 \
  "platform_specific_setup" \
  "$WRAPPER_SRC/runtime.rs" || true
