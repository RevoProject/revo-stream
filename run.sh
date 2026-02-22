#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

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

# Keep libobs ABI aligned with plugins from ../core/lib/obs-plugins
export LD_LIBRARY_PATH="${REPO_ROOT}/core/lib:${SCRIPT_DIR}/src-tauri/data/lib:${LD_LIBRARY_PATH:-}"
export PATH="${REPO_ROOT}/core/bin:${PATH}"

if should_enable_egl_workaround; then
	echo "[run] Enabling EGL workaround flags (software GL + DMABUF disable)."
	REVO_FORCE_SOFTWARE_GL=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 pnpm tauri dev
else
	echo "[run] EGL workaround flags not required on this system."
	pnpm tauri dev
fi
