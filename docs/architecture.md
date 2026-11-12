# Architecture — RevoStream (GPUI rewrite)

## Goal
Describe the current architecture in a human-friendly way: what, where, and why.

## High level
```
GPUI UI process (revo-gpui)
        |
        | typed BackendCommand / snapshots / preview frames
        v
Dedicated engine process (revo-engine, libobs)
```

## Why two processes?
- libobs is heavy and crash-prone — isolation protects the UI.
- The UI stays responsive (no blocking of the foreground executor).
- Easier testing (mock backend without libobs).

## Responsibility boundaries
| Layer | Responsibility |
| --- | --- |
| `revo-core` | Domain models, settings, IPC, transport, `BackendService` |
| `revo-backend` | libobs runtime, scene/source operations, IPC server, `revo-engine` |
| `revo-gpui` | UI, actions, modals, windows, client services |

## Command flow (example: StartRecording)
1. A GPUI view triggers an action.
2. The action maps to `BackendCommand::StartRecording`.
3. `BackendService` sends the command to the engine (typed IPC).
4. The engine executes the operation in libobs.
5. The engine updates the snapshot (recording=true).
6. The UI receives the snapshot and renders state.

## Preview frames
- Current: PNG data URI in a bounded newest-wins slot.
- Target: raw RGBA / native textures (TODO).

## State and settings
- `BackendSnapshot` is the source of truth for the UI (scenes, sources, audio, outputs, settings).
- The UI keeps view state (modal stack, focus, layout) and does not duplicate backend logic.

## Transport
- Unix: Unix socket
- Windows: loopback TCP + `--endpoint-file`

## Legacy (parity reference)
`src-tauri/`, `src/`, `static/` remain until the final cutover.  
They are not the primary target, but support parity and behavior tests.

## Key rules (from AGENTS.md)
- Backend is independent of Tauri/GPUI.
- libobs only in `revo-engine`.
- Preview/audio: bounded newest-wins, never block the UI.
- One logical change per commit.