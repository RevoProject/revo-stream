# How to add a feature (praktyczny przewodnik)

## Zasada ogólna
Mały, skupiony PR. Jeden logical change. Conventional Commits.

## Ścieżka A: nowa komenda backendu
1. Dodaj wariant do `BackendCommand` w `crates/revo-core/src/service.rs`.
2. Dodaj obsługę w engine (`crates/revo-backend`).
3. Zaktualizuj `BackendSnapshot`, jeśli potrzeba nowego stanu.
4. Dodaj test w `revo-core` (jeśli logika czysta).
5. Wywołaj komendę z UI (GPUI akcja).
6. Uruchom `bash scripts/core/verify.sh`.

## Ścieżka B: nowy widok/modal w GPUI
1. Dodaj komponent w `crates/revo-gpui/src/components/...`.
2. Zarejestruj w `mod.rs` i routingu modali.
3. Użyj shared theme (RevoClassic/RevoFuture).
4. Zachowaj layout/spacing (parity contract).
5. Dodaj testy widoku, jeśli możliwe.

## Ścieżka C: nowe ustawienie
1. Dodaj pole do `AppSettings` (`crates/revo-core/src/models.rs`).
2. Ustaw default w `crates/revo-core/src/settings.rs`.
3. Obsłuż w backendzie (load/save).
4. Dodaj UI w settings modal.
5. Zaktualizuj snapshot, jeśli potrzeba.

## Checklist przed PR
- [ ] `bash scripts/core/verify.sh` przechodzi
- [ ] Testy dodane/zaktualizowane
- [ ] Docs zaktualizowane (jeśli zmienia zachowanie)
- [ ] UI: RevoClassic/RevoFuture na 1080x600, 1280x720, 1440x900
- [ ] Performance-sensitive: before/after (jeśli dotyczy)
- [ ] Brak sekretów w logach/screenshotach

## Gdzie pytać
- Issues: template feature request
- PR: template z checklistą
- Docs: `docs/onboarding.md`, `docs/architecture.md`