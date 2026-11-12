# ADR-001: Separate engine process for libobs

## Status
Accepted

## Context
libobs is heavy and crash-prone. In Tauri everything ran in one process.

## Decision
Extract libobs into a dedicated `revo-engine` process, and let the UI (`revo-gpui`) communicate through typed IPC.

## Consequences
- Better UI stability.
- Harder state synchronization (snapshots, preview frames).
- Additional packaging complexity (two binaries).