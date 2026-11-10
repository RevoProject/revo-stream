from pathlib import Path
root = Path.home() / ".cargo/git/checkouts"
# Every fetched revision is patched: the workspace and the extracted backend
# can pin different libobs-rs revisions, so a hard-coded commit would silently
# skip the one that actually gets compiled.
matches = sorted(set(root.glob("libobs-rs-*/[0-9a-f]*/libobs/build.rs")))
if not matches:
    raise SystemExit("libobs build.rs not found")


def patch(path):
    text = path.read_text()
    print(f"Patching libobs build.rs: {path}")
    if "builder = builder.layout_tests(false);" in text:
        print("    libobs bindgen patch already present")
        return
    needle = (
        '        .clang_arg(format!("-I{}", "headers/obs"));'
    )
    if needle not in text:
        raise SystemExit(
            "Expected clang_arg line not found in libobs/build.rs"
        )
    replacement = (
        needle
        + "\n\n"
        + '    #[cfg(target_os = "macos")]'
        + "\n"
        + "    {"
        + "\n"
        + "        builder = builder.layout_tests(false);"
        + "\n"
        + "    }"
    )
    path.write_text(
        text.replace(
            needle,
            replacement,
            1
        )
    )
    print("libobs bindgen macOS patch applied")


for path in matches:
    patch(path)
