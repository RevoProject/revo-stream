# RevoStream Agent Guide

## Current State

- Application version: `0.4.1`.
- Migration branch: `feature/gpui-rewrite`.
- Current runnable application: Tauri 2, Svelte 5/SvelteKit, and a Rust/libobs
  backend in `src-tauri/`.
- First extracted core slice: framework-neutral source and planner models in
  `crates/revo-core/`, consumed by the Tauri adapter through a path dependency.
- Target application: one native Rust process rendered by GPUI, with no
  webview or JavaScript/Rust IPC boundary.
- Migration specification and dependency analysis: `MIGRATIONS.md`.
- Supported production platform is Linux. Windows and macOS remain
  experimental and have active release/runtime tooling.

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
- Move libobs ownership toward a dedicated service thread before allowing GPUI
  executor tasks to access runtime operations. Do not broaden the existing raw
  pointer `Send`/`Sync` assumptions.
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

- `src/`: legacy Svelte UI and route entry points retained for parity.
- `static/`: legacy semantic theme and component CSS retained as the visual
  source of truth during migration.
- `src-tauri/src/`: Rust/libobs backend plus current Tauri application adapter.
- `crates/revo-core/`: framework-neutral Rust domain models and, incrementally,
  backend services shared by the compatibility and GPUI applications.
- `src-tauri/Cargo.toml`: current Rust manifest. Tauri dependencies remain
  until final cutover.
- `src-tauri/resources/revo-root/`: generated/staged OBS runtime resources;
  ignored by Git.
- `scripts/linux/`, `scripts/macos/`, `scripts/windows/`: platform runtime,
  packaging, verification, and release scripts.
- `scripts/nix/`: Nix development, build, run, repair, and packaging scripts.
- `MIGRATIONS.md`: authoritative migration architecture, dependency mapping,
  phases, visual targets, risks, and test strategy.
- `docs/building.md`: current Tauri-era platform build instructions.
- `docs/scripts.md`: script reference; verify entries against the filesystem
  before relying on them because migration cleanup may make it stale.

## Development Commands

The commands below operate on the current compatibility application.

### Frontend

- `pnpm install --frozen-lockfile`: install pinned frontend/Tauri CLI
  dependencies.
- `pnpm check`: run SvelteKit synchronization and Svelte type checking.
- `pnpm build`: create the current static frontend build.
- `pnpm dev`: run only the Vite development server.

### Desktop application

- `cargo test --manifest-path crates/revo-core/Cargo.toml`: run the extracted
  core tests without Tauri, WebKit, GTK, or libobs system dependencies.
- `./run.sh`: run `pnpm tauri dev` with libobs paths and the conditional Linux
  EGL/DMABUF workaround.
- `./run-cef.sh`: run the optional `cef` feature after validating
  `CEF_ROOT` or `CEF_INCLUDE_DIR`/`CEF_LIB_DIR`. The current CEF bridge is not a
  complete embedded CEF implementation; see `OBS_CEF_FEASIBILITY.md`.
- `cargo check --manifest-path src-tauri/Cargo.toml`: check Rust code when host
  Tauri/GTK/WebKit and libobs development dependencies are available.
- `cargo test --manifest-path src-tauri/Cargo.toml`: run Rust tests in a fully
  provisioned native environment.
- `./clean.sh`: remove `src-tauri/target` and `.svelte-kit`.

### NixOS

- `bash scripts/nix/dev.sh`: compile/run the Rust application in the Nix
  environment.
- `bash scripts/nix/dev.sh tauri`: run the complete Tauri development path.
- `bash scripts/nix/compile.sh`: build DEB and RPM bundles.
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

- `scripts/appimage/build-appimage.sh`: build the current Tauri AppImage.
- `scripts/linux/`: build/stage OBS, install dependencies, package portable
  releases, and verify Linux artifacts.
- `scripts/macos/`: prepare OBS/runtime dependencies, build and sign the DMG,
  and verify architecture, rpaths, dependencies, and bundle contents.
- `scripts/windows/`: prepare and verify OBS runtime files, create the portable
  ZIP, and smoke-test Windows packages.
- `.github/workflows/release.yml`: current cross-platform release pipeline.

Do not assume Tauri bundle paths or WebKit dependencies apply to the final
GPUI package. Define and test the GPUI resource layout before removing the
existing release path.

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
