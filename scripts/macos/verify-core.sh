#!/usr/bin/env bash
# macOS gate for the framework-neutral core crates (revo-core, revo-gpui).
# GPUI builds natively on macOS (Metal), so the shared pipeline runs as-is.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
exec bash "$SCRIPT_DIR/../core/verify.sh"
