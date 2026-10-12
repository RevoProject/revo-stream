#!/usr/bin/env bash
set -euo pipefail

SIMDE="$(brew --prefix simde)"
HEADER="$SIMDE/include/simde/x86/sse2.h"
if [ ! -f "$HEADER" ]; then
  echo "SIMDe header not found: $HEADER"
  exit 1
fi
echo "SIMDe header found:"
ls -l "$HEADER"
