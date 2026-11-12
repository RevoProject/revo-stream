# Onboarding — RevoStream (GPUI rewrite)

Ten dokument ma pomóc nowej osobie przejść od „klonuję repo” do „uruchamiam i rozumiem projekt” w ~15 minut.

## 1. Co to jest
RevoStream to desktopowa aplikacja do streamingu/recordingu (sceny, źródła, preview, audio, wyjścia).  
Obecna gałąź (`feature/gpui-rewrite`) to **migracja** z Tauri/Svelte do natywnego Rust + GPUI.

## 2. Dwa procesy (najważniejsze)
- `revo-gpui` — interfejs (GPUI, bez webview)
- `revo-engine` — osobny proces posiadający OBS/libobs

UI i engine rozmawiają przez **typed IPC** (Unix socket / TCP).

## 3. Najszybszy start (bez libobs)
```bash
cargo run --manifest-path crates/revo-gpui/Cargo.toml
```
To uruchamia UI na danych mock (najlepsze do developmentu UI).

## 4. Uruchomienie z realnym backendem
```bash
cargo build --manifest-path crates/revo-backend/Cargo.toml --bin revo-engine
cargo run --manifest-path crates/revo-gpui/Cargo.toml --features obs-backend -- --obs
```

## 5. Standardowa walidacja
```bash
bash scripts/core/verify.sh
```
Uruchamia: `git diff --check`, `cargo fmt --check`, testy `revo-core`/`revo-gpui`, `cargo check`, metadata `src-tauri`.

## 6. Gdzie co jest
- `crates/revo-core/` — modele domenowe, settings, IPC, transport, `BackendService`
- `crates/revo-backend/` — libobs runtime, operacje, IPC server, `revo-engine`
- `crates/revo-gpui/` — UI (widoki, modale, okna, serwisy)
- `src/`, `src-tauri/`, `static/` — legacy (parity reference, nie usuwać)

## 7. Podstawowy przepływ (mentalny model)
1. UI (GPUI) wywołuje akcję.
2. Akcja trafia do `BackendService` jako `BackendCommand`.
3. Engine (libobs) wykonuje operację i aktualizuje stan.
4. UI dostaje snapshot (i ewentualnie preview frame).

## 8. Najczęstsze pułapki
- Brak hostowych pakietów (libobs, GTK/WebKit) → sprawdź `docs/building.md`.
- Zła ścieżka manifestu — każdy crate budujemy osobno (`--manifest-path`).
- Próba użycia Tauri jako głównego UI — to legacy, nie główny target.

## 9. Jak zacząć kontrybuować
- Przeczytaj `CONTRIBUTING.md` i `AGENTS.md`.
- Zrób mały, skupiony PR (Conventional Commits).
- Uruchom `verify.sh`.
- Jeśli zmieniasz UI: sprawdź RevoClassic/RevoFuture na 1080x600, 1280x720, 1440x900.

## 10. Gdzie szukać pomocy
- `docs/building.md` — build per platforma
- `docs/scripts.md` — opis skryptów
- `MIGRATIONS.md` — architektura i status migracji
- Issues z template’ami (bug/build/performance/feature)