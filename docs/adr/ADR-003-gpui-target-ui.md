# ADR-003: GPUI as the target UI

## Status
Accepted (migration in progress)

## Context
Tauri/Svelte requires a webview and JS/Rust IPC. Goal: native, fast UI.

## Decision
Migrate to GPUI (Rust) as the primary UI, keeping Tauri as a parity reference until cutover.

## Consequences
- No webview and no JS boundary.
- More responsibility for native features (tray, dialogs, clipboard).
- Need parity with the existing UI.