# RevoStream 0.4.1 - Windows RC checklist

Baseline: commit `6e3b8db` (2026-10-14).

## Release blockers

- [ ] Run the Windows job on `windows-2022` and confirm `Smoke test Windows MSI` passes: silent install, 12-second launch, silent uninstall and removal of `revo-ui.exe`.
- [ ] Confirm `Smoke test Windows portable` passes after extracting `revo-ui-windows-x64.zip`.
- [ ] Test on a clean Windows VM without developer tools. Verify whether Microsoft Visual C++ 2015-2022 Redistributable and WebView2 Evergreen must be bundled or documented as requirements.
- [ ] Record for at least one minute, close RevoStream with the native `X`, and confirm the MP4 is playable, non-empty and finalized.
- [ ] Repeat shutdown with active streaming. Confirm there are no orphaned `revo-ui.exe`, `obs-ffmpeg-mux.exe`, Chromium or WebView processes.
- [ ] Verify MSI upgrade from `0.4.0` to `0.4.1`, followed by uninstall. Confirm user settings are preserved during upgrade.

## OBS and capture

- [ ] Confirm application startup no longer enters Windows `Not Responding` while OBS modules and D3D11 initialize.
- [ ] Confirm `Stop OBS` releases scenes, sources, capture handles, browser sessions and outputs without terminating the application process.
- [ ] Confirm native window close performs the same graceful OBS shutdown before process exit.
- [ ] Add and render Windows Window Capture. Confirm the native OBS window list is populated and the selected window appears in preview.
- [ ] Add display, camera, media, browser and audio sources; edit, remove and recreate each one.
- [ ] Confirm newly created video sources larger than the canvas are immediately fitted while preserving aspect ratio.
- [ ] Switch scenes with active capture sources and confirm preview, recording and streaming continue correctly.

## NVENC

- [ ] Test with a supported NVIDIA GPU and current driver. Confirm the log contains the selected NVENC encoder ID and recording/streaming starts.
- [ ] Test without an NVIDIA GPU or with an unavailable driver. Confirm a failed hardware output retries exactly once with x264 and succeeds.
- [ ] Confirm non-encoder errors such as an invalid stream URL or unwritable recording path do not cause a pointless hardware-to-x264 retry loop.
- [ ] Confirm `obs-nvenc.dll` exists in MSI and portable packages. Do not bundle `nvEncodeAPI64.dll`; it is supplied by the NVIDIA driver.
- [ ] Optional: add diagnostics distinguishing module loaded, encoder registered, compatible GPU unavailable and encoder successfully started.

## Native Tauri windows

- [ ] Open Graphic Planner in an installed release and confirm it renders instead of showing a white window.
- [ ] Open Debug Console with F10 and confirm it renders.
- [ ] Open Browser Source interaction and confirm the dedicated `interact.html` window loads and accepts input.
- [ ] Check WebView2 DevTools for missing `_app` assets in all three windows.

## UI and display scaling

- [ ] Test Windows display scaling at 100%, 125%, 150% and 200%.
- [ ] Test one and multiple monitors with different DPI values.
- [ ] Resize the Tools panel above and below 360 px. Large panels should show descriptions; smaller panels should show all four tool names without clipping.
- [ ] Verify Settings and Source Editor modals at low window height; Save and Close must remain visible.
- [ ] Verify long source names wrap inside their cards.

## Plugins

- [ ] Enable `obs-vst`, restart OBS and confirm `vst_filter` is registered. Test a known-safe VST plugin and its path picker.
- [ ] Enable `obs-websocket`, set a password, restart OBS and verify a client can connect to port 4455 and authenticate.
- [ ] Confirm disabling either optional module prevents it from loading on the next OBS start.

## Release workflow

- [ ] Dispatch with `Publish dev release` disabled. All three platforms should upload artifacts without creating a tag or release.
- [ ] Dispatch with `Publish dev release` enabled. Confirm a `build-YYYYMMDD-HHMM` prerelease contains MSI, Windows portable ZIP, DMG, AppImage, DEB, RPM and Linux portable files.
- [ ] Confirm the tag, release notes and workflow summary all reference the exact `github.sha` built by the jobs.
- [ ] Confirm release notes use the expected range beginning at the latest reachable version or `build-*` tag.
- [ ] Push a production tag such as `0.4.1` and confirm the production release workflow starts. The workflow also accepts `v0.4.1`.
- [ ] Configure Windows EXE/MSI code signing before public production distribution to reduce SmartScreen warnings.

## Already verified automatically

- [x] Frontend type checking and production build pass.
- [x] Static build emits `source-transform.html`, `debug-console.html`, `interact.html` and `preview.html`.
- [x] Rust library tests pass: 14 passed, 3 environment-dependent tests ignored.
- [x] Rust release check passes.
- [x] Windows workflow YAML contains MSI and portable smoke test steps.
- [x] Hardware encoder retry is bounded to two streaming attempts.
- [x] Tools descriptions are controlled by panel-height container queries.

## Separate platform work

- [ ] Wayland desktop capture still requires a real XDG Desktop Portal ScreenCast and PipeWire integration test; X11 capture support does not prove Wayland support.
- [ ] Run the existing NixOS runtime/package pipeline on a physical NixOS system.
