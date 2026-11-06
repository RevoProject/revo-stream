use std::collections::{BTreeMap, HashMap};

#[cfg(feature = "obs-backend")]
use std::time::Duration;

#[cfg(feature = "obs-backend")]
use base64::Engine as _;

use gpui::{
    div, prelude::*, px, size, App, Application, Bounds, Context, Entity, IntoElement, MouseButton,
    Pixels, Render, SharedString, Subscription, TitlebarOptions, Window, WindowBounds,
    WindowHandle, WindowOptions,
};
use revo_core::models::AppSettings;
use revo_core::service::{BackendCommand, BackendError, BackendSnapshot};

#[cfg(feature = "obs-backend")]
use gpui::{Image as GpuiImage, ImageFormat};

use crate::services::AnyBackend;
#[cfg(feature = "obs-backend")]
use crate::services::{capture_preview_png, ObsBackendRuntime};

use crate::{
    actions::{AuxiliaryWindowKind, ComponentEvent, QuickSourceAction},
    components::{
        home::{
            DemoMode, HeaderBar, HeaderBarState, SceneItem, ScenesPanel, SourceItem, SourcesPanel,
        },
        modal::{SettingsEvent, SettingsModal},
        modal_shell::{modal_card, ModalConfig, ModalSize},
        modules::{
            audio_mixer::{AudioChannel, AudioMixerPanel, MixerEvent},
            audio_mixer_advanced_modal::{AdvancedDraft, AudioMixerAdvancedModal},
            audio_mixer_context_menu::AudioMixerContextMenu,
            graphic_planner::{GraphicPlannerPanel, PlannerEvent, PlannerSource, PlannerTransform},
            transitions::{
                blend_transition_params, TransitionEvent, TransitionItem, TransitionKind,
                TransitionsPanel,
            },
        },
    },
    theme::RevoTheme,
    windows::{
        debug_console::DebugConsoleWindow, interact::InteractWindow, preview::PreviewWindow,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutMode {
    Wide,
    Medium,
    Compact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelKind {
    Scenes,
    Sources,
    Tools,
}

impl LayoutMode {
    pub fn for_width(width: f32) -> Self {
        if width >= 1200.0 {
            Self::Wide
        } else if width >= 800.0 {
            Self::Medium
        } else {
            Self::Compact
        }
    }

    pub fn panel_order(self) -> [PanelKind; 3] {
        match self {
            Self::Wide => [PanelKind::Scenes, PanelKind::Sources, PanelKind::Tools],
            Self::Medium | Self::Compact => {
                [PanelKind::Scenes, PanelKind::Tools, PanelKind::Sources]
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActiveModal {
    None,
    Settings,
    History,
    AddScene,
    AddSource,
    SourceEditor(String),
    AudioMixerAdvanced(String),
    GraphicPlanner,
    QuickAction {
        source_id: String,
        action: QuickSourceAction,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceState {
    pub active_modal: ActiveModal,
    pub last_command: Option<BackendCommand>,
    pub status: String,
    /// One-shot auxiliary-window request routed from the Tools panel;
    /// consumed (`take`) by the Workspace to open/focus the window.
    pub requested_window: Option<AuxiliaryWindowKind>,
    /// Persisted settings as most recently reported by the backend snapshot;
    /// seeds the settings draft when the modal opens.
    pub settings: Option<AppSettings>,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            active_modal: ActiveModal::None,
            last_command: None,
            status: "Native mock backend ready".into(),
            requested_window: None,
            settings: None,
        }
    }
}

/// Pure snapshot panels: maps a `BackendSnapshot` onto the Scenes panel items
/// in backend scene-order, keyed by the snapshot's active/locked sets.
fn snapshot_scene_items(snapshot: &BackendSnapshot) -> Vec<SceneItem> {
    snapshot
        .scenes
        .iter()
        .map(|name| SceneItem {
            name: name.clone(),
            active: snapshot.selected_scene.as_deref() == Some(name.as_str()),
            locked: snapshot.locked_scenes.iter().any(|locked| locked == name),
        })
        .collect()
}

/// Pure snapshot panels: maps the snapshot's source items onto the Sources
/// panel rows, with locks taken from the snapshot's locked-source id list.
fn snapshot_source_items(snapshot: &BackendSnapshot) -> Vec<SourceItem> {
    snapshot
        .source_items
        .iter()
        .map(|source| SourceItem {
            id: source.id.clone(),
            name: source.name.clone(),
            source_type: source.source_type.clone(),
            visible: source.visible,
            locked: snapshot
                .locked_sources
                .iter()
                .any(|locked| locked == &source.id),
        })
        .collect()
}

impl WorkspaceState {
    pub fn route(&mut self, event: ComponentEvent) -> Option<BackendCommand> {
        let command = match event {
            ComponentEvent::OpenSettings => {
                self.active_modal = ActiveModal::Settings;
                return None;
            }
            ComponentEvent::OpenHistory => {
                self.active_modal = ActiveModal::History;
                return None;
            }
            ComponentEvent::OpenGraphicPlanner => {
                self.active_modal = ActiveModal::GraphicPlanner;
                return None;
            }
            ComponentEvent::OpenPreviewWindow => {
                self.requested_window = Some(AuxiliaryWindowKind::Preview);
                self.status = "Preview window requested".into();
                return None;
            }
            ComponentEvent::OpenDebugConsoleWindow => {
                self.requested_window = Some(AuxiliaryWindowKind::DebugConsole);
                self.status = "Debug console window requested".into();
                return None;
            }
            ComponentEvent::OpenBrowserInteractionWindow => {
                self.requested_window = Some(AuxiliaryWindowKind::BrowserInteraction);
                self.status = "Browser interaction window requested".into();
                return None;
            }
            ComponentEvent::AddScene => {
                self.active_modal = ActiveModal::AddScene;
                return None;
            }
            ComponentEvent::AddSource => {
                self.active_modal = ActiveModal::AddSource;
                return None;
            }
            ComponentEvent::EditSource(id) => {
                self.active_modal = ActiveModal::SourceEditor(id);
                return None;
            }
            ComponentEvent::QuickSourceAction { id, action } => {
                self.active_modal = ActiveModal::QuickAction {
                    source_id: id,
                    action,
                };
                return None;
            }
            ComponentEvent::Undo => BackendCommand::Undo,
            ComponentEvent::Redo => BackendCommand::Redo,
            ComponentEvent::SetStudioMode(enabled) => BackendCommand::SetStudioMode(enabled),
            ComponentEvent::SetRealtimeRefresh(enabled) => {
                BackendCommand::SetRealtimeRefresh(enabled)
            }
            ComponentEvent::StartObs => BackendCommand::StartObs,
            ComponentEvent::StopObs => BackendCommand::StopObs,
            ComponentEvent::StartRecording => BackendCommand::StartRecording,
            ComponentEvent::StopRecording => BackendCommand::StopRecording,
            ComponentEvent::StartStreaming => BackendCommand::StartStreaming,
            ComponentEvent::StopStreaming => BackendCommand::StopStreaming,
            ComponentEvent::ForcePreviewResolution => BackendCommand::ForcePreviewResolution,
            ComponentEvent::SelectScene(name) => BackendCommand::SelectScene(name),
            ComponentEvent::RenameScene { from, to } => BackendCommand::RenameScene { from, to },
            ComponentEvent::ReorderScene { name, to_index } => {
                BackendCommand::ReorderScene { name, to_index }
            }
            ComponentEvent::SelectSource(id) => BackendCommand::SelectSource(id),
            ComponentEvent::ToggleSourceVisibility(id) => {
                BackendCommand::ToggleSourceVisibility(id)
            }
            ComponentEvent::ToggleSourceLock(id) => BackendCommand::ToggleSourceLock(id),
            ComponentEvent::MoveSource { id, direction } => {
                BackendCommand::MoveSource { id, direction }
            }
        };

        self.status = format!("Backend command: {command:?}");
        self.last_command = Some(command.clone());
        Some(command)
    }

    pub fn close_modal(&mut self) {
        self.active_modal = ActiveModal::None;
    }

    /// Inline pure state reflecting one backend snapshot: header/navigation
    /// flags live on their panels, but the status line surfaces the backend's
    /// last error (real libobs runtime failures do not interrupt dispatch).
    pub fn apply_snapshot(&mut self, snapshot: &BackendSnapshot) {
        if let Some(last_error) = &snapshot.last_error {
            self.status = format!("Backend error: {last_error}");
        } else if self.status.starts_with("Backend error: ") || self.status.starts_with("Backend: ")
        {
            self.status = "Backend state synced".into();
        }
        self.settings = snapshot.settings.clone();
    }

    pub fn route_mixer(&mut self, event: MixerEvent) -> Option<BackendCommand> {
        let command = match event {
            MixerEvent::VolumeChanged { id, percent, mode } => {
                self.status = format!("Mixer {id} holds {percent:.1}% as {mode:?}");
                return None;
            }
            MixerEvent::VolumeCommitted { id, percent } => BackendCommand::SetSourceVolume {
                id,
                percent: percent as f64,
            },
            MixerEvent::MuteToggled { id, muted } => BackendCommand::SetSourceMuted { id, muted },
            MixerEvent::LockToggled { id, locked } => {
                BackendCommand::SetSourceVolumeLocked { id, locked }
            }
            MixerEvent::ContextMenuRequested { id } => {
                self.status = format!("Mixer context menu for {id}");
                return None;
            }
            MixerEvent::OpenFilters { id } => {
                self.status = format!("Audio filters channel: {id}");
                return None;
            }
            MixerEvent::OpenAdvanced { id } => {
                self.status = format!("Advanced audio properties: {id}");
                return None;
            }
            MixerEvent::AdvancedConfirmed { id } => {
                self.status = format!("Advanced audio confirmed: {id}");
                return None;
            }
            MixerEvent::AdvancedCancelled => {
                self.status = "Advanced audio cancelled".into();
                return None;
            }
        };
        self.status = format!("Backend command: {command:?}");
        self.last_command = Some(command.clone());
        Some(command)
    }

    pub fn route_transition(&mut self, event: TransitionEvent) -> Vec<BackendCommand> {
        let commands = match event {
            TransitionEvent::Apply { kind, duration_ms } => {
                vec![
                    BackendCommand::SetTransitionKind(kind),
                    BackendCommand::SetTransitionDuration(duration_ms),
                ]
            }
            TransitionEvent::Discard => {
                self.status = "Transitions draft discarded".into();
                Vec::new()
            }
        };
        if let Some(command) = commands.last() {
            self.status = format!("Backend command: {command:?}");
            self.last_command = Some(command.clone());
        }
        commands
    }

    pub fn route_planner(&mut self, event: PlannerEvent) -> Vec<BackendCommand> {
        match event {
            PlannerEvent::Apply { items } => {
                let commands = items
                    .into_iter()
                    .map(|item| BackendCommand::SetSourceTransform {
                        id: item.id,
                        x: item.x,
                        y: item.y,
                        width: item.width,
                        height: item.height,
                        rotation: item.rotation,
                    })
                    .collect::<Vec<_>>();
                if let Some(command) = commands.last() {
                    self.status = format!("Backend command: {command:?}");
                    self.last_command = Some(command.clone());
                }
                commands
            }
            PlannerEvent::Select(id) => {
                self.status = format!("Planner selected {id}");
                Vec::new()
            }
            PlannerEvent::Op { id, op } => {
                self.status = format!("Planner {op:?} on {id}");
                Vec::new()
            }
            PlannerEvent::Cancel => {
                self.status = "Planner closed".into();
                Vec::new()
            }
        }
    }
}

/// Handles for the auxiliary windows opened from the Tools panel. Each
/// handle is kept so repeated requests focus the existing window instead of
/// duplicating it; a stale (closed) handle is replaced on the next request.
#[derive(Default)]
pub struct AuxWindows {
    pub preview: Option<WindowHandle<PreviewWindow>>,
    pub debug_console: Option<WindowHandle<DebugConsoleWindow>>,
    pub interact: Option<WindowHandle<InteractWindow>>,
}

pub struct Workspace {
    pub header: Entity<HeaderBar>,
    pub scenes: Entity<ScenesPanel>,
    pub sources: Entity<SourcesPanel>,
    pub demo: Entity<DemoMode>,
    pub tools_mixer: Entity<AudioMixerPanel>,
    pub tools_transitions: Entity<TransitionsPanel>,
    pub mixer_menu: Entity<AudioMixerContextMenu>,
    pub mixer_advanced: Entity<AudioMixerAdvancedModal>,
    pub planner: Entity<GraphicPlannerPanel>,
    pub settings_modal: Option<Entity<SettingsModal>>,
    pub advanced_drafts: HashMap<String, AdvancedDraft>,
    pub aux_windows: AuxWindows,
    pub state: WorkspaceState,
    backend: AnyBackend,
    theme: RevoTheme,
    #[cfg(feature = "obs-backend")]
    preview_frame: Option<std::sync::Arc<GpuiImage>>,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    /// WindowOptions for one auxiliary window: centered on the active
    /// display at the given size, titled, and focused on open.
    fn auxiliary_window_options(
        title: &'static str,
        width: Pixels,
        height: Pixels,
        cx: &App,
    ) -> WindowOptions {
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(width, height),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                ..TitlebarOptions::default()
            }),
            focus: true,
            ..WindowOptions::default()
        }
    }

    /// Focuses an existing auxiliary window; false when the handle is stale
    /// (the window was closed by the user).
    fn focus_auxiliary<V: 'static + Render>(
        handle: &WindowHandle<V>,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        let app: &mut App = cx;
        handle
            .update(app, |_, window, _| window.activate_window())
            .is_ok()
    }

    /// Opens (or focuses) the requested auxiliary window with roots
    /// constructed from the shared backend clone.
    fn open_auxiliary_window(&mut self, kind: AuxiliaryWindowKind, cx: &mut Context<'_, Self>) {
        match kind {
            AuxiliaryWindowKind::Preview => {
                let handle = self.aux_windows.preview.take();
                match handle {
                    Some(handle) if Self::focus_auxiliary(&handle, cx) => {
                        self.aux_windows.preview = Some(handle);
                        self.state.status = "Preview window focused".into();
                    }
                    _ => {
                        let backend = self.backend.clone();
                        let options = Self::auxiliary_window_options(
                            "Preview - RevoStream",
                            px(1280.),
                            px(720.),
                            cx,
                        );
                        self.aux_windows.preview = cx
                            .open_window(options, |_, cx| {
                                cx.new(|cx| PreviewWindow::new(backend.clone(), None, cx))
                            })
                            .ok();
                        self.state.status = "Preview window opened".into();
                    }
                }
            }
            AuxiliaryWindowKind::DebugConsole => {
                let handle = self.aux_windows.debug_console.take();
                match handle {
                    Some(handle) if Self::focus_auxiliary(&handle, cx) => {
                        self.aux_windows.debug_console = Some(handle);
                        self.state.status = "Debug console window focused".into();
                    }
                    _ => {
                        let backend = self.backend.clone();
                        let options = Self::auxiliary_window_options(
                            "Debug Console - RevoStream",
                            px(900.),
                            px(600.),
                            cx,
                        );
                        self.aux_windows.debug_console = cx
                            .open_window(options, |_, cx| {
                                cx.new(|cx| DebugConsoleWindow::new(backend.clone(), cx))
                            })
                            .ok();
                        self.state.status = "Debug console window opened".into();
                    }
                }
            }
            AuxiliaryWindowKind::BrowserInteraction => {
                let handle = self.aux_windows.interact.take();
                match handle {
                    Some(handle) if Self::focus_auxiliary(&handle, cx) => {
                        self.aux_windows.interact = Some(handle);
                        self.state.status = "Browser interaction window focused".into();
                    }
                    _ => {
                        let backend = self.backend.clone();
                        let source_id = self.sources.read(cx).selected_id.clone();
                        let options = Self::auxiliary_window_options(
                            "Browser Interaction - RevoStream",
                            px(1280.),
                            px(720.),
                            cx,
                        );
                        self.aux_windows.interact = cx
                            .open_window(options, |_, cx| {
                                cx.new(|cx| {
                                    InteractWindow::new(backend.clone(), source_id.clone(), cx)
                                })
                            })
                            .ok();
                        self.state.status = "Browser interaction window opened".into();
                    }
                }
            }
        }
        cx.notify();
    }

    pub fn new(cx: &mut Context<'_, Self>) -> Self {
        let use_obs_backend = std::env::args().any(|argument| argument == "--obs");
        Self::with_backend(cx, use_obs_backend)
    }

    pub fn new_mock(cx: &mut Context<'_, Self>) -> Self {
        Self::with_backend(cx, false)
    }

    fn with_backend(cx: &mut Context<'_, Self>, use_obs_backend: bool) -> Self {
        let theme = RevoTheme::future();
        let backend = if use_obs_backend {
            #[cfg(feature = "obs-backend")]
            {
                AnyBackend::obs(None, cx.foreground_executor())
            }
            #[cfg(not(feature = "obs-backend"))]
            {
                let _ = use_obs_backend;
                AnyBackend::mock()
            }
        } else {
            AnyBackend::mock()
        };

        let header = cx.new(|_| HeaderBar {
            state: HeaderBarState {
                demo_mode: !use_obs_backend,
                can_undo: true,
                can_redo: true,
                version: format!("v{}", env!("CARGO_PKG_VERSION")),
                ..HeaderBarState::default()
            },
            theme,
        });
        let scenes = cx.new(|_| ScenesPanel {
            scenes: vec![
                SceneItem {
                    name: "Starting Soon".into(),
                    active: false,
                    locked: false,
                },
                SceneItem {
                    name: "Main Camera".into(),
                    active: true,
                    locked: false,
                },
                SceneItem {
                    name: "Be Right Back".into(),
                    active: false,
                    locked: false,
                },
            ],
            preview_scene: Some("Main Camera".into()),
            studio_mode: false,
            rename_draft: None,
            theme,
        });
        let sources = cx.new(|_| SourcesPanel {
            sources: vec![
                SourceItem {
                    id: "camera".into(),
                    name: "Camera".into(),
                    source_type: "v4l2_input".into(),
                    visible: true,
                    locked: false,
                },
                SourceItem {
                    id: "lower-third".into(),
                    name: "Lower Third".into(),
                    source_type: "text_ft2_source_v2".into(),
                    visible: true,
                    locked: false,
                },
                SourceItem {
                    id: "browser-chat".into(),
                    name: "Chat Overlay".into(),
                    source_type: "browser_source".into(),
                    visible: true,
                    locked: true,
                },
            ],
            selected_id: Some("lower-third".into()),
            theme,
        });
        let demo = cx.new(|_| DemoMode {
            theme,
            message: "Native GPUI shell running with representative mock scene data.".into(),
        });

        let mixer_channels = vec![
            AudioChannel::new("camera", "Camera"),
            AudioChannel::new("lower-third", "Lower Third"),
            AudioChannel::new("browser-chat", "Chat Overlay"),
        ];
        let advanced_drafts: HashMap<String, AdvancedDraft> = mixer_channels
            .iter()
            .map(|channel| (channel.id.clone(), AdvancedDraft::default()))
            .collect();
        let tools_mixer = cx.new(|_| AudioMixerPanel::new(mixer_channels, theme));
        let tools_transitions = cx.new(|_| {
            let fade = blend_transition_params(&BTreeMap::new(), TransitionKind::Fade);
            let item = TransitionItem {
                id: "transition-1".into(),
                name: "Transition 1".into(),
                kind: TransitionKind::Fade,
                params: fade,
            };
            TransitionsPanel::new(vec![item], theme)
        });
        tools_transitions.update(cx, |panel, cx| {
            panel.set_active("transition-1");
            panel.begin_edit();
            cx.notify();
        });
        let mixer_menu = cx.new(|_| AudioMixerContextMenu {
            theme,
            ..AudioMixerContextMenu::default()
        });
        let mixer_advanced = cx.new(|_| AudioMixerAdvancedModal::new(theme));
        let planner = cx.new(|_| GraphicPlannerPanel {
            sources: vec![
                PlannerSource {
                    id: "camera".into(),
                    name: "Camera".into(),
                    base_width: 1920,
                    base_height: 1080,
                    transform: PlannerTransform::default(),
                },
                PlannerSource {
                    id: "lower-third".into(),
                    name: "Lower Third".into(),
                    base_width: 640,
                    base_height: 480,
                    transform: PlannerTransform::default(),
                },
            ],
            selected_id: Some("lower-third".into()),
            canvas: "1920x1080".into(),
            open: false,
            theme,
        });

        let subscriptions = vec![
            cx.subscribe(&header, |this, _, event, cx| {
                this.route_event(event.clone(), cx);
            }),
            cx.subscribe(&scenes, |this, _, event, cx| {
                this.route_event(event.clone(), cx);
            }),
            cx.subscribe(&sources, |this, _, event, cx| {
                this.route_event(event.clone(), cx);
            }),
            cx.subscribe(&tools_mixer, |this, _, event, cx| {
                this.handle_mixer_event(event.clone(), cx);
            }),
            cx.subscribe(&tools_transitions, |this, _, event, cx| {
                this.handle_transition_event(event.clone(), cx);
            }),
            cx.subscribe(&mixer_menu, |this, _, event, cx| {
                this.handle_mixer_event(event.clone(), cx);
            }),
            cx.subscribe(&mixer_advanced, |this, _, event, cx| {
                this.handle_mixer_event(event.clone(), cx);
            }),
            cx.subscribe(&planner, |this, _, event, cx| {
                this.handle_planner_event(event.clone(), cx);
            }),
        ];

        let mut workspace = Self {
            header,
            scenes,
            sources,
            demo,
            tools_mixer,
            tools_transitions,
            mixer_menu,
            mixer_advanced,
            planner,
            settings_modal: None,
            advanced_drafts,
            aux_windows: AuxWindows::default(),
            state: WorkspaceState::default(),
            backend,
            theme,
            #[cfg(feature = "obs-backend")]
            preview_frame: None,
            _subscriptions: subscriptions,
        };

        if use_obs_backend {
            workspace.state.status = "Native libobs backend requested (--obs)".into();
        }

        // One initial snapshot refresh on startup so native state (including
        // real-data panels for the obs backend) lands without user input.
        workspace.refresh_snapshot(cx);
        #[cfg(feature = "obs-backend")]
        if use_obs_backend {
            workspace.spawn_preview_loop(cx);
        }
        workspace
    }

    /// Runs one snapshot request on the GPUI background executor and applies
    /// the result back on the foreground context. The backend service call
    /// blocks only the background thread, never the UI executor.
    fn refresh_snapshot(&mut self, cx: &mut Context<'_, Self>) {
        let backend = self.backend.clone();
        cx.spawn(async move |workspace, cx| {
            let snapshot = cx
                .background_executor()
                .spawn(async move { backend.snapshot() })
                .await;
            let _ = workspace.update(cx, |workspace, cx| match snapshot {
                Ok(snapshot) => workspace.apply_snapshot(snapshot, cx),
                Err(error) => workspace.handle_snapshot_error(error, cx),
            });
        })
        .detach();
    }

    /// Applies one backend snapshot to the workspace panels: header
    /// lifecycle flags, scene rows, source rows, mixer levels, and the
    /// status-line backend error state.
    pub fn apply_snapshot(&mut self, snapshot: BackendSnapshot, cx: &mut Context<'_, Self>) {
        self.scenes.update(cx, |panel, cx| {
            panel.scenes = snapshot_scene_items(&snapshot);
            panel.studio_mode = snapshot.studio_mode;
            cx.notify();
        });
        self.sources.update(cx, |panel, cx| {
            panel.sources = snapshot_source_items(&snapshot);
            panel.selected_id = snapshot.selected_source.clone();
            cx.notify();
        });
        self.tools_mixer.update(cx, |mixer, cx| {
            for (id, level) in &snapshot.audio_levels {
                mixer.set_level(id, *level as f32);
            }
            cx.notify();
        });
        self.header.update(cx, |header, cx| {
            header.state.obs_running = snapshot.obs_running;
            header.state.recording = snapshot.recording;
            header.state.streaming = snapshot.streaming;
            header.state.studio_mode = snapshot.studio_mode;
            header.state.realtime_refresh = snapshot.realtime_refresh;
            cx.notify();
        });
        self.state.apply_snapshot(&snapshot);
        cx.notify();
    }

    fn handle_snapshot_error(&mut self, error: BackendError, cx: &mut Context<'_, Self>) {
        self.state.status = format!("Backend error: {error}");
        cx.notify();
    }

    #[cfg(feature = "obs-backend")]
    /// Bounded preview loop: every 150 ms captures the live scene output on
    /// the backend worker (blocking `service.call` is backgrounded by the
    /// executor) and refreshes the newest PNG frame on the foreground.
    /// Terminates by failing `weak.update` once the Workspace entity drops.
    fn spawn_preview_loop(&mut self, cx: &mut Context<'_, Self>) {
        let backend = self.backend.clone();
        cx.spawn(async move |workspace, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(
                    crate::services::obs::PREVIEW_CAPTURE_INTERVAL_MS,
                ))
                .await;
            let frame = cx
                .background_executor()
                .spawn(async move { capture_preview_png(&backend, None) })
                .await;
            let frame = frame
                .ok()
                .and_then(|data_uri| crate::services::decode_png_data_uri(&data_uri))
                .map(|bytes| std::sync::Arc::new(GpuiImage::from_bytes(ImageFormat::Png, bytes)));
            let alive = workspace
                .update(cx, |workspace, cx| {
                    workspace.preview_frame = frame;
                    cx.notify();
                })
                .is_ok();
            if !alive {
                return;
            }
        })
        .detach();
    }

    fn dispatch(
        &mut self,
        commands: impl IntoIterator<Item = BackendCommand>,
        cx: &mut Context<'_, Self>,
    ) {
        for command in commands {
            self.state.last_command = Some(command.clone());
            if let Err(error) = self.backend.try_dispatch(command) {
                self.state.status = format!("Backend error: {error}");
            }
        }
        self.refresh_snapshot(cx);
    }

    fn handle_mixer_event(&mut self, event: MixerEvent, cx: &mut Context<'_, Self>) {
        match event {
            MixerEvent::ContextMenuRequested { id } => {
                self.mixer_menu.update(cx, |menu, cx| {
                    menu.open_menu(&id);
                    cx.notify();
                });
            }
            MixerEvent::OpenAdvanced { id } => {
                self.mixer_menu.update(cx, |menu, cx| {
                    menu.close();
                    cx.notify();
                });
                let name = self
                    .tools_mixer
                    .read(cx)
                    .channel(&id)
                    .map_or_else(|| id.clone(), |channel| channel.name.clone());
                let draft = self.advanced_drafts.get(&id).cloned().unwrap_or_default();
                self.mixer_advanced.update(cx, |modal, cx| {
                    modal.open(id.clone(), name, draft);
                    cx.notify();
                });
                self.state.active_modal = ActiveModal::AudioMixerAdvanced(id);
            }
            MixerEvent::AdvancedConfirmed { .. } => {
                self.mixer_advanced.update(cx, |modal, cx| {
                    modal.close();
                    cx.notify();
                });
                self.state.close_modal();
                self.state.status = "Advanced audio properties applied".into();
            }
            MixerEvent::AdvancedCancelled => {
                self.mixer_advanced.update(cx, |modal, cx| {
                    modal.close();
                    cx.notify();
                });
                self.state.close_modal();
            }
            other => {
                if let Some(command) = self.state.route_mixer(other) {
                    self.dispatch([command], cx);
                }
            }
        }
        cx.notify();
    }

    fn handle_transition_event(&mut self, event: TransitionEvent, cx: &mut Context<'_, Self>) {
        let commands = self.state.route_transition(event);
        self.dispatch(commands, cx);
        cx.notify();
    }

    fn handle_planner_event(&mut self, event: PlannerEvent, cx: &mut Context<'_, Self>) {
        if let PlannerEvent::Cancel = event {
            self.state.close_modal();
        }
        let commands = self.state.route_planner(event);
        self.dispatch(commands, cx);
        cx.notify();
    }

    fn route_event(&mut self, event: ComponentEvent, cx: &mut Context<'_, Self>) {
        self.update_header_flags(&event, cx);
        if let Some(command) = self.state.route(event) {
            if let Err(error) = self.backend.try_dispatch(command) {
                self.state.status = format!("Backend error: {error}");
            }
            self.refresh_snapshot(cx);
        }
        if let Some(kind) = self.state.requested_window.take() {
            self.open_auxiliary_window(kind, cx);
        }
        if self.state.active_modal == ActiveModal::Settings && self.settings_modal.is_none() {
            self.open_settings_modal(cx);
        }
        cx.notify();
    }

    /// Creates the settings modal entity seeded from the last known persisted
    /// settings (or framework defaults) and subscribes to its Save/Cancel
    /// events.
    fn open_settings_modal(&mut self, cx: &mut Context<'_, Self>) {
        let settings = self
            .state
            .settings
            .clone()
            .unwrap_or_else(revo_core::settings::default_settings);
        let entity = cx.new(|cx| SettingsModal::new(settings, cx));
        let subscription = cx.subscribe(&entity, |this, _, event, cx| match event {
            SettingsEvent::Saved(settings) => {
                this.state.settings = Some(settings.clone());
                if let Err(error) = this.backend.try_dispatch(BackendCommand::SaveSettings {
                    settings: settings.clone(),
                }) {
                    this.state.status = format!("Backend error: {error}");
                }
                this.close_settings_modal(cx);
                this.refresh_snapshot(cx);
            }
            SettingsEvent::Cancelled => this.close_settings_modal(cx),
        });
        self._subscriptions.push(subscription);
        self.settings_modal = Some(entity);
    }

    /// Drops the settings entity and closes the modal when it is active.
    fn close_settings_modal(&mut self, cx: &mut Context<'_, Self>) {
        self.settings_modal = None;
        if self.state.active_modal == ActiveModal::Settings {
            self.state.close_modal();
            cx.notify();
        }
    }

    /// Closes whichever modal is currently active, cleaning up the matching
    /// entity state.
    fn close_active_modal(&mut self, cx: &mut Context<'_, Self>) {
        match &self.state.active_modal {
            ActiveModal::Settings => self.close_settings_modal(cx),
            ActiveModal::AudioMixerAdvanced(_)
            | ActiveModal::GraphicPlanner
            | ActiveModal::None => {
                // Entity-hosted overlays keep their own lifecycle; the modal
                // state just reverts to none.
                self.state.close_modal();
                cx.notify();
            }
            ActiveModal::History
            | ActiveModal::AddScene
            | ActiveModal::AddSource
            | ActiveModal::SourceEditor(_)
            | ActiveModal::QuickAction { .. } => {
                self.state.close_modal();
                cx.notify();
            }
        }
    }

    /// Immediate local header feedback for lifecycle events; the following
    /// backend snapshot refresh reconciles this with authoritative state.
    fn update_header_flags(&self, event: &ComponentEvent, cx: &mut Context<'_, Self>) {
        self.header.update(cx, |header, cx| {
            match event {
                ComponentEvent::StartObs => header.state.obs_running = true,
                ComponentEvent::StopObs => header.state.obs_running = false,
                ComponentEvent::StartRecording => header.state.recording = true,
                ComponentEvent::StopRecording => header.state.recording = false,
                ComponentEvent::StartStreaming => header.state.streaming = true,
                ComponentEvent::StopStreaming => header.state.streaming = false,
                ComponentEvent::SetStudioMode(enabled) => header.state.studio_mode = *enabled,
                ComponentEvent::SetRealtimeRefresh(enabled) => {
                    header.state.realtime_refresh = *enabled
                }
                _ => return,
            }
            cx.notify();
        });
    }

    fn tools(&self, cx: &mut Context<'_, Self>) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .size_full()
            .bg(self.theme.sidebar)
            .border_1()
            .border_color(self.theme.border)
            .rounded_md()
            .text_color(self.theme.text)
            .child(div().text_color(self.theme.text_strong).child("Tools"))
            .child(self.tools_mixer.clone())
            .child(self.tools_transitions.clone())
            .child(
                div()
                    .id("open-graphic-planner")
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(self.theme.surface_active)
                    .child("Graphic planner")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.route_event(ComponentEvent::OpenGraphicPlanner, cx);
                    })),
            )
            .child(
                div()
                    .id("open-preview-window")
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(self.theme.surface_active)
                    .child("Preview window")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.route_event(ComponentEvent::OpenPreviewWindow, cx);
                    })),
            )
            .child(
                div()
                    .id("open-debug-console-window")
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(self.theme.surface_active)
                    .child("Debug console window")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.route_event(ComponentEvent::OpenDebugConsoleWindow, cx);
                    })),
            )
            .child(
                div()
                    .id("open-browser-interaction-window")
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(self.theme.surface_active)
                    .child("Browser interaction window")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.route_event(ComponentEvent::OpenBrowserInteractionWindow, cx);
                    })),
            )
            .child(
                div()
                    .text_color(self.theme.text_muted)
                    .child(self.state.status.clone()),
            )
    }

    /// The demo/preview surface. With the `obs-backend` feature the newest
    /// PNG preview frame from the real libobs scene output is rendered as a
    /// GPUI image; until the first frame lands (or for the mock backend) the
    /// demo placeholder is shown, including its "dims" status line.
    fn preview_surface(&self) -> gpui::Div {
        let base = div().flex_1().min_h(px(200.)).child(self.demo.clone());
        #[cfg(feature = "obs-backend")]
        {
            match &self.preview_frame {
                Some(frame) => base.child(
                    div().id("preview-frame").size_full().child(
                        gpui::img(std::sync::Arc::clone(frame))
                            .size_full()
                            .object_fit(gpui::ObjectFit::Contain),
                    ),
                ),
                // Documented placeholder: gpui 0.2.2 `img()` accepts
                // `Arc<Image>` bytes directly, so this surfaces the libobs
                // capture state instead of a blank surface while the stream
                // of frames warms up.
                None => base.child(
                    div()
                        .text_color(self.theme.text_muted)
                        .child("OBS preview: waiting for first frame (150ms capture loop)"),
                ),
            }
        }
        #[cfg(not(feature = "obs-backend"))]
        {
            base
        }
    }

    fn lower_panels(&self, cx: &mut Context<'_, Self>, mode: LayoutMode) -> gpui::Div {
        let [first, second, third] = mode.panel_order();
        match mode {
            LayoutMode::Wide => div()
                .flex()
                .flex_row()
                .gap_2()
                .child(div().flex_1().child(self.panel(cx, first)))
                .child(div().flex_1().child(self.panel(cx, second)))
                .child(div().flex_1().child(self.panel(cx, third))),
            LayoutMode::Medium => div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .child(div().flex_1().child(self.panel(cx, first)))
                        .child(div().flex_1().child(self.panel(cx, second))),
                )
                .child(self.panel(cx, third)),
            LayoutMode::Compact => div()
                .flex()
                .flex_col()
                .gap_2()
                .child(self.panel(cx, first))
                .child(self.panel(cx, second))
                .child(self.panel(cx, third)),
        }
    }

    fn panel(&self, cx: &mut Context<'_, Self>, panel: PanelKind) -> gpui::Div {
        match panel {
            PanelKind::Scenes => div().child(self.scenes.clone()),
            PanelKind::Sources => div().child(self.sources.clone()),
            PanelKind::Tools => div().child(self.tools(cx)),
        }
    }

    fn modal_content(&self) -> Option<(ModalConfig, Vec<SharedString>)> {
        match &self.state.active_modal {
            ActiveModal::None => None,
            ActiveModal::Settings => None,
            ActiveModal::History => Some((
                ModalConfig::standard("Action History", ModalSize::Medium),
                vec!["Undo and redo history will be supplied by the backend service.".into()],
            )),
            ActiveModal::AddScene => Some((
                ModalConfig::standard("Add Scene", ModalSize::Small),
                vec!["Scene name".into()],
            )),
            ActiveModal::AddSource => Some((
                ModalConfig::standard("Add Source", ModalSize::Large),
                vec!["Choose a native OBS source type".into()],
            )),
            ActiveModal::SourceEditor(id) => Some((
                ModalConfig::standard("Edit Source", ModalSize::Workspace),
                vec![SharedString::from(format!("Source: {id}"))],
            )),
            ActiveModal::AudioMixerAdvanced(_) | ActiveModal::GraphicPlanner => None,
            ActiveModal::QuickAction { source_id, action } => Some((
                ModalConfig::quick(format!("Quick {action:?}")),
                vec![SharedString::from(format!("Source: {source_id}"))],
            )),
        }
    }
}

/// Decoding of `data:image/png;base64,...` frames lives in
/// `services::obs::decode_png_data_uri` so the auxiliary preview and
/// interact windows share it with the workspace loop.

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let mode = LayoutMode::for_width(f32::from(window.viewport_size().width));
        let mut root = div()
            .relative()
            .flex()
            .flex_col()
            .gap_2()
            .size_full()
            .p_2()
            .bg(self.theme.background)
            .text_color(self.theme.text)
            .child(self.header.clone())
            .child(self.preview_surface())
            .child(self.lower_panels(cx, mode));

        if let Some((config, lines)) = self.modal_content() {
            // Backdrop closes on click; the card is a sibling layer above it,
            // so clicks inside the card never reach the backdrop.
            root = root
                .child(
                    div()
                        .id("workspace-modal-backdrop")
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom_0()
                        .left_0()
                        .bg(self.theme.overlay)
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.close_active_modal(cx);
                            }),
                        ),
                )
                .child(
                    div()
                        .id("workspace-modal-layer")
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom_0()
                        .left_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(modal_card(
                            &config,
                            self.theme,
                            div().flex().flex_col().gap_2().children(lines),
                            None,
                        )),
                );
            if config.show_close {
                root = root.child(
                    div()
                        .id("workspace-modal-close")
                        .absolute()
                        .top_4()
                        .right_4()
                        .cursor_pointer()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(self.theme.surface_active)
                        .text_color(self.theme.text)
                        .child("Close")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.close_active_modal(cx);
                        })),
                );
            }
        }

        if self.mixer_menu.read(cx).open {
            root = root.child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .left_0()
                    .child(self.mixer_menu.clone()),
            );
        }

        match &self.state.active_modal {
            ActiveModal::AudioMixerAdvanced(_) => {
                root = root.child(
                    div()
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom_0()
                        .left_0()
                        .bg(self.theme.overlay)
                        .child(self.mixer_advanced.clone()),
                );
            }
            ActiveModal::GraphicPlanner => {
                root = root.child(
                    div()
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom_0()
                        .left_0()
                        .bg(self.theme.overlay)
                        .child(self.planner.clone()),
                );
            }
            ActiveModal::Settings => {
                if let Some(entity) = self.settings_modal.clone() {
                    root = root.child(
                        div()
                            .absolute()
                            .top_0()
                            .right_0()
                            .bottom_0()
                            .left_0()
                            .bg(self.theme.overlay)
                            .child(entity),
                    );
                }
            }
            _ => {}
        }

        root
    }
}

pub fn run_native_app() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1280.), px(720.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(1080.), px(600.))),
                focus: true,
                ..WindowOptions::default()
            },
            |_, cx| cx.new(Workspace::new),
        )
        .expect("failed to open RevoStream native window");
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::MockBackendRuntime;
    use revo_core::service::{BackendService, PlacedSource, SourceSnapshotItem};

    #[test]
    fn layout_breakpoints_are_classified_at_contract_boundaries() {
        assert_eq!(LayoutMode::for_width(1199.9), LayoutMode::Medium);
        assert_eq!(LayoutMode::for_width(1200.0), LayoutMode::Wide);
        assert_eq!(LayoutMode::for_width(799.9), LayoutMode::Compact);
        assert_eq!(LayoutMode::for_width(800.0), LayoutMode::Medium);
    }

    #[test]
    fn lower_panel_order_matches_each_responsive_contract() {
        assert_eq!(
            LayoutMode::Wide.panel_order(),
            [PanelKind::Scenes, PanelKind::Sources, PanelKind::Tools]
        );
        assert_eq!(
            LayoutMode::Medium.panel_order(),
            [PanelKind::Scenes, PanelKind::Tools, PanelKind::Sources]
        );
        assert_eq!(
            LayoutMode::Compact.panel_order(),
            [PanelKind::Scenes, PanelKind::Tools, PanelKind::Sources]
        );
    }

    #[test]
    fn modal_events_open_typed_workspace_state_and_close_cleanly() {
        let mut state = WorkspaceState::default();
        state.route(ComponentEvent::EditSource("camera".into()));
        assert_eq!(
            state.active_modal,
            ActiveModal::SourceEditor("camera".into())
        );

        state.route(ComponentEvent::QuickSourceAction {
            id: "title".into(),
            action: QuickSourceAction::EditText,
        });
        assert_eq!(
            state.active_modal,
            ActiveModal::QuickAction {
                source_id: "title".into(),
                action: QuickSourceAction::EditText,
            }
        );
        state.close_modal();
        assert_eq!(state.active_modal, ActiveModal::None);
    }

    #[test]
    fn lifecycle_events_cross_only_the_typed_backend_boundary() {
        let mut state = WorkspaceState::default();
        assert_eq!(
            state.route(ComponentEvent::StartStreaming),
            Some(BackendCommand::StartStreaming)
        );
        assert_eq!(state.last_command, Some(BackendCommand::StartStreaming));
        assert_eq!(state.active_modal, ActiveModal::None);

        assert_eq!(
            state.route(ComponentEvent::SetStudioMode(true)),
            Some(BackendCommand::SetStudioMode(true))
        );
        assert_eq!(
            state.last_command,
            Some(BackendCommand::SetStudioMode(true))
        );
    }

    #[test]
    fn auxiliary_window_events_set_requested_window_without_backend_commands() {
        let mut state = WorkspaceState::default();
        state.route(ComponentEvent::OpenPreviewWindow);
        assert_eq!(
            state.requested_window.take(),
            Some(AuxiliaryWindowKind::Preview)
        );
        assert_eq!(state.requested_window.take(), None);
        assert_eq!(state.last_command, None);

        state.route(ComponentEvent::OpenDebugConsoleWindow);
        assert_eq!(
            state.requested_window.take(),
            Some(AuxiliaryWindowKind::DebugConsole)
        );

        state.route(ComponentEvent::OpenBrowserInteractionWindow);
        assert_eq!(
            state.requested_window.take(),
            Some(AuxiliaryWindowKind::BrowserInteraction)
        );
        assert_eq!(state.active_modal, ActiveModal::None);
    }

    #[test]
    fn mock_service_executes_lifecycle_scene_and_source_commands() {
        let service = BackendService::new(4, MockBackendRuntime::default).unwrap();
        service.execute(BackendCommand::StartObs).unwrap();
        service
            .execute(BackendCommand::SelectScene("Be Right Back".into()))
            .unwrap();
        service
            .execute(BackendCommand::SelectSource("camera".into()))
            .unwrap();
        service
            .execute(BackendCommand::ToggleSourceVisibility("camera".into()))
            .unwrap();

        let snapshot = service.snapshot().unwrap();
        assert!(snapshot.obs_running);
        assert_eq!(snapshot.selected_scene.as_deref(), Some("Be Right Back"));
        assert_eq!(snapshot.selected_source.as_deref(), Some("camera"));
        assert!(!snapshot.visible_sources.contains(&"camera".into()));
        service.shutdown().unwrap();
    }

    #[test]
    fn mixer_events_route_into_typed_backend_commands() {
        let mut state = WorkspaceState::default();
        assert_eq!(
            state.route_mixer(MixerEvent::VolumeCommitted {
                id: "camera".into(),
                percent: 150.0,
            }),
            Some(BackendCommand::SetSourceVolume {
                id: "camera".into(),
                percent: 150.0,
            })
        );
        assert_eq!(
            state.last_command,
            Some(BackendCommand::SetSourceVolume {
                id: "camera".into(),
                percent: 150.0,
            })
        );

        assert_eq!(
            state.route_mixer(MixerEvent::MuteToggled {
                id: "camera".into(),
                muted: true,
            }),
            Some(BackendCommand::SetSourceMuted {
                id: "camera".into(),
                muted: true,
            })
        );
        assert_eq!(
            state.route_mixer(MixerEvent::LockToggled {
                id: "camera".into(),
                locked: true,
            }),
            Some(BackendCommand::SetSourceVolumeLocked {
                id: "camera".into(),
                locked: true,
            })
        );
        assert_eq!(state.route_mixer(MixerEvent::AdvancedCancelled), None);
        assert_eq!(
            state.route_mixer(MixerEvent::OpenFilters {
                id: "camera".into()
            }),
            None
        );
    }

    #[test]
    fn transition_apply_routes_kind_then_clamped_duration() {
        let mut state = WorkspaceState::default();
        let commands = state.route_transition(TransitionEvent::Apply {
            kind: "fade".into(),
            duration_ms: 5,
        });
        assert_eq!(
            commands,
            vec![
                BackendCommand::SetTransitionKind("fade".into()),
                BackendCommand::SetTransitionDuration(5),
            ]
        );
        assert_eq!(
            state.last_command,
            Some(BackendCommand::SetTransitionDuration(5))
        );
        assert!(state.route_transition(TransitionEvent::Discard).is_empty());
    }

    #[test]
    fn planner_apply_routes_one_transform_command_per_source() {
        use crate::components::modules::graphic_planner::PlannerApplyItem;

        let mut state = WorkspaceState::default();
        let commands = state.route_planner(PlannerEvent::Apply {
            items: vec![PlannerApplyItem {
                id: "camera".into(),
                x: 640,
                y: 300,
                width: 640,
                height: 480,
                rotation: 0,
            }],
        });
        assert_eq!(
            commands,
            vec![BackendCommand::SetSourceTransform {
                id: "camera".into(),
                x: 640,
                y: 300,
                width: 640,
                height: 480,
                rotation: 0,
            }]
        );
        assert_eq!(state.last_command, commands.last().cloned());
        assert!(state
            .route_planner(PlannerEvent::Select("camera".into()))
            .is_empty());
        assert_eq!(state.status, "Planner selected camera");
    }

    #[test]
    fn advanced_and_planner_travel_through_active_modal_state() {
        let mut state = WorkspaceState::default();
        state.route(ComponentEvent::OpenGraphicPlanner);
        assert_eq!(state.active_modal, ActiveModal::GraphicPlanner);
        state.close_modal();

        state.route_mixer(MixerEvent::OpenAdvanced {
            id: "camera".into(),
        });
        assert_eq!(state.status, "Advanced audio properties: camera");
        state.active_modal = ActiveModal::AudioMixerAdvanced("camera".into());
        state.close_modal();
        assert_eq!(state.active_modal, ActiveModal::None);
    }

    #[test]
    fn snapshot_panel_items_preserve_backend_order_and_flags() {
        let snapshot = BackendSnapshot {
            selected_scene: Some("Main Cameras".into()),
            scenes: vec!["Starting Soon".into(), "Main Cameras".into()],
            locked_scenes: vec!["Main Cameras".into()],
            locked_sources: vec!["browser-chat".into()],
            source_items: vec![
                SourceSnapshotItem {
                    id: "camera".into(),
                    name: "Camera".into(),
                    source_type: "v4l2_input".into(),
                    visible: true,
                    audio_capable: true,
                },
                SourceSnapshotItem {
                    id: "lower-third".into(),
                    name: "Lower Third".into(),
                    source_type: "text_ft2_source_v2".into(),
                    visible: false,
                    audio_capable: false,
                },
            ],
            source_transforms: vec![PlacedSource {
                id: "camera".into(),
                x: 1,
                y: 2,
                width: 3,
                height: 4,
                rotation: 0,
            }],
            ..BackendSnapshot::default()
        };

        let scenes = snapshot_scene_items(&snapshot);
        assert_eq!(
            scenes,
            vec![
                SceneItem {
                    name: "Starting Soon".into(),
                    active: false,
                    locked: false,
                },
                SceneItem {
                    name: "Main Cameras".into(),
                    active: true,
                    locked: true,
                },
            ]
        );

        let sources = snapshot_source_items(&snapshot);
        assert_eq!(
            sources,
            vec![
                SourceItem {
                    id: "camera".into(),
                    name: "Camera".into(),
                    source_type: "v4l2_input".into(),
                    visible: true,
                    locked: false,
                },
                SourceItem {
                    id: "lower-third".into(),
                    name: "Lower Third".into(),
                    source_type: "text_ft2_source_v2".into(),
                    visible: false,
                    locked: false,
                },
            ]
        );
        assert_eq!(
            snapshot.source_transforms,
            vec![PlacedSource {
                id: "camera".into(),
                x: 1,
                y: 2,
                width: 3,
                height: 4,
                rotation: 0,
            }]
        );
    }

    #[test]
    fn workspace_state_snapshot_surfaces_backend_errors_and_recovery() {
        let mut state = WorkspaceState::default();
        state.route(ComponentEvent::SelectScene("Main Cameras".into()));
        let errored = String::from("obs_startup failed");
        state.apply_snapshot(&BackendSnapshot {
            last_error: Some(errored.clone()),
            ..BackendSnapshot::default()
        });
        assert_eq!(state.status, "Backend error: obs_startup failed");

        state.apply_snapshot(&BackendSnapshot::default());
        assert_eq!(state.status, "Backend state synced");
    }

    #[test]
    fn mock_service_snapshot_round_trip_feeds_panel_builders() {
        let service = BackendService::new(4, MockBackendRuntime::default).unwrap();
        let snapshot = service.snapshot().unwrap();
        service.shutdown().unwrap();

        let scenes = snapshot_scene_items(&snapshot);
        assert!(scenes.iter().any(|scene| scene.name == "Main Camera"));
        // The mock runtime models locking via the flat id list only; source
        // items are a real-backend snapshot field and stay empty here.
        assert!(snapshot.source_items.is_empty());
        assert!(snapshot.locked_sources.contains(&"browser-chat".into()));
        assert!(snapshot_scene_items(&snapshot).iter().all(|scene| {
            (snapshot.selected_scene.as_deref() == Some(scene.name.as_str())) == scene.active
        }));
    }
}
