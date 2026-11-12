# Troubleshooting — RevoStream

## Build: libobs / OBS runtime
**Objawy:** błędy linkowania, brak `libobs`, `obs.dll` not found.
**Sprawdź:**
- Zainstalowane dev packages (`libobs-dev`, OpenSSL, GLib).
- `docs/building.md` → sekcja per platforma.
- Czy używasz real engine: `--features obs-backend -- --obs`.

## Build: brak pakietów GTK/WebKit (legacy Tauri)
**Objawy:** błędy przy `cargo check --manifest-path src-tauri/Cargo.toml`.
**Uwaga:** to legacy path. Dla GPUI nie są potrzebne.

## Uruchomienie: engine nie startuje
**Sprawdź:**
- Czy `revo-engine` zbudowany.
- `REVO_ENGINE_SOCK` jeśli łączysz się z zewnętrznym engine.
- Logi UI i engine.

## Windows: obs.dll nie znaleziony
**Znane:** Windows jest częściowo wspierany.
**Sprawdź:**
- `launch.cmd` w paczce.
- Ścieżki runtime (`revo-root`).
- Czy używasz packaged artifact z release.

## macOS: problemy z rpaths / signing
**Znane:** macOS eksperymentalny.
**Sprawdź:**
- `REVO_TARGET`, `REVO_ARCH`.
- Skrypty `scripts/macos/*`.
- Diagnostyka: workflow `macos-diagnostics.yml`.

## UI: preview wolny / niska jakość
**Znane:** preview aktualnie base64 PNG.
**Sprawdź:**
- Ustawienie jakości preview.
- Real engine vs mock.
- GPU/driver (Vulkan loader na Linux).

## Testy: `verify.sh` pada
**Sprawdź:**
- `git diff --check` (whitespace).
- `cargo fmt --check` (formatowanie).
- Czy nie ma niezacommitowanych zmian w formatowaniu.

## Gdzie szukać dalej
- `docs/building.md`
- `docs/scripts.md`
- Issues z template’ami (build/performance)