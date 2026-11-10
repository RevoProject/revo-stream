#!/usr/bin/env bash
# Prove that the libobs-rs revision this build compiles is macOS-ready.
#
# Which revision is compiled comes from the lockfiles (`revo-lib` tracks
# `libobs-rs` `branch = "main"` with no rev), so this checks the pinned
# revisions instead of whatever checkout directory happens to sort first. A
# revision can be ready either because it was patched or because upstream
# already carries the fix; the assertions are on the resulting source.
set -euo pipefail

ROOT="${HOME}/.cargo/git/checkouts"
LOCKS=(src-tauri/Cargo.lock crates/revo-backend/Cargo.lock crates/revo-gpui/Cargo.lock)

mapfile -t revisions < <(
  for lock in "${LOCKS[@]}"; do
    [ -f "$lock" ] || continue
    grep -oE 'git\+https://github\.com/libobs-rs/libobs-rs\?branch=main#[0-9a-f]+' "$lock" |
      sed -E 's/.*#([0-9a-f]{7}).*/\1/'
  done | sort -u
)

if [ "${#revisions[@]}" -eq 0 ]; then
  echo "ERROR: no libobs-rs revision found in ${LOCKS[*]}"
  exit 1
fi

failures=0
checked=0
for revision in "${revisions[@]}"; do
  checkout="$(find "$ROOT" -maxdepth 2 -type d -name "$revision" -path '*/libobs-rs-*/*' -print -quit)"
  if [ -z "$checkout" ]; then
    # Other jobs fetch other graphs; a revision this job never fetched cannot be
    # verified here, and the build would fail on its own if it needed it.
    echo "NOTE: $revision is not fetched in this job, skipped"
    continue
  fi
  checked=$((checked + 1))
  echo "============================================================"
  echo "VERIFY libobs-rs $revision"
  echo "============================================================"

  build_rs="$checkout/libobs/build.rs"
  if [ ! -f "$build_rs" ]; then
    echo "ERROR: missing $build_rs"
    failures=$((failures + 1))
    continue
  fi
  if grep -q "layout_tests(false)" "$build_rs"; then
    echo "OK: bindgen layout tests are disabled"
  else
    echo "ERROR: bindgen layout tests are still enabled in $build_rs"
    failures=$((failures + 1))
  fi

  other_rs="$checkout/libobs-wrapper/src/utils/initialization/other.rs"
  runtime_rs="$checkout/libobs-wrapper/src/runtime.rs"
  if [ ! -f "$other_rs" ] || [ ! -f "$runtime_rs" ]; then
    echo "ERROR: libobs-wrapper sources are missing under $checkout"
    failures=$((failures + 1))
    continue
  fi

  if grep -B1 '^use std::rc::Rc;$' "$other_rs" | grep -q 'cfg(target_os = "linux")'; then
    echo "ERROR: Rc is still Linux-only in $other_rs"
    failures=$((failures + 1))
  elif ! grep -q '^use std::rc::Rc;$' "$other_rs"; then
    echo "ERROR: Rc import is missing in $other_rs"
    failures=$((failures + 1))
  else
    echo "OK: Rc is available on macOS"
  fi

  # Either an explicit non-Linux stub, or the guard struct is Linux-gated.
  if grep -q '^pub(crate) struct PlatformSpecificGuard;$' "$other_rs"; then
    echo "OK: non-Linux PlatformSpecificGuard stub present"
  elif grep -B1 '^pub(crate) struct PlatformSpecificGuard {' "$other_rs" |
    grep -q 'cfg(target_os = "linux")'; then
    echo "OK: PlatformSpecificGuard is Linux-only"
  else
    echo "ERROR: PlatformSpecificGuard would compile Linux-only fields on macOS"
    failures=$((failures + 1))
  fi

  # The old shape called the display-less macOS setup with an argument.
  if grep -q 'platform_specific_setup(info\.nix_display\.clone())?' "$runtime_rs"; then
    echo "ERROR: $runtime_rs still passes a display argument to the Linux-only setup"
    failures=$((failures + 1))
  else
    echo "OK: platform_specific_setup call matches its definition"
  fi
done

if [ "$checked" -eq 0 ]; then
  echo "ERROR: none of the pinned revisions (${revisions[*]}) are fetched"
  exit 1
fi
if [ "$failures" -gt 0 ]; then
  echo "ERROR: $failures libobs-rs check(s) failed"
  exit 1
fi
echo "Pinned libobs-rs revision(s) are macOS-buildable"
