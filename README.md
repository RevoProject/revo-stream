# RevoStream

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white&color=%23CE412B)
![Tauri](https://img.shields.io/badge/tauri-%232E7EEA.svg?style=for-the-badge&logo=tauri&logoColor=%23FFFFFF)
![Svelte](https://img.shields.io/badge/svelte-%23f1413d.svg?style=for-the-badge&logo=svelte&logoColor=white)

![Linux](https://img.shields.io/badge/Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black)
![License](https://img.shields.io/github/license/revoproject/revo-stream?style=for-the-badge)

Desktop application for streamers and live production teams — build scenes,
manage sources, and run streaming or recording workflows in one place.
Powered by [revo-lib](https://github.com/RevoProject/revo-lib), built with
Rust, Tauri, and Svelte.

> [!WARNING]
> RevoStream is officially supported on Linux.
> Windows and macOS can be run in experimental/dev mode — see
> [docs/building.md](docs/building.md).

## Highlights

- Fast native backend powered by OBS/libobs
- Modern desktop UI for scene and source control
- Streaming and recording workflows out of the box
- RTMP, RTMPS, SRT, RIST, WHIP, WebRTC — official support for Kick
- Built-in and custom themes (`.revotheme` import)
- Dedicated utility windows, including Graphic Planner

## Quick start

**Stable release** — download from the
[Releases page](https://github.com/RevoProject/revo-stream/releases).

**From source (Linux)**

```bash
pnpm install
./run.sh          # development mode
```

**NixOS**

```bash
bash scripts/nix/compile.sh   # build deb + rpm
bash scripts/nix/run.sh       # run the built binary
```

Full instructions for all platforms: [docs/building.md](docs/building.md) ·
script reference: [docs/scripts.md](docs/scripts.md)

## Tech stack

Rust · Tauri · Svelte · OBS/libobs (via [revo-lib](https://github.com/RevoProject/revo-lib))

## Contributing

The project is in an early MVP stage — contributions are welcome:

- Open an issue with a bug report or feature request
- Discuss major changes before implementation
- Submit pull requests with a clear scope and summary

PRs for libobs bindings and performance optimizations are especially welcome.

## License

GNU Affero General Public License v3.0 — see [LICENSE](LICENSE).
