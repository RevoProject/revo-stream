#!/usr/bin/env bash
# Smoke test for the packaged GPUI app bundle.
#
# The engine process is started from inside the bundle and must publish a local
# endpoint, accept a connection, push its boot snapshot and answer a snapshot
# request. The UI process is not started: it needs a GPU-backed window session,
# which a CI runner does not provide, so its checks stay structural (bundle
# layout, Info.plist, signature).
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

python3 - "$ENDPOINT" "$ENGINE_PID" << 'PY'
import json, os, socket, struct, sys

endpoint, engine_pid = sys.argv[1], int(sys.argv[2])

if endpoint.startswith("unix:"):
    client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    client.connect(endpoint[len("unix:"):])
elif endpoint.startswith("tcp:"):
    host, _, port = endpoint[len("tcp:"):].rpartition(":")
    client = socket.create_connection((host, int(port)), timeout=10)
else:
    raise SystemExit(f"unrecognised engine endpoint: {endpoint}")
client.settimeout(20)


def read_frame():
    header = b""
    while len(header) < 4:
        chunk = client.recv(4 - len(header))
        if not chunk:
            raise SystemExit("engine closed the stream mid-header")
        header += chunk
    length = struct.unpack("<I", header)[0]
    payload = b""
    while len(payload) < length:
        chunk = client.recv(min(65536, length - len(payload)))
        if not chunk:
            raise SystemExit("engine closed the stream mid-payload")
        payload += chunk
    return json.loads(payload.decode())


def send(message):
    payload = json.dumps(message).encode()
    client.sendall(struct.pack("<I", len(payload)) + payload)


boot = read_frame()
if "Snapshot" not in boot:
    raise SystemExit(f"boot frame is not a snapshot: {boot}")
print(f"  boot snapshot received (obs_running={boot['Snapshot']['obs_running']})")

send("RequestSnapshot")
reply = read_frame()
if "Snapshot" not in reply:
    raise SystemExit(f"snapshot reply is not a snapshot: {reply}")
print("  snapshot round-trip ok")

os.kill(engine_pid, 0)
print("  engine alive while the client is connected")
client.close()
PY

# The engine serves one client and exits when that client disconnects, so a
# clean exit here is the expected outcome, not a failure.
for _ in $(seq 1 100); do
  if ! kill -0 "$ENGINE_PID" 2>/dev/null; then break; fi
  sleep 0.1
done
if kill -0 "$ENGINE_PID" 2>/dev/null; then
  fail "engine stayed alive after the client disconnected"
fi
echo "  engine exited cleanly on disconnect"
echo "GPUI macOS bundle smoke test passed"
