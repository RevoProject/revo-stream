from pathlib import Path
root = Path.home() / ".cargo/git/checkouts"
matches = list(
    root.glob("libobs-rs-*/7e76c91/libobs/build.rs")
)
if not matches:
    raise SystemExit(
        "libobs build.rs not found"
    )
path = matches[0]
text = path.read_text()
print(f"Using libobs build.rs: {path}")
if "builder = builder.layout_tests(false);" in text:
    print("libobs bindgen patch already present")
    raise SystemExit(0)
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
