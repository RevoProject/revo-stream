# RevoStream
![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white&color=%23CE412B)
![Tauri](https://img.shields.io/badge/tauri-%232E7EEA.svg?style=for-the-badge&logo=tauri&logoColor=%23FFFFFF)
![Svelte](https://img.shields.io/badge/svelte-%23f1413d.svg?style=for-the-badge&logo=svelte&logoColor=white)

![Linux](https://img.shields.io/badge/Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black)
![License](https://img.shields.io/github/license/revoproject/revo-stream?style=for-the-badge)

## Introduction
RevoStream is a desktop application for streamers and live production teams.
It helps you build scenes, manage sources, and run streaming or recording workflows in one place.

Powered by [revo-project/revo-lib](https://github.com/RevoProject/revo-lib), RevoStream is built with Rust, Tauri, and Svelte.

> [!WARNING]
> RevoStream is officially supported on Linux.
> Windows and macOS can be run in experimental/dev mode (instructions below).

## Quick navigation
- [Highlights](#highlights)
- [Streaming](#streaming)
- [Themes](#themes)
- [Usage](#usage)
- [Windows (experimental)](#windows-experimental)
- [macOS (experimental)](#macos-experimental)
- [OBS bootstrapper (if needed)](#obs-bootstrapper-if-needed)
- [Tech stack](#tech-stack)
- [Contributing](#contributing)
- [License](#license)

## Highlights
- Fast native backend powered by OBS/libobs
- Modern desktop UI for scene and source control
- Built-in tools for streaming and recording workflows
- Dedicated utility windows, including Graphic Planner
- Close to 50,000 lines of code

## Streaming
- Supports streaming services available in recent OBS Studio builds
- Official support for Kick streaming
- Protocol support: RTMP, RTMPS, SRT, RIST, WHIP, WebRTC

## Themes
- Use built-in themes or create your own custom theme for your mood
- Quickly import themes via .revotheme extension

### Goals
- Worldwide language support
- Support for more operating systems (e.g. Windows, macOS, Android)

## Usage
### Stable release
Download the latest version from the [Releases page](https://github.com/RevoProject/revo-stream/releases).

### Build from source
Install dependencies and build:

```bash
pnpm build
```

### Run locally
- Development mode: `./run.sh`
- CEF development mode: `./run-cef.sh`
- Stable-release AppImage: `./scripts/appimage/build-appimage.sh`

## Windows (experimental)
Use this only for local/dev testing.

### Prerequisites
- Rust toolchain (stable)
- Node.js 20+
- `pnpm`
- Visual Studio 2022 Build Tools (MSVC C++ toolchain)
- WebView2 runtime

### Steps
```powershell
# from revo-ui/
pnpm install

# frontend check
pnpm build

# tauri backend
cd src-tauri
cargo build --no-default-features
cd ..

# run app in dev mode
pnpm tauri dev
```

If OBS symbols/modules are missing at runtime, follow [OBS bootstrapper (if needed)](#obs-bootstrapper-if-needed).

## macOS (experimental)
Use this only for local/dev testing.

### Prerequisites
- Rust toolchain (stable)
- Node.js 20+
- `pnpm`
- Xcode + Command Line Tools

### Steps
```bash
# from revo-ui/
pnpm install

# frontend check
pnpm build

# tauri backend
cd src-tauri
cargo build --no-default-features
cd ..

# run app in dev mode
pnpm tauri dev
```

If OBS symbols/modules are missing at runtime, follow [OBS bootstrapper (if needed)](#obs-bootstrapper-if-needed).

## OBS bootstrapper (if needed)
Because this project is based on `revo-lib` + `libobs-rs`, you may need to bootstrap matching OBS binaries on non-Linux setups or when system OBS is incompatible.

### Option A: Use local/system OBS (preferred on Linux)
Install/update OBS on the host system and ensure runtime can find `libobs` and plugins.

### Option B: Use libobs-rs bootstrapper flow (recommended for Windows/macOS dev)
In the `revo-lib` crate (sibling repository), enable/use the `libobs-bootstrapper` path from `libobs-rs` so OBS binaries are downloaded at runtime.

Typical flow:
1. Add `libobs-bootstrapper` integration in `revo-lib` startup path.
2. On app startup, bootstrap OBS binaries into app-local directory.
3. Point runtime env paths (`OBS_DATA_PATH`, `OBS_PLUGIN_PATH`, and platform library path) to bootstrapped output.

If you want, a ready-to-use bootstrap implementation can be added directly to `revo-lib` startup code.

## Tech stack
- Rust (via `revo-lib` based on `libobs-rs`)
- Tauri
- Svelte
- OBS/libobs via revo-lib

## Scripts

### Root-level Tauri scripts
- `run.sh` — run app in development mode (no compilation)
- `run-cef.sh` — run app with CEF backend (no compilation)
- `clean.sh` — clean all build artifacts

### AppImage build
- `scripts/appimage/build-appimage.sh` — build a stable AppImage release bundle

### NixOS scripts (`scripts/nix/`)
- `compile.sh` — build the app (deb + rpm bundles) under a Nix shell
- `dev.sh` — compile and run in development mode under a Nix shell (`cargo` or `tauri`)
- `run.sh` — run a pre-built binary with Nix-provided GStreamer deps
- `all.sh` — full NixOS pipeline: compile → package → clean
- `package-bundle.sh` — package a built binary into a self-extracting `.run` archive
- `itvt.sh` — one-file launcher that auto-builds if binary is missing

### Deployment
- `scripts/deploy-dist.sh` — deploy `dist/` to the `main` branch
- `scripts/deploy-dist-debug.sh` — deploy `dist/` to the `debug` branch

## Contributing
The project is currently in an early MVP stage
Contributions are welcome

If you want to help:
- Open an issue with a bug report or feature request
- Discuss major changes before implementation
- Submit a pull request with a clear scope and summary

Pull requests for libobs bindings and performance optimizations are especially welcome

**Let's build the future of streaming together**

## License
This project is distributed under the license included in this repository



