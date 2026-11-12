# Onboarding — RevoStream (GPUI rewrite)

This document helps a new contributor go from "clone the repo" to "run and understand the project" in about 15 minutes.

## 1. What this is
RevoStream is a desktop streaming/recording application (scenes, sources, preview, audio, outputs).  
The current branch (`feature/gpui-rewrite`) is a **migration** from Tauri/Svelte to native Rust + GPUI.

## 2. Two processes (most important)
- `revo-gpui` — the UI (GPUI, no webview)
- `revo-engine` — a separate process that owns OBS/libobs

The UI and engine communicate through **typed IPC** (Unix socket / TCP).

## 3. Fastest start (no libobs)
```bash
cargo run --manifest-path crates/revo-gpui/Cargo.toml
```
This runs the UI with mock data (best for UI development).

## 4. Run with the real backend
```bash
cargo build --manifest-path crates/revo-backend/Cargo.toml --bin revo-engine
cargo run --manifest-path crates/revo-gpui/Cargo.toml --features obs-backend -- --obs
```

## 5. Standard validation
```bash
bash scripts/core/verify.sh
```
Runs: `git diff --check`, `cargo fmt --check`, tests for `revo-core`/`revo-gpui`, `cargo check`, and `src-tauri` metadata.

## 6. Where things live
- `crates/revo-core/` — domain models, settings, IPC, transport, `BackendService`
- `crates/revo-backend/` — libobs runtime, operations, IPC server, `revo-engine`
- `crates/revo-gpui/` — UI (views, modals, windows, client services)
- `src/`, `src-tauri/`, `static/` — legacy (parity reference, do not remove)

## 7. Mental model (basic flow)
1. The GPUI view triggers an action.
2. The action becomes a `BackendCommand`.
3. The engine (libobs) executes the operation and updates state.
4. The UI receives a snapshot (and optionally a preview frame).

## 8. Common pitfalls
- Missing host packages (libobs, GTK/WebKit) — see `docs/building.md`.
- Wrong manifest path — each crate is built separately with `--manifest-path`.
- Treating Tauri as the primary UI — it is legacy, not the main target.

## 9. How to start contributing
- Read `CONTRIBUTING.md` and `AGENTS.md`.
- Make a small, focused PR (Conventional Commits).
- Run `verify.sh`.
- For UI changes: check RevoClassic/RevoFuture at 1080x600, 1280x720, 1440x900.

## 10. Where to get help
- `docs/building.md` — builds per platform
- `docs/scripts.md` — script reference
- `MIGRATIONS.md` — architecture and migration status
- Issue templates (bug/build/performance/feature)