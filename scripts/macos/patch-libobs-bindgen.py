"""Disable bindgen layout tests for the macOS OBS bindings.

The workspace (`src-tauri`) and the extracted backend (`crates/revo-backend`)
can pin different libobs-rs revisions, so every fetched checkout is inspected
instead of a hard-coded commit. Newer upstream revisions already disable layout
tests, and rewriting those would inject a statement into the middle of a builder
chain, so an already-correct checkout is left untouched.
"""

from pathlib import Path
import sys

from libobs_pins import pinned_revisions

root = Path.home() / ".cargo/git/checkouts"
matches = sorted(set(root.glob("libobs-rs-*/[0-9a-f]*/libobs/build.rs")))
if not matches:
    raise SystemExit("libobs build.rs not found")

NEEDLE = '        .clang_arg(format!("-I{}", "headers/obs"));'
ALREADY = (
    ".layout_tests(false)",
    "builder = builder.layout_tests(false);",
)
INSERTION = (
    NEEDLE
    + "\n\n"
    + '    #[cfg(target_os = "macos")]\n'
    + "    {\n"
    + "        builder = builder.layout_tests(false);\n"
    + "    }"
)


def patch(path):
    """True once the checkout disables bindgen layout tests on macOS."""
    text = path.read_text()
    if any(marker in text for marker in ALREADY):
        print(f"  already correct: {path}")
        return True
    if NEEDLE not in text:
        print(f"  unrecognised build.rs layout, skipping: {path}", file=sys.stderr)
        return False
    path.write_text(text.replace(NEEDLE, INSERTION, 1))
    print(f"  macOS patch applied: {path}")
    return True


handled = {path.parent.parent.name for path in matches if patch(path)}

# A different revision in another lockfile is fine to skip, but the revision
# this build actually compiles has to be handled.
missing = [
    revision
    for revision in pinned_revisions()
    if any(str(path).endswith(f"/{revision}/libobs/build.rs") for path in matches)
    and revision not in handled
]
if missing:
    raise SystemExit(
        "the pinned libobs-rs revision could not be patched for macOS: "
        + ", ".join(missing)
    )
