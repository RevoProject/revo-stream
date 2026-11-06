#!/usr/bin/env bash
# Repair Cargo's debug profile after an interrupted build leaves fingerprints
# but removes generated build_script_* executables. Run inside shell.nix.
# Usage: bash repair-debug-profile.sh [path-to-Cargo.toml]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
MANIFEST="${1:-$PROJECT_DIR/src-tauri/Cargo.toml}"
TARGET_DIR="$(cd "$(dirname "$MANIFEST")" && pwd)/target"
LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT

set +e
cargo check --manifest-path "$MANIFEST" --no-default-features 2>&1 | tee "$LOG"
status=${PIPESTATUS[0]}
set -e

if [ "$status" -eq 0 ]; then
  exit 0
fi

if grep -Fq "$TARGET_DIR/debug/build/" "$LOG" \
  && grep -Eq 'never executed|No such file or directory \(os error 2\)' "$LOG"; then
  echo >&2
  echo "Detected an interrupted/corrupt Cargo debug profile." >&2
  echo "Cleaning only target/debug and rebuilding once..." >&2
  cargo clean --manifest-path "$MANIFEST" --profile dev
  exec cargo check --manifest-path "$MANIFEST" --no-default-features
fi

echo "Cargo check failed for a reason unrelated to stale build-script artifacts; not cleaning." >&2
exit "$status"
