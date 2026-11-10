"""Which libobs-rs revision is actually compiled?

`revo-lib` depends on `libobs-rs` `branch = "main"` with no rev, so the answer
is not in the manifests — it is in the lockfiles. The release pipeline patches
checked-out `libobs-rs` sources, so it has to assert that the pinned revision
was handled instead of patching whatever happened to be fetched.
"""

import re
from pathlib import Path

LOCK_FILES = (
    "src-tauri/Cargo.lock",
    "crates/revo-backend/Cargo.lock",
    "crates/revo-gpui/Cargo.lock",
)

PATTERN = re.compile(
    r"git\+https://github\.com/libobs-rs/libobs-rs\?branch=main#([0-9a-f]{7,})"
)


def pinned_revisions(project_root=None):
    """Short revision ids (cargo checkout directory names) the locks pin."""
    root = Path.cwd() if project_root is None else project_root
    revisions: set[str] = set()
    for relative in LOCK_FILES:
        lock = root / relative
        if not lock.is_file():
            continue
        revisions.update(
            match.group(1)[:7] for match in PATTERN.finditer(lock.read_text())
        )
    return sorted(revisions)


def checkout_for(root, revision, relative):
    matches = sorted(set(root.glob(f"libobs-rs-*/{revision}/{relative}")))
    return matches[-1] if matches else None
