# Building RevoStream

- [Linux (supported)](#linux-supported)
- [NixOS](#nixos)
- [Windows (experimental)](#windows-experimental)
- [macOS (experimental)](#macos-experimental)
- [OBS bootstrapper (if needed)](#obs-bootstrapper-if-needed)

## Linux (supported)

### Prerequisites

- Rust toolchain (stable)
- Node.js 20+
- `pnpm`
- System OBS Studio (for `libobs` and plugins)

### Build and run

```bash
pnpm install
pnpm build          # frontend check/build
./run.sh            # development mode (pnpm tauri dev)
./run-cef.sh        # development mode with CEF backend
```

### Release bundles

```bash
./scripts/appimage/build-appimage.sh   # AppImage release bundle
```

## NixOS

All NixOS tooling lives in `scripts/nix/` and uses `shell.nix`
(OBS, GStreamer, WebKitGTK and X11 capture libraries are provided by Nix).

### Build release bundles (deb + rpm)

```bash
bash scripts/nix/compile.sh
```

Artifacts land in `src-tauri/target/release/bundle/`.

### Development

```bash
bash scripts/nix/dev.sh            # cargo run (Rust only)
bash scripts/nix/dev.sh tauri      # pnpm tauri dev (frontend + Rust)
```

### Run a pre-built binary

```bash
bash scripts/nix/run.sh            # runs src-tauri/target/release/revo-ui
bash scripts/nix/run.sh /path/to/revo-ui
```

### Full pipeline

```bash
bash scripts/nix/all.sh            # compile → package .run → clean
```

`shell.nix` also prepares `REVO_ROOT` (libobs effects and OBS plugins linked
from the Nix store) on first shell entry.

## Build AppImage from Nix

- Enter the Nix shell:
```bash
nix-shell
```

- Build the AppImage:
```bash
APPIMAGE_EXTRACT_AND_RUN=1 \
steam-run ./scripts/appimage/build-appimage.sh
```

> `APPIMAGE_EXTRACT_AND_RUN=1` allows the AppImage tooling to run without FUSE,
> which is required on NixOS when `libfuse.so.2` is not available.

## Windows (experimental)

Use only for local/dev testing.

### Prerequisites

- Rust toolchain (stable)
- Node.js 20+
- `pnpm`
- Visual Studio 2022 Build Tools (MSVC C++ toolchain)
- WebView2 runtime

### Steps

```powershell
pnpm install
pnpm build
cd src-tauri
cargo build --no-default-features
cd ..
pnpm tauri dev
```

If OBS symbols/modules are missing at runtime, see
[OBS bootstrapper](#obs-bootstrapper-if-needed).

## macOS (experimental)

Use only for local/dev testing.

### Prerequisites

- Rust toolchain (stable)
- Node.js 20+
- `pnpm`
- Xcode + Command Line Tools

### Steps

```bash
pnpm install
pnpm build
cd src-tauri
cargo build --no-default-features
cd ..
pnpm tauri dev
```

If OBS symbols/modules are missing at runtime, see
[OBS bootstrapper](#obs-bootstrapper-if-needed).

## OBS bootstrapper (if needed)

The project is based on `revo-lib` + `libobs-rs`, so on non-Linux setups — or
when the system OBS is incompatible — you may need matching OBS binaries.

### Option A: use local/system OBS (preferred on Linux)

Install OBS on the host and make sure the runtime can find `libobs` and plugins.

### Option B: libobs-rs bootstrapper flow (recommended for Windows/macOS dev)

In the `revo-lib` crate (sibling repository), use the `libobs-bootstrapper`
path from `libobs-rs` so OBS binaries are downloaded at runtime:

1. Add `libobs-bootstrapper` integration to the `revo-lib` startup path.
2. On app startup, bootstrap OBS binaries into an app-local directory.
3. Point runtime env paths (`OBS_DATA_PATH`, `OBS_PLUGIN_PATH`, platform
   library path) at the bootstrapped output.
