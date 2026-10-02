#!/usr/bin/env bash
set -euo pipefail

if [ -z "${1:-}" ]; then
  echo "Usage: $0 \"commit message\""
  exit 1
fi

MSG="$1"
THIS_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$THIS_DIR/.." && pwd)"
DIST_DIR="$PROJECT_DIR/dist"
TARGET_REPO="https://github.com/open-itvt/desktop-app-dist.git"
TMP_DIR=$(mktemp -d)

trap 'rm -rf "$TMP_DIR"' EXIT

echo "=== Cloning target repo (shallow, depth=1) ==="
git clone --depth 1 "$TARGET_REPO" "$TMP_DIR/repo"
cd "$TMP_DIR/repo"

# Use correct author for the dist repo
git config user.name "Klubuntu"
source "$PROJECT_DIR/.env" 2>/dev/null || true
git config user.email "${GIT_USER_EMAIL:-49614906+Klubuntu@users.noreply.github.com}"

echo "=== Removing remote assets/ ==="
rm -rf assets/ 2>/dev/null || true

echo "=== Copying local dist/ contents ==="
cp -r "$DIST_DIR"/. .

# Touch version stamp to ensure a commit has content even when files match
echo "$(date -Iseconds) — $MSG" > .deploy-version

echo "=== Committing ==="
git add -A
git commit -m "$MSG" 2>/dev/null || {
  # If still nothing to commit (e.g. files identical), force empty commit
  git commit --allow-empty -m "$MSG"
}

echo "=== Force pushing ==="
source "$PROJECT_DIR/.env" 2>/dev/null || true
REPO_WITH_TOKEN="https://Klubuntu:${GITHUB_TOKEN:-}@github.com/open-itvt/desktop-app-dist.git"
git push "$REPO_WITH_TOKEN" HEAD:main --force

echo "=== Done ==="
