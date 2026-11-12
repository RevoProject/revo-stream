# RevoStream Agent Guide

## Current State

- Application version: `0.5.0-pre1` (documentation target; `package.json` and
  the four Rust manifests must be bumped to match).
- Migration branch: `feature/gpui-rewrite`.
- `revo-lib` is pinned to the revision
  `66325a65d1e0a0db9a0f3eedc585af554f2440e3` in both `crates/revo-backend` and
  `src-tauri`; keep the manifests and lockfiles in sync when bumping it.
  `libobs-rs` remains a transitive dependency whose commit is pinned by the
  lockfiles.
- Primary application: `crates/revo-gpui/` — the native GPUI shell. It runs on
  `MockBackendRuntime` by default and on the real backend with
  `--features obs-backend -- --obs`.
- Real backend path: the UI process contains no libobs code. `AnyBackend::Obs`
  is a `BackendService<EngineClient>` that spawns or connects to the dedicated
  `revo-engine` process (`crates/revo-backend/src/bin/revo-engine.rs`), which
  owns `ObsBackendRuntime`/`ObsRuntime`. Commands, snapshots, preview frames,
  browser interaction, logs, source/filter settings, fonts, and devices travel
  over `revo_core::ipc` using the platform transport in
  `revo_core::transport`.
- Legacy compatibility target: `src-tauri/` remains buildable as the parity
  reference (Tauri commands, `TauriObsPlatform`, webview auxiliary windows) but
  has no interactive UI. `src/routes/+page.svelte` is a placeholder notice.
- Extracted backend: `crates/revo-backend/` owns `ObsRuntime`, `SceneState`,
  `ObsState`, preview stats, and the runtime/scenes/sources/settings/devices/
  logging/permissions/utils modules with zero Tauri dependencies. It also hosts
  the IPC server (`engine.rs`), the typed `ObsBackendRuntime` adapter
  (`obs_service.rs`), and the standalone `revo-engine` binary. `src-tauri/`
  is now a thin compatibility adapter: Tauri commands, `TauriObsPlatform`, and
  the webview auxiliary-window UI. The real backend is already wired behind
  `BackendService` for GPUI through `EngineClient`; remaining cutover blockers
  are final Tauri/Svelte removal, macOS on-hardware verification,
  installed-artifact validation, and the open browser/video/tray decisions.
- Extracted core slices: framework-neutral source, planner, media-option,
  settings, scene, theme, and plugin models and pure helpers in
  `crates/revo-core/`, consumed through path dependencies.
  `revo-core::settings` owns default providers, normalization, and parsing
  recovery. `revo-core::planner` owns the pure scene-canvas geometry
  (fit/stretch scale, centering, resolution parsing) shared by adapter source
  operations and the GPUI planner. `revo-core::plugins` owns plugin
  naming and enabled-module normalization. Settings file I/O and plugin window
  capability policy remain in `revo-backend`. `revo-core` also provides a
  bounded, typed `BackendService` worker thread with request/reply operations,
  snapshots, nonblocking GPUI dispatch, and lifecycle tests.
- `crates/revo-gpui/` is the primary application: Header, preview/program,
  Scenes, Sources, Tools, responsive layouts, modal routing, Audio Mixer (with
  context menu and advanced-properties modal), Transitions, the Graphic Planner
  driven by `revo-core::planner` geometry, settings/source/filter/plugin/
  history workflows, and auxiliary Preview, Debug Console, and Browser
  Interaction windows. Default builds run `MockBackendRuntime`;
  `--features obs-backend` selects the engine-backed `AnyBackend::Obs`, which
  reaches libobs through `services/engine.rs::EngineClient` and the separate
  `revo-engine` process. Preview frames arrive as raw RGBA in a bounded
  newest-wins slot and upload as `gpui::RenderImage` textures. The
  settings modal persists through `SaveSettings` with a typed draft state
  machine (dirty tracking, URL/resolution validation, option cycling).
- The GPUI packaging layout is what the release workflow builds:
  `scripts/linux/package-gpui.sh` stages the UI binary, the `revo-engine`
  binary, launcher, desktop entry, icons and OBS runtime (`prepare-runtime.sh`
  layout) into a portable tree and emits `tar.gz` + `.run`;
  `scripts/linux/package-gpui-bundles.sh` turns the same tree into DEB, RPM and
  AppImage, deriving package dependencies from the staged ELF closure instead of
  a hand-maintained list; `scripts/linux/verify-gpui-release.sh` validates all
  five artifacts and writes checksums. `scripts/macos/package-gpui-app.sh`
  assembles `RevoStream.app` (UI + engine +
  `Contents/Resources/resources/revo-root`) so the existing
  ensure-resources/normalize-rpaths/codesign/verify/DMG scripts keep working,
  and `scripts/windows/package-gpui.ps1` produces the portable ZIP, NSIS setup
  EXE and WiX MSI. The engine transport is platform-neutral
  (`revo_core::transport`): a Unix socket on Unix, a loopback TCP endpoint
  published through `--endpoint-file` on Windows, and the engine derives its own
  resource/data paths (`utils::fs::init_standalone_app_paths`) because no GUI
  adapter injects them.
- Preview frames travel as binary raw-RGBA IPC frames (`revo_core::ipc`
  `PreviewFrame`, `RVPF` magic + header + pixels) and are uploaded as
  `gpui::RenderImage` textures; no capture is PNG-encoded or base64-expanded on
  the engine or UI side. The retained-frame freeze/stale fallback stores the raw
  frame behind an `Arc`, and the engine now publishes frames on that path too.
  Browser-interaction frames remain PNG data URIs (they come from a CDP
  screencast, not libobs). The Tauri parity adapter keeps returning data URIs
  and reuses the same capture/validation/publish phases.
- Remaining cutover blockers: remove the Tauri/Svelte compatibility path,
  verify the macOS
  libobs hooks on hardware, validate installed artifacts, and decide the
  browser-dock, WebRTC/native-video, and tray strategy. The Linux release job
  still `cargo check`s the Tauri adapter as the parity reference.
- The migrated Svelte component families (`Home`, `Modal`, `QuickModal`,
  `Modules`) and all auxiliary routes are removed. `src/routes/+page.svelte`
  is a placeholder notice; the legacy Tauri adapter has no interactive UI.
  The Tauri Rust backend remains as the behavioral parity reference for the
  remaining cutover.
- Target application: a native Rust GPUI UI process plus the dedicated
  `revo-engine` libobs process, with no webview and no JavaScript/Rust IPC
  boundary. The engine boundary is typed Rust IPC by design.
- Migration specification and dependency analysis: `MIGRATIONS.md`. Its
  original single-process target is superseded by the engine-process split
  described here and implemented in the code.
- Supported production platform is Linux. Windows is partially supported: the
  GPUI interface runs, but core functionality (libobs engine, capture,
  recording, and streaming) may not work. macOS remains experimental. Both have
  active release/runtime tooling.

The GPUI application has not replaced the Tauri application yet. Keep the
Tauri/Svelte path buildable as the parity reference until the corresponding
GPUI migration checkpoints pass. Do not remove frontend files, Tauri
configuration, packaging code, or compatibility adapters merely because they
are absent from the final architecture.

## Migration Requirements

- Preserve the current component hierarchy and workflows: Header, preview and
  program surfaces, Scenes, Sources, Tools, Audio Mixer, Transitions, Plugins,
  settings, source editors, filters, quick actions, Graphic Planner, Browser
  Interaction, Preview, and Debug Console.
- Preserve RevoClassic and RevoFuture semantic colors exactly. Native GPUI
  views must consume a shared theme rather than hard-coded per-view colors.
- Preserve layout order, proportions, minimum sizes, spacing, radii, dock
  behavior, and responsive breakpoints. See the visual parity contract in
  `MIGRATIONS.md`.
- Keep backend/business logic independent of both Tauri and GPUI. GUI adapters
  should call typed services rather than expose string-based in-process IPC.
- Keep libobs ownership isolated in the dedicated `revo-engine` process. Do not
  add libobs code to the GPUI UI process or broaden the existing raw pointer
  `Send`/`Sync` assumptions.
- Keep high-frequency preview and audio delivery bounded to the newest value.
  Never block the GPUI foreground executor with libobs work, image encoding,
  filesystem traversal, dialogs, CDP waits, or process waits.
- Treat browser embedding, native video/WebRTC presentation, tray integration,
  dialogs, and packaging as explicit platform decisions. GPUI does not replace
  these Tauri/webview capabilities automatically.
- Make one logical change per Conventional Commit. Keep cleanup,
  documentation, backend extraction, and UI implementation in separate
  commits.

## Repository Layout

- `src/`: legacy Svelte entry points and remaining parity components.
- `static/`: legacy semantic theme and component CSS retained as the visual
  source of truth during migration.
- `crates/revo-core/`: framework-neutral Rust domain models, settings, planner,
  plugins, typed IPC/transport, and the bounded `BackendService` worker.
- `crates/revo-backend/`: framework-neutral libobs backend, IPC server,
  `ObsBackendRuntime`, and the `revo-engine` binary.
- `crates/revo-gpui/`: primary native GPUI application and engine client.
- `src-tauri/src/`: legacy Tauri compatibility adapter — commands,
  `TauriObsPlatform`, and webview auxiliary windows.
- `src-tauri/Cargo.toml`: legacy compatibility manifest; not the primary
  application manifest. Tauri dependencies remain until final cutover.
- `src-tauri/resources/revo-root/`: generated/staged OBS runtime resources;
  ignored by Git.
- There is no root Cargo workspace manifest yet: each crate has its own
  manifest and lockfile, and commands build/test them by manifest path.
- `scripts/linux/`, `scripts/macos/`, `scripts/windows/`: platform runtime,
  packaging, verification, and release scripts.
- `scripts/nix/`: Nix development, build, run, repair, and packaging scripts.
- `MIGRATIONS.md`: authoritative migration architecture, dependency mapping,
  phases, visual targets, risks, and test strategy.
- `docs/building.md`: GPUI-first platform build instructions, with the legacy
  Tauri/Svelte path documented separately at the end.
- `docs/scripts.md`: GPUI-first script reference (core gate, NixOS, Linux,
  Windows, macOS, release workflow) plus a separate legacy Tauri script list;
  verify entries against the filesystem before relying on them because
  migration cleanup may make it stale.
- `CONTRIBUTING.md`: contributor policy for licensing, closed-source code,
  AI-assisted contributions, validation, and pull requests.
- `.github/`: issue templates, the pull request template, funding metadata, and
  the release/diagnostic workflows.

## Development Commands

The native GPUI application is the primary target. The legacy Tauri/Svelte
commands are kept for parity checks until final cutover.

### Native GPUI

- `bash scripts/core/verify.sh`: run the standard validation pipeline for
  `revo-core`/`revo-gpui` (formatting, tests, checks, compatibility metadata)
  without Tauri/GTK/WebKit/libobs host dependencies.
- `cargo test --manifest-path crates/revo-core/Cargo.toml`: run the extracted
  core tests without Tauri, WebKit, GTK, or libobs system dependencies.
- `cargo test --manifest-path crates/revo-gpui/Cargo.toml`: run native GPUI
  application tests.
- `cargo check --manifest-path crates/revo-gpui/Cargo.toml`: check the native
  GPUI application.
- `cargo run --manifest-path crates/revo-gpui/Cargo.toml`: run the native GPUI
  mock-data shell.
- `cargo build --manifest-path crates/revo-backend/Cargo.toml --bin revo-engine`:
  build the libobs engine process (requires libobs/libssl development
  packages).
- `cargo run --manifest-path crates/revo-gpui/Cargo.toml --features obs-backend -- --obs`:
  run the GPUI application against the real engine instead of mock data.
- `REVO_ENGINE_SOCK=<endpoint>`: connect to an externally started engine
  (`unix:<path>` or `tcp:127.0.0.1:<port>`) instead of spawning one.

### Legacy Tauri/Svelte compatibility

- `pnpm install --frozen-lockfile`: install pinned frontend/Tauri CLI
  dependencies.
- `pnpm check`: run SvelteKit synchronization and Svelte type checking.
- `pnpm build`: create the legacy static frontend build.
- `pnpm dev`: run only the Vite development server.
- `./run.sh`: run `pnpm tauri dev` with libobs paths and the conditional Linux
  EGL/DMABUF workaround.
- `./run-cef.sh`: run the optional `cef` feature after validating
  `CEF_ROOT` or `CEF_INCLUDE_DIR`/`CEF_LIB_DIR`. The current CEF bridge is not a
  complete embedded CEF implementation; see `OBS_CEF_FEASIBILITY.md`.
- `cargo check --manifest-path src-tauri/Cargo.toml`: check the legacy adapter
  when host Tauri/GTK/WebKit and libobs development dependencies are available.
- `cargo test --manifest-path src-tauri/Cargo.toml`: run legacy adapter tests in
  a fully provisioned native environment.
- `./clean.sh`: remove `src-tauri/target` and `.svelte-kit`.

### NixOS

- `bash scripts/nix/dev.sh`: run the native GPUI shell in the Nix
  environment (default; delegates to `run-gpui.sh`); `dev.sh tauri` runs
  the Tauri backend without UI.
- `bash scripts/nix/dev.sh tauri-frontend`: run the legacy Tauri frontend path.
- `bash scripts/nix/run-gpui.sh`: launch the native GPUI shell with the
  X11/Vulkan runtime stack resolved by nix and the host NVIDIA driver stack
  (verified on the NVIDIA container runtime; requires a desktop session).
- `bash scripts/nix/compile.sh`: build the native GPUI release binary
  (default); `compile.sh tauri` builds legacy DEB and RPM bundles.
- `bash scripts/nix/run.sh`: run the default release binary with Nix-provided
  media dependencies; an explicit binary path may be supplied.
- `bash scripts/nix/all.sh`: run the compile, portable-package, and cleanup
  pipeline.
- `bash scripts/nix/package-bundle.sh`: create the self-extracting `.run`
  archive.
- `bash scripts/nix/repair-debug-profile.sh`: repair interrupted Cargo debug
  profile artifacts.
- `bash scripts/nix/debug.sh`: run the Nix debug workflow.

### Release and packaging

- `scripts/appimage/build-appimage.sh`: build the current Tauri AppImage
  (legacy parity path, not part of the GPUI release pipeline).
- `bash scripts/linux/package-gpui.sh [OBS_INSTALL]`: stage the GPUI tree and
  build the portable tar.gz and self-extracting `.run`.
- `bash scripts/linux/package-gpui-bundles.sh [STAGED_DIR]`: build the GPUI
  DEB, RPM and AppImage from that staged tree.
- `bash scripts/linux/verify-gpui-release.sh`: validate every Linux GPUI
  artifact and write `*-linux-x64-SHA256SUMS.txt`.
- `./scripts/windows/package-gpui.ps1`: GPUI ZIP/NSIS/MSI, checked by
  `verify-gpui-package.ps1` and `smoke-test-gpui.ps1` (the smoke test drives the
  packaged engine's endpoint and reads its boot snapshot).
- `bash scripts/macos/package-gpui-app.sh`: assemble the GPUI `.app` (needs
  `REVO_TARGET` and `REVO_ARCH`); `scripts/macos/smoke-test-gpui.sh` starts the
  engine from inside the bundle.
- `scripts/linux/`: build/stage OBS, install dependencies, package portable
  releases, and verify Linux artifacts.
- `scripts/macos/`: prepare OBS/runtime dependencies, build and sign the DMG,
  and verify architecture, rpaths, dependencies, and bundle contents.
- `scripts/windows/`: prepare and verify OBS runtime files, create the portable
  ZIP, and smoke-test Windows packages.
- `.github/workflows/release.yml`: current cross-platform release pipeline.

Do not assume Tauri bundle paths or WebKit dependencies apply to the GPUI
packages: the GPUI pipeline stages the runtime itself (`revo-root` next to the
binary on Linux, `Contents/Resources/resources/revo-root` inside the macOS
`.app`) and resolves it through `revo_backend::settings::core::resolve_root_dir`
plus `revo_backend::utils::fs::init_standalone_app_paths`, not through the Tauri
resource directory.

## Validation Rules

- Run `git diff --check` for documentation and source edits.
- Run the narrowest relevant checks first, then broader checks when native
  dependencies are available.
- A missing host package such as DBus, GTK, GLib, Pango, Cairo, libsoup,
  JavaScriptCoreGTK, WebKitGTK, or libobs is an environment failure, not a
  reason to bypass tests or rewrite unrelated code.
- Preserve and report pre-existing working-tree changes. Do not stage or commit
  unrelated user changes.
- For visual work, compare RevoClassic and RevoFuture at `1080x600`,
  `1280x720`, and `1440x900`, including the `1200px` and `800px` responsive
  transitions. Semantic colors must match exactly; major layout metrics should
  remain within the documented `2px` tolerance.
- Validate installed release artifacts, not only development builds, before
  removing any Tauri packaging or runtime dependency.
- Keep the `README.md` migration checklist and this guide's Current State
  section synchronized when a migration checkpoint lands or the application
  version changes.
