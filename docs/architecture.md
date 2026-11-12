# Architecture — RevoStream (GPUI rewrite)

## Cel
Opisać aktualną architekturę w sposób „ludzki”: co, gdzie i dlaczego.

## Wysoki poziom
```
GPUI UI process (revo-gpui)
        |
        | typed BackendCommand / snapshots / preview frames
        v
Dedicated engine process (revo-engine, libobs)
```

## Dlaczego dwa procesy?
- libobs jest ciężki i podatny na crash → izolacja chroni UI.
- UI pozostaje responsywne (brak blokowania foreground executora).
- Łatwiejsze testowanie (mock backend bez libobs).

## Granice odpowiedzialności
| Warstwa | Odpowiedzialność |
| --- | --- |
| `revo-core` | Modele domenowe, settings, IPC, transport, `BackendService` |
| `revo-backend` | Runtime libobs, operacje scen/źródeł, IPC server, `revo-engine` |
| `revo-gpui` | UI, akcje, modale, okna, serwisy klienckie |

## Przepływ komendy (przykład: StartRecording)
1. Widok GPUI wywołuje akcję.
2. Akcja mapuje na `BackendCommand::StartRecording`.
3. `BackendService` wysyła komendę do engine (typed IPC).
4. Engine wykonuje operację w libobs.
5. Engine aktualizuje snapshot (recording=true).
6. UI otrzymuje snapshot i renderuje stan.

## Preview frames
- Aktualnie: PNG data URI w bounded newest-wins slot.
- Docelowo: raw RGBA / natywne tekstury (TODO).

## Ustawienia i stan
- `BackendSnapshot` to źródło prawdy dla UI (sceny, źródła, audio, wyjścia, settings).
- UI trzyma stan widoków (modal stack, focus, layout), nie duplikuje logiki backendu.

## Transport
- Unix: Unix socket
- Windows: loopback TCP + `--endpoint-file`

## Legacy (parity reference)
`src-tauri/`, `src/`, `static/` pozostają do czasu finalnego cutover.  
Nie są głównym targetem, ale służą do parity i testów zachowania.

## Kluczowe zasady (z AGENTS.md)
- Backend niezależny od Tauri/GPUI.
- libobs tylko w `revo-engine`.
- Preview/audio: bounded newest-wins, bez blokowania UI.
- Jeden logical change per commit.