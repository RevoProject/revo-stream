# Plan naprawy

## 1. Graphic Planner — properties modal się zawiesza
**Pliki**: `src/lib/components/Modules/GraphicPlanner.svelte`
**Problem**: modal właściwości (X, Y, W, H, Rot, Crop) powoduje zamrożenie okna.
**Przyczyna**: prawdopodobnie reaktywność Svelte — cykl `patchActive()` → history checkpoint → rerender zawiesza się przy szybkiej zmianie wielu pól.
**Fix**: 
- Debounce historii (nie zapisuj checkpoint co każdą klawiszę)
- Używaj `on:input` zamiast `on:change` dla pól numerycznych
- Dodaj `parseFloat` guard przed zapisem do transforms

## 2. Text source — font picker
**Pliki**: `src/routes/+page.svelte` (sourceParamSchemas), `EditSourceModal.svelte`
**Problem**: tylko 4 fonty w `<datalist>`, brak przeglądarki fontów systemowych
**Fix**:
- Backend: dodaj komendę `obs_list_system_fonts` która zwraca listę fontów z fc-list lub fontconfig
- Frontend: przebuduj pole font_face na `<select>` z wyszukiwarką (combobox)
- Dodaj podgląd fontu w polu (opcjonalnie)

## 3. Image source — file picker w modalu po prawej
**Pliki**: `EditSourceModal.svelte`, `+page.svelte`
**Problem**: image_source pokazuje tylko tekstowe pole "file" bez przycisku Pickera
**Fix**:
- Dodaj przycisk "📁 Browse" obok pola `file` w EditSourceModal
- Użyj `@tauri-apps/plugin-dialog` `open()` z filtrami obrazów
- Dodaj też w AddSourceModal możliwość wybrania pliku przy tworzeniu źródła

## 4. Xcomposite window capture — brak pickera okien
**Pliki**: `src-tauri/src/sources/window_picker.rs`, `+page.svelte`
**Problem**: pokazuje "Title:Class" zamiast rozwijanej listy okien
**Przyczyna**: frontend czeka na `obs_list_window_picker_items` ale:
- Na Wayland/NixOS `wmctrl` może nie być dostępny albo zwracać puste dane
- Lista nie jest odświeżana
- Na NixOS brak `wmctrl` w shell.nix
**Fix**:
- Dodaj `wmctrl` (i `xdotool`/`xwininfo`) do `shell.nix` buildInputs
- Dodaj fallback enumeracji okien przez XCB/X11 bindings gdy wmctrl nie działa
- Frontend: odśwież listę przy otwarciu modalu + przycisk "Refresh"

## 5. Pulse/Pipewire audio — brak pactl
**Pliki**: `src-tauri/src/sources/devices/audio.rs`, `shell.nix`, `init.rs`
**Problem**: `failed to run pactl: No such file or directory`
**Fix**:
- Dodaj `pulseaudio` (lub `pulseaudio-light`) do `shell.nix`
- Alternatywnie: sprawdź `PULSE_SERVER` i `PIPEWIRE_RUNTIME_DIR` — jeśli pipewire jest aktywny, użyj pipewire-pulse (pactl powinien być dostępny przez nixpkgs#pulseaudio)
- **PipeWire**: NixOS 25.05 domyślnie używa PipeWire. OBS ma moduł `linux-pipewire` który jest blokowany w init.rs:259. Odblokuj go.
- Dodaj obsługę `pipewire_input_capture`/`pipewire_output_capture` jako aliasów do pulse

## 6. ALSA capture — nie działa / brak wsparcia
**Pliki**: `helpers.rs`
**Problem**: ALSA input capture nie jest zaimplementowane w helperach
**Fix**:
- Dodaj obsługę `alsa_input_capture` w `apply_source_params` (mapowanie `device_id`)
- To jest standardowy OBS source type więc powinien działać jeśli tylko parametry są poprawnie przekazywane
- Dodaj komendę `obs_list_alsa_devices` do enumeracji urządzeń ALSA

## 7. Image slideshow — nie działa / brak implementacji
**Pliki**: `helpers.rs`, `EditSourceModal.svelte`, `+page.svelte`
**Problem**: `slideshow` nie jest obsługiwany w backendzie ani frontendzie
**Fix**:
- Backend: dodaj sekcję `slideshow` w `apply_source_params` w helpers.rs
- Backend: dodaj komendę do zarządzania listą plików (dodaj/usuń/przestaw)
- Frontend: dodaj UI do zarządzania playlistą zdjęć (lista plików + przyciski Add/Remove/Move Up/Down + file picker)
- Wykorzystaj istniejący przycisk "Select file" do dodawania pojedynczych plików

## 8. Plugins — sprawdź czy działają poprawnie
**Pliki**: plugins_profiles.rs, PluginsModal.svelte, init.rs
**Sprawdzić**:
- Czy `runtime_plugins_dir` (`data/plugins/`) istnieje i jest wykrywany
- Czy plugin profile działają (save/load aktywnego profilu)
- Czy blokowanie modułów działa (init.rs:255-261)
- UI w PluginsModal.svelte: czy lista się renderuje, czy toggle działa

**W NixOS**: OBS pluginy są w nix store (już zsymlinkowane przez nas do `core/lib/obs-plugins/`). `runtime_plugins_dir` wskazuje na `data/plugins/` w projekcie — trzeba utworzyć ten katalog jeśli nie istnieje.

## 9. Audio mixer — sprawdź i popraw
**Pliki**: AudioMixer.svelte, AudioMixerAdvancedModal.svelte, +page.svelte
**Sprawdzić**:
- Czy poziom dźwięku (volume) jest poprawnie odczytywany i zapisywany
- Czy mute działa (param `muted`)
- Czy monitoring działa
- Czy wizualny level meter pokazuje rzeczywisty poziom (obecnie symulowany)

**Fix**:
- Dodaj rzeczywisty pomiar poziomu audio przez `obs_source_get_peak` lub `obs_get_audio_levels`
- Dodaj przycisk mute w mixer strip

## 10. Preview — stabilność i wydajność
**Pliki**: preview.rs, +page.svelte (preview loop)
**Problemy**:
- Każdy screenshot to pełny rerender sceny → CPU heavy
- PNG encoding na CPU (image crate) → wolne
- Brak cache'owania ramek

**Fix**:
- Zredukuj rozdzielczość preview dla trybu "medium" i niższych (już częściowo zrobione przez `getRenderScale`)
- Dodaj throttling: nie odświeżaj preview częściej niż co 33ms (30fps) nawet w trybie RAF
- Rozważ zapis do pliku zamiast base64 (mniejszy narzut IPC)
- Użyj istniejącego `previewInFlight`/`previewPendingRequest` (już działa)
