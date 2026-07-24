#!/usr/bin/env bash
# Pakuje binarke desktop-app w jeden plik .sh (self-extracting archive)
# Uzycie: bash scripts/nix/package-bundle.sh [sciezka-do-binarki]
# Wynik: iTVT-2.0.0-x86_64-linux.run (jeden plik do dystrybucji)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
BINARY="${1:-$PROJECT_DIR/src-tauri/target/release/desktop-app}"

if [ ! -f "$BINARY" ]; then
  echo "Blad: binarka nie znaleziona w $BINARY"
  echo "Najpierw zbuduj: cd $PROJECT_DIR && bash scripts/nix/compile.sh"
  exit 1
fi

VERSION="$(node -p "require('$PROJECT_DIR/package.json').version")-nixos"
ARCH="$(uname -m)"
OUTPUT="$PROJECT_DIR/iTVT-${VERSION}-${ARCH}-linux.run"

echo "=== Pakuje ($(stat -c%s "$BINARY") bajtow) ==="

BOUNDARY="__ITVT_ARCHIVE__"

cat > "$OUTPUT" << 'EOF'
#!/usr/bin/env bash
# iTVT AppBundle — self-extracting one-file launcher for NixOS

set -euo pipefail
B="$0"
TMP_DIR=""
EXTRACT_ONLY=0

if [ "${1:-}" = "--extract" ]; then
  EXTRACT_ONLY=1
  TMP_DIR="${2:-/tmp/itvt-extract}"
else
  TMP_DIR="$(mktemp -d)"
  trap 'rm -rf "$TMP_DIR"' EXIT
fi

# Find and extract embedded archive
LINE=$(grep -n "^__ITVT_ARCHIVE__$" "$B" | tail -1 | cut -d: -f1)
LINE=$((LINE + 1))
mkdir -p "$TMP_DIR"
tail -n +$LINE "$B" | base64 -d | gzip -d > "$TMP_DIR/desktop-app" 2>/dev/null || {
  echo "Blad wyodrebniania"
  exit 1
}
chmod +x "$TMP_DIR/desktop-app"

[ $EXTRACT_ONLY -eq 1 ] && { echo "Wyodrebniono do $TMP_DIR/desktop-app"; exit 0; }

# GStreamer env — nix eval first, then fallback to NixOS system paths
GL=""
for p in gst_all_1.gstreamer gst_all_1.gst-plugins-base gst_all_1.gst-plugins-good \
         gst_all_1.gst-plugins-bad gst_all_1.gst-plugins-ugly gst_all_1.gst-libav; do
  d="$(nix eval --impure "nixpkgs#$p" --raw 2>/dev/null || true)"
  [ -n "$d" ] && [ -d "$d/lib/gstreamer-1.0" ] && GL="${GL:+$GL:}$d/lib/gstreamer-1.0"
done
for d in /run/current-system/sw/lib/gstreamer-1.0 \
         /nix/var/nix/profiles/default/lib/gstreamer-1.0 \
         ~/.nix-profile/lib/gstreamer-1.0; do
  [ -d "$d" ] && GL="${GL:+$GL:}$d"
done

export GST_PLUGIN_SYSTEM_PATH="$GL"
export GST_PLUGIN_PATH="$GL"
export GST_REGISTRY_REUSE_PLUGIN_SCANNER="no"
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_USE_GL=software

# GPU detection
GPU="$(lspci 2>/dev/null | grep -iE "vga|3d|display" | grep -ioE "intel|nvidia|amd" | head -1 || echo "auto")"
case "$GPU" in
  nvidia) FLAKE="github:nix-community/nixGL#nixGLNVIDIA" ;;
  amd)    FLAKE="github:nix-community/nixGL#nixGLAMD" ;;
  *)      FLAKE="github:nix-community/nixGL" ;;
esac

exec nix run --impure "$FLAKE" -- "$TMP_DIR/desktop-app"
EOF

echo "$BOUNDARY" >> "$OUTPUT"
gzip -c "$BINARY" | base64 -w 72 >> "$OUTPUT"
echo "" >> "$OUTPUT"

chmod +x "$OUTPUT"
echo "=== Gotowe: $OUTPUT ==="
echo "    Uruchom: ./$(basename "$OUTPUT")"
