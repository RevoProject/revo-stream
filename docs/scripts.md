# Scripts

The GPUI application (`revo-gpui` + `revo-engine`) is the primary build target.
Legacy Tauri/Svelte scripts are listed separately at the end.

## Core validation (`scripts/core/`)

- `verify.sh` — standard validation gate for the framework-neutral crates
  (`revo-core`, `revo-gpui`): whitespace check, `cargo fmt --check`, core and
  GPUI tests, GPUI `cargo check`, and Tauri compatibility metadata. Requires no
  GTK/WebKit/libobs host packages. Run after every core slice:
  `bash scripts/core/verify.sh`

Platform equivalents:

- `scripts/macos/verify-core.sh` — runs `scripts/core/verify.sh` on macOS
  (GPUI builds natively via Metal).
- `scripts/windows/verify-core.ps1` — PowerShell equivalent for Windows
  (without the Linux pkg-config metadata step).

## Native GPUI development on NixOS (`scripts/nix/`)

- `dev.sh` — run the native GPUI shell (default; delegates to `run-gpui.sh`),
  the Tauri backend without UI (`dev.sh tauri`), or the legacy frontend
  (`dev.sh tauri-frontend`).
- `run-gpui.sh` — the verified GPUI launcher: resolves the X11 client libraries
  and the Vulkan loader with nix, extends `LD_LIBRARY_PATH`, stages the
  `revo-root` OBS layout, and runs the shell with the host NVIDIA driver stack
  (works on the NVIDIA container runtime; requires a desktop session with
  `DISPLAY` set). GPU rendering uses default Vulkan device discovery — do not
  set `VK_ICD_FILENAMES`.
  With `REVO_GPUI_OBS=1` the launcher builds `revo-engine` inside `shell.nix`
  and starts the UI with `--obs`:
  `REVO_GPUI_OBS=1 bash scripts/nix/run-gpui.sh`
- `stage-obs-root.sh` — stage the shared `.local/revo-root` OBS runtime layout
  from the Nix store and print the resolved `obs-studio` store path.
- `compile.sh` — build the native GPUI release binary (default) or the legacy
  Tauri deb + rpm bundles (`bash scripts/nix/compile.sh tauri`).
- `run.sh` — run a built binary: `gpui` (default,
  `crates/revo-gpui/target/release/revo-gpui`), `tauri`, or an explicit path,
  with per-binary staleness checks and Nix-provided media dependencies.
- `all.sh` — full pipeline: compile (gpui by default) -> package `.run` ->
  clean. `all.sh tauri` runs the same pipeline for the legacy bundles.
- `package-bundle.sh` — package a built GPUI or Tauri binary into a
  self-extracting `.run` archive (`gpui` by default); the output is
  `RevoStream-<version>-nixos-<arch>-linux.run` at the repository root.
- `repair-debug-profile.sh` — repair an interrupted Cargo debug profile;
  accepts an optional `Cargo.toml` path.
- `debug.sh` — run the GPUI shell in debug mode (default); `tauri` and
  `embedded` run the legacy adapter.

## Linux packaging (`scripts/linux/`)

The staged tree is the single source of truth for every Linux artifact:

```text
revo-gpui-<version>-linux-x64/
  revo-gpui          native UI binary
  revo-engine        libobs engine process
  revo-root/         staged OBS runtime
  share/             desktop entry + hicolor icons
  launch.sh          sets REVO_ROOT and private library paths
  README.txt         host requirements
```

- `install-dependencies.sh` — install Linux build and packaging dependencies
  (Debian/Ubuntu).
- `build-obs.sh` — build the matching OBS runtime.
- `prepare-runtime.sh [OBS_INSTALL]` — stage OBS libraries, plugins, and helper
  dependencies into `src-tauri/resources/revo-root`.
- `package-gpui.sh [OBS_INSTALL]` — build the portable tree and emit
  `revo-gpui-<version>-linux-x64-portable.tar.gz` plus a self-extracting
  `.run` (when makeself is available).
- `package-gpui-bundles.sh [STAGED_DIR]` — build DEB, RPM, and AppImage from
  that tree. Package dependencies are derived from the staged ELF closure, not
  a hand-maintained list: libraries resolved inside `revo-root` are bundled,
  host libraries become declared dependencies.
- `verify-gpui-release.sh` — verify all five GPUI artifacts, unpack them, check
  the staged layout, and write `*-linux-x64-SHA256SUMS.txt`.

Build the binaries before packaging:

```bash
cargo build --release --features obs-backend --manifest-path crates/revo-gpui/Cargo.toml
cargo build --release --bin revo-engine --manifest-path crates/revo-backend/Cargo.toml
```

## Windows packaging (`scripts/windows/`)

- `prepare-revo-root.ps1` — prepare the `revo-root` directory skeleton and
  RevoStream data.
- `patch-libobs-bootstrapper.ps1` — patch the libobs bootstrapper dummy-DLL
  feature for the CI build.
- `verify-obs-runtime.ps1` — verify the bootstrapped OBS runtime.
- `prepare-runtime.ps1` — stage OBS DLLs, plugins, and data into
  `src-tauri/resources/revo-root`.
- `verify-runtime.ps1` — verify the staged Windows runtime.
- `package-gpui.ps1 [-SkipMsi]` — package the UI + engine + runtime into:
  - `revo-gpui-<version>-windows-x64.zip`
  - `revo-gpui-<version>-windows-x64.exe` (NSIS)
  - `revo-gpui-<version>-windows-x64.msi` (WiX)
- `verify-gpui-package.ps1` — check that every artifact exists, the portable ZIP
  extracts to a complete tree, and both binaries are x64 PE images.
- `smoke-test-gpui.ps1` — start the packaged `revo-engine`, wait for its
  loopback endpoint, connect, and verify the boot snapshot. The UI is not
  started because it needs a GPU-backed desktop session.

Windows is partially supported: the GPUI interface runs, but core libobs
functionality may not work.

## macOS packaging (`scripts/macos/`)

- `populate-revo-root.sh` — stage the OBS runtime and RevoStream data into
  `src-tauri/resources/revo-root` (requires `REVO_TARGET`, plus optional
  `REVO_OBS_BUILD` / `REVO_OBS_INSTALL`).
- `package-gpui-app.sh` — assemble `RevoStream.app` from the UI binary,
  `revo-engine`, and `Contents/Resources/resources/revo-root` (requires
  `REVO_TARGET` and `REVO_ARCH`; optional `REVO_ROOT_STAGED`). It writes to the
  location the Tauri bundler used, so the shared post-processing scripts keep
  working.
- `ensure-app-resources.sh`, `normalize-app-dylib-paths.sh`,
  `codesign-app.sh`, `verify-app.sh` — post-process the `.app`:
  bundle resources, rewrite dylib paths, sign, and verify it.
- `build-dmg.sh` — build the DMG (requires `REVO_TARGET` and `REVO_ARCH`;
  `REVO_APP_EXTRA_BINARIES=revo-engine` makes `verify-app.sh` check the bundled
  engine as well).
- `smoke-test-gpui.sh <path-to-.app>` — validate the bundle layout,
  `Info.plist`, and signature, then start the bundled engine and check its
  endpoint handshake.

The OBS build and runtime helpers (`checkout-obs.sh`, `configure-obs.sh`,
`install-obs.sh`, `audit-runtime-dependencies.sh`, `verify-*`) support the
release workflow. macOS is experimental until the libobs hooks are verified on
real hardware.

## Release workflow

`.github/workflows/release.yml` builds the GPUI UI and engine for every
platform:

- **Linux:** install dependencies -> build OBS -> stage runtime -> check the
  legacy Tauri adapter -> build GPUI + engine -> `package-gpui.sh` ->
  `package-gpui-bundles.sh` -> `verify-gpui-release.sh`.
- **Windows:** stage runtime -> bootstrap OBS -> build GPUI + engine ->
  `package-gpui.ps1` -> `verify-gpui-package.ps1` ->
  `smoke-test-gpui.ps1`.
- **macOS:** build/audit OBS -> build GPUI + engine -> `package-gpui-app.sh` ->
  post-processing -> `smoke-test-gpui.sh` -> `build-dmg.sh`.

## Legacy Tauri/Svelte scripts

These remain for parity checks until final cutover and are not part of the GPUI
release pipeline:

- `run.sh` — run the Tauri app in development mode (`pnpm tauri dev`).
- `run-cef.sh` — run the Tauri app with the `cef` feature after validating
  `CEF_ROOT` or `CEF_INCLUDE_DIR`/`CEF_LIB_DIR`.
- `clean.sh` — remove `src-tauri/target` and `.svelte-kit`.
- `scripts/appimage/build-appimage.sh` — build the legacy Tauri AppImage
  (`pnpm tauri build --bundles appimage`).
- `scripts/linux/verify-release.sh`, `scripts/linux/package-portable.sh` —
  verify/package the Tauri bundle layout under `src-tauri/target/release/bundle`
  (the GPUI equivalents above use `crates/revo-gpui/target/release/bundle`).
- `scripts/windows/create-portable-zip.ps1`, `verify-msi.ps1`,
  `smoke-test-msi.ps1`, `smoke-test-portable.ps1` — legacy Windows Tauri
  artifact checks.
