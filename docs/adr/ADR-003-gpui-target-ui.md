# ADR-003: GPUI jako docelowy UI

## Status
Accepted (w trakcie migracji)

## Kontekst
Tauri/Svelte wymaga webview i JS/Rust IPC. Cel: natywny, szybki UI.

## Decyzja
Migrować do GPUI (Rust) jako główny UI, utrzymując Tauri jako parity reference do czasu cutover.

## Konsekwencje
- Brak webview i JS boundary.
- Większa odpowiedzialność za natywne funkcje (tray, dialogi, clipboard).
- Konieczność parity z istniejącym UI.