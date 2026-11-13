# Zmiany — revo-gpui 0.5.0-pre1 (Windows x64), 2026-11-13

## Objaw

`revo-gpui.exe` uruchamiał się, ale `revo-engine.exe` (revo-core / libobs) nigdy
nie startował: brak procesu `revo-engine`, brak endpointu IPC, brak katalogu
logów, podgląd bez obrazu, w UI widoczne sceny demo
(„Starting Soon / Main Camera / Be Right Back”).

## Przyczyna 1 (główna): brak flagi `--obs`

W `crates/revo-gpui/src/app.rs` backend wybierany jest na podstawie argumentów:

```rust
let use_obs_backend = std::env::args().any(|argument| argument == "--obs");
// ...
let backend = if use_obs_backend { AnyBackend::obs() } else { AnyBackend::mock() };
```

`launch.cmd` uruchamiał UI bez `--obs`, więc aplikacja działała na wbudowanym
mock backendzie (dane demonstracyjne) i nie próbowała nawet uruchomić silnika.

## Przyczyna 2 (ukryta): zła ścieżka `REVO_ENGINE_BIN`

`launch.cmd` ustawiał `REVO_ENGINE_BIN=%APPDIR%revo-engine.exe`, ale w tej
paczkie silnik znajduje się w `revo-root\core\lib\revo-engine.exe`.
Kolejność szukania w `crates/revo-gpui/src/services/engine.rs`:

1. `REVO_ENGINE_BIN` (tylko gdy `is_file()`),
2. `revo-engine.exe` obok `revo-gpui.exe`,
3. `../revo-backend/target/{debug,release}/revo-engine.exe` (ścieżki dev).

Żadna z nich nie istniała, więc `EngineClient::new()` kończyło się błędem
`engine binary not found`, a worker panikował z
`failed to start revo-engine process`. Samo dodanie `--obs` nie wystarczało.

## Zmienione pliki

### `launch.cmd`

```diff
 set "APPDIR=%~dp0"
 set "REVO_ROOT=%APPDIR%revo-root"
-set "REVO_ENGINE_BIN=%APPDIR%revo-engine.exe"
+rem revo-engine.exe lives inside the runtime directory in this package (not next to the UI).
+if not defined REVO_ENGINE_BIN set "REVO_ENGINE_BIN=%APPDIR%revo-root\core\lib\revo-engine.exe"
 if not defined REVO_DATA_DIR set "REVO_DATA_DIR=%LOCALAPPDATA%\com.klubuntuprojects.revo"
 set "PATH=%APPDIR%revo-root\core\lib;%PATH%"
-start "" "%APPDIR%revo-gpui.exe" %*
+if not exist "%REVO_ENGINE_BIN%" echo [revo] WARNING: engine binary not found: "%REVO_ENGINE_BIN%" - OBS runtime will not start.
+rem --obs is required: without it the UI runs the built-in mock backend and never spawns revo-engine.
+start "" "%APPDIR%revo-gpui.exe" --obs %*
```

Dodatkowo `REVO_ENGINE_BIN` nie jest już nadpisywany, jeśli użytkownik ustawił
go samodzielnie.

### `README.txt`

Poprawiony opis pakunku: faktyczna lokalizacja `revo-engine.exe`, wymóg flagi
`--obs`, znaczenie zmiennych środowiskowych, lokalizacja logów i diagnostyka
(F10 = Debug Console).

## Weryfikacja

| Konfiguracja | Wynik |
| --- | --- |
| `revo-gpui.exe` bez `--obs` | 0 procesów `revo-engine`, brak logów — mock backend |
| `--obs` + stara ścieżka `%APPDIR%revo-engine.exe` | silnik nie startuje (`engine binary not found`) |
| `--obs` + `revo-root\core\lib\revo-engine.exe` | `revo-engine.exe` jako proces potomny UI, endpoint `tcp:127.0.0.1:<port>`, log: D3D11 OK, video/audio reset, pętla `preview-capture` |
| finalny `launch.cmd` | UI + silnik, świeży log w `%LOCALAPPDATA%\com.klubuntuprojects.revo\logs\2026\` |

Test samodzielnego silnika (`revo-engine.exe --endpoint-file <plik>`) również
przechodzi: publikuje endpoint i inicjalizuje libobs, więc runtime `revo-root`
i `obs.dll` są kompletne.

## Jak zastosować

Skopiuj zawartość tego katalogu (`launch.cmd`, `README.txt`, `ZMIANY.md`) do
katalogu głównego paczki `revo-gpui-0.5.0-pre1-windows-x64`, nadpisując pliki.
`launch.cmd.orig` to kopia oryginału — służy do wycofania zmiany.

## Uwagi dla pakowania / CI

- `README.txt` w oryginalnej paczce twierdził, że silnik jest uruchamiany
  automatycznie, a `revo-engine.exe` leży obok UI — oba stwierdzenia były
  nieprawdziwe dla tego artefaktu.
- Alternatywa zgodna z pierwotnym README: dostarczać `revo-engine.exe` obok
  `revo-gpui.exe` (wtedy `REVO_ENGINE_BIN` nie jest potrzebny), ale flaga
  `--obs` nadal jest wymagana.
- Warto dodać test dymny w CI: start paczki i sprawdzenie, że istnieje proces
  `revo-engine` oraz katalog logów.
