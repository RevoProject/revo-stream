#!/usr/bin/env bash
# Build the native GPUI application into DEB, RPM and AppImage packages.
#
# Usage:
#   bash scripts/linux/package-gpui-bundles.sh [STAGED_DIR]
#
# STAGED_DIR is the GPUI layout produced by scripts/linux/package-gpui.sh
# (default: crates/revo-gpui/target/release/bundle/revo-gpui-<version>-linux-x64).
# The staged tree is the single source of truth for every artifact: UI binary,
# engine binary, revo-root runtime, desktop entry and icons.
#
# Dependencies are derived from the real ELF requirements of the staged tree,
# not from a hand-maintained list: libraries resolved inside revo-root are
# bundled and therefore not declared, host libraries become package
# dependencies. The OBS build already copied its private FFmpeg/x264/… closure
# into revo-root, so what is left on the host is exactly what a package must
# require.
#
# Outputs (under crates/revo-gpui/target/release/bundle/):
#   revo-gpui-<version>-linux-x64.deb
#   revo-gpui-<version>-linux-x64.rpm
#   revo-gpui-<version>-linux-x64.AppImage
set -euo pipefail
shopt -s nullglob

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$PROJECT_DIR"

BUNDLE_DIR="crates/revo-gpui/target/release/bundle"
APP_ID="com.klubuntuprojects.revo"
APP_NAME="RevoStream"
# Install prefix inside DEB/RPM. The launcher is generated per package so the
# runtime path is absolute and does not depend on a portable directory.
EXEC_PREFIX="/usr/lib/revo-stream"
ARCH="$(uname -m)"
DEB_ARCH="$ARCH"
case "$ARCH" in
  x86_64) DEB_ARCH=amd64 ;;
  aarch64) DEB_ARCH=arm64 ;;
esac

VERSION="$(awk '/^\[package\]/{f=1} f && /^version = /{gsub(/[",]/,""); print $3; exit}' \
  "$PROJECT_DIR/crates/revo-gpui/Cargo.toml")"
test -n "$VERSION"

STAGED="${1:-$BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64}"
test -x "$STAGED/revo-gpui" || { echo "staged tree missing: $STAGED" >&2; exit 1; }
test -x "$STAGED/revo-engine" || { echo "staged engine missing: $STAGED" >&2; exit 1; }
test -f "$STAGED/revo-root/core/lib/libobs.so" || { echo "staged revo-root missing" >&2; exit 1; }
test -f "$STAGED/share/applications/$APP_ID.desktop" || { echo "staged desktop entry missing" >&2; exit 1; }

WORK="$(mktemp -d "${TMPDIR:-/tmp}/revo-gpui-packages.XXXXXX")"
# Cleanup must survive read-only files inherited from a store-backed runtime.
trap 'chmod -R u+rwX "$WORK" 2>/dev/null || true; rm -rf "$WORK"' EXIT

echo "=== Collecting host library dependencies from the staged tree ==="
# Every ELF in the tree, resolved with revo-root on the search path so bundled
# libraries resolve inside the package instead of on the host.
mapfile -t ELF_FILES < <(
  find "$STAGED" -type f \( -name 'revo-gpui' -o -name 'revo-engine' -o -path '*/revo-root/*' \) -print0 |
  while IFS= read -r -d '' candidate; do
    file -b "$candidate" 2>/dev/null | grep -q '^ELF ' && printf '%s\n' "$candidate"
  done
)
test "${#ELF_FILES[@]}" -gt 0 || { echo "no ELF files found under $STAGED" >&2; exit 1; }

STAGED_REAL="$(realpath "$STAGED")"
HOST_LIBS=()
while IFS= read -r library; do
  resolved="$(realpath "$library" 2>/dev/null || echo "$library")"
  case "$resolved" in
    "$STAGED_REAL"/*) continue ;;          # bundled inside revo-root
  esac
  case "$(basename "$library")" in
    ld-linux*|linux-gate*|linux-vdso*) continue ;;
  esac
  HOST_LIBS+=("$library")
done < <(
  for elf in "${ELF_FILES[@]}"; do
    LD_LIBRARY_PATH="$STAGED_REAL/revo-root/core/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
      ldd "$elf" 2>/dev/null | awk '/=> \// { print $3 }'
  done | sort -u
)
test "${#HOST_LIBS[@]}" -gt 0 || { echo "no host libraries resolved; the scan failed" >&2; exit 1; }
printf '  %s\n' "${HOST_LIBS[@]}" | sed 's/^/  host lib: /' >&2

# RPM can require bare sonames, which is exactly what the loader checks.
RPM_REQUIRES=()
for library in "${HOST_LIBS[@]}"; do
  RPM_REQUIRES+=("$(basename "$library")(64bit)")
done
mapfile -t RPM_REQUIRES < <(printf '%s\n' "${RPM_REQUIRES[@]}" | sort -u)

# DEB needs the owning package names.
DEB_REQUIRES=()
if command -v dpkg >/dev/null 2>&1; then
  for library in "${HOST_LIBS[@]}"; do
    owner="$(dpkg -S "$library" 2>/dev/null | cut -d: -f1 || true)"
    if [ -z "$owner" ]; then
      echo "WARNING: no Debian package owns $library; it will not be declared." >&2
      continue
    fi
    DEB_REQUIRES+=("$owner")
  done
  mapfile -t DEB_REQUIRES < <(printf '%s\n' "${DEB_REQUIRES[@]}" | sort -u)
fi

DESCRIPTION="Native GPUI streaming and recording studio for Linux (libobs engine in a separate process)."

write_launcher() {
  local launcher="$1" prefix="$2"
  cat > "$launcher" << EOF
#!/usr/bin/env bash
# RevoStream GPUI launcher: wires the installed OBS runtime and private libs.
set -euo pipefail
PREFIX="$prefix"
export REVO_ROOT="\${REVO_ROOT:-\$PREFIX/revo-root}"
export LD_LIBRARY_PATH="\$PREFIX/revo-root/core/lib\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}"
export REVO_ENGINE_BIN="\${REVO_ENGINE_BIN:-\$PREFIX/revo-engine}"
exec "\$PREFIX/revo-gpui" "\$@"
EOF
  chmod 0755 "$launcher"
}

write_desktop_entry() {
  local file="$1" exec_line="$2"
  cat > "$file" << EOF
[Desktop Entry]
Name=$APP_NAME
Comment=Native GPUI streaming and recording studio
Exec=$exec_line
Icon=revo-gpui
Terminal=false
Type=Application
Categories=AudioVideo;Video;Network;
StartupWMClass=RevoStream
EOF
}

copy_runtime() {
  local root="$1"
  mkdir -p "$root$EXEC_PREFIX"
  cp -a "$STAGED/revo-gpui" "$STAGED/revo-engine" "$root$EXEC_PREFIX/"
  cp -a "$STAGED/revo-root" "$root$EXEC_PREFIX/revo-root"
  write_launcher "$root$EXEC_PREFIX/launch.sh" "$EXEC_PREFIX"
  mkdir -p "$root/usr/bin"
  ln -sf "$EXEC_PREFIX/launch.sh" "$root/usr/bin/revo-gpui"
  install -Dm644 "$STAGED/share/applications/$APP_ID.desktop" \
    "$root/usr/share/applications/$APP_ID.desktop"
  if [ -n "$2" ]; then
    sed -i "s|^Exec=.*|Exec=$2|" "$root/usr/share/applications/$APP_ID.desktop"
  fi
  for icon in "$STAGED"/share/icons/hicolor/*/apps/revo-gpui.png; do
    size="$(basename "$(dirname "$(dirname "$icon")")")"
    install -Dm644 "$icon" "$root/usr/share/icons/hicolor/$size/apps/revo-gpui.png"
  done
  printf '%s\n' "$VERSION" > "$root/usr/share/revo-gpui-version"
}

echo "=== Building DEB ==="
if command -v dpkg-deb >/dev/null 2>&1; then
  DEB_ROOT="$WORK/deb"
  rm -rf "$DEB_ROOT"; mkdir -p "$DEB_ROOT/DEBIAN"
  copy_runtime "$DEB_ROOT" "revo-gpui"
  {
    echo "Package: revo-gpui"
    echo "Version: $VERSION"
    echo "Architecture: $DEB_ARCH"
    echo "Section: video"
    echo "Priority: optional"
    echo "Maintainer: RevoProject <https://github.com/RevoProject>"
    if [ "${#DEB_REQUIRES[@]}" -gt 0 ]; then
      printf 'Depends: %s\n' "$(IFS=', '; echo "${DEB_REQUIRES[*]}")"
    fi
    echo "Description: $APP_NAME native GPUI application"
    echo " $DESCRIPTION"
  } > "$DEB_ROOT/DEBIAN/control"
  dpkg-deb --root-owner-group --build "$DEB_ROOT" \
    "$BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64.deb" >/dev/null
  echo "    deb: revo-gpui-${VERSION}-linux-x64.deb"
else
  echo "SKIP DEB: dpkg-deb not available" >&2
fi

echo "=== Building RPM ==="
if command -v rpmbuild >/dev/null 2>&1; then
  TOPDIR="$WORK/rpm"
  mkdir -p "$TOPDIR"/{BUILD,RPMS,SPECS,SOURCES,SRPMS,BUILDROOT}
  # The RPM payload is the FHS tree the DEB uses as well: staging it once keeps
  # the two packages identical apart from their metadata.
  RPM_ROOT="$WORK/rpm-root"
  rm -rf "$RPM_ROOT"
  copy_runtime "$RPM_ROOT" "revo-gpui"
  tar -C "$RPM_ROOT" -czf "$TOPDIR/SOURCES/payload.tar.gz" .
  REQUIRES_BLOCK=""
  for require in "${RPM_REQUIRES[@]}"; do
    REQUIRES_BLOCK+="Requires: $require"$'\n'
  done
  cat > "$TOPDIR/SPECS/revo-gpui.spec" << EOF
Name:           revo-gpui
Version:        $VERSION
Release:        1
Summary:        $APP_NAME native GPUI application
License:        AGPL-3.0-only
URL:            https://github.com/RevoProject/revo-stream
Source0:        payload.tar.gz
BuildArch:      $ARCH
$REQUIRES_BLOCK
%description
$DESCRIPTION

%prep
rm -rf payload
mkdir -p payload
tar -xzf %{SOURCE0} -C payload

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}
cp -a payload/. %{buildroot}/

%files
$EXEC_PREFIX
/usr/bin/revo-gpui
/usr/share/applications/$APP_ID.desktop
/usr/share/icons
/usr/share/revo-gpui-version

%changelog
* $(date -u '+%a %b %d %Y') RevoProject <https://github.com/RevoProject> - $VERSION-1
- Native GPUI build.
EOF
  rpmbuild --define "_topdir $TOPDIR" -bb "$TOPDIR/SPECS/revo-gpui.spec" \
    > "$WORK/rpmbuild.log" 2>&1 || { tail -40 "$WORK/rpmbuild.log" >&2; exit 1; }
  built="$(find "$TOPDIR/RPMS" -name 'revo-gpui-*.rpm' -print -quit)"
  test -n "$built"
  cp "$built" "$BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64.rpm"
  echo "    rpm: revo-gpui-${VERSION}-linux-x64.rpm"
else
  echo "SKIP RPM: rpmbuild not available" >&2
fi

echo "=== Building AppImage ==="
APPIMAGETOOL="${APPIMAGETOOL:-}"
if [ -z "$APPIMAGETOOL" ] && command -v appimagetool >/dev/null 2>&1; then
  APPIMAGETOOL="$(command -v appimagetool)"
fi
if [ -z "$APPIMAGETOOL" ]; then
  cache="$PROJECT_DIR/.cache/appimagetool"
  mkdir -p "$cache"
  cached="$cache/appimagetool-x86_64.AppImage"
  if [ ! -x "$cached" ]; then
    curl -fL --retry 3 \
      "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage" \
      -o "$cached"
    chmod +x "$cached"
  fi
  APPIMAGETOOL="$cached"
fi
APPDIR="$WORK/revo-gpui.AppDir"
rm -rf "$APPDIR"
copy_runtime "$APPDIR" "AppRun"
rm -f "$APPDIR$EXEC_PREFIX/launch.sh"
rm -f "$APPDIR/usr/bin/revo-gpui"
cat > "$APPDIR/AppRun" << 'EOF'
#!/usr/bin/env bash
# AppImage entry point: the staged revo-root travels inside the image.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
PREFIX="$HERE/usr/lib/revo-stream"
export REVO_ROOT="${REVO_ROOT:-$PREFIX/revo-root}"
export LD_LIBRARY_PATH="$PREFIX/revo-root/core/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export REVO_ENGINE_BIN="${REVO_ENGINE_BIN:-$PREFIX/revo-engine}"
exec "$PREFIX/revo-gpui" "$@"
EOF
chmod 0755 "$APPDIR/AppRun"
write_desktop_entry "$APPDIR/$APP_ID.desktop" "AppRun"
primary_icon="$(
  find "$STAGED/share/icons/hicolor/256x256/apps/revo-gpui.png" \
    "$STAGED/share/icons/hicolor/128x128/apps/revo-gpui.png" 2>/dev/null |
    head -1
)"
if [ -n "$primary_icon" ]; then
  # appimagetool resolves `Icon=<name>` to <AppDir>/<name>.png, so the icon is
  # copied to that name and the desktop entry keeps the bare name.
  cp "$primary_icon" "$APPDIR/revo-gpui.png"
  sed -i "s|^Icon=.*|Icon=revo-gpui|" "$APPDIR/$APP_ID.desktop"
fi
APPIMAGE_OUTPUT="$BUNDLE_DIR/revo-gpui-${VERSION}-linux-x64.AppImage"
rm -f "$APPIMAGE_OUTPUT"
(
  export APPIMAGE_EXTRACT_AND_RUN=1
  # --gnu-ldc selects the runtime non-interactively; older builds do not know
  # the flag, so fall back to the default runtime instead of hanging on a prompt.
  ARCH="$ARCH" "$APPIMAGETOOL" --gnu-ldc "$APPDIR" "$APPIMAGE_OUTPUT" ||
    ARCH="$ARCH" "$APPIMAGETOOL" "$APPDIR" "$APPIMAGE_OUTPUT"
)
test -s "$APPIMAGE_OUTPUT"
echo "    appimage: revo-gpui-${VERSION}-linux-x64.AppImage"

echo "=== Done ==="
