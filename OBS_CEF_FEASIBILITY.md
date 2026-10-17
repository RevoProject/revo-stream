# OBS CEF feasibility audit

Status: **unsupported / not integrated**.

This document records why the presence of an `obs-browser` or CEF library is
not sufficient to enable native OBS browser support in RevoStream.

## Version baseline

- Release OBS tag declared by the project: `32.1.2`
  - `.github/workflows/release.yml`
  - `scripts/linux/build-obs.sh`
  - `scripts/macos/checkout-obs.sh`
- Rust bindings: libobs API `32.1.0`
  - pinned `libobs-rs` commit `7e76c91cd5ae46eee44533bb6e6c69afe10b2dac`
  - `src-tauri/Cargo.lock`
- Windows bootstrap tool: `cargo-obs-build 2.0.3`
- Windows OBS build now passes `--tag 32.1.2`; previous builds selected a
  matching patch dynamically and were not fully reproducible.

No compatible CEF/obs-browser version has been established. It must be built
and validated against the same OBS runtime/toolchain instead of downloading an
arbitrary latest binary.

## Current browser implementation

RevoStream registers a custom input source with ID `browser_source` in:

- `src-tauri/src/sources/browser.rs`

It launches an external Chromium-family executable, captures PNG snapshots
and submits asynchronous RGBA video frames. It is video-only and is not an
embedded CEF runtime.

Browser Interaction is a separate CDP implementation in:

- `src-tauri/src/sources/browser_interact.rs`

It uses another headless Chromium process, `Page.startScreencast`, screenshot
capture and CDP input events.

Both are opt-in behind:

`Settings > Other > Experimental Future > Enable built-in browser`

The default is disabled.

## Platform build status

### Linux

`scripts/linux/build-obs.sh` configures OBS with `ENABLE_BROWSER=OFF`.
No complete obs-browser/CEF runtime is staged or verified.

### macOS

`scripts/macos/configure-obs.sh` configures OBS with `ENABLE_BROWSER=OFF`.
No CEF framework/helper layout is staged or verified.

### Windows

`cargo-obs-build` browser support is disabled by default. The release workflow
does not pass `--browser`. Current staging does not validate the complete set
of CEF DLLs, subprocesses, `.pak`/`.bin` resources, locales and plugin data.

## Runtime blockers

1. The custom backend and native obs-browser both use `browser_source`.
2. If native obs-browser registers first, custom registration exits early.
3. Source creation currently expects the custom worker status map and rejects
   a native source as `Failed to start browser snapshot worker`.
4. Runtime module loading explicitly blocks `obs-browser`, `libcef`,
   `libEGL`, `libGLESv2` and `libvk_swiftshader`.
5. The current interaction UI sends CDP commands to an external Chromium
   process; native OBS sources do not expose that CDP session.

The pinned libobs bindings do expose generic source interaction APIs:

- `obs_source_send_mouse_click`
- `obs_source_send_mouse_move`
- `obs_source_send_mouse_wheel`
- `obs_source_send_focus`
- `obs_source_send_key_click`

Native interaction would need a separate adapter using those APIs and a source
with `OBS_SOURCE_INTERACTION`. It cannot reuse the current CDP implementation
unchanged.

## Licensing and redistribution

Project package metadata now matches the root license: `AGPL-3.0-only`.

Before shipping CEF, release packaging must include and verify all required
CEF/Chromium notices and third-party licenses. No such notice bundle or
verification currently exists in this repository.

## Required implementation sequence

Native OBS browser support can only be claimed after all of the following:

1. Decide distinct ownership/IDs for custom and native browser sources.
2. Remove custom-worker assumptions from native source creation/update paths.
3. Implement generic libobs mouse/keyboard/focus interaction adapter.
4. Build obs-browser against the pinned OBS runtime on each platform.
5. Stage and verify every CEF runtime/resource/helper/locale file.
6. Validate architecture and module ABI before loading.
7. Remove runtime blocks only after complete package verification.
8. Add runtime status: backend selected, module/version, compatibility result.
9. Add per-platform tests for load, source creation, frames, interaction and
   shutdown.
10. Add redistribution notices and checksum/version validation.

Until then builds without CEF remain the supported/default configuration. The
application must not silently fall back from a requested native backend to the
external high-CPU backend.
