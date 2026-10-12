from pathlib import Path
root = Path.home() / ".cargo/git/checkouts"
matches = list(
    root.glob("libobs-rs-*/7e76c91/libobs-wrapper/src")
)
if not matches:
    raise SystemExit(
        "libobs-wrapper source not found"
    )
src = matches[0]
other = src / "utils/initialization/other.rs"
runtime = src / "runtime.rs"
if not other.exists():
    raise SystemExit(
        f"Missing file: {other}"
    )
if not runtime.exists():
    raise SystemExit(
        f"Missing file: {runtime}"
    )
print(f"Using libobs-wrapper source: {src}")
# --------------------------------------------------------
# Rc must exist on macOS.
# --------------------------------------------------------
text = other.read_text()
linux_rc = (
    '#[cfg(target_os = "linux")]\n'
    'use std::rc::Rc;'
)
if linux_rc in text:
    text = text.replace(
        linux_rc,
        'use std::rc::Rc;',
        1
    )
    print(
        "Removed Linux-only cfg from Rc import"
    )
elif 'use std::rc::Rc;' in text:
    print(
        "Rc import is already unconditional"
    )
else:
    raise SystemExit(
        "use std::rc::Rc; not found in libobs-wrapper"
    )
lines = text.splitlines()
# --------------------------------------------------------
# PlatformSpecificGuard
# --------------------------------------------------------
struct_index = None
for i, line in enumerate(lines):
    if (
        line.strip()
        == "pub(crate) struct PlatformSpecificGuard {"
    ):
        struct_index = i
        break
if struct_index is None:
    raise SystemExit(
        "PlatformSpecificGuard struct not found"
    )
if (
    struct_index == 0
    or lines[struct_index - 1].strip()
    != '#[cfg(target_os = "linux")]'
):
    derive_index = struct_index
    if (
        derive_index > 0
        and lines[derive_index - 1].strip()
        == "#[derive(Debug)]"
    ):
        derive_index -= 1
    lines.insert(
        derive_index,
        '#[cfg(target_os = "linux")]'
    )
# Re-find struct.
struct_index = None
for i, line in enumerate(lines):
    if (
        line.strip()
        == "pub(crate) struct PlatformSpecificGuard {"
    ):
        struct_index = i
        break
if struct_index is None:
    raise SystemExit(
        "PlatformSpecificGuard struct disappeared"
    )
depth = 0
end_index = None
started = False
for i in range(struct_index, len(lines)):
    line = lines[i]
    if "{" in line:
        depth += line.count("{")
        started = True
    if "}" in line and started:
        depth -= line.count("}")
        if depth == 0:
            end_index = i
            break
if end_index is None:
    raise SystemExit(
        "Could not find end of PlatformSpecificGuard"
    )
stub_exists = any(
    line.strip()
    == "pub(crate) struct PlatformSpecificGuard;"
    for line in lines
)
if not stub_exists:
    lines[end_index + 1:end_index + 1] = [
        "",
        '#[cfg(not(target_os = "linux"))]',
        "#[derive(Debug)]",
        "pub(crate) struct PlatformSpecificGuard;",
    ]
# --------------------------------------------------------
# Drop is Linux-only.
# --------------------------------------------------------
for i, line in enumerate(lines):
    if (
        line.strip()
        == "impl Drop for PlatformSpecificGuard {"
    ):
        if (
            i == 0
            or lines[i - 1].strip()
            != '#[cfg(target_os = "linux")]'
        ):
            lines.insert(
                i,
                '#[cfg(target_os = "linux")]'
            )
        break
other.write_text(
    "\n".join(lines) + "\n"
)
# --------------------------------------------------------
# runtime.rs
# --------------------------------------------------------
runtime_text = runtime.read_text()
old = (
    "platform_specific_setup(info.nix_display.clone())?"
)
new = "platform_specific_setup()?"
if old in runtime_text:
    runtime_text = runtime_text.replace(
        old,
        new,
        1
    )
    runtime.write_text(runtime_text)
    print(
        "Removed nix_display argument for macOS"
    )
elif new in runtime_text:
    print(
        "macOS platform_specific_setup call already patched"
    )
else:
    raise SystemExit(
        "Expected platform_specific_setup call not found"
    )
print(
    "libobs-wrapper macOS patch applied"
)
