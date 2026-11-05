# Tauri/Svelte to GPUI Migration Guide

## Overview

RevoStream is moving from a Tauri 2 application with a Svelte 5/SvelteKit
frontend to a native Rust application rendered by
[GPUI](https://gpui.rs/). The migration removes the webview, JavaScript/Rust
IPC boundary, and duplicated cross-window browser state. The target is one
Rust process containing the GPUI application, typed UI state, and the existing
libobs-based backend.

The migration must preserve the current component layout and user workflows:

- Header controls for OBS lifecycle, recording, streaming, settings, history,
  realtime refresh, and Studio Mode.
- The preview/program area and transition presentation.
- The lower Scenes, Sources, and Tools panels, including resize and dock
  behavior.
- Audio Mixer, Transitions, Plugins, source editors, filters, settings, quick
  actions, context menus, and confirmation dialogs.
- Graphic Planner, Browser Interaction, Preview, and Debug Console feature
  boundaries, whether shown in the main window or in auxiliary windows.
- Existing scene, profile, settings, theme, plugin, recording, streaming,
  browser-source, and diagnostics behavior where it is applicable to a native
  UI.

This is an incremental rewrite, not a one-commit replacement. The Tauri and
Svelte application remains the parity reference and should stay buildable
until the corresponding GPUI checkpoints pass. Backend/business modules must
first be separated from Tauri, then consumed by both a temporary Tauri adapter
and the new GPUI application. Tauri configuration, frontend packages, and
Svelte source can be removed only after the final parity checkpoint.

## Architecture Comparison

### Current Tauri/Svelte model

The current process contains a Rust backend and one or more webviews:

```text
Svelte components and browser state
        |
        | invoke(command, JSON), emit/listen, postMessage, localStorage
        v
Tauri command/window/plugin layer
        |
        v
ObsState -> libobs, settings, profiles, devices, files, Chromium/CDP
```

`src/routes/+page.svelte` is currently the application coordinator as well as
the main view. It owns most scene/source state, backend calls, timers, preview
polling, settings drafts, undo/redo closures, modal state, dock geometry, and
window synchronization. Auxiliary routes are rendered in Tauri windows or
embedded as Windows modals.

The Rust backend exposes approximately one hundred string-addressed Tauri
commands. `tauri::State<ObsState>` is passed through many otherwise
framework-neutral functions. Tauri also supplies paths, dialogs, webview
windows, child webviews, events, tray integration, packaging, and the async
runtime.

### Target GPUI model

The target process uses typed Rust calls and observable GPUI entities:

```text
GPUI windows and Render implementations
        |
        | actions, callbacks, Entity<T> updates, subscriptions
        v
Application entities and typed service facade
        |
        | typed requests/results and immutable snapshots
        v
Dedicated OBS service thread -> libobs
        |
        +-> device/file/browser workers
```

Recommended state boundaries are:

| Entity/service | Responsibility |
| --- | --- |
| `Workspace` | Main composition root, selected panels, modal stack, focus, and global commands. |
| `BackendStatus` | OBS startup/shutdown state, version, errors, and shutdown coordination. |
| `SceneStore` | Scene list/order, program and preview scenes, scene locks, thumbnails. |
| `SourceStore` | Sources, selection, visibility/order, reusable sources, properties, and filters. |
| `PreviewModel` | Latest frame, generation, freeze state, dimensions, and transition state. |
| `AudioModel` | Meter snapshots, mute/volume/monitoring controls, and audio filters. |
| `OutputState` | Recording/streaming state, output paths, service state, and errors. |
| `SettingsStore` | Persisted settings, profiles, draft changes, accessibility, and native theme. |
| `PlannerState` | Canvas resolution, source transforms, selection, history, crop/resize/rotate state. |
| `DockState` | Dock placement and size plus the selected browser strategy. |
| `DebugState` | Logs, trace snapshots, preview statistics, and filters. |
| `ObsService` | The sole owner of `ObsRuntime` and all non-callback libobs pointer access. |

GPUI entities are retained state. Views implement `Render` and derive the
current element tree from entity state. A mutation updates an entity and calls
`cx.notify()` to schedule rendering. `cx.subscribe`/observers and typed
callbacks replace Tauri events, browser `postMessage`, and storage events.

Application commands should be declared as typed GPUI actions and bound in the
appropriate key context. Examples include `StartRecording`, `OpenSettings`,
`SelectScene`, `Undo`, and `ToggleStudioMode`. `cx.bind_keys` and
`on_action` replace global DOM keyboard listeners. Focus is represented by
`FocusHandle`/`Focusable`, not `document.activeElement` and query selectors.

The new architecture must not reproduce IPC as a string-based in-process
dispatcher. Public service methods and messages should use domain types such
as `SceneId`, `SourceId`, `TransitionKind`, `CanvasResolution`, and
`SourceTransform`.

## Dependency Mapping

### Rust dependencies

| Dependency | Status | Replacement | Notes |
| --- | --- | --- | --- |
| `tauri-build` | Remove | Normal Cargo build script and chosen packager | Keep native `libobs` linker directives from `build.rs`; remove `tauri_build::build()`. Move source-tree mutation out of the build script. |
| `tauri` | Replace | `gpui` plus narrow platform crates | GPUI owns application/window/render state. Typed methods, entities, actions, and subscriptions replace commands and events. |
| `tauri-plugin-dialog` | Replace | `rfd`, or `ashpd` where Linux portals are required | Run blocking dialogs away from the GPUI foreground executor and return `PathBuf`. |
| `tauri-plugin-opener` | Remove or replace | `open` or platform APIs | No frontend plugin call is currently present. Retain equivalent behavior only for validated external URL/file actions. |
| `serde` | Keep | None | Required by settings, profiles, scene interchange, themes, diagnostics, and CDP messages. |
| `serde_json` | Keep | None | Required for persistence and external formats. Prefer typed internal models over `Value` where practical. |
| `libc` | Keep initially | Optional targeted platform crates later | Used for Unix process groups, dynamic library checks, and time helpers. |
| `image` (`png`) | Keep | None initially | Keep screenshots/export and Chromium decoding. The live preview should move to raw RGBA or a GPUI image/texture path. |
| `base64` | Keep initially | Remove from native preview path | Still needed while CDP screencast frames are base64 encoded. |
| `uuid` | Keep, reassess | `tempfile` for temporary directories | Popup labels disappear, but collision-safe Chromium directories remain useful. |
| `tungstenite` | Keep | None initially | Existing synchronous CDP implementation remains isolated on browser worker threads. |
| `zip` | Keep | None | Required for `.revotheme` import. Continue validating extracted paths. |
| `revo-cef` | Remove unless implemented | A proven browser-engine integration | The current feature does not call `revo-cef`; `cef_dock_render_frame` actually starts headless Chromium. |
| `revo-lib` | Keep | None | Core libobs bindings and runtime metadata. Pin a revision/tag for reproducible builds. |
| `gpui` | Add | Replaces Tauri/Svelte UI | Pin to a known Zed revision or released version because GPUI APIs evolve with Zed. |
| `url` | Add | Replaces `tauri::Url` | Use for browser-source and external URL parsing/validation. |
| `directories` or `directories-next` | Add | Replaces Tauri path resolution | Build a single `AppPaths` value for resources, config, data, cache, logs, and temp files. |
| `tray-icon` | Conditional add | Replaces Tauri tray support | Add only if minimize-to-tray remains required and GPUI does not provide the needed platform integration. |
| `flume`/`crossbeam-channel` or equivalent | Add | OBS actor transport | Use typed requests and one-shot responses; select one channel implementation during backend extraction. |
| `tempfile` | Recommended add | Safer temporary browser directories/files | Avoid manually managed UUID paths where ownership can be expressed by a guard. |

The current `staticlib` and `cdylib` outputs, `custom-protocol` feature, and
Tauri `cef` feature should be removed from the final GPUI manifest unless an
external consumer or real CEF implementation requires them. The normal target
is an `rlib` backend crate plus one desktop binary.

### Frontend dependencies

These packages remain during the compatibility phase and are removed after
the GPUI parity gate.

| Dependency | Status | Replacement | Notes |
| --- | --- | --- | --- |
| `@tauri-apps/api` | Remove at final cutover | GPUI windows/entities/actions and direct Rust services | Includes `invoke`, events, window/webview APIs, DPI types, and `convertFileSrc`. |
| `@tauri-apps/plugin-dialog` | Remove at final cutover | Native Rust dialog crate | Paths become `PathBuf`, not bridge strings. |
| `@tauri-apps/plugin-opener` | Remove | `open` or platform API if needed | Currently no frontend import was found. |
| `svelte` | Remove at final cutover | Rust types implementing `Render` | Preserve component boundaries, not the Svelte runtime. |
| `@sveltejs/kit` | Remove | GPUI application/window construction | Routes become views or auxiliary GPUI windows. |
| `@sveltejs/adapter-static` | Remove | None | Static web assets are no longer the application shell. |
| `@sveltejs/vite-plugin-svelte` | Remove | None | No Svelte compilation after cutover. |
| `vite` | Remove | Cargo build tooling | No web development server or frontend bundle. |
| `typescript` | Remove | Rust compiler | Shared TypeScript models become Rust domain/view-model types. |
| `svelte-check` | Remove | `cargo check`, Clippy, and GPUI tests | Keep the old check in CI until the Svelte adapter is deleted. |
| `@tauri-apps/cli` | Remove at final cutover | Cargo plus selected packaging tooling | Release scripts must be updated before removal. |

### Tauri API replacements

| Existing API/behavior | GPUI/native strategy |
| --- | --- |
| `#[tauri::command]` + `invoke` | Typed methods on `ObsService` and application entities. Do not preserve string command names internally. |
| `tauri::State<ObsState>` | An owned `ObsService` handle plus GPUI `Entity<T>` state. The service thread owns `ObsRuntime`. |
| `emit`, `emitTo`, `listen` | Shared entity updates, subscriptions, typed callbacks, or channel messages. |
| `WebviewWindow` and window labels | `cx.open_window` with a per-window root entity and shared application entities. |
| Child `Webview` geometry control | No direct GPUI equivalent. Choose external browser, native reimplementation, screenshot/CDP surface, or a separately proven browser host. |
| `getCurrentWindow`, close requests | GPUI `Window` handles and one application shutdown coordinator. |
| `LogicalPosition`/`LogicalSize` | GPUI bounds, pixels, window options, and layout callbacks. |
| Tauri tray/menu | GPUI support if available at the pinned revision, otherwise `tray-icon` routed into foreground actions. |
| Dialog plugin | `rfd`/`ashpd` on a background task, returning validated `PathBuf`s. |
| Opener plugin and `window.open` | Validated calls through `open`/platform APIs, or a GPUI auxiliary window for native content. |
| `convertFileSrc` | Load bytes or paths directly into a GPUI image resource. |
| `app.path()` | Injected `AppPaths` built from executable/resource layout and `directories`. |
| Tauri capability JSON | Typed Rust APIs and centralized validation. There is no webview capability boundary after cutover. |
| `tauri::async_runtime::spawn_blocking` | GPUI background executor or a dedicated worker/service thread. Never block the foreground executor. |

### Webview-only features

| Web feature | Native implementation strategy |
| --- | --- |
| CSS flexbox | GPUI `div().flex()`, `flex_row`/`flex_col`, gaps, dimensions, alignment, padding, and overflow styles. |
| CSS variables and theme stylesheets | A typed native `Theme` containing semantic colors, typography, spacing, radii, density, scale, motion, and accessibility values. |
| Media/container queries | Choose layout variants from available GPUI/window bounds during render. |
| CSS transitions/keyframes | GPUI animation/frame state where needed; honor reduced-motion settings. |
| `localStorage` | Versioned Rust settings/UI-state persistence. Window-independent state lives in shared entities. |
| `postMessage` and storage events | Shared entities and typed subscriptions. |
| DOM queries and `getBoundingClientRect` | GPUI layout/bounds callbacks and explicit element/view state. |
| `MutationObserver`/`ResizeObserver` | Direct model and layout notifications; the current observers exist mainly to align a child webview. |
| HTML drag and drop | GPUI mouse/drag listeners and typed drop state. |
| Browser focus trap | Focus handles, key contexts, modal focus scopes, and actions. |
| Clipboard API | GPUI clipboard APIs. |
| `FileReader` | Native file reads after a dialog returns a path. |
| `<img>`/canvas screenshots | GPUI image elements or a custom element consuming image bytes/raw RGBA. |
| `<video>` WebRTC/stinger playback | A native video surface/decoder integration; prototype this before claiming parity. GPUI alone is not a media player. |
| `<iframe>`/external website dock | Product decision: external browser, screenshot/CDP surface, native feature, or a proven embedded browser engine. |
| User-agent platform detection | Rust `cfg` plus runtime platform APIs. |
| Remote Google Fonts | Bundle licensed font files and load them through the application; do not depend on network font imports. |

### Visual parity contract

The GPUI rewrite must preserve the current colors exactly and the layout as
closely as native text and platform rendering permit. GPUI colors can use the
same hexadecimal values, so there is no reason to approximate the palette.
Layout should preserve component order, proportions, minimum sizes, spacing,
radii, and responsive breakpoints. Small glyph rasterization differences are
acceptable; arbitrary redesign is not.

The current visual sources of truth are:

- RevoClassic base tokens: `src/routes/+page.svelte`, the global `:root`
  block near the main page styles.
- RevoFuture palette, spacing, radius, and motion tokens:
  `static/revo-themes.css`.
- RevoFuture component styling: `static/revo-ui.css`.
- Main preview and three-panel geometry: `src/routes/+page.svelte`.
- Dock geometry: `src/lib/components/Home/Experimental/DockPanel.svelte`.

Do not copy raw colors into individual GPUI views. Define a native semantic
theme once, then use `theme.surface`, `theme.text`, `theme.accent`, and similar
fields everywhere. This preserves the current ability to change themes and
accessibility palettes without rewriting components.

#### RevoClassic color mapping

| CSS token | Exact GPUI value | Intended use |
| --- | --- | --- |
| `--bg` | `#0f1115` | Application background. |
| `--surface` | `#151820` | Primary panels and preview background. |
| `--surface-2` | `#1b1f2a` | Elevated panels, headers, and dialogs. |
| `--surface-3` | `#232838` | Active controls and tertiary surfaces. |
| `--border` | `#2b3142` | Default borders. |
| `--border-strong` | `#3a4258` | Emphasized controls and focus-adjacent borders. |
| `--text` | `#e5e7eb` | Primary text. |
| `--text-muted` | `#a1a1aa` | Secondary descriptions and inactive labels. |
| `--accent` | `#5b7cfa` | Primary actions and selection. |
| `--accent-strong` | `#3b5bdb` | Pressed/strong accent state. |
| `--success` | `#22c55e` | Online/success state. |
| `--danger` | `#ef4444` | Record, stop, destructive, and error state. |
| `--warning` | `#f59e0b` | Warning state. |

The extended classic tokens from `static/revo-themes.css` must also be copied:
strong text `#ffffff`, secondary text `#c8ceda`, disabled text `#6b7280`,
subtle border `#232838`, accent hover `#7690ff`, danger hover `#f26767`, and
success hover `#34d399`.

#### RevoFuture color mapping

| Semantic role | Exact GPUI value |
| --- | --- |
| Background | `#080810` |
| Header/window background | `#0a111b` |
| Surface | `#0c131b` |
| Elevated surface | `#0f1720` |
| Active surface | `#152238` |
| Sidebar | `#0a1017` |
| Selected blue surface | `#081838` |
| Subtle/default/strong border | `#182230` / `#202c3c` / `#2c3a4e` |
| Selected blue border | `#1c3a78` |
| Primary/hover/pressed blue | `#285cfc` / `#4c78fc` / `#2048c8` |
| Danger/hover/pressed red | `#e42c2c` / `#f04448` / `#c82028` |
| Success/hover green | `#20e438` / `#3cff78` |
| Warning | `#f0ac50` |
| Strong/default/secondary/muted text | `#f4f7fc` / `#e2e8f2` / `#b8c2d2` / `#8b98a9` |
| Disabled text | `#5a6678` |

Native theme example:

```rust
use gpui::{rgb, Rgba};

struct RevoTheme {
    background: Rgba,
    surface: Rgba,
    surface_elevated: Rgba,
    surface_active: Rgba,
    border: Rgba,
    border_strong: Rgba,
    text: Rgba,
    text_muted: Rgba,
    accent: Rgba,
    accent_strong: Rgba,
    success: Rgba,
    danger: Rgba,
    warning: Rgba,
}

impl RevoTheme {
    fn classic() -> Self {
        Self {
            background: rgb(0x0f1115),
            surface: rgb(0x151820),
            surface_elevated: rgb(0x1b1f2a),
            surface_active: rgb(0x232838),
            border: rgb(0x2b3142),
            border_strong: rgb(0x3a4258),
            text: rgb(0xe5e7eb),
            text_muted: rgb(0xa1a1aa),
            accent: rgb(0x5b7cfa),
            accent_strong: rgb(0x3b5bdb),
            success: rgb(0x22c55e),
            danger: rgb(0xef4444),
            warning: rgb(0xf59e0b),
        }
    }
}
```

If the pinned GPUI revision uses a different concrete color type, keep these
values and adapt only the constructor/type. Alpha colors such as overlays and
glows should preserve their current RGBA alpha, including the RevoFuture
settings overlay `rgba(0, 0, 0, 0.62)`.

#### Layout metrics

Use native constants instead of scattering pixel values across views:

| Existing layout rule | Native metric/behavior |
| --- | --- |
| Main window minimum | `1080 x 600` logical pixels. |
| RevoFuture header | `84px` high. |
| Main content maximum | `1400px` where the current theme applies it. |
| Workspace gap/padding | `12px` gap and `12px 12px 0` padding. |
| Preview wrapper | Maximum width `960px`, centered, flexible height. |
| Preview frame | Preserve source aspect ratio, usually `16:9`, black background, `1px` border, `18px` radius. |
| Lower panel band | Default `240px`, minimum `180px`; preserve at least `200px` for preview while resizing. |
| Three lower columns | Equal-width Scenes, Sources, Tools columns with `16px` gap. |
| Lower panel container | `20px` padding, `24px` outer margin, `12px` radius. |
| RevoFuture spacing scale | `4, 8, 12, 16, 20, 24, 28, 32px`. |
| RevoFuture radius scale | `6, 8, 10, 12, 16, 20px`, plus pill radius. |
| Side dock | Default `420px`, minimum `280px`, maximum `62%` of window width. |
| Bottom dock | Clamp height between `220px` and `460px`, targeting `33%` of window height. |
| Dock header | `34px` high. |
| Medium breakpoint | At `1200px`, use two columns: Scenes/Tools above Sources. |
| Compact breakpoint | At `800px`, stack Scenes, Tools, Sources and stack dock/workspace vertically. |

The native shell should follow this structure:

```text
Window (column, full size, background token)
├── HeaderBar (84px for RevoFuture)
└── Workspace (flexible, optional left/right/bottom dock)
    └── WorkspaceMain (column)
        ├── PreviewBand (flexible, minimum 200px)
        │   └── PreviewFrame (centered, max 960px, source aspect ratio)
        ├── ResizeHandle
        └── LowerPanels (240px default, 180px minimum)
            ├── Scenes (equal width)
            ├── Sources (equal width)
            └── Tools (equal width)
```

Illustrative GPUI layout:

```rust
fn workspace_shell(theme: &RevoTheme, panel_height: f32) -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(theme.background)
        .text_color(theme.text)
        .child(header_bar().h(px(84.0)))
        .child(
            div()
                .flex_1()
                .min_h(px(0.0))
                .flex()
                .flex_col()
                .child(
                    preview_band()
                        .flex_1()
                        .min_h(px(200.0))
                        .items_center()
                        .justify_center(),
                )
                .child(panel_resize_handle())
                .child(
                    div()
                        .h(px(panel_height.max(180.0)))
                        .flex()
                        .gap_4()
                        .m_6()
                        .p_5()
                        .border_1()
                        .border_color(theme.border)
                        .rounded_xl()
                        .child(scenes_panel().flex_1())
                        .child(sources_panel().flex_1())
                        .child(tools_panel().flex_1()),
                ),
        )
}
```

This example describes the parity target, not a requirement to keep all views
in one function. The final implementation should split each panel into its own
`Render` type and select row/column variants from current window bounds.

#### Visual acceptance criteria

- At `1280x720`, `1440x900`, and the `1080x600` minimum, compare GPUI and
  Tauri reference screenshots for RevoClassic and RevoFuture.
- Semantic colors must match their source hex/RGBA values exactly.
- Major container bounds, gaps, padding, and radii should be within `2px` at
  the reference logical resolution unless native font metrics require a
  documented exception.
- The preview aspect ratio and containment must match exactly; content must
  never stretch.
- At `1200px` and `800px`, verify the same responsive panel ordering described
  above.
- Verify default, hover, pressed, selected, disabled, focus, success, warning,
  danger, recording, and streaming states.
- Repeat visual checks with high contrast, reduced motion, UI density, font
  scale, and each color-vision palette.
- Keep golden screenshots in tests or release artifacts so later GPUI changes
  cannot silently redesign the application.

## Component Migration Guide

### Component boundaries

Map each Svelte feature boundary to a Rust view or entity rather than copying
the main page into one large `Render` implementation:

| Svelte surface | GPUI target |
| --- | --- |
| `src/routes/+page.svelte` | `Workspace` composition root plus smaller application entities. |
| `HeaderBar.svelte` | `HeaderBar` view rendering output/lifecycle actions and shared status. |
| `ScenesPanel.svelte` | `ScenesPanel` view backed by `SceneStore`; typed reorder/select/context actions. |
| `SourcesPanel.svelte` | `SourcesPanel` view backed by `SourceStore`; typed visibility, lock, order, edit, and quick actions. |
| `DockPanel.svelte` | `DockLayout` entity/view containing panel placement and resize state. |
| `AudioMixer*.svelte` | `AudioMixer` and dialog entities backed by `AudioModel`. |
| `Transitions.svelte` | `TransitionsPanel` backed by typed transition settings and validation tasks. |
| `SettingsModal.svelte` | `SettingsDialog` with a draft entity separate from persisted `SettingsStore`. |
| Source/filter modal family | Per-dialog draft entities; commit through typed service requests. |
| Quick modal family | Small typed modal views sharing a common modal shell. |
| `/source-transform` | `PlannerWindow`/`PlannerView` sharing `PlannerState`. |
| `/interact` | `BrowserInteractionView` consuming decoded frames and forwarding GPUI input events to CDP. |
| `/preview` | `PreviewWindow` observing the shared `PreviewModel`. |
| `/debug-console` | `DebugConsoleWindow` observing `DebugState`. |
| `/webview` | Remove or replace according to the browser-dock decision. |

Reusable views should receive entity handles or compact view-model values.
They should emit typed actions/callbacks and should not call libobs directly.
Keep transient edit state in the owning dialog/entity so cancellation does not
mutate persisted or backend state.

### Svelte to GPUI example

Svelte:

```svelte
<script lang="ts">
  export let scene: SceneInfo;
  export let onSelect: (name: string) => void;
</script>

<button class:active={scene.active} on:click={() => onSelect(scene.name)}>
  {scene.name}
</button>
```

Equivalent GPUI shape:

```rust
use gpui::{div, prelude::*, rgb, Context, MouseButton, SharedString, Window};

struct SceneRow {
    name: SharedString,
    active: bool,
}

impl SceneRow {
    fn select(&mut self, cx: &mut Context<Self>) {
        // In production, dispatch a typed SelectScene action or update SceneStore.
        cx.notify();
    }
}

impl Render for SceneRow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let background = if self.active { rgb(0x2457d6) } else { rgb(0x20242b) };

        div()
            .id(("scene-row", self.name.clone()))
            .flex()
            .items_center()
            .w_full()
            .px_3()
            .py_2()
            .bg(background)
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.select(cx)))
            .child(self.name.clone())
    }
}
```

The exact event method can change with the pinned GPUI revision. Keep the
domain action (`SelectScene`) stable and isolate framework syntax in the view.

### Layout mapping

| CSS/Svelte layout | GPUI style |
| --- | --- |
| `display: flex` | `.flex()` |
| `flex-direction: column` | `.flex_col()` |
| `justify-content` | `.justify_start()`, `.justify_center()`, `.justify_between()`, etc. |
| `align-items` | `.items_start()`, `.items_center()`, `.items_end()`, etc. |
| `gap`, padding, margin | `.gap_*()`, `.p_*()`, `.px_*()`, `.m_*()` or pixel values. |
| fixed/fractional dimensions | `.w(...)`, `.h(...)`, `.w_full()`, `.size_full()`, flex growth/shrink styles. |
| border/radius/shadow | `.border_*()`, `.border_color(...)`, `.rounded_*()`, `.shadow_*()`. |
| scroll/overflow | GPUI overflow/scroll handles and list primitives at the pinned revision. |
| absolute overlays/z-index | Overlay children/layers with explicit positioning and ordering. |
| responsive columns | Render layout variants from current bounds; preserve minimum 1080x600 behavior and test narrower windows deliberately. |

The current shell should be recreated in this order: full-window column,
Header Bar, preview/program band, horizontal resize handle, and the lower
Scenes/Sources/Tools row. Preserve panel minimum sizes and scrolling before
porting visual detail.

### State and event mapping

| Svelte/browser mechanism | GPUI mechanism |
| --- | --- |
| top-level `let` state | Fields on focused entities. |
| `$:` derived expression | A computed method or explicit mutation followed by `cx.notify()`. |
| `bind:value`/`bind:checked` | Controlled input state updated by typed handlers. |
| `createEventDispatcher` | Typed actions, callbacks, or entity subscriptions. |
| `onMount`/`onDestroy` | Entity/window construction, stored subscriptions/tasks, and drop/close cleanup. |
| `tick()` | Deferred/next-frame work or focus after window/view creation. |
| global keyboard handlers | `actions!`, key bindings, key contexts, and `on_action`. |
| timers and `requestAnimationFrame` | GPUI tasks/timers or backend producers; cancellation is owned by the entity. |
| toast timeout | Notification entity plus a cancellable delayed task. |
| undo closures | Typed commands or before/after snapshots with explicit domain ownership. |

### Window and event migration

`init`, `requestInit`, and `saveTransforms` should disappear. The planner is
opened with or observes the shared `PlannerState`; save invokes a typed command
containing transforms. `revo:theme-refresh`, `postMessage`, and storage-event
theme synchronization become one `Theme` entity observed by every window.

Preview, planner, interaction, and debug console windows should share model
entities with the main workspace. Window closure must not destroy backend
state. Main-window close, tray quit, and explicit quit must all enter one
idempotent state machine:

```text
Running -> Confirming (if dirty/active output) -> ShuttingDown -> Stopped
```

## Backend Integration

### Extract a framework-neutral backend

Create a Rust workspace with a backend library and application adapters. A
possible shape is:

```text
crates/revo-core/       models, settings, scenes, sources, runtime, devices
crates/revo-gpui/       GPUI application, entities, views, platform adapters
crates/revo-tauri/      temporary compatibility adapter during migration
```

Names may be adjusted to repository conventions. The important rule is that
`revo-core` cannot import Tauri or GPUI. It accepts injected `AppPaths`, typed
requests, and narrow platform callbacks. Existing serialization and libobs
logic should move with minimal behavior changes.

Replace `tauri::State<ObsState>` parameters with service methods or internal
`&mut ObsRuntime` functions. Replace `tauri::AppHandle` uses with narrow
abstractions for paths, foreground dispatch, or window requests; do not pass a
GUI framework handle into business logic.

### Own libobs on one service thread

`ObsRuntime` contains raw libobs pointers and is currently marked `Send` and
`Sync`. A mutex prevents simultaneous access but does not guarantee OS-thread
affinity. GPUI tasks may run on different executor threads, so preserving this
model would make thread-safety assumptions less clear.

The preferred design is a long-lived OBS service thread:

```rust
enum ObsRequest {
    Start { root: Option<PathBuf>, reply: Reply<Result<BackendSnapshot>> },
    SelectScene { scene: SceneId, reply: Reply<Result<SceneSnapshot>> },
    StartRecording { path: PathBuf, reply: Reply<Result<OutputSnapshot>> },
    Shutdown { reply: Reply<Result<()>> },
}
```

The concrete reply/channel types should be selected during implementation.
Only the service thread owns and mutates `ObsRuntime`. Filesystem writes, PNG
encoding, and other pure CPU work should happen after copying the required
data out of libobs ownership. Browser CDP workers remain separate so a stalled
websocket cannot block scene or output operations.

macOS currently requires selected OBS startup/shutdown work on the main
dispatch queue. Implement a narrow foreground dispatcher for those calls;
do not move all libobs work onto GPUI's foreground executor.

### GPUI async tasks

GPUI views start work through their context and update entities on completion.
The exact signatures must follow the pinned GPUI revision, but the ownership
pattern is:

```rust
impl Workspace {
    fn refresh_scenes(&mut self, cx: &mut Context<Self>) {
        let service = self.obs.clone();
        cx.spawn(async move |this, cx| {
            let snapshot = service.list_scenes().await?;
            this.update(cx, |workspace, cx| {
                workspace.scenes = snapshot;
                cx.notify();
            })?;
            anyhow::Ok(())
        })
        .detach();
    }
}
```

Rules:

- Never block the foreground executor with CDP waits, process waits, device
  enumeration, image encoding, filesystem traversal, or native dialogs.
- Keep one task/subscription owner so timers and loops stop when the entity or
  window closes.
- Prefer backend-published immutable snapshots over repeatedly polling full
  lists.
- Bound high-frequency data. Preview and audio meters should retain only the
  newest value rather than queueing every frame/sample.
- Convert service errors into typed UI errors at the entity boundary.

### Preview and audio

The current preview path performs GPU readback, PNG encoding, base64 encoding,
IPC transfer, webview decoding, and image display. The first GPUI version
should remove base64/IPC and pass raw RGBA bytes or a native image handle while
retaining the current preview generation, sequence, freeze, stale-frame, and
last-good-frame rules. Do not attempt cross-API zero-copy texture sharing until
the correct CPU path has parity and profiling data.

Audio meters should publish bounded snapshots at a fixed rate or expose an
atomic/latest-value store read by a GPUI timer. Avoid one queued service
request per source every 100 ms.

### Paths and packaging

Create an `AppPaths` value at startup with executable, resource, config, data,
cache, log, and temporary directories. Preserve `--revo-root`, `--obs-path`,
`REVO_ROOT`, packaged OBS resources, plugin discovery, graphics modules,
FFmpeg paths, and development fallbacks. Remove assumptions containing
`src-tauri` only after the final package layout is defined.

Tauri currently provides bundle metadata, icons, Windows NSIS configuration,
macOS plist injection, and Linux dependency declarations. Select and validate
a native Rust packaging approach before removing it. Preserve camera and
microphone usage descriptions, and audit screen-capture/accessibility
permissions. Recalculate Linux dependencies: WebKitGTK/libsoup can be removed
only when no embedded webview remains; GTK/AppIndicator may still be required
by chosen dialog/tray crates.

## Step-by-Step Migration Plan

### Phase 0: Baseline and decisions

1. Record golden screenshots, workflows, settings/profile samples, and
   performance baselines for the current application.
2. Add a parity checklist for every main panel, modal, auxiliary window,
   shortcut, import/export flow, and platform permission flow.
3. Pin the intended GPUI/Zed revision.
4. Decide the browser dock and WebRTC/video-surface scope. Prototype these
   separately because they have no automatic GPUI equivalent.
5. Checkpoint: existing Tauri application and tests still build in the
   supported development environment.

### Phase 1: Backend extraction

1. Create the Cargo workspace and framework-neutral backend crate.
2. Move pure models, settings, scene/source helpers, devices, logging, and
   serialization without behavior changes.
3. Add `AppPaths` and remove Tauri path access from core modules.
4. Keep thin Tauri command wrappers so the current UI remains functional.
5. Add unit tests around extracted settings, transforms, source
   classification, collection/profile import/export, and path resolution.
6. Checkpoint: Tauri UI passes its current smoke tests through the extracted
   backend.

### Phase 2: OBS service ownership

1. Introduce the dedicated service thread and typed request/response API.
2. Move `ObsRuntime` ownership to that thread and document callback exceptions.
3. Convert command groups incrementally: lifecycle, scenes, sources, preview,
   output, settings/profiles, then diagnostics.
4. Centralize shutdown and make it idempotent.
5. Add concurrency tests with a fake backend and stress scene/preview/output
   request ordering.
6. Checkpoint: no UI adapter accesses raw OBS pointers or locks `ObsRuntime`
   directly.

### Phase 3: GPUI application shell

1. Add the GPUI binary, main window, native theme tokens, application entities,
   key bindings, focus model, modal layer, toast layer, and shutdown flow.
2. Recreate Header, preview band, resize handle, and lower three-panel layout
   with placeholders.
3. Implement settings load/save, profile selection, theme selection, and
   accessibility scale/density/reduced-motion state.
4. Checkpoint: GPUI shell starts without libobs, resizes correctly, supports
   keyboard focus, and persists UI settings.

### Phase 4: Scenes and sources

1. Port scene list, program/preview selection, creation, rename, removal,
   locking, reorder, resolution, and thumbnails.
2. Port source list, selection, visibility, lock, reorder, reusable-source
   selection, add/edit/remove, and properties.
3. Port dialogs and context menus as typed draft entities.
4. Implement typed undo/redo rather than closures over mutable view state.
5. Checkpoint: scene/source parity tests pass against the same backend fixtures
   and a live libobs smoke environment.

### Phase 5: Native preview and outputs

1. Implement raw-RGBA/native-image preview with bounded latest-frame delivery.
2. Preserve preview freeze, generation, and last-good-frame behavior.
3. Port Studio Mode and transitions.
4. Port recording, streaming, encoder preference, status, close warnings, and
   output error handling.
5. Port audio meters and mixer controls.
6. Checkpoint: preview remains responsive during scene/source edits and output
   files/streams match the Tauri baseline.

### Phase 6: Editing tools and auxiliary windows

1. Port filters, advanced audio, transitions, plugins, settings, quick actions,
   history, and source-specific editors.
2. Port Graphic Planner with deterministic transform math and typed history.
3. Open Preview and Debug Console as GPUI windows sharing existing entities.
4. Port Browser Interaction using the selected CDP/frame strategy.
5. Checkpoint: every auxiliary feature works both as intended in-window and in
   an auxiliary GPUI window where that behavior remains required.

### Phase 7: Browser/video and platform integration

1. Implement the approved browser-dock strategy; delete unsupported fallback
   paths instead of retaining misleading CEF naming.
2. Implement or explicitly retire the WebRTC/native video preview path.
3. Add tray behavior, native dialogs, external opening, clipboard, and
   macOS/Windows/Linux permission/platform handling.
4. Establish GPUI package layouts, rpaths, OBS resources/plugins, signing, and
   release artifacts on all supported platforms.
5. Checkpoint: installed-package smoke tests pass, not only development runs.

### Phase 8: Final cutover

1. Run the full parity, accessibility, performance, and packaging matrix.
2. Remove the Tauri adapter, command macros, configs, capabilities, Tauri
   plugins/build dependency, Svelte source, package manifests/lockfile, Vite,
   and web-only assets.
3. Update scripts, CI, README, building documentation, and release workflows.
4. Remove WebKitGTK/libsoup and other webview dependencies only after package
   inspection confirms they are no longer transitively required.
5. Checkpoint: a clean checkout builds, tests, packages, installs, runs, and
   shuts down using only the GPUI application.

## Known Risks and Open Questions

### Critical risks

- **libobs ownership and thread affinity:** current unsafe `Send`/`Sync`
  assumptions must not leak into GPUI executor tasks. The service-thread design
  needs validation on every platform.
- **Preview integration:** raw CPU frames are the safe initial path. Direct
  libobs/GPUI GPU texture sharing is platform/backend-specific and should be a
  later optimization.
- **Shutdown ordering:** recording/streaming finalization, browser workers,
  libobs teardown, auxiliary windows, tray quit, and main-window close must be
  coordinated once.
- **Packaging:** libobs modules, data, FFmpeg, graphics backends, browser
  processes, rpaths, signing, and permissions are more complex than the UI
  toolkit change.

### Open product/technical questions

- Should the Browser Dock open externally, use the existing screenshot/CDP
  surface, embed a real browser engine, or be removed?
- Is interactive WebRTC preview required at GPUI cutover, or can the libobs
  preview be the only initial native preview?
- Must arbitrary existing `.revotheme` CSS remain import-compatible? GPUI can
  support a token format or constrained converter, but cannot apply arbitrary
  CSS directly.
- Is minimize-to-tray required on every platform, and which tray crate/version
  is acceptable with GPUI's event loop?
- Which GPUI revision will be pinned, and will selected UI primitives come
  directly from GPUI or from a separately versioned component crate?
- Should auxiliary tools always be separate windows, always in-app modals, or
  preserve the current platform-dependent behavior?
- What is the final workspace/package layout, and which native packager will
  replace Tauri bundling?
- Can the unpinned `revo-lib` Git dependency be pinned before migration work
  begins?

### Compatibility risks

- CSS themes, container queries, color filters, remote fonts, and browser
  animation semantics require native equivalents rather than mechanical
  translation.
- Focus trapping, IME/text input, clipboard, keyboard layouts, drag/reorder,
  context menus, accessibility, and high-DPI behavior need explicit native
  tests.
- The main Svelte route interleaves state, timers, rendering, and backend calls;
  missing an implicit dependency during decomposition can cause parity bugs.
- CDP calls and process polling contain blocking waits and must remain off the
  GPUI foreground and OBS service threads.
- Existing JSON/`HashMap<String, String>` boundaries are flexible but weakly
  typed. Convert them gradually at the libobs boundary, not in one broad
  refactor.

## Testing Strategy

### Baseline and parity fixtures

- Capture screenshots at the minimum window size, common desktop sizes, and
  high-DPI scales for every main workspace state and modal.
- Save representative settings, profiles, scene collections, OBS exports,
  themes, and source-property payloads as versioned fixtures.
- Record keyboard shortcuts, focus order, context menus, drag/reorder behavior,
  error dialogs, close confirmations, and auxiliary-window workflows.
- Measure startup, idle CPU, preview frame rate/latency, scene-switch latency,
  memory, and shutdown/finalization time.

### Unit tests

- Domain types, source classification, validation, transform math, crop/resize/
  rotate calculations, transition settings, and undo/redo commands.
- Settings/profile/theme parsing and version migration.
- Scene collection and OBS profile import/export round trips.
- `AppPaths` development, installed, and environment/CLI override resolution.
- Browser URL validation and archive path traversal protection.
- Reducer/entity mutations using a fake `ObsService`.

### Service and concurrency tests

- Request ordering and cancellation on the OBS service actor.
- Concurrent preview, scene edit, source edit, audio, recording, and streaming
  requests without deadlock or starvation.
- Idempotent startup/shutdown and repeated close/quit requests.
- Latest-frame behavior under slow rendering/encoding and stale generation
  rejection.
- Browser worker timeout and process cleanup without blocking OBS operations.

### GPUI tests

- Use GPUI's test context for entity state, actions, key bindings, focus,
  rendering decisions, and window-independent view logic where supported by
  the pinned revision.
- Test modal focus scopes, Escape behavior, Tab order, disabled actions,
  selection state, and keyboard-only operation.
- Snapshot or visually compare stable component states where deterministic
  rendering infrastructure permits it.
- Keep style values behind native theme tokens so light/dark/high-contrast
  variants can share behavioral tests.

### Live backend integration

- Start/stop OBS repeatedly and verify resources are released without exiting
  the application.
- Create, rename, reorder, lock, switch, import, and export scenes.
- Add/edit/remove every supported source and filter type.
- Validate preview while switching scenes, editing sources, opening tools, and
  recording/streaming concurrently.
- Verify recordings are non-empty and finalized after normal stop and window
  close. Verify stream reconnect/error behavior.
- Test audio meters, mute, monitoring, tracks, filters, and device changes.
- Test Chromium/CDP startup, interaction, timeout, and cleanup if browser
  features remain.

### Platform and package matrix

- Linux: X11 and Wayland/portal behavior, font/device/window enumeration,
  AppImage/DEB/RPM dependencies, tray/dialog behavior, and OBS plugin loading.
- Windows: 100/125/150/200% scaling, mixed-DPI monitors, capture sources,
  NVENC fallback, installed and portable layouts, process cleanup, and signing.
- macOS: camera, microphone, screen recording, and accessibility permissions;
  main-thread OBS startup/shutdown; app bundle resources/rpaths; codesigning and
  notarization.
- Test clean machines without developer dependencies. Inspect packages to
  confirm webview libraries and Node/frontend artifacts are absent after final
  cutover.

The final acceptance gate is behavioral parity for supported features plus a
clean single-language build: no webview runtime, no JavaScript IPC, no frontend
build step, deterministic backend shutdown, and installed packages that can
locate and load the complete OBS runtime.
