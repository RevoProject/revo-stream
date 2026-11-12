# ADR-002: Typed IPC zamiast stringowych komend

## Status
Accepted

## Kontekst
Tauri używało ~100 stringowych komend. Migracja do GPUI wymagała typowanych kontraktów.

## Decyzja
Użyć `BackendCommand` enum + snapshoty + length-prefixed JSON.

## Konsekwencje
- Lepsza bezpieczeństwo typów i testowalność.
- Konieczność aktualizacji kontraktu przy każdej nowej funkcji.
- Mniej błędów runtime.