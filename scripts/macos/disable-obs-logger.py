"""Guard the OBS native logger gating for macOS builds.

The logger used to live in the Tauri adapter, where CI patched it out here at
build time. It now lives in `revo-backend` and the gating is part of the
source, so this script only verifies that `obs_logger` and its
`base_set_log_handler` call site stay excluded on macOS. Any drift fails here
instead of surfacing as a confusing unresolved-module error later in the build.
"""

import sys
from pathlib import Path

LOGGING_MOD = Path("crates/revo-backend/src/logging/mod.rs")
INIT_MOD = Path("crates/revo-backend/src/runtime/init.rs")

BODY_WINDOW = 6


def balanced(text):
    return (
        text.count("(") == text.count(")")
        and text.count("[") == text.count("]")
    )


def cfg_gates(lines):
    """Yield `(gate_text, guarded_body_text)` for every `#[cfg(...)]` item."""
    index = 0
    while index < len(lines):
        stripped = lines[index].strip()
        if stripped.startswith("#[cfg("):
            collected = []
            cursor = index
            while cursor < len(lines):
                collected.append(lines[cursor].strip())
                joined = " ".join(collected)
                if balanced(joined):
                    body = [
                        line.strip()
                        for line in lines[cursor + 1 : cursor + 1 + BODY_WINDOW]
                        if line.strip()
                    ]
                    yield joined, " ".join(body)
                    index = cursor
                    break
                cursor += 1
        index += 1


def check(label, path, marker):
    if not path.exists():
        return [f"{label}: {path} not found — the OBS logger moved; update this guard"]
    lines = path.read_text().splitlines()
    for gate, body in cfg_gates(lines):
        if marker not in body:
            continue
        gate = " ".join(gate.split())
        if "macos" in gate:
            return [f"{label}: OBS logger is still enabled on macOS: {gate}"]
        print(f"{label} gate: {gate}")
        return []
    if any(marker in line for line in lines):
        return [f"{label}: `{marker}` is used in {path} without a cfg gate"]
    return [f"{label}: `{marker}` not found in {path}"]


errors = []
errors += check("module", LOGGING_MOD, "pub mod obs_logger;")
errors += check("call", INIT_MOD, "crate::logging::obs_logger::obs_log_handler")

if errors:
    for error in errors:
        print(f"ERROR: {error}", file=sys.stderr)
    sys.exit(1)

print("OBS logger is excluded on macOS")
