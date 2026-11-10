"""Make a checked-out libobs-wrapper buildable on macOS.

`revo-lib` tracks `libobs-rs` `branch = "main"`, so the revision that gets
compiled comes from the lockfiles and can differ between the workspace and the
extracted backend. Older revisions Linux-gated `Rc`, the platform guard and a
display-less `platform_specific_setup`, none of which compile on macOS; newer
upstreams fixed all three. Every fetched checkout is therefore either verified
as already macOS-ready or patched, and a checkout matching neither shape is
reported and skipped rather than aborting a build that does not compile it.
"""

from pathlib import Path
import sys

from libobs_pins import pinned_revisions

LINUX_RC = '#[cfg(target_os = "linux")]\nuse std::rc::Rc;'
UNCONDITIONAL_RC = "use std::rc::Rc;"
NOT_LINUX = '#[cfg(not(target_os = "linux"))]'
GUARD_STRUCT = "pub(crate) struct PlatformSpecificGuard {"
GUARD_STUB = "pub(crate) struct PlatformSpecificGuard;"
GUARD_DROP = "impl Drop for PlatformSpecificGuard {"
LINUX = '#[cfg(target_os = "linux")]'
SETUP_WITH_ARG = "platform_specific_setup(info.nix_display.clone())?"
SETUP_PATCHED = "platform_specific_setup()?"
SETUP_UPSTREAM_ARG = "platform_specific_setup(info.nix_display.clone())"

root = Path.home() / ".cargo/git/checkouts"
matches = sorted(set(root.glob("libobs-rs-*/[0-9a-f]*/libobs-wrapper/src")))
if not matches:
    raise SystemExit("libobs-wrapper source not found")


def has_non_linux_guard(text):
    """True when a non-Linux `PlatformSpecificGuard` is declared."""
    lines = text.splitlines()
    for index, line in enumerate(lines):
        if line.strip() != GUARD_STUB:
            continue
        if any(NOT_LINUX in candidate for candidate in lines[max(0, index - 3):index]):
            return True
    return False


def patch(src):
    other = src / "utils/initialization/other.rs"
    runtime = src / "runtime.rs"
    if not other.exists() or not runtime.exists():
        print(f"  unexpected layout, skipping: {src}", file=sys.stderr)
        return False

    print(f"libobs-wrapper checkout: {src}")
    text = other.read_text()

    # --------------------------------------------------------
    # Rc must be in scope on macOS.
    # --------------------------------------------------------
    if LINUX_RC in text:
        text = text.replace(LINUX_RC, UNCONDITIONAL_RC, 1)
        print("  removed the Linux-only cfg from the Rc import")
    elif UNCONDITIONAL_RC not in text:
        print(f"  no Rc import to un-gate, skipping: {src}", file=sys.stderr)
        return False
    else:
        print("  Rc import is already unconditional")

    # --------------------------------------------------------
    # The platform guard must exist on macOS, and the Linux
    # variant must keep its display fields, Drop impl and setup body.
    # --------------------------------------------------------
    lines = text.splitlines()
    struct_index = next(
        (index for index, line in enumerate(lines) if line.strip() == GUARD_STRUCT),
        None,
    )
    if struct_index is None:
        if not has_non_linux_guard("\n".join(lines)):
            print(f"  no PlatformSpecificGuard found, skipping: {src}", file=sys.stderr)
            return False
        print("  PlatformSpecificGuard already has a non-Linux stub")
    else:
        if lines[struct_index - 1].strip() != LINUX:
            insert_at = struct_index
            if lines[insert_at - 1].strip() == "#[derive(Debug)]":
                insert_at -= 1
            lines.insert(insert_at, LINUX)
        struct_index = next(
            index for index, line in enumerate(lines) if line.strip() == GUARD_STRUCT
        )
        depth, started, end_index = 0, False, None
        for index in range(struct_index, len(lines)):
            line = lines[index]
            depth += line.count("{")
            if "{" in line:
                started = True
            if started and "}" in line:
                depth -= line.count("}")
                if depth == 0:
                    end_index = index
                    break
        if end_index is None:
            print(
                f"  unterminated PlatformSpecificGuard, skipping: {src}",
                file=sys.stderr,
            )
            return False
        if has_non_linux_guard("\n".join(lines)):
            print("  PlatformSpecificGuard already has a non-Linux stub")
        else:
            lines[end_index + 1:end_index + 1] = [
                "",
                NOT_LINUX,
                "#[derive(Debug)]",
                GUARD_STUB,
            ]
            print("  added the non-Linux PlatformSpecificGuard stub")
        for index, line in enumerate(lines):
            if line.strip() == GUARD_DROP:
                if lines[index - 1].strip() != LINUX:
                    lines.insert(index, LINUX)
                break

    other.write_text("\n".join(lines) + "\n")

    # --------------------------------------------------------
    # The setup call site has to match the setup definition.
    # --------------------------------------------------------
    runtime_text = runtime.read_text()
    if SETUP_WITH_ARG in runtime_text:
        runtime.write_text(runtime_text.replace(SETUP_WITH_ARG, SETUP_PATCHED, 1))
        print("  dropped the nix_display argument from the macOS setup call")
    elif SETUP_PATCHED in runtime_text:
        print("  macOS setup call already patched")
    elif SETUP_UPSTREAM_ARG in runtime_text:
        print("  upstream setup call already takes the display argument")
    else:
        print(
            f"  unrecognised platform_specific_setup call, skipping: {src}",
            file=sys.stderr,
        )
        return False
    return True


handled = {src.parent.parent.name for src in matches if patch(src)}

# A revision fetched for another lockfile may be skipped, but the revision this
# build compiles has to end up macOS-ready.
missing = [
    revision
    for revision in pinned_revisions()
    if any(str(src).endswith(f"/{revision}/libobs-wrapper/src") for src in matches)
    and revision not in handled
]
if missing:
    raise SystemExit(
        "the pinned libobs-rs revision could not be made macOS-ready: "
        + ", ".join(missing)
    )
print(f"libobs-wrapper ready on {len(handled)}/{len(matches)} checkout(s)")
