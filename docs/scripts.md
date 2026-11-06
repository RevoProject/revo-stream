# Scripts

## Core crates (`scripts/core/`)

- `verify.sh` — standard validation gate for the framework-neutral migration
  crates (`revo-core`, `revo-gpui`): whitespace check, `cargo fmt --check`,
  core and GPUI tests, GPUI `cargo check`, and Tauri compatibility metadata.
  Requires no GTK/WebKit/libobs host packages. Run after every core slice:
  `bash scripts/core/verify.sh`

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
