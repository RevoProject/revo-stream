# Troubleshooting — RevoStream

## Build: libobs / OBS runtime
**Symptoms:** linker errors, missing `libobs`, `obs.dll` not found.
**Check:**
- Installed dev packages (`libobs-dev`, OpenSSL, GLib).
- `docs/building.md` → platform section.
- Whether you use the real engine: `--features obs-backend -- --obs`.

## Build: missing GTK/WebKit (legacy Tauri)
**Symptoms:** errors with `cargo check --manifest-path src-tauri/Cargo.toml`.
**Note:** this is the legacy path. GPUI does not need these.

## Run: engine does not start
**Check:**
- Whether `revo-engine` is built.
- `REVO_ENGINE_SOCK` if connecting to an external engine.
- UI and engine logs.

## Windows: obs.dll not found
**Known:** Windows is partially supported.
**Check:**
- `launch.cmd` in the package.
- Runtime paths (`revo-root`).
- Whether you use a packaged release artifact.

## macOS: rpaths / signing issues
**Known:** macOS is experimental.
**Check:**
- `REVO_TARGET`, `REVO_ARCH`.
- Scripts in `scripts/macos/*`.
- Diagnostics workflow: `macos-diagnostics.yml`.

## UI: slow preview / low quality
**Known:** preview is currently base64 PNG.
**Check:**
- Preview quality setting.
- Real engine vs mock.
- GPU/driver (Vulkan loader on Linux).

## Tests: `verify.sh` fails
**Check:**
- `git diff --check` (whitespace).
- `cargo fmt --check` (formatting).
- No uncommitted formatting changes.

## Where to look next
- `docs/building.md`
- `docs/scripts.md`
- Issue templates (build/performance)