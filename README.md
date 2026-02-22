# RevoStream
![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white&color=%23CE412B)
![Tauri](https://img.shields.io/badge/tauri-%232E7EEA.svg?style=for-the-badge&logo=tauri&logoColor=%23FFFFFF)
![Svelte](https://img.shields.io/badge/svelte-%23f1413d.svg?style=for-the-badge&logo=svelte&logoColor=white)

![Linux](https://img.shields.io/badge/Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black)
![License](https://img.shields.io/github/license/revoproject/revo-stream?style=for-the-badge)

## Introduction
RevoStream is a desktop application for streamers and live production teams.
It helps you build scenes, manage sources, and run streaming or recording workflows in one place.

Powered by [revo-project/revo-lib](https://github.com/RevoProject/revo-lib), RevoStream is built with Rust, Tauri, and Svelte.

> [!WARNING]
> RevoStream currently supports Linux only. The current version may be unstable.

## Quick navigation
- [Highlights](#highlights)
- [Streaming](#streaming)
- [Themes](#themes)
- [Usage](#usage)
- [Tech stack](#tech-stack)
- [Contributing](#contributing)
- [License](#license)

## Highlights
- Fast native backend powered by OBS/libobs
- Modern desktop UI for scene and source control
- Built-in tools for streaming and recording workflows
- Dedicated utility windows, including Graphic Planner
- Close to 50,000 lines of code

## Streaming
- Supports streaming services available in recent OBS Studio builds
- Official support for Kick streaming
- Protocol support: RTMP, RTMPS, SRT, RIST, WHIP, WebRTC

## Themes
- Use built-in themes or create your own custom theme for your mood
- Quickly import themes via .revotheme extension

### Goals
- Worldwide language support
- Support for more operating systems (e.g. Windows, macOS, Android)

## Usage
### Stable release
Download the latest version from the [Releases page](https://github.com/RevoProject/revo-stream/releases).

### Build from source
Install dependencies and build:

```bash
pnpm build
```

### Run locally
- Development mode: `./run.sh`
- Stable-release build: `./run-stable.sh`

## Tech stack
- Rust (with custom created libobs bindings)
- Tauri
- Svelte
- OBS/libobs via revo-lib

## Other Scripts
- clean.sh - for cleanup all built code

## Contributing
The project is currently in an early MVP stage
Contributions are welcome

If you want to help:
- Open an issue with a bug report or feature request
- Discuss major changes before implementation
- Submit a pull request with a clear scope and summary

Pull requests for libobs bindings and performance optimizations are especially welcome

**Let's build the future of streaming together**

## License
This project is distributed under the license included in this repository



