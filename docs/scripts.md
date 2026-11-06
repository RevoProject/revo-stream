# Scripts

## Core crates (`scripts/core/`)

- `verify.sh` — standard validation gate for the framework-neutral migration
  crates (`revo-core`, `revo-gpui`): whitespace check, `cargo fmt --check`,
  core and GPUI tests, GPUI `cargo check`, and Tauri compatibility metadata.
  Requires no GTK/WebKit/libobs host packages. Run after every core slice:
  `bash scripts/core/verify.sh`

Platform equivalents for the same gate:

- `scripts/macos/verify-core.sh` — runs `scripts/core/verify.sh` on macOS
  (GPUI builds natively via Metal).
- `scripts/windows/verify-core.ps1` — PowerShell equivalent for Windows
  (without the Linux pkg-config metadata step).

## GPUI development on NixOS (`scripts/nix/`)

- `dev.sh gpui` — run the native GPUI mock-data shell
  (`crates/revo-gpui`) inside the Nix shell via nixGL.
- `run.sh gpui` — run a previously built release GPUI binary
  (`crates/revo-gpui/target/release/revo-gpui`), with staleness checks against
  the core crate sources; `run.sh tauri` (default) keeps the existing Tauri
  behavior. The library path now also exposes fontconfig, Wayland, and the
  Vulkan loader required by GPUI.

## Root-level Tauri scripts

- `run.sh` — run app in development mode (`pnpm tauri dev`)
- `run-cef.sh` — run app with CEF backend (`pnpm tauri dev --features cef`)
- `clean.sh` — clean all build artifacts

## AppImage build

- `scripts/appimage/build-appimage.sh` — build a stable AppImage release bundle
  (`pnpm tauri build --bundles appimage`)

## NixOS scripts (`scripts/nix/`)

- `compile.sh` — build the app (deb + rpm bundles) under a Nix shell
- `dev.sh` — compile and run in development mode under a Nix shell
  (`bash scripts/nix/dev.sh [tauri]`)
- `run.sh` — run a pre-built binary (`src-tauri/target/release/revo-ui` by
  default) with Nix-provided GStreamer deps
- `all.sh` — full NixOS pipeline: compile → package → clean
- `package-bundle.sh` — package a built binary into a self-extracting `.run` archive

## Release scripts

The Linux/macOS/Windows release and packaging scripts still produce the
current Tauri-era artifacts and are unchanged. Per `MIGRATIONS.md` Phase 7,
the GPUI package layout must be defined and tested before any Tauri packaging
path is removed or replaced.
