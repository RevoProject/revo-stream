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
TARGET_BRANCH="debug"
TMP_DIR=$(mktemp -d)

trap 'rm -rf "$TMP_DIR"' EXIT

echo "=== Cloning target repo (shallow, depth=1, branch=$TARGET_BRANCH) ==="
git clone --depth 1 --branch "$TARGET_BRANCH" "$TARGET_REPO" "$TMP_DIR/repo" 2>/dev/null || {
  echo "=== Branch '$TARGET_BRANCH' not found — cloning main and creating ==="
  git clone --depth 1 "$TARGET_REPO" "$TMP_DIR/repo"
  cd "$TMP_DIR/repo"
  git checkout -b "$TARGET_BRANCH" 2>/dev/null || true
  cd - >/dev/null
}

cd "$TMP_DIR/repo"

git config user.name "Klubuntu"
source "$PROJECT_DIR/.env" 2>/dev/null || true
git config user.email "${GIT_USER_EMAIL:-49614906+Klubuntu@users.noreply.github.com}"

echo "=== Removing remote assets/ ==="
rm -rf assets/ 2>/dev/null || true

echo "=== Copying local dist/ contents ==="
cp -r "$DIST_DIR"/. .

echo "$(date -Iseconds) — $MSG" > .deploy-version

echo "=== Committing ==="
git add -A
git commit -m "$MSG" 2>/dev/null || git commit --allow-empty -m "$MSG"

echo "=== Force pushing ==="
source "$PROJECT_DIR/.env" 2>/dev/null || true
REPO_WITH_TOKEN="https://Klubuntu:${GITHUB_TOKEN:-}@github.com/open-itvt/desktop-app-dist.git"
git push "$REPO_WITH_TOKEN" HEAD:"$TARGET_BRANCH" --force 2>/dev/null || {
  echo "=== Push failed — check token permissions"
  exit 1
}

echo "=== Done — pushed to $TARGET_BRANCH branch ==="
