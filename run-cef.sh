#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
TAURI_DIR="${SCRIPT_DIR}/src-tauri"

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

# Prefer explicit env from user.
CEF_INCLUDE_DIR="${CEF_INCLUDE_DIR:-}"
CEF_LIB_DIR="${CEF_LIB_DIR:-}"

if [[ -z "${CEF_INCLUDE_DIR}" || -z "${CEF_LIB_DIR}" ]]; then
  if [[ -n "${CEF_ROOT:-}" ]]; then
    CEF_INCLUDE_DIR="${CEF_INCLUDE_DIR:-${CEF_ROOT}/include}"
    CEF_LIB_DIR="${CEF_LIB_DIR:-${CEF_ROOT}/Release}"
  fi
fi

if [[ -z "${CEF_INCLUDE_DIR}" || -z "${CEF_LIB_DIR}" ]]; then
  echo "[run-cef] Missing CEF paths. Set CEF_ROOT or both CEF_INCLUDE_DIR and CEF_LIB_DIR."
  echo "[run-cef] Example: CEF_ROOT=/opt/cef_binary_..._linux64 ./run-cef.sh"
  exit 1
fi

if [[ ! -f "${CEF_INCLUDE_DIR}/cef_capi.h" ]]; then
  echo "[run-cef] Not found: ${CEF_INCLUDE_DIR}/cef_capi.h"
  exit 1
fi

if [[ ! -f "${CEF_LIB_DIR}/libcef.so" ]]; then
  echo "[run-cef] Not found: ${CEF_LIB_DIR}/libcef.so"
  exit 1
fi

export CEF_INCLUDE_DIR
export CEF_LIB_DIR
export LD_LIBRARY_PATH="${REPO_ROOT}/core/lib:${CEF_LIB_DIR}:${TAURI_DIR}/data/lib:${TAURI_DIR}/data/plugins:${LD_LIBRARY_PATH:-}"
export PATH="${REPO_ROOT}/core/bin:${PATH}"

if should_enable_egl_workaround; then
  echo "[run-cef] Enabling EGL workaround flags (software GL + DMABUF disable)."
  REVO_FORCE_SOFTWARE_GL=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 pnpm tauri dev --features cef
else
  echo "[run-cef] EGL workaround flags not required on this system."
  pnpm tauri dev --features cef
fi
