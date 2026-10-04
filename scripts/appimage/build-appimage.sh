#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
SRC_TAURI="${REPO_ROOT}/src-tauri"
RES_ROOT="${SRC_TAURI}/resources/revo-root"

should_enable_egl_workaround() {
	local is_ubuntu_like=0
	local has_egl_issue=0

	if [[ -r /etc/os-release ]]; then
		# shellcheck disable=SC1091
		source /etc/os-release
		local id_lc="${ID,,}"
		local id_like_lc="${ID_LIKE,,}"
		if [[ "${id_lc}" == "ubuntu" || "${id_lc}" == "linuxmint" || "${id_like_lc}" == *"ubuntu"* ]]; then
			is_ubuntu_like=1
		fi
	fi

	if command -v eglinfo >/dev/null 2>&1; then
		local egl_out
		egl_out="$(eglinfo 2>&1 || true)"
		if grep -Eqi 'libEGL warning|driver \(null\)|failed to create dri2 screen|DRI2: failed to create screen' <<<"${egl_out}"; then
			has_egl_issue=1
		fi
	fi

	[[ ${is_ubuntu_like} -eq 1 || ${has_egl_issue} -eq 1 ]]
}


APPIMAGE_CACHE="${REPO_ROOT}/.cache/appimagetool"
APPIMAGETOOL="${APPIMAGE_CACHE}/appimagetool-x86_64.AppImage"
APPDIR="${SRC_TAURI}/target/release/bundle/appimage/revo-ui.AppDir"
APPIMAGE_OUTPUT="${REPO_ROOT}/revo-ui-x86_64.AppImage"

download_appimagetool() {
	mkdir -p "${APPIMAGE_CACHE}"

	if [[ -x "${APPIMAGETOOL}" ]]; then
		echo "[build-appimage] Using cached appimagetool:"
		"${APPIMAGETOOL}" --version 2>/dev/null || true
		return
	fi

	echo "[build-appimage] Downloading latest official appimagetool..."
	if ! command -v curl >/dev/null 2>&1; then
		echo "[build-appimage] ERROR: curl is required to download appimagetool." >&2
		exit 1
	fi

	curl -fL --retry 3 \
		"https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage" \
		-o "${APPIMAGETOOL}"

	chmod +x "${APPIMAGETOOL}"
	echo "[build-appimage] appimagetool ready:"
	"${APPIMAGETOOL}" --version 2>/dev/null || true
}

pkg_libdir() {
	pkg-config --variable=libdir "$1"
}

copy_glob() {
	local src_dir="$1"
	local pattern="$2"
	local dst="$3"

	[[ -d "${src_dir}" ]] || return 0

	shopt -s nullglob
	local files=( "${src_dir}"/${pattern} )
	shopt -u nullglob

	if (( ${#files[@]} )); then
		cp -aL "${files[@]}" "${dst}/"
	fi
}

# Do not bundle glibc/system loader libraries. These must come from the host.
is_system_library() {
	case "$(basename "$1")" in
		ld-linux*.so*|libc.so*|libm.so*|libpthread.so*|libdl.so*|librt.so*|libresolv.so*|libutil.so*|libnsl.so*|libcrypt.so*|libanl.so*|libgcc_s.so*)
			return 0
			;;
		*)
			return 1
			;;
	esac
}

declare -A COPIED_LIBS=()

copy_nix_library() {
	local lib="$1"
	[[ -f "${lib}" ]] || return 0
	[[ "${lib}" == /nix/store/* ]] || return 0
	is_system_library "${lib}" && return 0

	local base
	base="$(basename "${lib}")"

	if [[ -n "${COPIED_LIBS[${base}]:-}" ]]; then
		return 0
	fi

	COPIED_LIBS["${base}"]=1
	cp -aL "${lib}" "${APPDIR}/usr/lib/"
	echo "[build-appimage] + ${base}"
}

collect_ldd_deps() {
	local binary="$1"
	[[ -f "${binary}" ]] || return 0

	while IFS= read -r dep; do
		[[ -n "${dep}" ]] || continue
		copy_nix_library "${dep}"
	done < <(
		ldd "${binary}" 2>/dev/null |
		sed -n 's/.*=> \(\/nix\/store\/[^ ]*\).*/\1/p'
	)
}

collect_recursive_deps() {
	local pending=()
	local lib

	while IFS= read -r lib; do
		pending+=( "${lib}" )
	done < <(find "${APPDIR}/usr/lib" -maxdepth 1 -type f -name '*.so*' -print)

	while (( ${#pending[@]} )); do
		lib="${pending[0]}"
		pending=( "${pending[@]:1}" )

		[[ -f "${lib}" ]] || continue

		local before="${#COPIED_LIBS[@]}"
		collect_ldd_deps "${lib}"

		if (( ${#COPIED_LIBS[@]} > before )); then
			while IFS= read -r newlib; do
				pending+=( "${newlib}" )
			done < <(find "${APPDIR}/usr/lib" -maxdepth 1 -type f -name '*.so*' -print)
		fi
	done
}

create_desktop_file() {
    local desktop_file="${APPDIR}/revo-ui.desktop"
    local icon_file="${APPDIR}/revo-ui.png"

    if [[ ! -f "${desktop_file}" ]]; then
        echo "[build-appimage] Creating ${desktop_file}"

        cat > "${desktop_file}" <<'EOF'
[Desktop Entry]
Name=Revo UI
Comment=RevoStream
Exec=revo-ui
Icon=revo-ui
Terminal=false
Type=Application
Categories=AudioVideo;Network;
StartupWMClass=revo-ui
EOF
    fi

    # Tauri already installed the icon in the hicolor tree.
    # appimagetool also accepts an icon next to the desktop file.
    if [[ ! -f "${icon_file}" ]]; then
        local source_icon="${APPDIR}/usr/share/icons/hicolor/256x256@2/apps/revo-ui.png"

        if [[ -f "${source_icon}" ]]; then
            cp -a "${source_icon}" "${icon_file}"
            echo "[build-appimage] Installed AppImage icon: ${icon_file}"
        else
            echo "[build-appimage] WARNING: Revo icon not found: ${source_icon}"
        fi
    fi
}

create_apprun() {
    local apprun="${APPDIR}/AppRun"

    if [[ -x "${apprun}" ]]; then
        echo "[build-appimage] AppRun already exists."
        return
    fi

    echo "[build-appimage] Creating ${apprun}"

    cat > "${apprun}" <<'EOF'
#!/bin/sh
HERE="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"

export LD_LIBRARY_PATH="$HERE/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

exec "$HERE/usr/bin/revo-ui" "$@"
EOF

    chmod +x "${apprun}"
}

collect_runtime_libraries() {
	echo "[build-appimage] Collecting Nix runtime libraries..."

	mkdir -p "${APPDIR}/usr/lib"

	# Tauri application itself.
	collect_ldd_deps "${SRC_TAURI}/target/release/revo-ui"

	# Libraries loaded dynamically by WebKit/GTK/GIO and therefore not
	# necessarily visible in the application's direct ldd output.
	local gtk_libdir glib_libdir gdk_pixbuf_libdir cairo_libdir
	local dbus_libdir libsoup_libdir webkit_libdir jsc_libdir

	gtk_libdir="$(pkg_libdir gtk+-3.0)"
	glib_libdir="$(pkg_libdir glib-2.0)"
	gdk_pixbuf_libdir="$(pkg_libdir gdk-pixbuf-2.0)"
	cairo_libdir="$(pkg_libdir cairo)"
	dbus_libdir="$(pkg_libdir dbus-1)"
	libsoup_libdir="$(pkg_libdir libsoup-3.0)"
	webkit_libdir="$(pkg_libdir webkit2gtk-4.1)"
	jsc_libdir="$(pkg_libdir javascriptcoregtk-4.1)"

	copy_glob "${gtk_libdir}" "libgdk-3.so*" "${APPDIR}/usr/lib"
	copy_glob "${gtk_libdir}" "libgtk-3.so*" "${APPDIR}/usr/lib"

	copy_glob "${glib_libdir}" "libglib-2.0.so*" "${APPDIR}/usr/lib"
	copy_glob "${glib_libdir}" "libgobject-2.0.so*" "${APPDIR}/usr/lib"
	copy_glob "${glib_libdir}" "libgio-2.0.so*" "${APPDIR}/usr/lib"
	copy_glob "${glib_libdir}" "libgmodule-2.0.so*" "${APPDIR}/usr/lib"

	copy_glob "${gdk_pixbuf_libdir}" "libgdk_pixbuf-2.0.so*" "${APPDIR}/usr/lib"
	copy_glob "${cairo_libdir}" "libcairo.so*" "${APPDIR}/usr/lib"
	copy_glob "${dbus_libdir}" "libdbus-1.so*" "${APPDIR}/usr/lib"

	copy_glob "${libsoup_libdir}" "libsoup-3.0.so*" "${APPDIR}/usr/lib"
	copy_glob "${webkit_libdir}" "libwebkit2gtk-4.1.so*" "${APPDIR}/usr/lib"
	copy_glob "${jsc_libdir}" "libjavascriptcoregtk-4.1.so*" "${APPDIR}/usr/lib"

	# OBS and locally built libraries may be referenced from the AppDir.
	if [[ -d "${REPO_ROOT}/core/lib" ]]; then
		while IFS= read -r lib; do
			collect_ldd_deps "${lib}"
		done < <(find "${REPO_ROOT}/core/lib" -type f -name '*.so*' -print)
	fi

	# Collect the complete Nix-store dependency closure of everything
	# currently copied into AppDir/usr/lib.
	collect_recursive_deps

	echo "[build-appimage] Bundled $(find "${APPDIR}/usr/lib" -maxdepth 1 -type f -name '*.so*' | wc -l) ELF libraries."
}

# Keep runtime resolution consistent with local core build (avoid system libobs fallback)
export LD_LIBRARY_PATH="${REPO_ROOT}/core/lib:${SRC_TAURI}/data/lib:${LD_LIBRARY_PATH:-}"
export PATH="${REPO_ROOT}/core/bin:${PATH}"

rm -rf "${RES_ROOT}"
mkdir -p "${RES_ROOT}"
touch "${RES_ROOT}/.keep"

if [[ -d "${REPO_ROOT}/data" ]]; then
	rsync -a --delete \
		"${REPO_ROOT}/data/" "${RES_ROOT}/data/"
fi

if [[ -d "${REPO_ROOT}/core/lib" ]]; then
	rsync -a --delete "${REPO_ROOT}/core/lib/" "${RES_ROOT}/lib/"
fi

if [[ -d "${REPO_ROOT}/core/share" ]]; then
	rsync -a --delete "${REPO_ROOT}/core/share/" "${RES_ROOT}/share/"
fi

if [[ -d "${REPO_ROOT}/core/bin" ]]; then
	rsync -a --delete "${REPO_ROOT}/core/bin/" "${RES_ROOT}/bin/"
fi

download_appimagetool

if should_enable_egl_workaround; then
	echo "[build-appimage] Enabling EGL workaround flags (software GL + DMABUF disable)."
	REVO_FORCE_SOFTWARE_GL=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 NO_STRIP=true pnpm tauri build --bundles appimage || {
		echo "[build-appimage] Tauri returned non-zero; checking whether AppDir was produced..."
	}
else
	echo "[build-appimage] EGL workaround flags not required on this system."
	NO_STRIP=true pnpm tauri build --bundles appimage || {
		echo "[build-appimage] Tauri returned non-zero; checking whether AppDir was produced..."
	}
fi

if [[ ! -d "${APPDIR}" ]]; then
	echo "[build-appimage] ERROR: AppDir was not produced: ${APPDIR}" >&2
	exit 1
fi

if [[ ! -x "${APPDIR}/usr/bin/revo-ui" ]]; then
    echo "[build-appimage] ERROR: ${APPDIR}/usr/bin/revo-ui is missing." >&2
    exit 1
fi

create_desktop_file
create_apprun
collect_runtime_libraries


echo "[build-appimage] Packaging ${APPDIR}"
rm -f "${APPIMAGE_OUTPUT}"

APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 "${APPIMAGETOOL}" 	"${APPDIR}" 	"${APPIMAGE_OUTPUT}"

echo "[build-appimage] Success: ${APPIMAGE_OUTPUT}"
