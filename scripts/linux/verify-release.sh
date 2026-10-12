#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob

BUNDLE="src-tauri/target/release/bundle"
for pattern in "$BUNDLE"/appimage/*.AppImage "$BUNDLE"/deb/*.deb \
  "$BUNDLE"/rpm/*.rpm "$BUNDLE"/*-linux-x64-portable.tar.gz "$BUNDLE"/*-linux-x64.run; do
  test -s "$pattern"
done
debs=("$BUNDLE"/deb/*.deb)
rpms=("$BUNDLE"/rpm/*.rpm)
archives=("$BUNDLE"/*-linux-x64-portable.tar.gz)
launchers=("$BUNDLE"/*-linux-x64.run)
for files in debs rpms archives launchers; do
  declare -n entries="$files"
  test "${#entries[@]}" -gt 0
done
for deb in "${debs[@]}"; do
  dpkg-deb --info "$deb"
  dpkg-deb --contents "$deb"
done
for rpm in "${rpms[@]}"; do
  rpm -qp --requires "$rpm"
  rpm -qplp "$rpm"
done
for archive in "${archives[@]}"; do tar -tzf "$archive"; done
for launcher in "${launchers[@]}"; do bash "$launcher" --check; done
VERSION="$(node -p "JSON.parse(require('fs').readFileSync('package.json', 'utf8')).version")"
(cd "$BUNDLE" && sha256sum -- appimage/*.AppImage deb/*.deb rpm/*.rpm \
  *-linux-x64-portable.tar.gz *-linux-x64.run > "revo-ui-${VERSION}-linux-x64-SHA256SUMS.txt")
