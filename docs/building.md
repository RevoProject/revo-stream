# Building RevoStream

RevoStream builds as **two native Rust binaries**:

| Binary | Crate | Role |
| --- | --- | --- |
| `revo-gpui` | `crates/revo-gpui` | GPUI desktop UI (no webview, no JS) |
| `revo-engine` | `crates/revo-backend` | Dedicated process owning OBS/libobs |

The UI spawns `revo-engine` as a sibling process and talks to it over a typed,
length-prefixed JSON protocol (Unix socket on Unix, loopback TCP on Windows).
It can also connect to an already running engine through
`REVO_ENGINE_SOCK=unix:<path>` or `REVO_ENGINE_SOCK=tcp:127.0.0.1:<port>`.

There is no root Cargo workspace. Build each crate by manifest path. Node.js,
`pnpm`, WebKitGTK, and Tauri are **not** required for the GPUI application; they
are only needed for the legacy Tauri/Svelte compatibility path described at the
end of this document.

- [Linux (supported)](#linux-supported)
- [NixOS](#nixos)
- [Windows (partially supported)](#windows-partially-supported)
- [macOS (experimental)](#macos-experimental)
- [Legacy Tauri/Svelte path](#legacy-taurisvelte-path)
- [OBS bootstrapper (if needed)](#obs-bootstrapper-if-needed)

## Linux (supported)

### Prerequisites

- Rust toolchain (stable)
- OBS Studio development packages (`libobs`, `libobs-dev`) plus the OBS runtime
  plugins you intend to load
- OpenSSL and GLib development packages (`pkg-config` must resolve them)
- X11/Wayland client libraries and a Vulkan loader for GPUI rendering

`scripts/linux/install-dependencies.sh` installs the build and packaging
dependencies on Debian/Ubuntu runners.

### Run with mock data

The fastest loop; no libobs host packages are needed:

```bash
cargo run --manifest-path crates/revo-gpui/Cargo.toml
```

### Run against the real engine

```bash
cargo build --manifest-path crates/revo-backend/Cargo.toml --bin revo-engine
cargo run --manifest-path crates/revo-gpui/Cargo.toml --features obs-backend -- --obs
```

The UI starts the engine itself. To use an externally started engine instead:

```bash
REVO_ENGINE_SOCK=unix:/run/user/1000/revo-engine.sock \
  cargo run --manifest-path crates/revo-gpui/Cargo.toml --features obs-backend -- --obs
```

### Validation

```bash
bash scripts/core/verify.sh
```

This runs the whitespace check, `cargo fmt --check`, `revo-core` and
`revo-gpui` tests, the GPUI `cargo check`, and the Tauri compatibility metadata
check. It needs no GTK/WebKit/libobs host packages.

### Release artifacts

Build the binaries first, then stage and package them:

```bash
cargo build --release --features obs-backend --manifest-path crates/revo-gpui/Cargo.toml
cargo build --release --bin revo-engine --manifest-path crates/revo-backend/Cargo.toml

bash scripts/linux/build-obs.sh
bash scripts/linux/prepare-runtime.sh "$RUNNER_TEMP/obs-install"

bash scripts/linux/package-gpui.sh          # portable tree + tar.gz + .run
bash scripts/linux/package-gpui-bundles.sh  # DEB + RPM + AppImage
bash scripts/linux/verify-gpui-release.sh   # verifies all five + checksums
```

Artifacts land in `crates/revo-gpui/target/release/bundle/`:

- `revo-gpui-<version>-linux-x64-portable.tar.gz`
- `revo-gpui-<version>-linux-x64.run`
- `revo-gpui-<version>-linux-x64.deb`
- `revo-gpui-<version>-linux-x64.rpm`
- `revo-gpui-<version>-linux-x64.AppImage`
- `*-linux-x64-SHA256SUMS.txt`

See [docs/scripts.md](scripts.md) for every script flag.

## NixOS

All NixOS tooling lives in `scripts/nix/` and uses `shell.nix`, which provides
OBS, GStreamer, X11 capture libraries, and the GPUI runtime stack.

### Development

```bash
bash scripts/nix/dev.sh            # native GPUI shell (default)
bash scripts/nix/dev.sh tauri      # legacy Tauri backend without UI
bash scripts/nix/dev.sh tauri-frontend
```

`run-gpui.sh` resolves the X11 client libraries and the Vulkan loader with
`nix`, extends `LD_LIBRARY_PATH`, and launches the shell with the host NVIDIA
driver stack. It needs a desktop session with `DISPLAY` set. Do **not** set
`VK_ICD_FILENAMES`; GPU rendering relies on default Vulkan device discovery.

To run the real libobs backend, build the engine inside the Nix shell:

```bash
REVO_GPUI_OBS=1 bash scripts/nix/run-gpui.sh
```

### Build the release binary

```bash
bash scripts/nix/compile.sh          # native GPUI binary (default)
bash scripts/nix/compile.sh tauri    # legacy Tauri DEB + RPM bundles
```

The GPUI binary is written to
`crates/revo-gpui/target/release/revo-gpui`.

### Run a pre-built binary

```bash
bash scripts/nix/run.sh                     # GPUI binary (default)
bash scripts/nix/run.sh tauri               # legacy revo-ui binary
bash scripts/nix/run.sh /path/to/revo-gpui  # explicit binary
```

`run.sh` refuses to run a binary older than its crate sources.

### Full pipeline

```bash
bash scripts/nix/all.sh        # compile -> package .run -> clean (GPUI)
bash scripts/nix/all.sh tauri  # same pipeline for the legacy bundles
```

The `.run` archive is produced by `scripts/nix/package-bundle.sh` and written to
the repository root as `RevoStream-<version>-nixos-<arch>-linux.run`.

`shell.nix` prepares `REVO_ROOT` (libobs effects and OBS plugins linked from the
Nix store) on first shell entry.

## Windows (partially supported)

> [!WARNING]
> The GPUI interface runs on Windows, but core functionality (libobs engine,
> capture, recording, and streaming) may not work. Use Windows builds for
> development and testing only.

Windows uses a loopback TCP endpoint instead of a Unix socket. The engine
publishes its endpoint through `--endpoint-file`, and the UI discovers it
automatically.

### Prerequisites

- Rust toolchain (stable) with the MSVC target
- Visual Studio 2022 Build Tools (MSVC C++ toolchain)
- OBS runtime DLLs staged into `src-tauri/resources/revo-root`

WebView2 and Node.js are **not** needed for the GPUI application; they belong to
the legacy Tauri path.

### Build and run

```powershell
./scripts/windows/prepare-revo-root.ps1
./scripts/windows/prepare-runtime.ps1

cargo build --release --features obs-backend --manifest-path crates/revo-gpui/Cargo.toml
cargo build --release --bin revo-engine --manifest-path crates/revo-backend/Cargo.toml
```

For the CI bootstrap flow, `cargo obs-build build --out-dir target/release` plus
`./scripts/windows/verify-obs-runtime.ps1` prepares the OBS binaries before
`prepare-runtime.ps1` stages them.

### Package

```powershell
./scripts/windows/package-gpui.ps1          # portable ZIP, NSIS EXE, WiX MSI
./scripts/windows/verify-gpui-package.ps1   # structure + x64 PE checks
./scripts/windows/smoke-test-gpui.ps1       # packaged engine handshake
```

Outputs land in `crates/revo-gpui/target/release/bundle/`:

- `revo-gpui-<version>-windows-x64.zip`
- `revo-gpui-<version>-windows-x64.exe` (NSIS)
- `revo-gpui-<version>-windows-x64.msi` (WiX)

`package-gpui.ps1 -SkipMsi` skips the MSI when WiX is unavailable.

The smoke tests do not start the UI: it needs a GPU-backed desktop session. They
start the packaged engine, wait for its endpoint, and verify the boot snapshot.

## macOS (experimental)

> [!WARNING]
> The macOS libobs hooks are not yet verified on real hardware. Use macOS builds
> for development only.

### Prerequisites

- Rust toolchain (stable) with the required Apple target
- Xcode and Command Line Tools
- OBS runtime staged into `src-tauri/resources/revo-root` (see
  `scripts/macos/populate-revo-root.sh`)

### Build and bundle

`package-gpui-app.sh` requires `REVO_TARGET` and `REVO_ARCH`:

```bash
cargo build --release --features obs-backend --target "$REVO_TARGET" \
  --manifest-path crates/revo-gpui/Cargo.toml
cargo build --release --bin revo-engine --target "$REVO_TARGET" \
  --manifest-path crates/revo-backend/Cargo.toml

REVO_TARGET=aarch64-apple-darwin REVO_ARCH=arm64 \
  bash scripts/macos/package-gpui-app.sh
```

The script assembles `RevoStream.app` with the UI binary, `revo-engine`, and
`Contents/Resources/resources/revo-root`, so the existing
`ensure-app-resources`, `normalize-app-dylib-paths`, `codesign-app`,
`verify-app`, and `build-dmg` steps keep working.

### Verify

```bash
bash scripts/macos/smoke-test-gpui.sh /path/to/RevoStream.app
```

The smoke test validates the bundle layout, `Info.plist`, and signature, then
starts the bundled engine and checks its endpoint handshake. The UI itself is
not started because CI runners have no GPU-backed window session.

## Legacy Tauri/Svelte path

The Tauri 2 + Svelte adapter remains buildable as the behavior-parity reference
until the GPUI cutover completes. Its main window has no interactive UI; the
Svelte routes were removed and `src/routes/+page.svelte` is a placeholder.

```bash
pnpm install
pnpm build
./run.sh          # pnpm tauri dev with libobs paths
./run-cef.sh      # optional cef feature (see OBS_CEF_FEASIBILITY.md)
```

Packaging for this path uses `scripts/appimage/build-appimage.sh` and the legacy
`scripts/nix/compile.sh tauri` target. Do not use these for GPUI releases.

## OBS bootstrapper (if needed)

The project is based on `revo-lib` + `libobs-rs`, so on non-Linux setups — or
when the system OBS is incompatible — you may need matching OBS binaries.

### Option A: use local/system OBS (preferred on Linux)

Install OBS on the host and make sure the runtime can find `libobs` and plugins.
`revo-engine` resolves `REVO_ROOT`, and the packaged layouts stage the runtime
next to the binaries (`revo-root/` on Linux and Windows,
`Contents/Resources/resources/revo-root` inside the macOS `.app`).

### Option B: libobs-rs bootstrapper flow (recommended for Windows/macOS dev)

In the `revo-lib` crate (sibling repository), use the `libobs-bootstrapper`
path from `libobs-rs` so OBS binaries are downloaded at runtime:

1. Add `libobs-bootstrapper` integration to the `revo-lib` startup path.
2. On app startup, bootstrap OBS binaries into an app-local directory.
3. Point runtime env paths (`OBS_DATA_PATH`, `OBS_PLUGIN_PATH`, platform
   library path) at the bootstrapped output.

On Windows CI, `scripts/windows/patch-libobs-bootstrapper.ps1` patches the
bootstrapper before `cargo obs-build build` produces the runtime.
