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

GPUI is the default target. The Tauri adapter remains available as the
`ObsRuntime` parity reference; its main window has no UI since the Svelte
components were removed.

- `compile.sh` — build the native GPUI release binary (default) or the legacy
  Tauri deb + rpm bundles (`bash scripts/nix/compile.sh tauri`)
- `dev.sh` — run the native GPUI shell in a Nix shell (default), the Tauri
  backend without UI (`dev.sh tauri`), or the legacy frontend (`dev.sh
  tauri-frontend`)
- `run.sh` — run a built binary: `gpui` (default,
  `crates/revo-gpui/target/release/revo-gpui`), `tauri`, or an explicit path,
  with per-binary staleness checks and Nix-provided media deps
- `all.sh` — full pipeline: compile (gpui by default) → package `.run` → clean
- `package-bundle.sh` — package a built GPUI or Tauri binary into a
  self-extracting `.run` archive (`gpui` by default)
- `repair-debug-profile.sh` — repair an interrupted Cargo debug profile;
  accepts an optional Cargo.toml path (defaults to the Tauri adapter)
- `debug.sh` — run the GPUI shell in debug mode (default); `tauri` and
  `embedded` run the legacy adapter

## Release scripts

The Linux/macOS/Windows release and packaging scripts still produce the
current Tauri-era artifacts and are unchanged. Per `MIGRATIONS.md` Phase 7,
the GPUI package layout must be defined and tested before any Tauri packaging
path is removed or replaced.
