from pathlib import Path
path = Path("src-tauri/Cargo.toml")
text = path.read_text()
text = text.replace(
    'cef = ["dep:revo-cef"]',
    'cef = []'
)
text = text.replace(
    'revo-cef = { path = "../../modules/revo-cef", optional = true }',
    ''
)
path.write_text(text)
print("CEF disabled for CI")
