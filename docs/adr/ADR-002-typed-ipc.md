# ADR-002: Typed IPC instead of string commands

## Status
Accepted

## Context
Tauri used about 100 string commands. The GPUI migration required typed contracts.

## Decision
Use a `BackendCommand` enum + snapshots + length-prefixed JSON.

## Consequences
- Better type safety and testability.
- Every new feature must update the contract.
- Fewer runtime errors.