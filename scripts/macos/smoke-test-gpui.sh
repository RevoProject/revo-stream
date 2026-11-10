#!/usr/bin/env bash
# Smoke test for the packaged GPUI app bundle.
#
# The engine process is started from inside the bundle and must publish a local
# endpoint, accept a connection and push its boot snapshot. The UI process is
# not started: it needs a GPU-backed window session, which a CI runner does not
# provide, so its checks stay structural (bundle layout, Info.plist, signature).
set -euo pipefail

APP="${1:?Usage: smoke-test-gpui.sh <path-to-.app>}"
ENGINE="$APP/Contents/MacOS/revo-engine"
UI="$APP/Contents/MacOS/revo-gpui"
ROOT="$APP/Contents/Resources/resources/revo-root"

test -x "$UI" || { echo "ERROR: UI binary is missing: $UI"; exit 1; }
test -x "$ENGINE" || { echo "ERROR: engine binary is missing: $ENGINE"; exit 1; }
test -d "$ROOT/core/lib" || { echo "ERROR: bundled runtime is missing: $ROOT"; exit 1; }

echo "=== Checking the bundle metadata ==="
if command -v plutil >/dev/null 2>&1; then
  plutil -lint "$APP/Contents/Info.plist"
fi
executable="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$APP/Contents/Info.plist")"
[ "$executable" = "revo-gpui" ] || { echo "ERROR: CFBundleExecutable is $executable"; exit 1; }
/usr/libexec/PlistBuddy -c 'Print :NSCameraUsageDescription' "$APP/Contents/Info.plist" >/dev/null
/usr/libexec/PlistBuddy -c 'Print :NSMicrophoneUsageDescription' "$APP/Contents/Info.plist" >/dev/null

if command -v codesign >/dev/null 2>&1; then
  codesign --verify --verbose=1 "$APP" >/dev/null
  echo "  bundle signature verifies"
fi

echo "=== Starting the packaged engine ==="
WORKDIR="${RUNNER_TEMP:-$(mktemp -d)}/revo-gpui-smoke-$$"
mkdir -p "$WORKDIR/data"
ENDPOINT_FILE="$WORKDIR/engine.endpoint"
LOG="$WORKDIR/engine.log"

export REVO_ROOT="$ROOT"
export REVO_DATA_DIR="$WORKDIR/data"
export REVO_GRAPHICS_MODULE="${REVO_GRAPHICS_MODULE:-$ROOT/core/lib/libobs-opengl.dylib}"

"$ENGINE" --endpoint-file "$ENDPOINT_FILE" >"$LOG" 2>&1 &
ENGINE_PID=$!

fail() {
  echo "ERROR: $1" >&2
  echo "--- engine log ---" >&2
  tail -n 40 "$LOG" >&2 || true
  kill "$ENGINE_PID" 2>/dev/null || true
  wait "$ENGINE_PID" 2>/dev/null || true
  exit 1
}

cleanup() {
  kill "$ENGINE_PID" 2>/dev/null || true
  wait "$ENGINE_PID" 2>/dev/null || true
  rm -rf "$WORKDIR"
}
trap cleanup EXIT

for _ in $(seq 1 150); do
  if [ -s "$ENDPOINT_FILE" ]; then break; fi
  if ! kill -0 "$ENGINE_PID" 2>/dev/null; then fail "engine exited before publishing its endpoint"; fi
  sleep 0.2
done
[ -s "$ENDPOINT_FILE" ] || fail "engine never published its endpoint"

ENDPOINT="$(tr -d '[:space:]' < "$ENDPOINT_FILE")"
echo "  endpoint: $ENDPOINT"

python3 - "$ENDPOINT" << 'PY'
import socket, sys

endpoint = sys.argv[1]
if endpoint.startswith("unix:"):
    client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    client.connect(endpoint[len("unix:"):])
elif endpoint.startswith("tcp:"):
    host, _, port = endpoint[len("tcp:"):].rpartition(":")
    client = socket.create_connection((host, int(port)), timeout=10)
else:
    raise SystemExit(f"unrecognised engine endpoint: {endpoint}")

client.settimeout(15)
header = client.recv(4)
if len(header) != 4:
    raise SystemExit("engine did not send a frame header")
import struct
length = struct.unpack("<I", header)[0]
if length <= 0:
    raise SystemExit(f"engine boot snapshot frame is empty ({length} bytes)")
received = 0
while received < length:
    chunk = client.recv(min(65536, length - received))
    if not chunk:
        break
    received += len(chunk)
print(f"  boot snapshot frame received ({received} of {length} bytes)")
client.close()
PY

sleep 1
kill -0 "$ENGINE_PID" 2>/dev/null || fail "engine exited while the client was connected"
echo "  engine is serving"
echo "GPUI macOS bundle smoke test passed"
