# ADR-001: Osobny proces engine dla libobs

## Status
Accepted

## Kontekst
libobs jest ciężki i podatny na crash. W Tauri wszystko działało w jednym procesie.

## Decyzja
Wyodrębnić libobs do dedykowanego procesu `revo-engine`, a UI (`revo-gpui`) komunikować się przez typed IPC.

## Konsekwencje
- Lepsza stabilność UI.
- Trudniejsza synchronizacja stanu (snapshoty, preview frames).
- Dodatkowa złożoność packaging (dwa binarki).