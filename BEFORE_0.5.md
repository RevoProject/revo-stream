# RevoStream — plan poprawek przed 0.5.0 (BEFORE_0.5)

Data: 11.11.2026. Cel wydania: `0.5.0-pr1`.
Priorytety: **P0** = blokuje wydanie, **P1** = duża usterka UX/funkcji, **P2** = dopracowanie.
Sekcja **25** na końcu zawiera przykładowe szkielety implementacji („jak bym to naprawił") dla
najważniejszych punktów — plan, nie wdrożenie; żaden plik źródłowy nie został zmieniony.

Zasady z AGENTS.md obowiązują: jeden logiczny commit (Conventional Commits), backend niezależny od GPUI,
żadnej pracy libobs/IO na foreground executorze, parity Tauri zostaje buildowalne.

---

## 0. Bump wersji do 0.5.0-pr1 (P0)

Punkty do zmiany (wszystkie na raz, osobny commit `chore(release): bump version to 0.5.0-pr1`):

- `crates/revo-core/Cargo.toml`, `crates/revo-backend/Cargo.toml`, `crates/revo-gpui/Cargo.toml`, `src-tauri/Cargo.toml` — `version = "0.4.1"` → `0.5.0-pr1` (sprawdzić, czy workspace nie wymusza jednego pola `workspace.package.version`).
- `package.json`, `src-tauri/tauri.conf.json` — `0.4.1` → `0.5.0-pr1`.
- `AGENTS.md` linia „Application version" → `0.5.0-pr1`.
- Nazwy artifactów release (`*-windows-x64`, `verify-gpui-release.sh`, workflow `release.yml`) pobierają wersję z Cargo, więc powinny pójść automatycznie — zweryfikować po bumpie.

---

## 1. Windows build (P0)

Stan: zip ✅, exe (NSIS) ✅, msi (WiX) ✅ — ale:

### 1.1 MSI nie tworzy skrótu w Menu Start (P0)

- `scripts/windows/package-gpui.ps1` ~linia 189: skrót Start Menu jest **świadomie pominięty**
  („No Start Menu shortcut here: a shortcut lives in a per-user profile…").
- Plan: dodać do WiX `Shortcut` w `per_user`/`perMachine` komponentcie (Directory `ProgramMenuFolder`),
  zależnie od trybu instalacji; przetestować `smoke-test-msi.ps1` (rozszerzyć o asercję, że plik `.lnk`
  powstaje po `msiexec /i`) i `verify-msi.ps1`.

### 1.2 Ikona Revo w Menu Start (exe i msi) (P0)

- NSIS: `MUI_ICON` jest już ustawiany z `src-tauri/icons/icon.ico` (package-gpui.ps1 ~114) —
  upewnić się, że `CreateShortcut` używa `$INSTDIR\revo-gpui.exe` (lub launcher) i że exe ma
  osadzoną ikonę (Windows Embed-Resources / `winres`) — obecnie ikona jest tylko w stage'u jako `revo-gpui.ico`,
  nie w pliku PE. Dodać zasobnik ikony do builda binarek (build.rs z `windows_resources`/`embed-resource`).
- MSI: `Icon` element WiX wskazujący zaimportowany `.ico` + `Shortcut Icon="revo.ico"`.

### 1.3 Opcjonalny skrót na pulpicie (P1)

- NSIS: `!insertoption MUI_PAGE_CUSTOMFUNCTION` / standardowa opcja „Create a desktop shortcut"
  w sekcji komponentów (`Section /o "Desktop shortcut"`).
- MSI: opcjonalna własność publiczna `ADDDESKTOPSHORTCUT=1` + folder `DesktopFolder` (bez UI-choice,
  bo MSI nie ma kreatora — dokumentacja w `docs/` wyjaśni `msiexec /i … ADDDESKTOPSHORTCUT=1`).

### 1.4 OBS nie startuje w buildzie Windows — revo-engine nie widzi `obs.dll` (P0)

- Objaw: uruchomienie silnika bezpośrednio ⇒ `STATUS_DLL_NOT_FOUND (0xC0000135)`;
  `smoke-test-gpui.ps1` ~26 i ~41 opisują, że naprawką jest `launch.cmd` ustawiający PATH do
  `revo-root\core\lib`.
- Plan (do wyboru, najlepiej 1+2):
  1. Silnik przy starcie woła `SetDefaultDllDirectories`/`AddDllDirectory` na
     `<exe_dir>\revo-root\core\lib` przed importem `obs.dll` — działa bez launchera;
     wymaga, by załadowanie libobs było dynamiczne (`libloading`) albo by bootstrap
     (`patch-libobs-bootstrapper.ps1`) był i tak dynamiczny — sprawdzić `crates/revo-backend/obs_service.rs`.
  2. Skopiować (lub zrobić junction) `obs.dll` + dependencies obok `revo-engine.exe` w stage'u
     `package-gpui.ps1`; zaktualizować `verify-gpui-package.ps1` (linia ~43 mówi, że launch.cmd jest
     „load-bearing" — po naprawie zaktualizować komentarz i testy).
- Kryterium: `revo-engine.exe` uruchomiony dwuklikiem (bez `launch.cmd`) loguje boot snapshot i podnosi libobs.

### 1.5 Autostart OBS z poziomu launch (P1)

- „launch.md" = ścieżka startowa (launch.cmd / skróty). Docelowo: GUI (`revo-gpui`) startuje silnik
  sam i od razu wydaje `Connect`/`StartObs` (tak jak `--obs` flaga na Linuxie), bez ręcznego
  włączania OBS. Zweryfikować w `crates/revo-gpui/src/services/engine.rs` kolejność: spawn engine →
  endpoint → auto `Initialize`/`Startup`. Dodać ustawienie „Launch OBS on startup" (parity z Tauri) —
 General (patrz 3.0).

---

## 2. Wszystkie platformy — zachowanie modali (P0/P1)

### 2.1 Brak scrolla w modalu ustawień (P0)

- `modal_shell.rs:127-137` — karta modalowa ma `overflow_y_scroll()`+`scrollbar_width(8px)`, ale
  Settings (`settings_modal.rs:1506` przez `modal_shell_with_body`) przy małych oknach nie pozwala
  przewijać treści (body prawdopodobnie wychodzi poza wysokość karty bez ograniczenia `max-h`).
- Plan: wymusić na body Settings `flex_1().min_h_0().overflow_y_scroll()` wewnątrz karty; pasek w stylu
  cienkiego paska Edge (GPUI `scrollbar` zmotywować na `theme.surface_active`). Dotyczy też modalów
  Edit Source (pkt 14) i Slideshow (14.6) — zrobić jeden helper w `modal_shell.rs`, którego używają
  wszystkie modale.

### 2.2 Modale przepuszczają kliknięcia przez tło i aktywują kontrolki pod spodem (P0)

- Overlay `modal_shell_with_body` (`modal_shell.rs:211-220`, `.bg(theme.overlay)`) nie blokuje
  hit-testowania ani nie jest w pełni nieprzezroczysty — kliknięcie obok karty wykonuje akcję pod modalem.
- Plan:
  - tło `theme.overlay` ustawić na nieprzezroczyste lub bardzo wysokie alpha (np. `rgba(0,0,0,0.75)`)
    w obu motywach (RevoClassic/RevoFuture) — `crates/revo-gpui/src/theme.rs`;
  - overlay przechwytuje zdarzenia myszy (`.on_mouse_down`/`.absolute()` + najwyższy z-index warstwy
    modalowej w `app.rs` render) i ewentualnie zamyka modal po kliknięciu w tło (ustawienie „click
    outside to dismiss" dla modalów bez formularzy);
  - formularze (Settings, Edit Source) mają **nie** zamykać się kliknięciem w tło, żeby nie gubić draftu.

### 2.3 „Close" → ikona „X" z czerwonym hoverem (P2)

- Zamiast przycisku „Close" w nagłówkach modali (`modal_shell.rs`, `modal_card`) — kwadratowa ikona X
  (Lucide `x` w `components/icon.rs`) po prawej; hover = czerwone tło/obwódka w kolorze semantycznym
  `theme.danger` (oba motywy). Zachować dostępność: `id`, tooltip „Close", `Escape` nadal działa.

---

## 3. Settings > General

### 3.1 Eksport/Import scen (RevoStream i OBS Studio) — natywne dialogi plików (P1)

- `settings_modal.rs:534` („Scene/profile transfer needs the native file dialog — pending") —
  akcje są atrapą. Plan: w GPUI użyć natywnego pickera (GPUI ma `cx.prompt_for_paths` /
  PathPromptOptions na Linuxie X11/GTK oraz analog w Windows/macOS); komenda backendu
  `ExportScenes { path }` / `ImportScenes { path }` (revo-backend już ma ścieżki transferu scen
  używane przez Tauri — mapować przez `BackendCommand` w `services/obs.rs` i `engine.rs`).
  Eksport bez wskazanego pliku = od razu zapis do `record_path`/data dir i powiadomienie (download
  jak w przeglądarce = dialog „Zapisz jako").
- Usunąć tekst „— pending" po wdrożeniu; ten sam mechanizm zasili 4.2, 11.2.

### 3.2 Dropdowny: klik tylko w pole, nie w cały wiersz (P1)

- `settings_modal.rs` `dropdown_row` (~330) / `cycle_row` (~280): obsługa kliku jest na całym wierszu
  (`action_row`/wiersz `.on_click`). Plan: `on_click` tylko na elemencie-select (prawa kolumna),
  etykieta pozostaje opisem; najechać kursorem tylko nad polem pokazuje `cursor pointer`.
  Dotyczy też `profile_field_row`, `toggle_row` zostaje jako jest (przełącznik).

### 3.3 Ustawienie autostartu OBS (P2, powiązane z 1.5)

- Dodać toggle „Launch OBS on startup" do General i realizować w `app.rs` przy starcie silnika.

---

## 4. Settings > Look

### 4.1 RevoClassic nie stosuje się do niektórych modali (Plugins, Settings) (P0)

- `plugins_modal.rs`, `settings_modal.rs` (i część `source_properties_modal.rs`) używają
  `theme.surface_elevated`/`overlay` częściowo hardkodowanymi kolorami albo inicjalizują własny
  `RevoTheme::default()` zamiast motywu aplikacji. Plan: audyt `RevoTheme::classic()` w tych plikach —
  motyw musi być przekazywany z `app.rs` przez `cx.update` (jeden source of truth); dodać test
  „settings modal renders with classic palette" (porównanie stałych w `theme.rs`).

### 4.2 „Extract Default Theme" nic nie rozpakowuje (P1)

- Akcja w Look (sekcja `section_body`, SettingsSection::Look) — podpiąć pod istniejące API backendu
  (revo-backend themes) z natywnym dialogiem zapisu/katalogu (mechanizm z 3.1). Docelowo: wybór
  katalogu docelowego → rozpakowanie przykładowego motywu (jak w Tauri) → komunikat o sukcesie.

---

## 5. Settings > Broadcast

### 5.1 Kursor tekstowy w polach (P1) — wspólne z 7/8

- `components/text_field.rs` (76 linii): `apply_line_keystroke` tylko append/backspace — brak pozycji
  kursora ani zaznaczenia. Plan: zamienić na GPUI `Entity<InputState>` (gpui `text::Input`) wszędzie,
  gdzie w GPUI shellu jest pole tekstowe (ustawienia, właściwości źródeł, quick text modal).
  Ujednolicić helperem `text_input_row(...)` w `modal_shell.rs`, żeby nie przepisywać 26 wywołań
  `profile_field_row`/`field_row`.
### 5.2 Pole „Key" szersze, margines 15px od Hide/Show (P2)

- Wiersz klucza w `settings_modal.rs` Broadcast: dać `flex_1` polu i `mr(15px)` przed przyciskiem
  Hide/Show; klucz maskowany (•••) z jawieniem po kliknięciu (istniejące `key_visible`).

---

## 6. Settings > Output

### 6.1 Audio/Video Encoder — puste; mają być listy z libobs (P1)

- Obecnie: `profile_field_row("Video Encoder"…)` (settings_modal.rs:820-821) i `RECORD_FORMATS`/consty
  hardkodowane (linie 37-74). Plan: rozszerzyć `BackendSnapshot` (`revo_core::BackendSnapshot`) o
  `encoder_options: {id,name}[]` per encodera; `services/obs.rs` pobiera je z enkapsulatora
  `obs_enum_encoder_types`/`obs_encoder_get_display_name` w `revo-backend/src/obs_service.rs` (już
  iteruje typy enkoderów przy tworzeniu). Mock backend zwraca znaną listę.
### 6.2 Bitrate / Custom muxer settings — możliwość wpisania liczby/tekstu (P1)

- Po 5.1 (prawdziwe pole tekstowe) walidacja w `settings_state.rs`: bitrate = liczba całkowita w
  zakresie (np. 10–500000 kbps), muxer settings = dowolny tekst; błędy pod polem, nie blokuj Save
  (parzyste z komunikatami Tauri).
### 6.3 Kursor w polach (P1) — patrz 5.1.
### 6.4 Puste pola Audio → dropdowny z libobs (P1) — patrz 6.1 (audio encoders + bitrate/sample rate z
  opcji enkodera, nie z constów).

---

## 7. Settings > Audio — listy urządzeń z libobs (P1)

- Desktop/Mic/Monitor: `devices` istnieją w `revo-backend/src/devices` (Tauri `get_audio_devices`);
  dodać `BackendCommand::ListAudioDevices` → `BackendSnapshot.audio_devices`, wypełnić dropdowny w
  sekcji Audio (`settings_modal.rs:872+`) i w `quick_device_modal.rs`. Na mock backendzie lista
  przykładowa; real backend enumeruje przez `obs_enum_input_types`/device id (Windows: WASAPI,
  Linux: pulse/openal, macOS: coreaudio).

---

## 8. Settings > Video

### 8.1 Resolucje z nazwami (P1)

- Base/Output resolution: lista `[{ "1920x1080", "Full HD (1080p)" }, { "1280x720", "HD (720p)" },
  { "2560x1440", "QHD (1440p)" }, { "3840x2160", "UHD (4K)" }, …]` jako dropdowny z podglądem
  „1280×720 (HD)"; opcja „Custom" rozwala pola liczbowe. Źródło listy: helper w `revo-core::planner`
  (already parses resolutions) → `RESOLUTION_PRESETS` w revo-core, współdzielone z GPUI.
### 8.2 FPS: Conditional pola (P1)

- `section_body` Video renderuje teraz wszystkie pola naraz; renderuj warunkowo wg `FPS_TYPES` (Common →
  dropdown COMMON_FPS, Integer → pola liczbowe, Fractional → pola „numerator/denominator" jak Tauri).
  Stan już istnieje w `ui_profile` — tylko warunkowy render w `settings_modal.rs`.

---

## 9. Settings > Shortcuts — dialog nagrywania klawiszy (P1)

- Lista skrótów: `settings_modal.rs:1104-1117` (14+ akcji, zapis w `ui_profile` jako string).
- Plan: przycisk „…" przy każdym skrócie otwiera mały modal centralny (nowy `shortcut_capture_modal.rs`
  w `components/modal/`): fokus na panelu, `window.on_key_down` łapie combo (Ctrl/Alt/Shift/Super + key),
  display typu `Ctrl+Shift+F9`, przyciski **Cancel/Accept** + `Escape`=cancel; Accept zapisuje do draftu,
  wykrywanie konfliktów (ten sam combo użyty gdzie indziej → ostrzeżenie), „×" czyszczący skrót.
- Backend: `BackendCommand::SetShortcut`? Na razie wystarczy `ui_profile` (SaveSettings), bo akcje są
  wykonywane w GPUI (actions.rs). Wykonywanie skrótów globalnych — osobny temat poza 0.5.

---

## 10. Settings > Accessibility — faktyczna implementacja (P1)

- Opcje są w UI i `ui_profile`, ale nic nie konsumuje ich w renderze. Plan:
  - **High Contrast**: wariant palety w `theme.rs` (`RevoTheme::classic_high_contrast()` i odpowiednik
    Future: grubsze obwódki, maksymalny kontrast teksta/tła); aplikować globalnie w `app.rs` po
    załadowaniu ustawień.
  - **Reduce Motion**: flaga w stanie aplikacji; wyłączyć animacje/przejścia (fade przy przełączaniu
    scen, animacje paska poziomu w audio mixerze, animacje dropdownów).
  - **Stronger Keyboard Focus**: widoczna 2-3px obwódka fokusu wokół aktywnego pola/przycisku
    (teraz fokus jest ledwo widoczny) — centralnie w helperach przycisków/pól.
  - **Font size** (Small/Default/Large/Extra large) → mnożnik rozmiaru bazowego teksta w `theme.rs`
    (skalowanie wszystkich `text_*`); **Font family** → „Domyslny / Szeryfowy / Monospace"
    (`FONT_FAMILIES` w settings_modal.rs:70 zmienić na Default/Serif/Monospace zgodnie z WCAG),
    mapowane na gpui `window.text_style().font_family` per theme; **Color vision modes** → palety
    protanopia/deuteranopia/tritanopia dla kolorów semantycznych akcentu (kalkulowane w `theme.rs`,
    tablice w revo-core theme models). Wszystkie zmiany natychmiastowe po Save bez restartu.
- Kryterium WCAG: kontrast tekstu ≥4.5:1 w każdym motywie + trybach (test jednostkowy na palecie).

---

## 11. Settings > Other

### 11.1 Języki z % tłumaczenia (P2)

- `settings_modal.rs:1167`: lista „English, Polski, Deutsch, Español". Dopóki nie ma tłumaczeń:
  etykiety „Polski (0%)", „Deutsch (0%)", „Español (0%)", „English (100%)"; opcja nieaktywna poza
  English (disabled dropdown item). Docelowo i18n-crate osobno (poza zakresem 0.5).
### 11.2 Profile Import/Export — dialogi plików (P1)

- Te same „pending" co 3.1/4.2 (settings_modal.rs ~1087, ~1178) — podpiąć pod natywny picker +
  istniejące komendy backendu profili (`runtime/scenes` transfer w revo-backend).

---

## 12. Confirmation dialogs start/stop stream & recording (P0)

- Ustawienia toggles istnieją, ale `quick_modal/stream_confirm_modal.rs` nie jest podłączony w
  ścieżkach Start/Stop w `app.rs` (brak referencji) ⇒ nagrywanie/stream startuje od razu.
- Plan: routing akcji `StartStreaming`/`StopStreaming`/`StartRecording`/`StopRecording`
  (header/tools/skróty/context menu) przez stan `ConfirmModal`: jeśli właściwy toggle ustawiony —
  otwórz modal z podsumowaniem (service/ścieżka pliku), Accept Emit → backend command, Cancel → no-op;
  „Do not ask again" = wyłączenie toggle'a i zapis przez SaveSettings. Testy w `app.rs` route tests.

---

## 13. Co jeszcze warto poprawić w ustawieniach (audyt kodu, P2 jeśli nie zaznaczono)

Z lektury `settings_modal.rs` / `settings_state.rs` wynika dodatkowa lista:

1. **Brak walidacji draftu przy Save** — nieprawidłowy URL (rtmp), bitrate itp. pokazują tylko
   komunikat przy polu; dodać zbiorczy stan „N błędów" w stopce + przełączanie do sekcji z błędem.
2. **Przycisk Save nie sygnalizuje zmian** — dodać stan „unsaved changes" (kropka/`•` przy pozycji railu,
   wierszu lub przy ikonie sekcji, która ma zmianę) + `Escape`/Close pyta „Discard changes?" jeśli draft
   nieczystry (helper `SettingsDraft::is_dirty` już policzalny).
3. **Dropdowny nie zamykają się po `Escape` i nie mają nawigacji klawiaturą** (strzałki/Enter w
   `dropdown_popup` ~394) — dostępność + spójność.
4. **Utrata pozycji scrolla i fokusu po zmianie sekcji/rerenderze** — `boxed_id` tworzy nowe id;
   stabilne id per sekcja, przywracanie offsetu.
5. **Blokada klików przez otwarty popup dropdowna** — popup poza kartą wychodzi poza overlay (patrz 2.2).
6. **BROADCAST_SERVES hardkodowane (linie 77-82)** — wczytywać z `services.json` jak Tauri/revo-backend;
   poza Twitch/YT/FB też np. Kick/TikTok/Custom RTMP + osobne pole Server.
7. **Brak informacji o stanie zapisu** — po SaveSettings backend zwraca wynik; dodać toast „Settings saved"
   / „Failed: …" (kopiowanie stylu powiadomień błędu z `app.rs` error banner).
8. **Tab „Output" sub-tabs Streaming/Recording/Audio** — przy wąskim oknie taby się nie przewijają;
   dodatkowo brak podglądu wartości „Advanced" po zwinięciu.
9. **Widoczność klucza streamu** — Hide/Show natychmiast, ale po 5 s autoukrywanie jak w Svelte
   (opcjonalnie, dla parity).
10. **Spelling w UI**: „familly"→„family", myślniki/„— pending" w tekstach użytkownika — teksty
    „Scene/profile transfer needs the native file dialog — pending" zastąpić realnymi komunikatami.
11. **Brak „Reset to defaults" per sekcja** (jest globalny w Tauri ścieżce) oraz brak wersji aplikacji
    w Settings > Other.
12. **UI scale (UI_SCALES)** nie wpływa nic na render GPUI — albo zaimplementować skalowanie,
    albo oznaczyć „(nieaktywne)".

---

## 14. Edit Source / Source properties (P0–P1)

Wspólne dla wszystkich typów źródeł (`source_properties_modal.rs`, 2194 linie):

- **14.0 Read-only pola** (P0): pozycje X/Y, Width, Height, Rotation, font size/text, filtry,
  ścieżka obrazka itp. są renderowane jako nieedytowalne. Po przejściu na GPUI `InputState` (pkt 5.1)
  każda komórka wiersza-property ma być prawdziwym polem z walidacją per typ (`PNA`/`NUMBER`/`DOUBLE`/
  `TEXT`/`LIST`/`COLOR`/`PATH` z pickera) tak, by „Position X must be a whole number" dało się
  rozwiązać wpisaniem wartości. **To warunek zapisania zmian nowego źródła** — dziś de facto blok.
- **14.1 Scroll w oknie properties** (P0): patrz 2.1 helper; lista property potrafi przekroczyć
  wysokość karty.
- **14.2 Text input mode (P1)**: „Text input mode" przełącza się na `false` i nie da się ustawić —
  duplikat górnego „mode"; zduplikowaną pozycję usunąć z listy (filtrować `is_duplicate_property`
  dla text sources), a samą logikę naprawić przez mapowanie enuma z backendu (opcja→indeks).
- **14.3 Color picker** (P1): `components/color_picker.rs` — oprócz wizualnego pickera dodać edytowalne
  pole HEX (InputState, walidacja `#rrggbb`/`rrggbbaa`, normalizacja) + RGBA liczbowo.
- **14.4 Apply → zamknięcie** (P1): po `Source updated` (sukces) modal ma się zamknąć — obecnie zostaje
  otwarty; zachować otwarty przy błędzie walidacji. (Uwaga: decyzja do potwierdzenia z parity Tauri.)
- **14.5 Flash przy zapisie ustawień źródła** (P1): po aktualizacji ustawień render całego okna
  przeładowuje kadr (widoczne mryganie). Plan: nie rebuildować drzewa — mutować istniejące entity
  (`notify` na source panelu + wymiana tylko klatek preview); sprawdzić ścieżkę `apply_snapshot`/
  `rebuild` w `app.rs` (measured containers, `studio_*_frame`) i ograniczyć re-render do paneli,
  których dotyczą zmiany; cache'ować `GpuiImage` niezmienionych klatek.
- **14.6 Image**: picker pliku w polu `file`/`file_path` (typ PATH) + edytowalne pole tekstowe ścieżki.
- **14.7 Display Capture**: pola edytowalne (patrz 14.0), dropdowny „…" (window/list capture) otwierać
  realną listę okien/ekranów z backendu (revo-backend devices/`obs` enum) zamiast atrapy.
- **14.8 Slideshow**: scroll (14.1), folder picker (PATH), lista plików z backendu.
- **14.9 Media/FFmpeg/VLC**: playlista — dodać sub-view „Lista plików" (dodaj/usuń/przesuń) z pickera
  plików, zapis listy jako wartość `local_file`/playlist; opcje (restart, close when ended) jako listy.
- **14.10 Camera/Video Capture/v4l2**: po wyborze urządzenia — odświeżenie właściwości z backendu
  (`RefreshSourceProperties` command → re-enumeracja `resolutions/video format/video standard/
  dv timing/frame rate/color range` dla wybranego device id), picker u góry ma zostać listą
  (aktywny wybór zaznaczony), nie readonly.

---

## 15. Interakcje scen/źródeł (P1–P2)

### 15.1 Dwuklik na źródło otwiera modal edycji (P1)

- `sources_panel.rs` nie ma obsługi double-click; dodać `detail()==2` (gpui ClickEvent) →
  emit `EditSource(name)` → `source_properties_modal` / `edit_source_modal.rs`. Docelowo też
  dwuklik na scenie = rename inline.

### 15.2 Ikony [active]/[locked] przy scenie (P2)

- `scenes_panel.rs` renderuje tekst `[active]`/`[locked]`; zamienić na ikony Lucide: `circle`/`radio`
  (active — akcentowy kolor motywu) i `lock` (zamknięta kłódka dla locked, `unlock` przy odblokowanej),
  z tooltipem i zachowaniem kontrastu w obu motywach.

### 15.3 Undo/Redo history nie odświeża sources/frame/scenes (P1)

- `history.rs` + route w `app.rs`: po Undo/Redy wymusić `apply_snapshot` dla scen/sources oraz
  przechwycenie kadru preview (backend `CapturePreview`), nie tylko banner „done". Dodać test
  integracyjny mock: undo → nazwa źródła w panelu wraca.

### 15.4 Confirmation przed usunięciem źródła/sceny (P0)

- Usuwanie scen/source (ikony kosza + kontekstówki) idzie natychmiast; przepuścić przez
  `confirm_modal.rs`: „Delete source X?" z „Don't ask again" (toggle `ui_profile`).
  Wpisy usuwania trafiają do history ⇒ cofnąć się da przez Undo (zależne od 15.3).

---

## 16. Display Capture mryga przy każdym odświeżeniu (P1)

- Preview klatkuje: co odświeżenie nowe `GpuiImage` (PNG decode na background executorze — już OK) ale
  podmiana wiersza `image` resetuje układ; wprowadzić podwójne buforowanie (stary kadr pozostaje dopóki
  nowy gotowy — flaga `frame_ready`, brak `None`-okienka) + throttling do FPS ustawienia; sprawdzić
  `app.rs` ~1365-1406 (ścieżka capture) i `preview_container_sizes`.

---

## 17. Audio Mixer (P1)

- **17.1 Menu kontekstowe „…"** otwiera się u góry okna zamiast obok przycisku
  (`audio_mixer_context_menu.rs` + `context_menu.rs` — anchoring): przekazywać punkt kotwiczenia
  (x,y z `ClickEvent`/bounds) tak jak dla menu scen.
- **17.2 Modal AudioFilters** (nowy): dodawanie/usuwanie/filtrowanie porządku filtrów audio źródła
  (backend: `obs_source_create` `*_filter` z revo-backend runtime, nowe `BackendCommand::AddSourceFilter/
  RemoveSourceFilter/ListSourceFilters`), własności filtra (np. noise gate thresholds) w pod-dialogu.
- **17.3 Badges aktywnych filtrów** pod nazwą kanału w mikserze: mały znacznik (np. `mic`/`gate`/`eq`) —
  zielona stonowana obwodka (nie jaskrawa) `theme.success` z `alpha(~0.35)` wypełnienia, biały/`text`
  tekst; hover = tooltip z listą filtrów.

---

## 18. Transitions (P1–P2)

`transitions.rs` (1360 linii):

- **18.1** Sekcja „Edit transition" nad „Add transition"; edycja domyślnie podświetlona dla aktualnej
  (aktywnej) transition.
- **18.2** Usunąć dublujący przycisk „Slide/Swipe" przy Add — zostaje jedno „Add transition" (typ jako
  dropdown w formularzu tworzenia).
- **18.3** Redesign modalu: czytelniejsza struktura (grid, grupowanie „Type/Duration/Defaults",
  podgląd animacji w mini-kadrze 16:9), zachować motywy; usunąć duplikaty kontrolek.

---

## 19. Studio mode (P1)

- **19.1** Nadpisek „Preview / Studio (program)" za wysoko nad kadrami — wyrównać: max 10px nad ramką
  (`app.rs:3128-3155` render kolumn, `preview_label`).
- **19.2** Kontrolki pod listą scen (centrowane): **Cut** (natychmiastowe przełączenie program=preview),
  **Fade** (przejście 300 ms), **Transition** (uruchamia domyślnie ustawione transition) oraz edytowalna
  długość w ms (domyślnie 300; nie nadpisuje własnej długości transition przypisanej scenie). Backend:
  `obs_transition_*` w revo-backend, nowe komendy `StudioCut/StudioFade/StudioTransition { duration_ms }`
  → `services/obs.rs` i mock.
- **19.3** Kontekstówka sceny: „Set preview", „Set program". W studio mode klik sceny **nie** zmienia
  programu (dziś `scenes_panel.rs:41-43` arms preview — zostaje), program zmienia się wyłącznie przez
  19.2/„Set program"; pozazw Studio mode zachowuje obecne zachowanie (klik = natychmiastowa zmiana).

---

## 20. Templates — mockup (P2)

- W Tools/panelu dodać przycisk „Templates" otwierający modal-maketę z listą szablonów (placeholder)
  i adnotacją: „Funkcja będzie dostępna w przyszłych wersjach". Bez logiki backendowej.

---

## 21. Graphic Planner → osobne okno + libobs (P2, duży zakres)

- Dziś modal (`graphic_planner_modal.rs` + `modules/graphic_planner.rs`, geometria z `revo-core::planner`).
- Plan: okno nie-modalne (GPUI `Window` / istniejące `windows/` pattern: preview.rs, debug_console.rs —
  „detached window" albo duży panel), interakcja: przeciąganie, resize (uchwyty narożne), obracanie
  (handle nad kadrem), snapy do krawędzi/centrum, klawisze strzałek; zapis **bezpośrednio na scenie
  libobs** (`obs_sceneitem_set_pos/rot/scale` przez nowe komendy `BackendCommand::SetSceneItemTransform`
  → revo-backend), nie tylko lokalny stan. RevoCore geometry już wspiera fit/stretch/center.
- Zakres do rozdzielenia na 2–3 commity (okno→interakcja→backend). Jeśli za dużo na 0.5: zostawić modal,
  ale podłączyć zapis do backendu.

---

## 22. Tools menu — usunąć „Preview Window", „Debug Console Window" (P2)

- Kontekstówka/Tools (`context_menu.rs`, wpisy prowadzące do `windows/preview.rs`,
  `windows/debug_console.rs`, `debug_console_modal.rs`) — wywalić pozycje z menu (planner zostaje);
  okna kodu nie usuwać (jeszcze wspierane przez adapter/parity), tylko odpiąć wejścia.

---

## 23. Nagrywanie: nazwa pliku (P1)

- Zmienić format z `record_{timestamp}.ext` na **`RevoRecord_{YYYYMMDD}_{HHMM}.ext`**
  (24h, bez separatora w godzinie zgodnie z przykładem `RevoRecord_20251025_1230.mp4`).
- Miejsca: nazwa pliku generowana w `revo-backend/src/obs_service.rs`/`runtime` (sprawdzić `filename`
  formatting i `record_no_spaces` toggle w `settings_state.rs:187`/`settings_modal.rs:802`) + mock
  `services/mock.rs` + testy jednostkowe revo-backend. Uwzględnić kolizje (`_2`, `_3`).

---

## 24. Header: klik w logo/napis „RevoStream" (P2)

- `header_bar.rs`: klik w logo/tytuł otwiera `confirm_modal` „Open official website?" z **Yes/No**;
  Yes = `open::that(…)` (rust `open` crate albo backend command `OpenExternal`) do oficjalnej strony.
  Nie otwierać przeglądarki bez potwierdzenia.

---

## 25. Przykładowe implementacje (szkielety, NIE wdrożone)

Ilustracyjne szkielety kodu „jak bym to poprawił", dopasowane do istniejącego API GPUI 0.2.2
(`.hover(|style| …)`, `.occlude()`, `ClickEvent::click_count()`, `prompt_for_paths` w
`gpui-0.2.2/src/app.rs:1116`) i do faktycznych linii wskazanych wyżej. Docelowe commity mają zawierać
testy; tu jest tylko kierunek rozwiązania.

### 25.1 Edytowalne pole z kursorem (5.1, 6, 14.0) — `components/editable_text.rs` (nowy plik)

```rust
use gpui::{div, prelude::*, Entity, FocusHandle, Focusable, Keystroke, SharedString, …};

pub struct EditableText {
    focus: FocusHandle,
    value: String,
    cursor: usize,            // indeks w CHARS, nie w bajtach
    anchor: Option<usize>,    // start zaznaczenia (Shift+strzałki)
    on_change: Option<Box<dyn Fn(&str, &mut Window, &mut App)>>,
}

impl EditableText {
    fn chars(&self) -> Vec<char> { self.value.chars().collect() }

    fn selection(&self) -> (usize, usize) {
        match self.anchor {
            Some(a) => (a.min(self.cursor), a.max(self.cursor)),
            None => (self.cursor, self.cursor),
        }
    }

    pub fn apply_keystroke(&mut self, ks: &Keystroke) -> bool {
        let shift = ks.modifiers.shift;
        match ks.key.as_str() {
            "left"  => { self.move_cursor(-1isize, shift); true }
            "right" => { self.move_cursor(1, shift); true }
            "home"  => { self.set_cursor(0, shift); true }
            "end"   => { self.set_cursor(self.chars().len(), shift); true }
            "backspace" => { self.delete_backward(); true }
            "delete"    => { self.delete_forward(); true }
            _ => match ks.key_char {
                Some(text) if !text.chars().any(char::is_control)
                    && !ks.modifiers.control && !ks.modifiers.alt && !ks.modifiers.platform => {
                    self.insert(text); true
                }
                _ => false,
            },
        }
    }

    fn insert(&mut self, text: &str) {
        let (s, e) = self.selection();
        let c = self.chars();
        self.value = c[..s].iter().chain(text.chars()).chain(c[e..].iter()).collect();
        self.cursor = s + text.chars().count();
        self.anchor = None;
    }

    /// klik myszą -> pozycja kursora z szerokości glifów
    fn place_caret(&mut self, x: f32, window: &mut Window) {
        // TextLayout::measure: sumujemy szerokość kolejnych znaków aż > x
        let layout = window.text_layout(/* StyledText z self.value, style pola */);
        self.cursor = layout.index_for_x(px(x - PADDING)); // szkicowo
        self.anchor = None;
    }
}

// Render: trzy segmenty tekstu + 1px „kreska" kursora (migający zaimek
// licznikiem czasu) i tło selection_range dla zaznaczenia; wiersz
// `track_focus(&self.focus)` + `.on_key_down(...)` + `.on_mouse_down(... place_caret)`.
```

Migracja: `field_row`/`profile_field_row` w `settings_modal.rs` i wiersze wartości w
`source_properties_modal.rs` dostają zamiast statycznego `div().child(value)` →
`.child(cx.render(cz_entity))`; stary `apply_line_keystroke` zostaje tylko dla pól, gdzie
kursor nie jest potrzebny (albo zostaje usunięty po przejściu wszystkich).

### 25.2 Shell modalowy: scroll + blokada klików + X (2.1, 2.2, 2.3) — `modal_shell.rs`

```rust
pub fn modal_shell_with_body(config, theme, body, footer, close) -> gpui::Div {
    div()
        .id("modal-overlay")
        .absolute().top_0().right_0().bottom_0().left_0()
        .flex().items_center().justify_center()
        .pt_15().pb_15()
        .bg(theme.overlay)          // theme.rs: overlay := hsla(0.,0.,0.,0.75) w obu paletach
        .occlude()                  // => klik nie przechodzi pod modal (jak backdrop w app.rs:3575)
        .on_mouse_down(MouseButton::Left, cx.listener(…)) // opcjonalnie: dismiss tylko gdy config.dismissible
        .child(modal_card(config, theme, body, footer, close))
}

// modal_card: karta clampowana do okna, body MUSi być przewijalne:
div().id("modal-card")
    .max_h_full()                   // nie więcej niż viewport-15px
    .child(
        div().id("modal-card-body")
            .flex_1().min_h_0()     // bez tego flex nie oddaje miejsca i overflow nie działa
            .overflow_y_scroll().scrollbar_width(px(6.)),
    )

// X zamiast „Close" (app.rs:3602 workspace-modal-close + modal_card header):
div().id("modal-close-x")
    .size(px(28.)).flex().items_center().justify_center()
    .rounded_md()
    .cursor_pointer()
    .hover(|style| style.bg(theme.danger))
    .child(crate::components::icon::icon(theme.text_muted, "x", 14.))
    .on_click(cx.listener(|this, _, _, cx| this.close_active_modal(cx)))
```

### 25.3 Dropdown: klik tylko w pole wartości (3.2) — `settings_modal.rs:330-390`

```rust
// BEFORE: .on_mouse_down na CAŁYM wierszu (linia 377). AFTER: handler tylko w prawo-box:
fn dropdown_row(&mut self, label: &str, key: &'static str, options: Vec<String>, cx: &mut Context<Self>) -> Div {
    div()                                     // wiersz bez handleru
        .flex().justify_between().items_center().min_h(px(32.))
        .child(div().child(label))            // etykieta nie klika
        .child(
            div()
                .id(boxed_id(format!("pdropdown-{key}")))
                .cursor_pointer().px_3().py_1().rounded_md().border_1()
                .bg(self.theme.surface_active)
                .child(display.clone())
                .child(icon(self.theme.text_muted, "chevron-down", 12.))
                .on_mouse_down(MouseButton::Left, cx.listener({   // <-- tylko tu
                    let options = options.clone();
                    move |this, event: &MouseDownEvent, _, cx| {
                        this.open_dropdown = Some(DropdownState {
                            key, options: options.clone(),
                            x: event.position.x.into(), y: event.position.y.into(),
                        });
                        cx.notify();
                    }
                })),
        )
}
// DropdownState.options: Vec<String> zamiast &'static [&'static str] ( dinâmica z libobs, 25.5).
```

### 25.4 Listy encoderów z libobs (6.1/6.4) — snapshot + obs_service + GPUI

```rust
// crates/revo-core/src/service.rs (przy BackendSnapshot:148)
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EncoderOption { pub id: String, pub display: String }
pub struct BackendSnapshot { /* … */
    pub video_encoders: Vec<EncoderOption>,
    pub audio_encoders: Vec<EncoderOption>,
    pub audio_devices: Vec<DeviceOption>,       // 25.6
}

// revo-backend/src/runtime/encoders.rs
pub fn list_encoder_options(video: bool) -> Vec<EncoderOption> {
    let mut out = Vec::new();
    for i in 0.. {
        let id = match unsafe { obs_enum_encoder_types(i) } { /* ptr->CString */ };
        if id.is_null() { break; }
        // obs_get_encoder_type(id) -> obs_encoder_type_info.can_stream/can_record
        let display = unsafe { obs_encoder_type_get_display_name(id) }.to_string_lossy();
        if matches_kind(video, &display) { out.push(EncoderOption { id, display }); }
    }
    out
}

// services/obs.rs — przy budowie BackendSnapshot po Startup/Refresh wypełnić pola;
// services/mock.rs — sztywne listy („x264 encoder”, „NVIDIA NVENC H.264”, „AAC encoder”…).
// settings_modal.rs — usunąć consty (linia 37-74) na rzecz:
.dropdown_row("Video Encoder", "record_video_encoder", snapshot.video_encoders.iter()
    .map(|e| e.display.clone()).collect(), cx)
```

### 25.5 Walidacja draftu + „unsaved changes" (13.1/13.2) — `settings_state.rs`

```rust
impl SettingsDraft {
    pub fn is_dirty(&self) -> bool { self.baseline != self.current }
    pub fn validation_errors(&self) -> Vec<(TextField, String)> {
        let mut errs = vec![];
        if !self.url_is_valid(&self.stream_url) {
            errs.push((TextField::StreamUrl, "URL must start with rtmp:// or rtmps://".into()));
        }
        // bitrate: u32 w 10..=500_000; rozdzielczość: /^\d{3,5}x\d{3,5}$/
        errs
    }
}

// footer w settings_modal.rs:
.child(div().id("settings-save")
    .bg(if self.draft.is_dirty() { theme.accent } else { theme.surface_active })
    .child(format!("Save{}", if self.draft.is_dirty() { " •" } else { "" })))
// Cancel/Close przy dirty: otwórz ConfirmModal „Discard unsaved changes?" zamiast cichego zamknięcia.
```

### 25.6 Natywny picker plików (3.1, 4.2, 11.2, 14.6/14.8) — `app.rs`

```rust
use gpui::{PathPromptOptions, Maybe}; // gpui-0.2.2 app.rs:1116 prompt_for_paths

fn pick_path(self: &mut Workspace, intent: FileIntent, cx: &mut Context<'_, Self>) {
    cx.spawn(move |this, cx| async move {
        let paths = cx.update(|cx| this.clone() /*window*/, |window, _| window.prompt_for_paths(
            PathPromptOptions { files: Maybe::Yes(true), directories: Maybe::Yes(!save_as),
                                multiple: false, prompt: None }))
            .await;
        if let Ok(Some(paths)) = paths {
            let Some(path) = paths.first() else { return };
            this.update_in(&mut cx, |this, _, cx| match intent {
                FileIntent::ExportScenes  => this.send(BackendCommand::ExportScenes { path: path.into() }, cx),
                FileIntent::ImportScenes  => this.send(BackendCommand::ImportScenes { path: path.into() }, cx),
                FileIntent::ExtractTheme(dir)  => …, FileIntent::ImageSource(name) => …,
            });
        }
    })
    .detach();
}
// BackendCommand::ExportScenes/ImportScenes mapuje w services/obs.rs na istniejące
// funkcje transferu scen z revo-backend (te same, które woła adapter Tauri).
```

### 25.7 Autostart OBS + obs.dll na Windows (1.4/1.5)

```rust
// revo-engine.rs main(), przed pierwszym użyciem libobs:
#[cfg(windows)]
fn preload_runtime_dir() {
    use windows::Win32::System::LibraryLoader::{
        AddDllDirectory, SetDefaultDllDirectories, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS };
    unsafe { SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS).ok(); };
    let root = revo_backend::settings::core::resolve_root_dir();
    let lib = root.join("core").join("lib");
    let wide: Vec<u16> = lib.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { AddDllDirectory(wide.as_ptr()) };
}
```

```rust
// crates/revo-backend/build.rs — jeśli obs.dll jest statycznym importem PE:
#[cfg(windows)]
println!("cargo:rustc-link-arg=/DELAYLOAD:obs.dll"); // + delayimp; hook dlayload
```

```powershell
# package-gpui.ps1 – wariant prosty (rekomendowany na PR1, obok delay-load):
Copy-Item "$Stage\revo-root\core\lib\*.dll" $Stage -Force   # obok revo-engine.exe
# wtedy launch.cmd przestaje być „load-bearing" (verify-gpui-package.ps1:43 aktualizacja).
```

```rust
// app.rs – po połączeniu z silnikiem:
if self.settings_flag("launch_obs_on_startup") && !snapshot.obs_running {
    self.send_backend(BackendCommand::StartupObs, cx); // ścieżka już używana przez flagę --obs
}
```

### 25.8 WiX: skrót Start Menu + desktop + ikona (1.1–1.3)

```xml
<!-- product.wxs: katalogi, ikona, własność desktop -->
<Directory Id="TARGETDIR" Name="SourceDir">
  <Directory Id="ProgramFiles64Folder"><Directory Id="INSTALLFOLDER" Name="RevoStream"/></Directory>
  <Directory Id="ProgramMenuFolder"><Directory Id="AppProgramMenuFolder" Name="RevoStream"/></Directory>
  <Directory Id="DesktopFolder" Name="Desktop"/>
</Directory>
<Icon Id="RevoIcon.ico" SourceFile="$(var.StageDir)\revo-gpui.ico"/>
<Property Id="ARPPRODUCTICON" Value="RevoIcon.ico"/>
<Property Id="ADDDESKTOPSHORTCUT" Value="1"/>

<!-- ICE38 z komentarza dotyczy plikowego KeyPath w komponencie per-machine;
     standardowe obejście: KeyPath = wartość rejestru per-user + RemoveFolder -->
<ComponentGroup Id="Shortcuts">
  <Component Id="StartMenuShortcut" Directory="AppProgramMenuFolder" Guid="*">
    <RegistryKey Root="HKCU" Key="Software\RevoProject\RevoStream">
      <RegistryValue Name="StartMenuShortcut" Value="1" Type="integer" KeyPath="yes"/>
    </RegistryKey>
    <Shortcut Id="StartMenuLnk" Name="RevoStream" Icon="RevoIcon.ico"
              Target="[INSTALLFOLDER]revo-gpui.exe" WorkingDirectory="[INSTALLFOLDER]"/>
    <RemoveFolder Id="RemoveAppMenu" On="uninstall"/>
  </Component>
  <Component Id="DesktopShortcut" Directory="DesktopFolder" Guid="*"
             Condition="ADDDESKTOPSHORTCUT=1">
    <RegistryValue Root="HKCU" Key="Software\RevoProject\RevoStream"
                   Name="DesktopShortcut" Value="1" Type="integer" KeyPath="yes"/>
    <Shortcut Id="DesktopLnk" Name="RevoStream" Icon="RevoIcon.ico"
              Target="[INSTALLFOLDER]revo-gpui.exe"/>
  </Component>
</ComponentGroup>
<Feature Id="MainFeature" …><ComponentGroupRef Id="Shortcuts"/></Feature>
```

```nsi
; NSIS: desktop jako opcjonalna sekcja (teraz tworzony zawsze, package-gpui.ps1:134)
Section /o "Desktop shortcut"      ; odznaczona domyślnie
    CreateShortCut "$DESKTOP\RevoStream.lnk" "$INSTDIR\revo-gpui.exe" "" "$INSTDIR\revo-gpui.exe" 0
SectionEnd
!insertmacro MUI_PAGE_COMPONENTS    ; strona wyboru sekcji
```

### 25.9 Ikona w pliku exe (1.2) — `crates/revo-gpui/build.rs`

```rust
#[cfg(windows)]
fn main() {
    // crate winresource = "0.1"
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../src-tauri/icons/icon.ico");
    res.set("ProductName", "RevoStream"); res.set("FileDescription", "RevoStream");
    let _ = res.compile();
}
#[cfg(not(windows))] fn main() {}
```
(dla `revo-engine` analogicznie; skróty z 25.8 wskazują ikonę z PE → menu Start pokazuje Revo.)

### 25.10 Nazwa nagrania (23) — `crates/revo-backend/src/runtime/helpers.rs:141-165`

```rust
// chrono dodać do [dependencies] revo-backend: chrono = "0.4" (feature clock)
pub fn record_file_name(ext: &str) -> String {
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M"); // RevoRecord_20251025_1230
    format!("RevoRecord_{stamp}.{ext}")
}
pub fn unique_record_file(dir: &Path, ext: &str) -> PathBuf {
    let mut candidate = dir.join(record_file_name(ext));
    for n in 2.. {                       // kolizja w tej minucie -> _2, _3…
        if !candidate.exists() { return candidate; }
        let stamp = chrono::Local::now().format("%Y%m%d_%H%M");
        candidate = dir.join(format!("RevoRecord_{stamp}_{n}.{ext}"));
    }
    unreachable!()
}
// w resolve_record_path: base_path.join(unique_record_file(...));
// respektujac record_no_spaces (tu bez spacji) i format z RECORD_FORMATS; testy w helpers.rs.
```

### 25.11 Potwierdzenia start/stop (12) — routing w `app.rs`

```rust
fn request_output_action(self: &mut Workspace, event: ComponentEvent, cx) {
    let confirm_key = match &event {
        Streaming(true)  => "confirm_start_stream",   Streaming(false) => "confirm_stop_stream",
        Recording(true)  => "confirm_start_record",   Recording(false) => "confirm_stop_record",
        _ => { self.dispatch(event, cx); return; }
    };
    if self.profile_flag(confirm_key) {
        self.pending_confirm = Some(event.clone());   // istniejący ActiveModal::Confirm
        self.state.active_modal = ActiveModal::Confirm;
        cx.notify();
    } else {
        self.dispatch(event, cx);
    }
}
// ConfirmModal: „Start streaming to Twitch?" + checkbox „Don't ask again"
// (wyłącza flagę i zapisuje przez SaveSettings) + Accept/Cancel; Accept => self.dispatch(pending).
```

### 25.12 Dialog nagrywania skrótu (9) — nowy `modal/shortcut_capture_modal.rs`

```rust
impl Render for ShortcutCaptureModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        modal_shell_with_body(&ModalConfig::small("Keyboard shortcut"), self.theme,
            div().flex().flex_col().items_center().gap_3().track_focus(&self.focus)
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if let Some(combo) = format_keystroke(&event.keystroke) {
                        this.combo = Some(combo);      // podmiana w locie, nadpisuje
                        cx.notify();
                    }
                }))
                .child(                                // mały panel na środku
                    div().w(px(220.)).h(px(64.)).flex().items_center().justify_center()
                        .rounded_md().border_1().border_color(self.theme.accent)
                        .child(self.combo.clone().unwrap_or_else(|| "Press keys…".into())),
                )
                .child(format!("Bieżący skrót: {}", self.current)),
            Some(footer_cancel_accept(self.theme,
                cx.listener(|_,_,_,cx| cx.emit(CaptureEvent::Cancelled)),
                cx.listener(|this,_,_,cx| { if let Some(c)=this.combo.clone() { cx.emit(CaptureEvent::Accepted(c)) } }))),
            None)
    }
}
fn format_keystroke(ks: &Keystroke) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    if ks.modifiers.control { parts.push("Ctrl"); }
    if ks.modifiers.alt     { parts.push("Alt"); }
    if ks.modifiers.shift   { parts.push("Shift"); }
    if ks.modifiers.platform{ parts.push("Super"); }
    match ks.key.as_str() {
        "" | "control" | "alt" | "shift" | "platform" => return None,   // sam modfier nie łapie
        k => { parts.push(k); Some(parts.join("+")) }                    // "ctrl+shift+f9"
    }
}
// Accept => this.draft.set_profile_value(shortcut_key, combo); kolizje: skan ui_profile i ostrzeżenie.
```

### 25.13 Studio mode: Cut/Fade/Transition + preview/program (19) — revo-core + app.rs

```rust
// revo-core/src/service.rs
pub enum BackendCommand { /* … */
    StudioCut, StudioFade { duration_ms: u32 }, StudioTransition { duration_ms: Option<u32> },
    SetPreviewScene(String), SetProgramScene(String),
}

// revo-gpui/src/services/obs.rs (mapowanie na revo-backend):
StudioCut => obs_set_current_scene(preview_scene)             // natychmiast
StudioFade { ms } => obs_transition_start_mode(OBS_TRANSITION_MODE_FADE, ms)
StudioTransition { ms } => obs_transition_start(default_kind, ms.unwrap_or(profile default))

// scenes_panel.rs — przyciski wyśrodkowane POD listą scen, tylko gdy studio_mode:
.child(div().flex().justify_center().gap_2()
    .child(button(theme, "Cut", …))
    .child(button(theme, "Fade", …))
    .child(button(theme, default_transition_kind, …))
    .child(fade_ms_field))                                    // InputState z 25.1, default 300
// kontekstówka sceny (context_menu.rs:51 Scene branch):
MenuEntry::Action(SetPreview, "Set preview".into()),
MenuEntry::Action(SetProgram, "Set program".into())           // .when(studio_mode)
// scenes_panel.rs:41-43 — klik sceny w studio mode zostaje przy „arm preview";
// zmiana PROGRAMU wyłącznie przez Cut/Fade/Transition/Set program.
// app.rs:3144-3153 — nadpiski kolumn: mt(px(10.)) max, nie więcej.
```

### 25.14 Podwójne buforowanie kadru — brak flashu (16, 14.5)

```rust
// app.rs pętli capture (~1365-1406):
match capture_and_decode(backend, preview_scene) {
    Ok(frame) => workspace.preview_frame = Some(Arc::new(frame)), // nowa klatka
    Err(_) => {}                              // KEEP LAST GOOD FRAME — nie kasuj
}
// render: .when_some(self.preview_frame.clone(), |d, img| d.child(img.into_element()))
//         bez gałęzi „None" pokazującej pusty kontener przy każdej wymianie.
// throttle: odświeżanie = min(FPS ustawienia, 30Hz); po Apply source properties
// NIE przebuduj drzewa — tylko notify() na panelach objętych zmianą + wymiana
// klatek (measured containers pozostają), patrz 14.5.
```

### 25.15 Dwuklik na źródle + ikony scen + usunięcie z potwierdzeniem (15.1, 15.2, 15.4)

```rust
// sources_panel.rs — na wierszu źródła:
.on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
    if event.click_count() == 2 {
        cx.emit(ComponentEvent::EditSourceProperties(id.clone())); // otwórz modal
    } else {
        this.select(id.clone(), cx);
    }
}))

// scenes_panel.rs:216-220 — „{} [active][locked]" => ikony:
.child(div().flex().items_center().gap_1()
    .child(SharedString::from(scene.name.clone()))
    .when(scene.active, |d| d.child(icon(self.theme.accent, "circle-dot", 12.)))   // „aktywna"
    .when(scene.locked, |d| d.child(icon(self.theme.text_muted, "lock", 12.))))    // zamknięta kłódka

// remove: natychmiast => confirm:
fn remove_scene(self, name, cx) {
    if self.profile_flag("confirm_delete") && !self.never_ask("delete") {
        self.pending_confirm = Some(DeleteTarget::Scene(name));
        self.state.active_modal = ActiveModal::Confirm;   // + hint „Cofnij przez Ctrl+Z"
    } else { self.emit(RemoveScene(name)); }
}
// Undo/Redo (15.3): po zdarzeniu wymuś pełny re-apply snapshotu + request_preview_capture,
// tak aby panele AND kadr preview wróciły do stanu z historii (test w app.rs route tests).
```

### 25.16 Audio mixer: kotwica menu + badge filtrów (17.1, 17.3)

```rust
// audio_mixer.rs przycisk „…": ( wzorzec ze scenes_panel.rs:158 — pozycja z ClickEvent)
.on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
    let pos = window.mouse_position();                     // albo event.position
    cx.emit(AudioMixerEvent::ShowFiltersMenu {
        source: name.clone(), anchor: (pos.x.as_f32(), pos.y.as_f32() + 8.),
    });
}))
// audio_mixer_context_menu.rs: zamiast stałej pozycji „u góry" —
div().absolute().left(px(anchor.0)).top(px(anchor.1)).occlude().child(menu_entries)

// badge pod nazwą kanału (17.3):
.when(!filters.is_empty(), |row| row.child(
    div().flex().flex_wrap().gap_1().pt_0p5().children(filters.iter().map(|f|
        div().px_1p5().py_0p5().rounded_full().border_1()
            .border_color(self.theme.success.alpha(0.45))  // stonowana zieleń, nie jaskrawa
            .bg(self.theme.success.alpha(0.10))
            .text_color(self.theme.text_strong).text_size(px(10.))
            .child(SharedString::from(f.label.clone())))))))
```

### 25.17 High contrast / font / vision jako realne ustawienia (10) — `theme.rs` + `app.rs`

```rust
impl RevoTheme {
    pub fn build(variant: ThemeVariant, a11y: &A11yProfile) -> Self {
        let mut t = match variant { Classic => Self::classic(), Future => Self::future() };
        if a11y.high_contrast {
            t.border = t.text;                    // granice = pełna kontrastowość
            t.surface = hsla(0., 0., 0., 1.);     // głęboka czerń
            t.accent = accent_high_contrast(variant);
            t.focus_ring_width = px(3.);          // „Stronger keyboard focus"
        }
        t.font_family = match a11y.font_family {  // Default | Serif | Monospace
            Default => None, Serif => Some("serif".into()), Monospace => Some("monospace".into()) };
        t.font_scale = match a11y.font_size { Small => 0.9, Default => 1.0, Large => 1.15, XL => 1.3 };
        t.apply_vision_lut(a11y.color_vision);    // LUT deuteranopia/protanopia/tritanopia na akcentach
        t
    }
}
// app.rs: po LoadedSettings => self.theme = RevoTheme::build(...); cx.notify();
// reduce_motion: self.motion = if a11y.reduce_motion { Motion::Instant } else { Motion::Animated };
// wszystkie helpery animacji (fade scen, miksér level, dropdown) sprawdzają self.motion.
// test: kontrast text/surface >= 4.5:1 dla każdej kombinacji (helper luminance w theme.rs).
```

### 25.18 „X" close (2.3) — już w 25.2; dodatkowo w `modal_card` nagłówek:

```rust
.child(config.title)
.child(spacer)
.child(close.unwrap_or_else(||
    div().id("card-close").cursor_pointer().p_1p5().rounded_md()
        .hover(|s| s.bg(theme.danger))
        .child(icon(theme.text_muted, "x", 14.))
        .on_click(cx.listener(|_,_,_,cx| cx.emit(ModalEvent::CloseRequested)))))
```

### 25.19 Zrzut całości implementacji (kolejność w kodzie)

1. `editable_text.rs` → podmiana pól w `settings_modal.rs` + `source_properties_modal.rs`
   (odblokowuje 14.0/14.3/5.1/6.2 i walidację „Position X…").
2. `modal_shell.rs` helper (scroll+occlude+X) → wszystkie modale (2.1/2.2/2.3).
3. `BackendSnapshot` rozszerzenia (encodery, urządzenia, rozdzielczości, studio komendy)
   → mapowania w `services/obs.rs` + `services/mock.rs` (6/7/8/19).
4. Routing potwierdzeń w `app.rs` (12/15.4) i nazwa pliku w `helpers.rs` (23).
5. Windows: `build.rs` ikona + delay-load/kopia DLL + WiX shortcuts (1.x).
6. Reszta UX (15.1/15.2/16/17/18/24) — małe, niezależne commity.

---

## Proponowana kolejność realizacji

1. Wersja + Windows P0 (1.1–1.4) — osobna ścieżka pakietowa, nie blokuje reszty kodu.
2. Infrastruktura UI P0: prawdziwe pole tekstowe z kursorem (InputState), scroll helper modali,
   przezroczystość/hit-test overlay, motyw classic dla modali — to odblokowuje większość 5–14.
3. Listy z libobs: encoders/devices/resolutions/urządzenia capture (6, 7, 8, 14.10) — jeden pass przez
   `BackendSnapshot` + `services/obs.rs` + mock.
4. Potwierdzenia: stream/rec/usuń (12, 15.4), dialog skrótów (9), accessibility (10).
5. Studio mode (19), Audio Mixer (17), Transitions (18), flash/refresh (14.5, 15.3, 16).
6. Dialogi plików (3.1, 4.2, 11.2, 14.6) — jeden helper natywnego pickera.
7. Drobne UX: X zamiast Close (2.3), ikony [active]/[locked] (15.2), nazwy nagrań (23), header logo (24),
   Tools cleanup (22), Templates mockup (20), Graphic Planner okno (21 — ostatnie, największy zakres).

## Walidacja

- `bash scripts/core/verify.sh` co każdy etap (format, testy, checki bez host-deps).
- `cargo test --manifest-path crates/revo-core/Cargo.toml` (planner, settings, nazwy plików) i
  `cargo test --manifest-path crates/revo-gpui/Cargo.toml` (route/confirm/studio/dropdowny) — testy do
  każdego pkt P0/P1 tam, gdzie da się bez libobs; `--features obs-backend` tylko parse-check.
- Windows: `verify-gpui-package.ps1`, `smoke-test-msi.ps1`, `smoke-test-gpui.ps1` (+ nowe asercje
  skrótu i autostartu OBS) na runnerze z sesją desktopową.
- Visual parity: RevoClassic/RevoFuture w 1080×600 / 1280×720 / 1440×900, progi 1200/800px, tolerancja
  2px; kontrast accessibility ≥4.5:1.
- `git diff --check` przed każdym commitem.

---

# PROGRESS

Format: data — zadanie (§ planu) — status

## 2026-11-11 — start

- [x] §0 Bump wersji 0.5.0-pr1 — 6 manifestów + 4 Cargo.lock + AGENTS.md (`0.4.1` → `0.5.0-pr1`)
- [x] §2.2 Overlay klików — Settings modal nie miał `.occlude()` (app.rs:3815) ani shell
      `modal_shell_with_body`; dodane w obu miejscach (klik nie przechodzi pod modal)
- [x] §2.3 X zamiast „Close" — helper `modal_shell::close_x_button` (hover `theme.danger`),
      podpięty w source-properties/add-scene/add-source + workspace-layer (app.rs) + planner
- [x] §3.2 Dropdowny — `dropdown_row` otwiera popup tylko z kliku w box wartości, etykieta
      nie jest już polem klikalnym (`settings_modal.rs:330`)
- [x] §5.1 Edytor z kursorem — nowy `components/editable_text.rs` (CaretSession: insert w
      środku, selection Shift+strzałki, Home/End, Ctrl+A, Ctrl+←/→, word-backspace, UTF-8-safe)
      + geometria kursora przez `layout_line`; zintegrowany w SettingsModal (sesje per pole,
      canvas-probe bounds → klik w środek tekstu ustawia kursor) — 12 unit + 3 testy GPUI
- [x] §5.2 Pole Key — kolumna `flex_1` (pole ciągnie się do przycisku), odstęp
      15px (`gap(px(15.))`); realna wartość trafia do CaretSession, maska „••••"
      tylko w renderze gdy `!key_visible`
- [x] §11.1 Języki — `dropdown_row_pairs`: zapisuje wartość („Polski"), wyświetla
      „Polski (0%)"/„English (100%)"; consts LANGUAGE_VALUES/LABELS
- [x] §15.2 Ikony scen — „ [active]/[locked]" → kropka „record" w kolorze akcentu
      + „lock" (muted), truncate nazwy zachowany
- [x] §2.1 Scroll Settings — body `min_h(relative(1))` zamiast `size_full`
- [x] §23 Nazwa nagrań — `RevoRecord_{YYYYMMDD}_{HHMM}.mp4` dla ścieżki-katalogu
      oraz `{stem}_{stamp}.{ext}` dla własnej nazwy (revo-backend `helpers.rs`,
      `utils::time::local_record_stamp()` bez chrono); kolizje: istniejąca pętla `_idx`.
      Testy dodane w `helpers.rs::record_name_tests` — **nie uruchomione lokalnie**:
      `revo-lib` wymaga systemowego libobs/gio (failure środowiskowy wg AGENTS); kod
      przejrzany, do potwierdzenia na hostcie z provisioningiem.
- [x] §14.0 + §14.2/14.3 (baza) Edit Source — CaretSession w `source_properties_modal`:
      sloty (transform 0-4, parametry, nazwa, nazwa filtra, parametry filtra) przez
      `slot_text/set_slot_text/focus_slot/slot_display/bounds_probe`; wszystkie pola
      `text_field`/`number_field`/`name_row` edytowalne w środku tekstu z kursorem
      (to samo naprawia „read only" w filtrach, HEX koloru i pole ścieżek);
      okno edycji źródła dostaje `min_h`-scroll razem z helperem karty (§2.1)
- [x] §14 regression „Position X must be a whole number" — nowe niesadzone źródło
      startowało z PIOTYMI draftami liczb; teraz seed „0" w `build()` (+ test
      `fresh_editor_applies_with_seeded_zero_transform`)
- [x] §14.4 Apply zamyka modal — `apply_full` po walidacji emituje `AppliedFull` +
      `Cancelled` (workspace traktuje Cancelled jako zamknięcie); błędy zostawiają
      otwarty draft; test `editor_apply_success_emits_save_then_close`
- [x] §12 Potwierdzenia start/stop stream/rec — `ActiveModal::Confirm` + `PendingConfirm`
      w `app.rs`: `route_event` bramuje 4 zdarzenia lifecycle PRZED optymistycznym
      flipem nagłówka, flagi z `ui_profile` (`confirm_start_stream`,
      `confirm_stop_stream`, `confirm_start_record`, `confirm_stop_record`; format
      string „true"/JSON bool); dialog ma realne Cancel/Accept (backdrop = cancel),
      Accept → `dispatch` + `update_header_flags`; 4 testy (2× gpui end-to-end)
- [x] §15.4 Confirmation przed usunięciem sceny/źródła — ten sam mechanizm:
      `confirm_request` obejmuje `RemoveScene/RemoveSource` przy flagze
      `confirm_delete` (nowy toggle „Confirm before deleting scenes and sources"
      w Settings > General); `PendingConfirm.history` liczony z pre-dispatch
      snapshotu i zapisywany przy Accept ⇒ undo/redo działa za dialogiem
- [x] §15.3 Undo/Redo odświeżenie — gałęzie `Undo`/`Redo` w `route_event` czyszczą
      `source_settings_cache` + `settings_warmed` (jak klik na historii), panele
      re-seedują się z pędu `dispatch → refresh_snapshot → apply_snapshot`;
      end-to-end test `undo_restores_removed_scene_in_scenes_panel`
- [x] §16 Display Capture mruganie — pętla podglądów (obs-backend) przypisuje kadr
      TYLKO gdy capture zwróci Some; pudło `None` utrzymuje ostatni dobry kadr
      (brak czarnej dziury co tick); sprawdzone `cargo check --features obs-backend`
- [x] §6/7/8 (baza) Katalogi z libobs — nowy `SnapshotSelectOptions`/`SnapshotOption`
      w `revo-core/src/service.rs` + pole `BackendSnapshot.select_options`
      (serde transport silnik→UI); `revo-backend/obs_service`: `refresh_select_options()`
      (encodery `get_settings_select_options` + urządzenia pulse) po `StartObs`,
      kasacja po `StopObs`/shutdown; nowy komend `RefreshSelectOptions`
      (revo-backend + mock; UI wysyła ją po `SaveSettings`);
      `revo-core::planner::RESOLUTION_PRESETS` + `resolution_preset_options()` (§8.1);
      mock publishuje pełny katalog od startu (§6–8 testable bez hosta)
- [x] §6/7/8 (UI) SettingsModal: `DropdownTarget {Profile|TextField}` + owned pary
      (value,label); helpery `catalog_options/option_row/option_field_row/resolution_row`;
      encodery streaming/rec, format nagrywania, Sample Rate/Channels, downscale,
      urządzenia Desktop/Mic/Monitor (z „Custom (…)" zachowującym wpis własny),
      Base/Output resolution jako listy „1920×1080 → Full HD (1080p)" + Custom;
      §8.2 FPS warunkowy (Common → common+custom, Integer → integer,
      Fractional → num+den); katalog wypychany do otwartego modala przy każdym
      `apply_snapshot` + seed przy otwarciu; testy: `dropdown_targets_write…`,
      `catalog_options_map_engine_pairs`, mock catalog, presets
- [ ] §10 reszta accessibility (konsumpcja flag w renderze, §25.17)
- [x] §4.1 RevoClassic w modalach — jeden source of truth: wszystkie konstruktury
      modali (SettingsModal::new/with_status, PluginsModal::new, AddSceneModal::new,
      AddSourceModal::new, SourcePropertiesModal::new/new_editor/build,
      QuickActionModal::new) przyjmują motyw z `Workspace.theme`;
      nowe `apply_theme_to_views` re-seeduje paletą header/panele/mikser-menu/
      planner/context/color/font/dropdown picker ORAZ otwarte modale przy żywej
      zmianie stylu (apply_theme_style). `RevoTheme` zyskał `PartialEq`.
      Test `classic_style_opens_modals_on_the_workspace_palette` (app.rs):
      classic → modal otwarty z klasyczną paletą + przełączenie na future
      re-seeduje otwarty modal. revo-gpui 173/173.
- [ ] §17/§18/§19, pickery plików (§3.1/4.2/11.2), Windows (1.x)

Weryfikacja: `bash scripts/core/verify.sh` → all core checks passed; revo-core 55/55;
revo-gpui 172/172; `cargo check --features obs-backend` 0 errors. Uwaga: ścieżka
revo-backend (`refresh_select_options`, `RefreshSelectOptions`) niekompilowalna lokalnie
(brak libobs/gio) — do potwierdzenia na hoście z provisioningiem.

## 2026-11-11 — kontynuacja #2 (goal)

- [x] §4.1 RevoClassic w modalach (P0) — patrz wpis wyżej; jeden source of truth
      `Workspace.theme` + re-seed otwartych modali/pickerów.
- [x] §5.1 (dokończenie) CaretSession wszędzie — nowy reusable `CaretField`
      (session + bounds-probe + `keystroke`/`place_caret`/`display`) w
      `editable_text.rs`; zmigrowane: `add_scene_modal` (name),
      `add_source_modal` (name + option carets), `plugins_modal` (profil name;
      usunięty `PluginsDraft::field_keystroke`), `font_picker` (filter),
      `transitions` (NewName/SelectedName/SelectedDuration/SelectedParam —
      draft_field + param_text_field przez `field_carets: HashMap<FieldSlot,…>`,
      digits-only zachowany), `quick_action_modal` (value). Wszystkie klikają
      kursor w środek tekstu, strzałki/Home/End/Ctrl+A działają.
      `apply_line_keystroke` zostaje już tylko w `debug_console` (search) i
      jako re-export w `settings_state` (ścieżka testowa).
- [x] §15.1 Dwuklik na źródło — `sources_panel.rs`: `on_click` sprawdza
      `event.click_count() >= 2` → emit `EditSource(id)` (pojedynczy klik
      dalej tylko selekcja); routing modala istniał (app.rs:405).
- [x] §22 Tools menu — usunięte wpisy „Preview window" i „Debug console
      window" z panelu Tools (app.rs); kod okien i routing zdarzeń zostaje
      (parity), testy route przechodzą bez zmian.
- [x] §17.1 Kotwica menu „…" Audio Mixer — `MixerEvent::ContextMenuRequested`
      przenosi punkt kliknięcia (prawoklik wiersza kanału + przycisk „…" przez
      `ClickEvent::position()`); `AudioMixerContextMenu.position` + klamp do
      viewportu (MARGIN/MENU_W/MENU_H) w renderze; test pozycji w
      `open_menu_tracks_source_and_selection`.
- [x] §24 Klik w logo/napis „RevoStream" — `ComponentEvent::OpenOfficialWebsite`
      z klikalnego brandu w `header_bar.rs`; `PendingConfirm.command` teraz
      `Option<BackendCommand>` + `cancel_label` („No"/„Yes"); accept →
      `open_external()` (xdg-open/open/cmd start, spawn bez czekania); stała
      `OFFICIAL_WEBSITE_URL`. Test `brand_click_asks_before_opening_the_website`.
- [x] §20 Templates — już funkcjonuje: przycisk w Tools + modal-makieta z
      adnotacją „future versions" i OK (app.rs ActiveModal::Templates).
- [x] §13.10 literówka „familly" — brak wystąpień w kodzie (poprawione wcześniej).
- [ ] §10 reszta accessibility (konsumpcja flag w renderze, §25.17)
- [ ] §17.2/17.3 filtry audio, §18 transitions redesign, §19 studio mode,
      pickery plików (§3.1/4.2/11.2), Windows (1.x)

Weryfikacja: `bash scripts/core/verify.sh` → all core checks passed;
revo-gpui 174/174 (172 → +1 paleta classic +1 brand confirm); `git diff --check`
czysto. Uwaga: `audio_mixer.rs` był przypadkowo nadpisany podczas edycji
i przywrócony z HEAD (plik nie miał zmian roboczych — stan sprzed edycji §17.1
odtworzony patchem, testy przechodzą).

## 2026-11-11 — kontynuacja #3 (goal)

- [x] §3.1 Import/Export kolekcji scen (Revo/RevoStream + OBS) — natywne dialogi
      plików: `Browse…` przy polu ścieżki (prompt_for_paths), eksport bez ścieżki →
      `prompt_for_new_path` (Save As), import bez ścieżki → open picker; wybrane
      ścieżki lądują w polu i emitują istniejące `SettingsEvent::Export/Import*SceneCollection`
      → `BackendCommand::*SceneCollection` (obs_service/engine mapowanie już było).
      Helper `run_transfer`/`TransferIntent` w `settings_modal.rs`, test
      `run_transfer_with_typed_path_emits_without_dialog`.
- [x] §4.2 "Extract default theme" — picker katalogu (directories) →
      `SettingsEvent::ExtractDefaultTheme` → `BackendCommand::ExtractDefaultTheme`
      → `revo_backend::settings::themes::themes_extract_default` (domyślne kolory,
      katalog `themes/default_local` pod wskazanym rootem). Przycisk w Settings>Look,
      usunięty tekst „needs the native file dialog".
- [x] §11.2 Profile Import/Export — natywne dialogi: Export profile →
      `prompt_for_new_path` (revo-profile.json) → `SettingsEvent::ExportProfile`
      → `BackendCommand::ExportProfile` → `export_profile_obs_to_file`; Import →
      open picker → `ImportProfile` → odczyt JSON + `import_profile_obs`.
      Przyciski w Settings>Other zamiast atrap `action_row` („— pending").
- [x] `revo-core::service::BackendCommand`: +`ExtractDefaultTheme{root_dir}`,
      `ExportProfile{path}`, `ImportProfile{path}` (serde transport). Mock obsługuje
      no-op. revo-backend/obs_service.rs ma handlery (kompilacja obs-backend wymaga
      libobs/gio — niezweryfikowane lokalnie, jak wcześniej).

Weryfikacja: `bash scripts/core/verify.sh` → all core checks passed;
revo-gpui 175/175.

## 2026-11-11 — kontynuacja #4 (goal)

- [x] §18.1 Edit transition nad Add transition + domyślne podświetlenie aktywnej —
      `TransitionsPanel::new` wybiera aktywną/przód jako edytowaną; render:
      sekcja „Edit transition: … (active)" nad „Add transition", lista kart poniżej.
      `apply()` zachowuje wybór na aktywnej pozycji.
- [x] §18.2 Jeden przycisk „Add" z wyborem typu (dropdown) — usunięty duplikujący
      flow cykliczny; typ nowej tranzycji wybierany przez wspólny dropdown
      (`DropdownField::TransitionKind` dla `__new__` obsłużony w `app.rs`).
- [ ] §18.3 Redesign modalu (grid/grupowanie/podgląd 16:9) — niezaimplementowane.

Weryfikacja: `bash scripts/core/verify.sh` → all core checks passed; revo-gpui 175/175
(test transitions zaktualizowany pod utrzymanie selekcji po Apply).

## 2026-11-12 — surowe RGBA zamiast base64 PNG (goal)

- [x] `revo-core::ipc`: binarne ramki preview — `PreviewFrame{request_id,width,
      height,rgba}`, `PREVIEW_FRAME_MAGIC` (`RVPF`) + 24 B nagłówek + piksele,
      `encode_preview_frame`/`decode_preview_frame`/`is_preview_frame`,
      limit 64 MiB; `EngineToUi::PreviewFrame{data_uri}` i
      `PreviewFrameData` zastąpione korelacyjnym `PreviewUnavailable{request_id}`
      (udaną odpowiedź niesie ramka binarna). Testy: round-trip, identyfikacja
      dwóch typów ramek, odrzucone uszkodzone ramki.
- [x] `revo-backend`: `CapturedPreviewFrame` ma akcesory i `validate()`,
      `ScreenshotCapture::Serve(Arc<…>)` / `Fresh`, `encode_screenshot(&frame)`
      (tylko Tauri), `validate_screenshot`, `publish_screenshot(...,
      Option<&Arc<…>>)`; `ObsRuntime.last_preview_frame` trzyma `Arc` surowej
      ramki. `obs_service::capture_preview_frame` zwraca `Arc<CapturedPreviewFrame>`
      i publikuje ramkę (wcześniej silnik w ogóle nie publikował, więc fallback
      zamrożenia na ścieżce engine nie działał).
- [x] `revo-backend::engine`: `send_preview_frame` zapisuje ramkę binarną (bez
      PNG/base64), `push_preview_frame` i `PreviewFrameOnce` korzystają z niej.
- [x] `revo-gpui`: klient czyta ramki po magicu (`EngineMessage::{Control,Preview}`,
      slot newest-wins), `capture_preview_frame` zwraca `PreviewFrame`;
      `services::preview_frame_image` uploaduje RGBA→BGRA jako
      `gpui::RenderImage`; `app.rs`, `windows/preview.rs` i kafelki scen trzymają
      `Arc<RenderImage>`. Interakcja przeglądarki nadal PNG data URI (CDP).
- [x] Tauri (parity) nadal zwraca data URI, ale używa tych samych faz
      capture/validate/publish.

Weryfikacja: `bash scripts/core/verify.sh` → all core checks passed;
revo-core 56/56, revo-gpui 175/175 (mock) i 182/182 (`--features obs-backend`).
`revo-backend`/`src-tauri` nie kompilują się lokalnie (brak glib/gio/webkit);
zmiany w tych klatkach sprawdza CI.
