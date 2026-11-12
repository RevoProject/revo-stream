# revo-core

Framework-neutral domain models and services for RevoStream.

## Contains
- Models (settings, scenes, plugins, sources)
- Planner geometry
- Typed IPC (`ipc.rs`)
- Transport (Unix socket / TCP)
- `BackendService` worker boundary

## Rules
- No Tauri/GPUI/libobs dependencies.
- Keep pure logic testable.