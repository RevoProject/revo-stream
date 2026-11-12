# Plan: Premium module (closed) + OSS core (AGPL)

## Cel
Zarabiać na zamkniętym module premium, jednocześnie rozwijając OSS core (AGPL) ze społecznością.

## Architektura granic
- **Core (OSS, AGPL):** modele, IPC, transport, backend libobs.
- **Premium (closed):** moduły premium, integracje, zaawansowane funkcje.
- **Granica:** stabilne API (typed IPC + plugin API). Premium zależy od core, core nie wie o premium.

## API graniczne (minimalne)
- `BackendCommand` / `BackendSnapshot` (typed).
- Plugin hooks: `on_scene_change`, `on_source_add`, `on_output_start`.
- Konfiguracja: `PremiumConfig` (feature flags).
- Wersjonowanie API: `api_version` w snapshot.

## Struktura modułów
```
revo-core (AGPL)
revo-backend (AGPL)
revo-gpui (AGPL)
revo-premium (closed)
  ├── src/
  ├── api/          # kontrakt z core
  ├── features/     # premium features
  └── tests/        # testy oparte na specyfikacji
```

## Proces clean-room (dla modułów premium)
1. **Specyfikacja** (co robi, API, zachowanie) — bez kodu.
2. **Implementacja** od zera na podstawie specyfikacji.
3. **Dokumentacja** (audit trail: data, autor, spec).
4. **Testy** oparte na specyfikacji, nie na kodzie źródłowym.
5. **Review** (sprawdzenie, czy nie ma kopii).

## Monetyzacja
- Sprzedaż premium modułu (licencja).
- Wsparcie / hosting / SLA.
- Marka/trademark jako dodatkowa wartość.

## Risks & mitigations
- **Przypadkowe kopiowanie AGPL** → clean-room + review.
- **Zależności** → sprawdź licencje bibliotek.
- **Divergence** → stabilne API, wersjonowanie.
- **Społeczność** → jasne granice (core OSS, premium closed).

## Roadmap (checklist)
- [ ] Zdefiniować API graniczne (core ↔ premium).
- [ ] Dodać plugin hooks w core.
- [ ] Utworzyć `revo-premium` jako osobny moduł (closed).
- [ ] Napisać specyfikację pierwszego premium feature.
- [ ] Implementować clean-room.
- [ ] Dodać testy (spec-based).
- [ ] Dokumentować proces (audit trail).
- [ ] Ustalić model licencjonowania premium.

## Krótki werdykt
- **Core OSS (AGPL)** + **Premium closed (clean-room)** + **stabilne API** = bezpieczny i skalowalny model.
- **Clean-room** wymaga dyscypliny, ale chroni przed naruszeniem licencji.