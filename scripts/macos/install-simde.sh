#!/usr/bin/env bash
set -euo pipefail

brew install simde
{
  echo "CPATH=$(brew --prefix simde)/include:${CPATH:-}"
  echo "C_INCLUDE_PATH=$(brew --prefix simde)/include:${C_INCLUDE_PATH:-}"
  echo "CPLUS_INCLUDE_PATH=$(brew --prefix simde)/include:${CPLUS_INCLUDE_PATH:-}"
} >> "$GITHUB_ENV"
