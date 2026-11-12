# RevoStream

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white&color=%23CE412B)
![GPUI](https://img.shields.io/badge/UI-GPUI-6E56CF?style=for-the-badge)
![OBS/libobs](https://img.shields.io/badge/OBS-libobs-302E31?style=for-the-badge)

![Linux](https://img.shields.io/badge/Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black)
![License](https://img.shields.io/github/license/revoproject/revo-stream?style=for-the-badge)

Desktop application for streamers and live production teams — build scenes,
manage sources, and run streaming or recording workflows in one place.
Powered by [revo-lib](https://github.com/RevoProject/revo-lib), built with
Rust and [GPUI](https://gpui.rs/).

The application runs as a native GPUI UI process plus a dedicated
`revo-engine` process that owns OBS/libobs. The Tauri 2 + Svelte adapter is
retained only as a compatibility and behavioral-parity reference while the
GPUI migration completes.

> [!WARNING]
> RevoStream is officially supported on Linux.
> Windows is **partially supported**: the GPUI interface runs, but core
> functionality (libobs engine, capture, recording, and streaming) may not work.
> macOS can be run in experimental/dev mode — see
> [docs/building.md](docs/building.md).

## Highlights

- Native Rust GPUI desktop UI — no webview and no JavaScript/Rust IPC boundary
- Dedicated `revo-engine` process owning OBS/libobs, connected through typed,
  length-prefixed JSON over a Unix socket or loopback TCP
- Fast libobs backend for scene, source, preview, audio, recording, and
  streaming control
- Streaming and recording workflows out of the box
- RTMP, RTMPS, SRT, RIST, WHIP, WebRTC — official support for Kick
- Built-in and custom themes (`.revotheme` import)
- Dedicated utility windows, including Graphic Planner, Preview, Debug Console,
  and Browser Interaction

## Migration checklist

Status for `0.5.0-pre-1` on `feature/gpui-rewrite`.

The project is migrating from Tauri 2 + Svelte 5/SvelteKit to native GPUI.
`MIGRATIONS.md` contains the full architecture, phase plan, parity contract,
risks, and test strategy.

### Completed

- [x] Extract framework-neutral domain models and helpers into `revo-core`
- [x] Extract the libobs backend into `revo-backend` with zero Tauri dependencies
- [x] Add the bounded, typed `BackendService` worker boundary and snapshots
- [x] Add the dedicated `revo-engine` process with typed IPC
- [x] Add platform-neutral transport: Unix socket and Windows loopback TCP
- [x] Port the main GPUI shell: Header, preview/program, Scenes, Sources, Tools
- [x] Port native modules: Audio Mixer, Transitions, and Graphic Planner
- [x] Port settings, source editor/properties, filters, plugins, history,
      preview, Debug Console, and Browser Interaction workflows
- [x] Replace the legacy Svelte main UI with a parity placeholder
- [x] Implement GPUI packaging for Linux, Windows, and macOS
- [x] Wire the cross-platform release workflow to build the GPUI UI + engine
- [x] Add the `scripts/core/verify.sh` validation gate and service/IPC/transport
      tests
- [x] Refresh `docs/building.md` and `docs/scripts.md` for the GPUI-first
      workflow
- [x] Pin the `revo-lib` Git dependency before cutover

### Remaining

- [ ] Final cutover: remove `src-tauri`, the remaining Svelte routes, Vite/pnpm
      frontend, and Tauri configuration
- [ ] Remove WebKitGTK/libsoup and other webview-only dependencies after
      installed-package inspection
- [ ] Replace base64 PNG preview frames with a raw-RGBA/native image path
- [ ] Verify the macOS libobs hooks on real hardware
- [ ] Validate installed artifacts on clean Linux, Windows, and macOS machines
- [ ] Decide and implement the browser-dock strategy
      (external browser, CDP surface, embedded engine, or retire it)
- [ ] Decide and implement the WebRTC/native video-surface scope
- [ ] Add platform tray, native dialogs, clipboard, and permission handling
- [ ] Verify accessibility, IME, high-DPI, focus traversal, and reduced-motion
      parity
- [ ] Delete the legacy compatibility paths after parity acceptance

## Quick start

**Stable release** — download from the
[Releases page](https://github.com/RevoProject/revo-stream/releases).

### Native GPUI application (recommended)

Run the UI with representative mock data (no libobs needed):

```bash
cargo run --manifest-path crates/revo-gpui/Cargo.toml
```

Run against the real libobs backend:

```bash
cargo build --manifest-path crates/revo-backend/Cargo.toml --bin revo-engine
cargo run --manifest-path crates/revo-gpui/Cargo.toml --features obs-backend -- --obs
```

Run the standard core/GPUI validation gate:

```bash
bash scripts/core/verify.sh
```

### Legacy Tauri compatibility path

The legacy path remains buildable as the parity reference:

```bash
pnpm install
./run.sh
```

### NixOS

```bash
bash scripts/nix/dev.sh       # run the native GPUI shell
bash scripts/nix/compile.sh   # build the native GPUI release binary
```

`bash scripts/nix/compile.sh tauri` builds the legacy Tauri bundles.

Full instructions for all platforms: [docs/building.md](docs/building.md) ·
script reference: [docs/scripts.md](docs/scripts.md).

## Tech stack

| Layer | Technology |
| --- | --- |
| UI | Rust + GPUI 0.2.2 (native rendering) |
| Domain/core | `crates/revo-core` — typed models, settings, planner, plugins, IPC, transport, bounded worker service |
| Engine | Rust + OBS/libobs via `revo-lib` (`crates/revo-backend`, `revo-engine` binary) |
| UI ↔ engine | Length-prefixed JSON over a Unix socket or loopback TCP, with newest-wins preview frames |
| Legacy compatibility | Tauri 2 + Svelte 5/SvelteKit (parity reference; removed at final cutover) |
| Packaging | Linux: tar.gz, `.run`, DEB, RPM, AppImage · Windows: ZIP, NSIS, MSI · macOS: `.app`, DMG (Windows/macOS experimental) |

### Repository layout

- `crates/revo-core/` — framework-neutral models, settings, planner, typed IPC,
  transport, and the `BackendService` worker.
- `crates/revo-backend/` — libobs runtime, operation modules, IPC server, and
  the `revo-engine` binary.
- `crates/revo-gpui/` — primary native GPUI application and engine client.
- `src-tauri/` — legacy Tauri compatibility adapter.
- `src/`, `static/` — remaining Svelte parity files and the semantic CSS source
  of truth.
- `scripts/`, `nix/`, `.github/workflows/` — build, packaging, verification, and
  release automation.

## Become a Sponsor

RevoStream is an open-source project built to empower streamers and live production teams. Your support helps us improve the application, develop new features, and build a better streaming experience for everyone.

Interested in supporting the project? We welcome sponsors and partners who share our vision.

**Sponsors and project partners will also be featured on our official website: [revostream.in](https://revostream.in).**

If you'd like to support RevoStream or discuss a partnership, feel free to reach out to us.

## Contributing

The project is in an active migration from Tauri/Svelte to GPUI and
contributions are welcome:

- Open an issue with a bug report or feature request
- Discuss major changes before implementation
- Submit pull requests with a clear scope and summary

PRs for libobs bindings, GPUI views, and performance optimizations are
especially welcome.

## License

GNU Affero General Public License v3.0 — see [LICENSE](LICENSE).
