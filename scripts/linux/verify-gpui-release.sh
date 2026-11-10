#!/usr/bin/env bash
# Verify the Linux GPUI release artifacts and write checksums.
#
# Every artifact must exist, unpack cleanly, and carry the staged runtime
# layout the launcher depends on. Checksums are named after the portable tree
# so the dev-release step can rename the whole set consistently.
set -euo pipefail
shopt -s nullglob

cd "$(cd "$(dirname "$0")/../.." && pwd)"

BUNDLE="crates/revo-gpui/target/release/bundle"
VERSION="$(awk '/^\[package\]/{f=1} f && /^version = /{gsub(/[",]/,""); print $3; exit}' \
  crates/revo-gpui/Cargo.toml)"
test -n "$VERSION"
PREFIX="revo-gpui-${VERSION}-linux-x64"
APP_ID="com.klubuntuprojects.revo"

echo "=== Checking artifact presence ==="
appimages=("$BUNDLE"/*.AppImage)
debs=("$BUNDLE"/*.deb)
rpms=("$BUNDLE"/*.rpm)
archives=("$BUNDLE"/*-linux-x64-portable.tar.gz)
launchers=("$BUNDLE"/*-linux-x64.run)
for entry in "${appimages[@]}" "${debs[@]}" "${rpms[@]}" "${archives[@]}" "${launchers[@]}"; do
  test -s "$entry" || { echo "ERROR: empty or missing artifact: $entry"; exit 1; }
  printf '  %10d  %s\n' "$(stat -c %s "$entry")" "$(basename "$entry")"
done
for group in appimages debs rpms archives launchers; do
  declare -n entries="$group"
  test "${#entries[@]}" -eq 1 || {
    echo "ERROR: expected exactly one $group artifact, found ${#entries[@]}"
    exit 1
  }
done

STAGED_DIR="$BUNDLE/${PREFIX}"
echo "=== Checking the staged tree is complete ==="
for required in revo-gpui revo-engine launch.sh README.txt \
  revo-root/core/lib/libobs.so revo-root/data/share/obs/libobs/default.effect \
  share/applications/$APP_ID.desktop; do
  test -e "$STAGED_DIR/$required" || {
    echo "ERROR: staged tree is missing $required"
    exit 1
  }
done

if command -v dpkg-deb >/dev/null 2>&1; then
  echo "=== Checking DEB ==="
  dpkg-deb --info "${debs[0]}"
  contents="$(dpkg-deb --contents "${debs[0]}")"
  for path in \
    "./usr/lib/revo-stream/revo-gpui" \
    "./usr/lib/revo-stream/revo-engine" \
    "./usr/lib/revo-stream/revo-root/core/lib/libobs.so" \
    "./usr/share/applications/com.klubuntuprojects.revo.desktop" \
    "./usr/share/metainfo/com.klubuntuprojects.revo.appdata.xml"; do
    grep -Fq "$path" <<<"$contents" || { echo "ERROR: DEB is missing $path"; exit 1; }
  done
fi

if command -v rpm >/dev/null 2>&1; then
  echo "=== Checking RPM ==="
  rpm -qp --requires "${rpms[0]}" 2>/dev/null || true
  # One query, then match against the captured list: piping rpm into grep -q
  # would kill rpm with SIGPIPE and fail under pipefail.
  rpm_files="$(rpm -qplp "${rpms[0]}" 2>/dev/null || true)"
  for path in \
    /usr/lib/revo-stream/revo-gpui \
    /usr/lib/revo-stream/revo-engine \
    /usr/lib/revo-stream/revo-root/core/lib/libobs.so \
    /usr/share/applications/$APP_ID.desktop \
    /usr/share/metainfo/$APP_ID.appdata.xml; do
    grep -Fxq "$path" <<<"$rpm_files" || { echo "ERROR: RPM is missing $path"; exit 1; }
  done
fi

echo "=== Checking portable archives ==="
archive_entries="$(tar -tzf "${archives[0]}")"
grep -q "${PREFIX}/revo-engine" <<<"$archive_entries" || {
  echo "ERROR: portable archive does not contain the engine binary"
  exit 1
}
grep -q "${PREFIX}/revo-root/core/lib/libobs.so" <<<"$archive_entries" || {
  echo "ERROR: portable archive does not contain the staged OBS runtime"
  exit 1
}
bash "${launchers[0]}" --check

echo "=== Checking AppImage header ==="
# The type2 runtime is an ELF binary whose magic sits at offset 8; that check
# does not need FUSE, unsquashfs or an executable AppImage runtime.
magic="$(od -An -tx1 -j8 -N8 "${appimages[0]}" | tr -s ' ' | sed -e 's/^ //' -e 's/ $//')"
if [ "$magic" != "41 49 02 00 00 00 00 00" ]; then
  echo "ERROR: ${appimages[0]} is not an AppImage type2 runtime"
  exit 1
fi
echo "  AppImage magic ok"

echo "=== Checking AppImage payload ==="
# The AppImage is executed (not piped through bash) so the type2 runtime can
# self-extract: APPIMAGE_EXTRACT_AND_RUN covers runners without FUSE.
extract_root="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/revo-gpui-appimage-check"
rm -rf "$extract_root"
mkdir -p "$extract_root"
appimage="$(cd "$(dirname "${appimages[0]}")" && pwd)/$(basename "${appimages[0]}")"
chmod +x "$appimage"
if (cd "$extract_root" && "$appimage" --appimage-extract >/dev/null 2>&1); then
  appdir="$extract_root/squashfs-root"
  test -x "$appdir/AppRun"
  test -x "$appdir/usr/lib/revo-stream/revo-gpui"
  test -x "$appdir/usr/lib/revo-stream/revo-engine"
  test -f "$appdir/usr/lib/revo-stream/revo-root/core/lib/libobs.so"
  test -f "$appdir/usr/lib/revo-stream/revo-root/data/share/obs/libobs/default.effect"
  test -f "$appdir/usr/share/applications/$APP_ID.desktop"
  desktop="$(find "$appdir" -maxdepth 3 -name "*.desktop" -print -quit)"
  test -n "$desktop" || { echo "ERROR: AppImage has no desktop entry"; exit 1; }
  grep -q "^Exec=AppRun$" "$desktop" || {
    echo "ERROR: AppImage desktop entry does not launch AppRun: $desktop"
    exit 1
  }
  echo "  AppImage payload is complete"
elif command -v 7z >/dev/null 2>&1; then
  # Fallback for hosts where the runtime cannot self-extract: read the archive
  # index instead of executing it.
  entries="$(7z l "$appimage" 2>/dev/null || true)"
  for path in AppRun usr/lib/revo-stream/revo-gpui usr/lib/revo-stream/revo-engine \
    usr/lib/revo-stream/revo-root/core/lib/libobs.so \
    usr/share/applications/$APP_ID.desktop; do
    grep -q "[[:space:]]$path\$" <<<"$entries" || {
      echo "ERROR: AppImage payload is missing $path"
      exit 1
    }
  done
  echo "  AppImage payload is complete (archive index)"
else
  echo "WARNING: could not extract the AppImage for payload verification" >&2
fi
rm -rf "$extract_root"

echo "=== Checksums ==="
(cd "$BUNDLE" && sha256sum -- \
  "${PREFIX}.AppImage" "${PREFIX}.deb" "${PREFIX}.rpm" \
  "${PREFIX}-portable.tar.gz" "${PREFIX}.run" > "${PREFIX}-SHA256SUMS.txt")
cat "$BUNDLE/${PREFIX}-SHA256SUMS.txt"

echo "Linux GPUI release files are complete"
