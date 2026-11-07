use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use gpui::{
    div, prelude::*, px, rgb, size, svg, App, Application, AssetSource, Bounds, Context, Entity,
    IntoElement, MouseButton, MouseDownEvent, Pixels, Render, SharedString, Subscription,
    TitlebarOptions, Window, WindowBounds, WindowHandle, WindowOptions,
};
use revo_core::models::AppSettings;
use revo_core::service::{BackendCommand, BackendError, BackendSnapshot, MoveDirection};

#[cfg(feature = "obs-backend")]
use gpui::{Image as GpuiImage, ImageFormat};

#[cfg(feature = "obs-backend")]
use crate::services::capture_preview_png;
use crate::services::AnyBackend;

use crate::{
    actions::{
        AuxiliaryWindowKind, ComponentEvent, ContextMenuAction, ContextMenuTarget,
        QuickSourceAction,
    },
    components::{
        context_menu::{ContextMenuEvent, WorkspaceContextMenu},
        home::{
            DemoMode, HeaderBar, HeaderBarState, SceneItem, ScenesPanel, SourceItem, SourcesPanel,
        },
        modal::{
            AddSceneEvent, AddSceneModal, AddSourceEvent, AddSourceModal, PluginsEvent,
            PluginsModal, SettingsEvent, SettingsModal, SourcePropertiesEvent,
            SourcePropertiesModal,
        },
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
        quick_modal::{QuickActionEvent, QuickActionModal},
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
    Plugins,
    AudioMixer,
    Transitions,
    Templates,
    AddScene,
    AddSource,
    SourceProperties(String),
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
    /// Workspace-side undo/redo log fed by the routed component events.
    pub history: crate::history::HistoryLog,
    /// One-shot auxiliary-window request routed from the Tools panel;
    /// consumed (`take`) by the Workspace to open/focus the window.
    pub requested_window: Option<AuxiliaryWindowKind>,
    /// One-shot right-click context-menu request routed from panel rows or
    /// the preview surface; consumed (`take`) by the Workspace to open the
    /// shared menu at the cursor position.
    pub requested_menu: Option<(ContextMenuTarget, (f32, f32))>,
    /// Persisted settings as most recently reported by the backend snapshot;
    /// seeds the settings draft when the modal opens.
    pub settings: Option<AppSettings>,
    /// Discovered libobs modules from the backend snapshot; seeds the
    /// plugins modal.
    pub plugins: Vec<revo_core::models::PluginInfo>,
    /// Saved plugin profile names from the backend snapshot.
    pub plugin_profiles: Vec<String>,
    /// Saved plugin profiles with their module enablement.
    pub plugin_profile_states: Vec<revo_core::models::PluginProfileState>,
    /// Name of the currently applied plugin profile.
    pub active_plugin_profile: Option<String>,
    /// Enabled module names of the currently applied profile.
    pub enabled_plugin_modules: Vec<String>,
    /// Source types the engine can create; seeds the add-source modal.
    pub available_source_types: Vec<revo_core::models::SourceTypeItem>,
    /// Placed source transforms from the last snapshot; seeds the source
    /// properties modal and mirrors the Svelte properties window.
    pub source_transforms: Vec<revo_core::service::PlacedSource>,
    /// False until the first backend snapshot has been applied; drives the
    /// boot splash instead of the workspace shell.
    pub booted: bool,
    /// Rotation angle of the boot-splash spinner (degrees); animated by a
    /// foreground task while [`Self::booted`] is false.
    pub splash_spin: f32,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            active_modal: ActiveModal::None,
            last_command: None,
            status: "Starting core...".into(),
            history: crate::history::HistoryLog::default(),
            requested_window: None,
            requested_menu: None,
            settings: None,
            plugins: Vec::new(),
            plugin_profiles: Vec::new(),
            plugin_profile_states: Vec::new(),
            active_plugin_profile: None,
            enabled_plugin_modules: Vec::new(),
            available_source_types: Vec::new(),
            source_transforms: Vec::new(),
            booted: false,
            splash_spin: 0.0,
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
            audio_capable: source.audio_capable,
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
            ComponentEvent::OpenPlugins => {
                self.active_modal = ActiveModal::Plugins;
                return None;
            }
            ComponentEvent::OpenMixer => {
                self.active_modal = ActiveModal::AudioMixer;
                self.status = "Audio Mixer opened".into();
                return None;
            }
            ComponentEvent::OpenTransitions => {
                self.active_modal = ActiveModal::Transitions;
                self.status = "Transitions opened".into();
                return None;
            }
            ComponentEvent::OpenTemplates => {
                self.status = "Templates: ready-made scene layouts are on the way".into();
                return None;
            }
            ComponentEvent::OpenGraphicPlanner => {
                self.active_modal = ActiveModal::GraphicPlanner;
                return None;
            }
            ComponentEvent::OpenPreviewWindow => {
                self.requested_window = Some(AuxiliaryWindowKind::Preview { source_id: None });
                self.status = "Preview window requested".into();
                return None;
            }
            ComponentEvent::OpenPreviewInWindow { target_id } => {
                self.requested_window = Some(AuxiliaryWindowKind::Preview {
                    source_id: target_id.clone(),
                });
                self.status = match &target_id {
                    Some(id) => format!("Preview window requested for {id}"),
                    None => "Preview window requested".into(),
                };
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
                // Browser sources skip the modal and go straight to the
                // interaction window (Svelte parity: the Interact button).
                if action == QuickSourceAction::Interact {
                    self.requested_window = Some(AuxiliaryWindowKind::BrowserInteraction);
                    self.status = "Opening browser interaction window".into();
                    return None;
                }
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
            ComponentEvent::SetSceneLock { name, locked } => {
                BackendCommand::SetSceneLock { name, locked }
            }
            ComponentEvent::RemoveScene(name) => BackendCommand::RemoveScene(name),
            ComponentEvent::SelectSource(id) => BackendCommand::SelectSource(id),
            ComponentEvent::ToggleSourceVisibility(id) => {
                BackendCommand::ToggleSourceVisibility(id)
            }
            ComponentEvent::ToggleSourceLock(id) => BackendCommand::ToggleSourceLock(id),
            ComponentEvent::MoveSource { id, direction } => {
                BackendCommand::MoveSource { id, direction }
            }
            ComponentEvent::RemoveSource(id) => BackendCommand::RemoveSource(id),
            ComponentEvent::OpenContextMenu { target, position } => {
                self.requested_menu = Some((target, position));
                return None;
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
        self.plugins = snapshot.plugins.clone();
        self.plugin_profiles = snapshot.plugin_profiles.clone();
        self.plugin_profile_states = snapshot.plugin_profile_states.clone();
        self.active_plugin_profile = snapshot.active_plugin_profile.clone();
        self.enabled_plugin_modules = snapshot.enabled_plugin_modules.clone();
        self.available_source_types = snapshot.available_source_types.clone();
        self.source_transforms = snapshot.source_transforms.clone();
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
            TransitionEvent::Added { name } => {
                self.status = format!("Transition '{name}' added to the draft list");
                Vec::new()
            }
            TransitionEvent::Removed => {
                self.status = "Transition removed from the draft list".into();
                Vec::new()
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

/// Measured container bounds shared with the per-frame bounds probes
/// (`gpui::canvas`); sized once per frame so the capture frames can keep a
/// scene-shaped aspect ratio (gpui 0.2.2 has no CSS aspect-ratio).
#[cfg(feature = "obs-backend")]
type MeasuredContainer = std::rc::Rc<std::cell::Cell<Option<gpui::Size<gpui::Pixels>>>>;

/// One measured container per preview surface: the single program band and
/// the two studio columns.
#[cfg(feature = "obs-backend")]
#[derive(Clone)]
struct PreviewContainerSizes {
    program: MeasuredContainer,
    studio_preview: MeasuredContainer,
    studio_program: MeasuredContainer,
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
    pub context_menu: Entity<WorkspaceContextMenu>,
    pub settings_modal: Option<Entity<SettingsModal>>,
    pub plugins_modal: Option<Entity<PluginsModal>>,
    pub add_scene_modal: Option<Entity<AddSceneModal>>,
    pub add_source_modal: Option<Entity<AddSourceModal>>,
    pub quick_action_modal: Option<Entity<QuickActionModal>>,
    pub source_properties_modal: Option<Entity<SourcePropertiesModal>>,
    /// Name of a just-created source; the next snapshot resolves it to an
    /// id and opens its properties modal (Svelte-style add → properties).
    pending_properties_source: Option<String>,
    /// History entry waiting for a snapshot to resolve the created source's
    /// id (undo needs the backend id for `RemoveSource`).
    pending_history_entry: Option<crate::history::HistoryEntry>,
    pub advanced_drafts: HashMap<String, AdvancedDraft>,
    pub aux_windows: AuxWindows,
    pub state: WorkspaceState,
    backend: AnyBackend,
    theme: RevoTheme,
    #[cfg(feature = "obs-backend")]
    preview_frame: Option<std::sync::Arc<GpuiImage>>,
    /// Studio-mode Preview column frame (the preview scene, not program).
    #[cfg(feature = "obs-backend")]
    studio_preview_frame: Option<std::sync::Arc<GpuiImage>>,
    /// Measured preview container bounds for the aspect-correct frames.
    #[cfg(feature = "obs-backend")]
    preview_container_sizes: PreviewContainerSizes,
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
            AuxiliaryWindowKind::Preview { source_id } => {
                let handle = self.aux_windows.preview.take();
                match handle {
                    Some(handle) if Self::focus_auxiliary(&handle, cx) => {
                        // Retarget an already open preview window when the
                        // request carries a scene/source id. The window's
                        // own capture loop redraws each frame, so no notify
                        // is needed here.
                        if let Some(id) = &source_id {
                            handle
                                .update(cx, |window, _, _| {
                                    window.source_id = Some(id.clone());
                                })
                                .ok();
                        }
                        self.aux_windows.preview = Some(handle);
                        self.state.status = match &source_id {
                            Some(id) => format!("Preview window retargeted to {id}"),
                            None => "Preview window focused".into(),
                        };
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
                                cx.new(|cx| {
                                    PreviewWindow::new(backend.clone(), source_id.clone(), cx)
                                })
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
                AnyBackend::obs()
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
            preview_scene: None,
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
                    audio_capable: true,
                    locked: false,
                },
                SourceItem {
                    id: "lower-third".into(),
                    name: "Lower Third".into(),
                    source_type: "text_ft2_source_v2".into(),
                    visible: true,
                    audio_capable: false,
                    locked: false,
                },
                SourceItem {
                    id: "browser-chat".into(),
                    name: "Chat Overlay".into(),
                    source_type: "browser_source".into(),
                    visible: true,
                    audio_capable: false,
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
        let context_menu = cx.new(|_| WorkspaceContextMenu {
            theme,
            ..WorkspaceContextMenu::default()
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
            cx.subscribe(&context_menu, |this, _, event, cx| {
                this.handle_context_menu_event(event.clone(), cx);
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
            context_menu,
            settings_modal: None,
            plugins_modal: None,
            add_scene_modal: None,
            add_source_modal: None,
            quick_action_modal: None,
            source_properties_modal: None,
            pending_properties_source: None,
            pending_history_entry: None,
            advanced_drafts,
            aux_windows: AuxWindows::default(),
            state: WorkspaceState::default(),
            backend,
            theme,
            #[cfg(feature = "obs-backend")]
            preview_frame: None,
            #[cfg(feature = "obs-backend")]
            studio_preview_frame: None,
            #[cfg(feature = "obs-backend")]
            preview_container_sizes: PreviewContainerSizes {
                program: std::rc::Rc::new(std::cell::Cell::new(None)),
                studio_preview: std::rc::Rc::new(std::cell::Cell::new(None)),
                studio_program: std::rc::Rc::new(std::cell::Cell::new(None)),
            },
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
            // `--obs` means "run against the real libobs engine": start it at
            // boot so the preview loop and the real scene data come up
            // without a manual Start click. The worker queue is FIFO, so the
            // follow-up snapshot executes after StartObs returns and carries
            // the initialized state (or its startup error) to the panels.
            workspace.dispatch([BackendCommand::StartObs], cx);
        }
        workspace.spawn_splash_spinner(cx);
        #[cfg(feature = "obs-backend")]
        if use_obs_backend {
            workspace.spawn_preview_loop(cx);
        }
        workspace
    }

    /// Animates the boot-splash spinner while the shell is not booted; the
    /// loop exits as soon as the first snapshot marks the state booted.
    fn spawn_splash_spinner(&mut self, cx: &mut Context<'_, Self>) {
        cx.spawn(async move |workspace, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(60))
                .await;
            let alive = workspace.update(cx, |workspace, cx| {
                if workspace.state.booted {
                    return false;
                }
                workspace.state.splash_spin = (workspace.state.splash_spin + 15.0) % 360.0;
                cx.notify();
                true
            });
            if !alive.unwrap_or(false) {
                return;
            }
        })
        .detach();
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
        if !self.state.booted {
            self.state.booted = true;
        }
        self.tools_mixer.update(cx, |mixer, cx| {
            for (id, level) in &snapshot.audio_levels {
                mixer.set_level(id, *level as f32);
            }
            cx.notify();
        });
        self.header.update(cx, |header, cx| {
            header.state.recording = snapshot.recording;
            header.state.streaming = snapshot.streaming;
            header.state.studio_mode = snapshot.studio_mode;
            cx.notify();
        });
        self.state.apply_snapshot(&snapshot);
        self.resolve_pending_properties(&snapshot, cx);
        cx.notify();
    }

    /// Builds the undo/redo entry for one routed component event from the
    /// live snapshot (Svelte parity: the per-action `recordHistory`
    /// closures). Source creation is deferred — its backend id only exists
    /// after the next snapshot, so `apply_snapshot` finalizes it.
    fn history_entry_for(
        &self,
        event: &ComponentEvent,
        cx: &mut Context<'_, Self>,
    ) -> Option<crate::history::HistoryEntry> {
        use crate::history::HistoryEntry;
        use revo_core::service::MoveDirection;
        let entry = |label: String, undo: Vec<BackendCommand>, redo: Vec<BackendCommand>| {
            Some(HistoryEntry { label, undo, redo })
        };
        match event {
            ComponentEvent::RemoveScene(name) => entry(
                format!("Remove scene '{name}'"),
                // The scene re-adds at the end of the list; exact index
                // restoration needs a backend reorder command on undo.
                vec![BackendCommand::AddScene(name.clone())],
                vec![BackendCommand::RemoveScene(name.clone())],
            ),
            ComponentEvent::RenameScene { from, to } => entry(
                format!("Rename scene '{from}' → '{to}'"),
                vec![BackendCommand::RenameScene {
                    from: to.clone(),
                    to: from.clone(),
                }],
                vec![BackendCommand::RenameScene {
                    from: from.clone(),
                    to: to.clone(),
                }],
            ),
            ComponentEvent::ReorderScene { name, to_index } => {
                let current_index = self
                    .scenes
                    .read(cx)
                    .scenes
                    .iter()
                    .position(|scene| &scene.name == name)?;
                entry(
                    format!("Move scene '{name}'"),
                    vec![BackendCommand::ReorderScene {
                        name: name.clone(),
                        to_index: current_index,
                    }],
                    vec![BackendCommand::ReorderScene {
                        name: name.clone(),
                        to_index: *to_index,
                    }],
                )
            }
            ComponentEvent::SetSceneLock { name, locked } => entry(
                format!("Scene lock '{}'", if *locked { "on" } else { "off" }),
                vec![BackendCommand::SetSceneLock {
                    name: name.clone(),
                    locked: !locked,
                }],
                vec![BackendCommand::SetSceneLock {
                    name: name.clone(),
                    locked: *locked,
                }],
            ),
            ComponentEvent::RemoveSource(id) => {
                let item = self
                    .sources
                    .read(cx)
                    .sources
                    .iter()
                    .find(|source| &source.id == id)
                    .cloned()?;
                entry(
                    format!("Remove source '{}'", item.name),
                    vec![BackendCommand::AddSource {
                        name: item.name.clone(),
                        source_type: item.source_type.clone(),
                    }],
                    vec![BackendCommand::RemoveSource(id.clone())],
                )
            }
            ComponentEvent::ToggleSourceVisibility(id) => entry(
                format!("Toggle visibility of '{id}'"),
                vec![BackendCommand::ToggleSourceVisibility(id.clone())],
                vec![BackendCommand::ToggleSourceVisibility(id.clone())],
            ),
            ComponentEvent::ToggleSourceLock(id) => entry(
                format!("Toggle lock of '{id}'"),
                vec![BackendCommand::ToggleSourceLock(id.clone())],
                vec![BackendCommand::ToggleSourceLock(id.clone())],
            ),
            ComponentEvent::MoveSource { id, direction } => {
                let opposite = match direction {
                    MoveDirection::Up => MoveDirection::Down,
                    MoveDirection::Down => MoveDirection::Up,
                    MoveDirection::Top => MoveDirection::Bottom,
                    MoveDirection::Bottom => MoveDirection::Top,
                };
                entry(
                    format!("Move source '{id}'"),
                    vec![BackendCommand::MoveSource {
                        id: id.clone(),
                        direction: opposite,
                    }],
                    vec![BackendCommand::MoveSource {
                        id: id.clone(),
                        direction: *direction,
                    }],
                )
            }
            _ => None,
        }
    }

    /// Resolves a pending just-created source name to its backend id from
    /// the snapshot and opens its properties modal (Svelte-style add →
    /// properties flow).
    fn resolve_pending_properties(
        &mut self,
        snapshot: &BackendSnapshot,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(pending) = self.pending_properties_source.take() else {
            return;
        };
        let match_id = snapshot
            .source_items
            .iter()
            .find(|source| source.name == pending)
            .map(|source| source.id.clone());
        // The same resolution finalizes the deferred undo entry.
        if let Some(mut entry) = self.pending_history_entry.take() {
            if entry.undo.is_empty() {
                if let Some(id) = &match_id {
                    entry.undo = vec![BackendCommand::RemoveSource(id.clone())];
                    self.state.history.record(entry);
                }
            }
        }
        match match_id {
            Some(id) => {
                // The snapshot has already been applied to the panels, so
                // the properties modal seeds with fresh data.
                self.open_source_properties_modal(&id, false, cx);
            }
            None => {
                self.state.status = format!("Source '{pending}' created (properties pending)");
            }
        }
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
                    crate::services::obs::UI_PREVIEW_INTERVAL_MS,
                ))
                .await;
            // Studio mode adds a second capture: the preview scene alongside
            // the program composite.
            let (backend, preview_scene) = match workspace.update(cx, |workspace, cx| {
                let (studio, scene) = {
                    let panel = workspace.scenes.read(cx);
                    (panel.studio_mode, panel.preview_scene.clone())
                };
                (workspace.backend.clone(), if studio { scene } else { None })
            }) {
                Ok(value) => value,
                Err(_) => return,
            };
            let backend_for_program = backend.clone();
            let program = cx
                .background_executor()
                .spawn(async move { capture_preview_png(&backend_for_program, None) })
                .await;
            let program = program
                .ok()
                .and_then(|data_uri| crate::services::decode_png_data_uri(&data_uri))
                .map(|bytes| std::sync::Arc::new(GpuiImage::from_bytes(ImageFormat::Png, bytes)));
            let studio_preview = match &preview_scene {
                Some(scene) => {
                    let scene = scene.clone();
                    let backend = backend.clone();
                    let frame = cx
                        .background_executor()
                        .spawn(async move { capture_preview_png(&backend, Some(&scene)) })
                        .await;
                    frame
                        .ok()
                        .and_then(|data_uri| crate::services::decode_png_data_uri(&data_uri))
                        .map(|bytes| {
                            std::sync::Arc::new(GpuiImage::from_bytes(ImageFormat::Png, bytes))
                        })
                }
                None => None,
            };
            let alive = workspace
                .update(cx, |workspace, cx| {
                    workspace.preview_frame = program;
                    workspace.studio_preview_frame = studio_preview;
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
            }
            MixerEvent::AdvancedConfirmed { .. } => {
                self.mixer_advanced.update(cx, |modal, cx| {
                    modal.close();
                    cx.notify();
                });
                self.state.status = "Advanced audio properties applied".into();
            }
            MixerEvent::AdvancedCancelled => {
                self.mixer_advanced.update(cx, |modal, cx| {
                    modal.close();
                    cx.notify();
                });
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
        // Undo/redo never reach the backend stubs; they replay the
        // workspace-side history log instead (Svelte parity).
        match &event {
            ComponentEvent::Undo => {
                if let Some((label, commands)) = self.state.history.undo() {
                    self.state.status = format!("Undo: {label}");
                    self.dispatch(commands, cx);
                } else {
                    self.state.status = "Nothing to undo".into();
                    cx.notify();
                }
                return;
            }
            ComponentEvent::Redo => {
                if let Some((label, commands)) = self.state.history.redo() {
                    self.state.status = format!("Redo: {label}");
                    self.dispatch(commands, cx);
                } else {
                    self.state.status = "Nothing to redo".into();
                    cx.notify();
                }
                return;
            }
            _ => {}
        }
        // The history entry is computed from the pre-dispatch snapshot, so
        // the inverse commands see the state the change replaces.
        let history_entry = self.history_entry_for(&event, cx);
        if let Some(command) = self.state.route(event) {
            if let Err(error) = self.backend.try_dispatch(command) {
                self.state.status = format!("Backend error: {error}");
            }
            self.refresh_snapshot(cx);
        }
        if let Some(entry) = history_entry {
            self.state.history.record(entry);
        }
        if let Some(kind) = self.state.requested_window.take() {
            self.open_auxiliary_window(kind, cx);
        }
        if let Some((target, position)) = self.state.requested_menu.take() {
            self.context_menu.update(cx, |menu, cx| {
                menu.open_menu(target, position);
                cx.notify();
            });
        }
        if self.state.active_modal == ActiveModal::Settings && self.settings_modal.is_none() {
            self.open_settings_modal(cx);
        }
        if self.state.active_modal == ActiveModal::Plugins && self.plugins_modal.is_none() {
            self.open_plugins_modal(cx);
        }
        if self.state.active_modal == ActiveModal::AddScene && self.add_scene_modal.is_none() {
            self.open_add_scene_modal(cx);
        }
        if self.state.active_modal == ActiveModal::AddSource && self.add_source_modal.is_none() {
            self.open_add_source_modal(cx);
        }
        if let ActiveModal::SourceProperties(id) = &self.state.active_modal {
            let id = id.clone();
            if self.source_properties_modal.is_none() {
                self.open_source_properties_modal(&id, false, cx);
            }
        }
        if let ActiveModal::SourceEditor(id) = &self.state.active_modal {
            let id = id.clone();
            if self.source_properties_modal.is_none() {
                // Edit opens the editor-mode properties window (Svelte
                // EditSourceModal parity).
                self.open_source_properties_modal(&id, true, cx);
            }
        }
        if let ActiveModal::QuickAction { source_id, action } = &self.state.active_modal {
            let (source_id, action) = (source_id.clone(), *action);
            if self.quick_action_modal.is_none() {
                self.open_quick_action_modal(&source_id, action, cx);
            }
        }
        cx.notify();
    }

    /// Routes one right-click context-menu choice into the same typed
    /// component-event workflow the panels use. Scene reordering resolves the
    /// visual up/down directions against the panel's current backend order.
    fn handle_context_menu_event(&mut self, event: ContextMenuEvent, cx: &mut Context<'_, Self>) {
        if let ContextMenuEvent::Chosen { target, action } = event {
            let events = self.context_menu_events(target, action, cx);
            for event in events {
                self.route_event(event, cx);
            }
        }
        cx.notify();
    }

    fn context_menu_events(
        &self,
        target: ContextMenuTarget,
        action: ContextMenuAction,
        cx: &mut Context<'_, Self>,
    ) -> Vec<ComponentEvent> {
        match (target, action) {
            (ContextMenuTarget::Scene { name, .. }, ContextMenuAction::Select) => {
                vec![ComponentEvent::SelectScene(name)]
            }
            (ContextMenuTarget::Scene { name, locked }, ContextMenuAction::ToggleLock) => {
                vec![ComponentEvent::SetSceneLock {
                    name,
                    locked: !locked,
                }]
            }
            (ContextMenuTarget::Scene { name, .. }, ContextMenuAction::Move(direction)) => {
                let scenes = self.scenes.read(cx).scenes.clone();
                let Some(index) = scenes.iter().position(|scene| scene.name == name) else {
                    return Vec::new();
                };
                let last = scenes.len().saturating_sub(1);
                let to_index = match direction {
                    MoveDirection::Up => index.saturating_sub(1),
                    MoveDirection::Down => (index + 1).min(last),
                    MoveDirection::Top => 0,
                    MoveDirection::Bottom => last,
                };
                vec![ComponentEvent::ReorderScene { name, to_index }]
            }
            (ContextMenuTarget::Scene { name, .. }, ContextMenuAction::PreviewInWindow) => {
                vec![ComponentEvent::OpenPreviewInWindow {
                    target_id: Some(name),
                }]
            }
            (ContextMenuTarget::Scene { name, .. }, ContextMenuAction::Remove) => {
                vec![ComponentEvent::RemoveScene(name)]
            }
            (ContextMenuTarget::Source { id, .. }, ContextMenuAction::Edit) => {
                vec![ComponentEvent::EditSource(id)]
            }
            (ContextMenuTarget::Source { id, .. }, ContextMenuAction::QuickAction) => {
                let Some(action) = self.sources.read(cx).quick_action_for(&id) else {
                    return Vec::new();
                };
                vec![ComponentEvent::QuickSourceAction { id, action }]
            }
            (ContextMenuTarget::Source { id, .. }, ContextMenuAction::ToggleVisibility) => {
                vec![ComponentEvent::ToggleSourceVisibility(id)]
            }
            (ContextMenuTarget::Source { id, .. }, ContextMenuAction::ToggleLock) => {
                vec![ComponentEvent::ToggleSourceLock(id)]
            }
            (ContextMenuTarget::Source { id, .. }, ContextMenuAction::Move(direction)) => {
                vec![ComponentEvent::MoveSource { id, direction }]
            }
            (ContextMenuTarget::Source { id, .. }, ContextMenuAction::PreviewInWindow) => {
                vec![ComponentEvent::OpenPreviewInWindow {
                    target_id: Some(id),
                }]
            }
            (ContextMenuTarget::Source { id, .. }, ContextMenuAction::Remove) => {
                vec![ComponentEvent::RemoveSource(id)]
            }
            (ContextMenuTarget::Preview, ContextMenuAction::ForcePreviewResolution) => {
                vec![ComponentEvent::ForcePreviewResolution]
            }
            (ContextMenuTarget::Preview, ContextMenuAction::ToggleStudioMode) => {
                let enabled = !self.header.read(cx).state.studio_mode;
                vec![ComponentEvent::SetStudioMode(enabled)]
            }
            // Entry/action pairs always come from `entries()` per target, so
            // the remaining combinations are unreachable.
            _ => Vec::new(),
        }
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

    /// Creates the plugins modal entity seeded from the last known backend
    /// plugin data (discovered modules, saved profile states, applied
    /// profile) and subscribes to its Save/Cancel events.
    fn open_plugins_modal(&mut self, cx: &mut Context<'_, Self>) {
        let draft = crate::components::modal::PluginsDraft::load(
            self.state.plugins.clone(),
            self.state.plugin_profiles.clone(),
            self.state
                .plugin_profile_states
                .iter()
                .map(|state| (state.profile_name.clone(), state.enabled_modules.clone()))
                .collect(),
            self.state.active_plugin_profile.clone(),
            self.state.enabled_plugin_modules.clone(),
        );
        let entity = cx.new(|cx| PluginsModal::new(draft, cx));
        let subscription = cx.subscribe(&entity, |this, _, event, cx| match event {
            PluginsEvent::Saved {
                profile_name,
                modules,
            } => {
                this.state.status = format!("Plugin profile '{profile_name}' saved");
                if let Err(error) = this.backend.try_dispatch(BackendCommand::SetPluginProfile {
                    profile_name: profile_name.clone(),
                    modules: modules.clone(),
                }) {
                    this.state.status = format!("Backend error: {error}");
                }
                this.close_plugins_modal(cx);
                this.refresh_snapshot(cx);
            }
            PluginsEvent::Cancelled => this.close_plugins_modal(cx),
        });
        self._subscriptions.push(subscription);
        self.plugins_modal = Some(entity);
    }

    /// Drops the plugins entity and closes the modal when it is active.
    fn close_plugins_modal(&mut self, cx: &mut Context<'_, Self>) {
        self.plugins_modal = None;
        if self.state.active_modal == ActiveModal::Plugins {
            self.state.close_modal();
            cx.notify();
        }
    }

    /// Creates the add-scene modal entity and subscribes to its Create and
    /// Cancel events; creation dispatches the AddScene backend command. The
    /// existing scene names seed the `scene-{nr}` default placeholder.
    fn open_add_scene_modal(&mut self, cx: &mut Context<'_, Self>) {
        let existing_names = self
            .scenes
            .read(cx)
            .scenes
            .iter()
            .map(|scene| scene.name.clone())
            .collect();
        let entity = cx.new(|cx| AddSceneModal::new(existing_names, cx));
        let subscription = cx.subscribe(&entity, |this, _, event, cx| match event {
            AddSceneEvent::Created { name } => {
                this.state.status = format!("Scene '{name}' created");
                if let Err(error) = this
                    .backend
                    .try_dispatch(BackendCommand::AddScene(name.clone()))
                {
                    this.state.status = format!("Backend error: {error}");
                }
                this.state.history.record(crate::history::HistoryEntry {
                    label: format!("Add scene '{name}'"),
                    undo: vec![BackendCommand::RemoveScene(name.clone())],
                    redo: vec![BackendCommand::AddScene(name.clone())],
                });
                this.close_add_scene_modal(cx);
                this.refresh_snapshot(cx);
            }
            AddSceneEvent::Cancelled => this.close_add_scene_modal(cx),
        });
        self._subscriptions.push(subscription);
        self.add_scene_modal = Some(entity);
    }

    /// Drops the add-scene entity and closes the modal when it is active.
    fn close_add_scene_modal(&mut self, cx: &mut Context<'_, Self>) {
        self.add_scene_modal = None;
        if self.state.active_modal == ActiveModal::AddScene {
            self.state.close_modal();
            cx.notify();
        }
    }

    /// Creates the add-source modal entity seeded from the backend's
    /// available source types and the existing source names (for the
    /// `source-{type}-{nr}` default placeholder); creation dispatches the
    /// AddSource command and then opens the new source's properties modal.
    fn open_add_source_modal(&mut self, cx: &mut Context<'_, Self>) {
        let existing_names = self
            .sources
            .read(cx)
            .sources
            .iter()
            .map(|source| source.name.clone())
            .collect();
        let entity = cx.new(|cx| {
            AddSourceModal::new(
                self.state.available_source_types.clone(),
                existing_names,
                cx,
            )
        });
        let subscription = cx.subscribe(&entity, |this, _, event, cx| match event {
            AddSourceEvent::Created { name, source_type } => {
                this.state.status = format!("Source '{name}' created");
                if let Err(error) = this.backend.try_dispatch(BackendCommand::AddSource {
                    name: name.clone(),
                    source_type: source_type.clone(),
                }) {
                    this.state.status = format!("Backend error: {error}");
                }
                // Svelte parity: after creating, the properties window for
                // the new source opens. The next snapshot resolves the name
                // to its backend id (also for the undo entry).
                this.pending_properties_source = Some(name.clone());
                this.pending_history_entry = Some(crate::history::HistoryEntry {
                    label: format!("Add source '{name}'"),
                    undo: Vec::new(),
                    redo: vec![BackendCommand::AddSource {
                        name: name.clone(),
                        source_type: source_type.clone(),
                    }],
                });
                this.close_add_source_modal(cx);
                this.refresh_snapshot(cx);
            }
            AddSourceEvent::Cancelled => this.close_add_source_modal(cx),
        });
        self._subscriptions.push(subscription);
        self.add_source_modal = Some(entity);
    }

    /// Creates the source properties modal entity seeded from the snapshot's
    /// source item and placed transform, subscribing to Apply / OpenEditor /
    /// Cancel events.
    fn open_source_properties_modal(
        &mut self,
        source_id: &str,
        editor: bool,
        cx: &mut Context<'_, Self>,
    ) {
        let item = self
            .sources
            .read(cx)
            .sources
            .iter()
            .find(|source| source.id == source_id)
            .cloned();
        let Some(item) = item else {
            self.state.status = format!("Source '{source_id}' not found for properties");
            cx.notify();
            return;
        };
        let transform = self
            .state
            .source_transforms
            .iter()
            .find(|placed| placed.id == source_id)
            .cloned();
        let canvas = self
            .state
            .settings
            .as_ref()
            .and_then(|settings| settings.scene_resolution.clone());
        let entity = cx.new(|cx| {
            if editor {
                SourcePropertiesModal::new_editor(
                    item.id.clone(),
                    item.name.clone(),
                    item.source_type.clone(),
                    transform,
                    canvas,
                    cx,
                )
            } else {
                SourcePropertiesModal::new(
                    item.id.clone(),
                    item.name.clone(),
                    item.source_type.clone(),
                    transform,
                    canvas,
                    cx,
                )
            }
        });
        let subscription = cx.subscribe(&entity, |this, _, event, cx| {
            let event = event.clone();
            match event {
                SourcePropertiesEvent::Applied {
                    id,
                    x,
                    y,
                    width,
                    height,
                    rotation,
                } => {
                    // Undo restores the transform from the pre-apply snapshot.
                    if let Some(previous) = this
                        .state
                        .source_transforms
                        .iter()
                        .find(|placed| placed.id == id)
                        .cloned()
                    {
                        this.state.history.record(crate::history::HistoryEntry {
                            label: format!("Transform '{}'", id),
                            undo: vec![BackendCommand::SetSourceTransform {
                                id: id.clone(),
                                x: previous.x,
                                y: previous.y,
                                width: previous.width,
                                height: previous.height,
                                rotation: previous.rotation,
                            }],
                            redo: vec![BackendCommand::SetSourceTransform {
                                id: id.clone(),
                                x,
                                y,
                                width,
                                height,
                                rotation,
                            }],
                        });
                    }
                    this.state.status = "Source transform applied".into();
                    this.dispatch(
                        [BackendCommand::SetSourceTransform {
                            id,
                            x,
                            y,
                            width,
                            height,
                            rotation,
                        }],
                        cx,
                    );
                }
                SourcePropertiesEvent::OpenEditor { id } => {
                    this.close_source_properties_modal(cx);
                    this.route_event(ComponentEvent::EditSource(id), cx);
                }
                SourcePropertiesEvent::Cancelled => this.close_source_properties_modal(cx),
            }
        });
        self._subscriptions.push(subscription);
        self.state.active_modal = ActiveModal::SourceProperties(source_id.to_owned());
        self.source_properties_modal = Some(entity);
        cx.notify();
    }

    /// Drops the source properties entity and closes the modal when active.
    fn close_source_properties_modal(&mut self, cx: &mut Context<'_, Self>) {
        self.source_properties_modal = None;
        if matches!(
            self.state.active_modal,
            ActiveModal::SourceProperties(_) | ActiveModal::SourceEditor(_)
        ) {
            self.state.close_modal();
            cx.notify();
        }
    }

    /// Creates the quick-action modal entity for one source and action; the
    /// Apply event maps to `UpdateSource` (Svelte `obs_update_source`).
    fn open_quick_action_modal(
        &mut self,
        source_id: &str,
        action: QuickSourceAction,
        cx: &mut Context<'_, Self>,
    ) {
        let Some(item) = self
            .sources
            .read(cx)
            .sources
            .iter()
            .find(|source| source.id == source_id)
            .cloned()
        else {
            self.state.status = format!("Source '{source_id}' not found for quick action");
            cx.notify();
            return;
        };
        let entity =
            cx.new(|cx| QuickActionModal::new(item.id.clone(), item.name.clone(), action, cx));
        let subscription = cx.subscribe(&entity, |this, _, event, cx| {
            let event = event.clone();
            match event {
                QuickActionEvent::Applied { id, action, value } => {
                    let Some(key) = crate::components::quick_modal::param_key(action) else {
                        this.state.status = "Quick action has no settings key".into();
                        cx.notify();
                        return;
                    };
                    // Fetch the current settings first so the undo entry can
                    // restore the previous value (Svelte
                    // recordSourceSettingsHistory parity).
                    let service = match &this.backend {
                        crate::services::AnyBackend::Obs(service) => Some(service.clone()),
                        crate::services::AnyBackend::Mock(_) => None,
                    };
                    let (id, value) = (id.clone(), value.clone());
                    let key_owned = key.to_string();
                    match service {
                        Some(service) => {
                            cx.spawn(async move |workspace, cx| {
                                let id_for_fetch = id.clone();
                                let previous = cx
                                    .background_executor()
                                    .spawn(async move {
                                        crate::services::obs::source_settings(
                                            &service,
                                            &id_for_fetch,
                                        )
                                        .ok()
                                        .and_then(|data| data.params.get(&key_owned).cloned())
                                    })
                                    .await;
                                let _ = workspace.update(cx, |this, cx| {
                                    this.apply_quick_action_edit(
                                        id, key, value, previous, cx,
                                    );
                                });
                            })
                            .detach();
                        }
                        None => {
                            // Mock backend: no settings store to diff against,
                            // so the edit applies without a history entry.
                            this.apply_quick_action_edit(id, key, value, None, cx);
                        }
                    }
                }
                QuickActionEvent::Interact { id: _ } => {
                    this.close_quick_action_modal(cx);
                    this.state.requested_window = Some(AuxiliaryWindowKind::BrowserInteraction);
                }
                QuickActionEvent::Cancelled => this.close_quick_action_modal(cx),
            }
        });
        self._subscriptions.push(subscription);
        self.quick_action_modal = Some(entity);
        cx.notify();
    }

    /// Applies one quick-action settings edit and records the undo entry
    /// when the previous value is known.
    fn apply_quick_action_edit(
        &mut self,
        id: String,
        key: String,
        value: String,
        previous: Option<String>,
        cx: &mut Context<'_, Self>,
    ) {
        let mut params = std::collections::HashMap::new();
        params.insert(key.clone(), value.clone());
        if let Some(previous) = previous {
            let mut undo_params = std::collections::HashMap::new();
            undo_params.insert(key.clone(), previous);
            self.state.history.record(crate::history::HistoryEntry {
                label: format!("Edit {key} of '{id}'"),
                undo: vec![BackendCommand::UpdateSource {
                    update: revo_core::models::SourceUpdate {
                        id: id.clone(),
                        name: String::new(),
                        source_type: String::new(),
                        params: undo_params,
                    },
                }],
                redo: vec![BackendCommand::UpdateSource {
                    update: revo_core::models::SourceUpdate {
                        id: id.clone(),
                        name: String::new(),
                        source_type: String::new(),
                        params,
                    },
                }],
            });
        }
        let redo_params = std::collections::HashMap::new();
        let _ = redo_params;
        self.state.status = format!("Quick action applied ({key})");
        let mut dispatch_params = std::collections::HashMap::new();
        dispatch_params.insert(key, value);
        self.dispatch(
            [BackendCommand::UpdateSource {
                update: revo_core::models::SourceUpdate {
                    id,
                    name: String::new(),
                    source_type: String::new(),
                    params: dispatch_params,
                },
            }],
            cx,
        );
        self.close_quick_action_modal(cx);
    }

    /// Drops the quick-action modal entity and closes its modal state.
    fn close_quick_action_modal(&mut self, cx: &mut Context<'_, Self>) {
        self.quick_action_modal = None;
        if matches!(self.state.active_modal, ActiveModal::QuickAction { .. }) {
            self.state.close_modal();
            cx.notify();
        }
    }

    /// Drops the add-source entity and closes the modal when it is active.
    fn close_add_source_modal(&mut self, cx: &mut Context<'_, Self>) {
        self.add_source_modal = None;
        if self.state.active_modal == ActiveModal::AddSource {
            self.state.close_modal();
            cx.notify();
        }
    }

    /// Closes whichever modal is currently active, cleaning up the matching
    /// entity state.
    fn close_active_modal(&mut self, cx: &mut Context<'_, Self>) {
        match &self.state.active_modal {
            ActiveModal::Settings => self.close_settings_modal(cx),
            ActiveModal::Plugins => self.close_plugins_modal(cx),
            ActiveModal::AddScene => self.close_add_scene_modal(cx),
            ActiveModal::AddSource => self.close_add_source_modal(cx),
            ActiveModal::SourceProperties(_) => self.close_source_properties_modal(cx),
            ActiveModal::QuickAction { .. } => self.close_quick_action_modal(cx),
            ActiveModal::AudioMixer | ActiveModal::Transitions => {
                // The panel entities persist; the modal overlay just closes.
                self.state.close_modal();
                cx.notify();
            }
            ActiveModal::AudioMixerAdvanced(_)
            | ActiveModal::GraphicPlanner
            | ActiveModal::None => {
                // Entity-hosted overlays keep their own lifecycle; the modal
                // state just reverts to none.
                self.state.close_modal();
                cx.notify();
            }
            ActiveModal::History | ActiveModal::Templates | ActiveModal::SourceEditor(_) => {
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
                ComponentEvent::StartRecording => header.state.recording = true,
                ComponentEvent::StopRecording => header.state.recording = false,
                ComponentEvent::StartStreaming => header.state.streaming = true,
                ComponentEvent::StopStreaming => header.state.streaming = false,
                ComponentEvent::SetStudioMode(enabled) => header.state.studio_mode = *enabled,
                _ => return,
            }
            cx.notify();
        });
    }

    /// The `.tools` section: four icon+label+description buttons over an
    /// embedded tool module (mixer/transitions when toggled), plus the
    /// auxiliary-window shortcuts below.
    fn tools(&self, cx: &mut Context<'_, Self>) -> gpui::Div {
        let mixer_button = self.tool_button(
            cx,
            "tool-audio-mixer",
            "tool-mixer",
            "Audio Mixer",
            "Volume, mute and devices",
            |this, _, _, cx| {
                this.route_event(ComponentEvent::OpenMixer, cx);
                cx.stop_propagation();
            },
        );
        let transitions_button = self.tool_button(
            cx,
            "tool-transitions",
            "tool-transitions",
            "Transitions",
            "Scene change effects",
            |this, _, _, cx| {
                this.route_event(ComponentEvent::OpenTransitions, cx);
                cx.stop_propagation();
            },
        );
        let plugins_button = self.tool_button(
            cx,
            "tool-plugins",
            "tool-plugins",
            "Plugins",
            "Extend your control room",
            |this, _, _, cx| {
                this.route_event(ComponentEvent::OpenPlugins, cx);
                cx.stop_propagation();
            },
        );
        let templates_button = self.tool_button(
            cx,
            "tool-templates",
            "tool-templates",
            "Templates",
            "Ready-made scene layouts",
            |this, _, _, cx| {
                this.route_event(ComponentEvent::OpenTemplates, cx);
                cx.stop_propagation();
            },
        );

        div()
            .flex()
            .flex_col()
            .gap_2()
            .size_full()
            .text_color(self.theme.text)
            .child(div().text_color(self.theme.text_strong).child("Tools"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(mixer_button)
                    .child(transitions_button)
                    .child(plugins_button)
                    .child(templates_button),
            )
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

    /// One `.tool-list` button: icon, two-line label with description, and a
    /// chevron, styled per the Svelte reference.
    #[allow(clippy::too_many_arguments)]
    fn tool_button(
        &self,
        cx: &Context<'_, Self>,
        id: &'static str,
        icon_name: &'static str,
        label: &'static str,
        desc: &'static str,
        on_click: impl Fn(&mut Workspace, &gpui::ClickEvent, &mut Window, &mut gpui::Context<'_, Workspace>)
            + 'static,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(SharedString::from(id))
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_2()
            .py_1p5()
            .rounded_lg()
            .bg(self.theme.surface)
            .border_1()
            .border_color(self.theme.border)
            .text_color(self.theme.text)
            .hover(|style| style.bg(self.theme.surface_active))
            .on_click(cx.listener(on_click))
            .child(crate::components::icon::icon(
                self.theme.text,
                icon_name,
                15.,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .child(
                        div()
                            .text_size(px(11.2))
                            .font_weight(gpui::FontWeight(500.))
                            .child(label),
                    )
                    .child(
                        div()
                            .text_color(self.theme.text_muted)
                            .text_size(px(9.2))
                            .truncate()
                            .child(desc),
                    ),
            )
            .child(crate::components::icon::icon(
                self.theme.text_muted,
                "chevron-right",
                13.,
            ))
    }

    /// One 16:9-shaped capture frame on a black stage: the black frame hugs
    /// the scene instead of stretching to the window, so the whole scene
    /// stays visible inside the frame (Svelte `.preview-frame` parity).
    /// gpui 0.2.2 has no CSS aspect-ratio, so the frame size is computed
    /// from the container bounds measured by [`Self::container_probe`] on
    /// the previous frame; until the first measurement lands the frame
    /// falls back to filling the container.
    #[cfg(feature = "obs-backend")]
    fn capture_frame(
        &self,
        id: impl Into<SharedString>,
        frame: Option<&std::sync::Arc<GpuiImage>>,
        waiting_label: &str,
        container: &MeasuredContainer,
        ratio: f32,
    ) -> gpui::Stateful<gpui::Div> {
        let mut stage = div()
            .id(id.into())
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0x000000))
            .border_1()
            .border_color(self.theme.border)
            .rounded_lg()
            .overflow_hidden();
        stage = match Self::aspect_frame_size(container, ratio) {
            Some((width, height)) => stage.w(width).h(height),
            None => stage.h_full().max_w_full(),
        };
        stage.child(match frame {
            Some(frame) => gpui::img(std::sync::Arc::clone(frame))
                .max_w_full()
                .max_h_full()
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element(),
            None => div()
                .flex()
                .items_center()
                .justify_center()
                .text_color(self.theme.text_muted)
                .text_size(px(11.))
                .child(waiting_label.to_string())
                .into_any_element(),
        })
    }

    /// Largest `ratio`-shaped rectangle fitting inside the measured
    /// container bounds.
    #[cfg(feature = "obs-backend")]
    fn aspect_frame_size(
        container: &MeasuredContainer,
        ratio: f32,
    ) -> Option<(gpui::Pixels, gpui::Pixels)> {
        let size = container.get()?;
        let (width, height) = (f32::from(size.width), f32::from(size.height));
        if width <= 0.0 || height <= 0.0 || ratio <= 0.0 {
            return None;
        }
        Some(if width / height > ratio {
            (px(height * ratio), px(height))
        } else {
            (px(width), px(width / ratio))
        })
    }

    /// Invisible bounds probe recording its container's size once per frame
    /// so the capture frames can keep a scene-shaped aspect ratio.
    #[cfg(feature = "obs-backend")]
    fn container_probe(cell: &MeasuredContainer) -> gpui::AnyElement {
        let cell = cell.clone();
        gpui::canvas(
            move |bounds, _, _| cell.set(Some(bounds.size)),
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
        .into_any_element()
    }

    /// One labeled preview column for studio mode; renders its frame or a
    /// bounded placeholder so the layout never jumps between frames.
    #[cfg(feature = "obs-backend")]
    fn preview_column(
        &self,
        label: &'static str,
        frame: Option<&std::sync::Arc<GpuiImage>>,
        waiting: bool,
        container: &MeasuredContainer,
        ratio: f32,
    ) -> gpui::Div {
        let waiting_label = if waiting {
            "waiting for frame…"
        } else {
            "no capture"
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap_1()
            .child(
                div()
                    .text_color(self.theme.text_secondary)
                    .text_size(px(11.))
                    .child(label),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .flex_1()
                    .min_h(px(180.))
                    .child(Self::container_probe(container))
                    .child(self.capture_frame(
                        format!("preview-column-{label}"),
                        frame,
                        waiting_label,
                        container,
                        ratio,
                    )),
            )
    }

    /// The demo/preview surface. With the `obs-backend` feature the newest
    /// PNG preview frame from the real libobs scene output is rendered as a
    /// GPUI image; until the first frame lands (or for the mock backend) the
    /// demo placeholder is shown, including its "dims" status line.
    /// The `.render` band from the Svelte layout: fills the space above the
    /// panel section, centers content, and keeps the preview surface inside
    /// a `min(960px, 100%)` live-preview column.
    fn preview_surface(&self, cx: &mut Context<'_, Self>) -> gpui::Stateful<gpui::Div> {
        let band = div()
            .id("preview-surface")
            .flex_1()
            .min_h(px(200.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .px_6()
            .pb_4()
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    let position = (f32::from(event.position.x), f32::from(event.position.y));
                    this.route_event(
                        ComponentEvent::OpenContextMenu {
                            target: ContextMenuTarget::Preview,
                            position,
                        },
                        cx,
                    );
                }),
            );
        #[cfg(feature = "obs-backend")]
        {
            // Scene aspect ratio for the capture frames: the configured
            // canvas resolution when known, otherwise the 16:9 default.
            let ratio = self
                .state
                .settings
                .as_ref()
                .and_then(|settings| settings.scene_resolution.as_deref())
                .and_then(revo_core::planner::parse_resolution)
                .map_or(
                    revo_core::planner::DEFAULT_SCENE_RESOLUTION_RATIO,
                    |(width, height)| width as f32 / height as f32,
                );
            let studio = self.scenes.read(cx).studio_mode;
            let preview_scene = self.scenes.read(cx).preview_scene.clone();
            if studio {
                // OBS-style studio split: the preview scene beside the live
                // program composite, each with its own label. `flex_1` gives
                // the split the full band height so both frames size like
                // the single-program view.
                let columns = div()
                    .flex()
                    .flex_row()
                    .gap_3()
                    .w_full()
                    .flex_1()
                    .max_w(px(960.))
                    .child(self.preview_column(
                        "Preview",
                        self.studio_preview_frame.as_ref(),
                        preview_scene.is_some(),
                        &self.preview_container_sizes.studio_preview,
                        ratio,
                    ))
                    .child(self.preview_column(
                        "Studio (Program)",
                        self.preview_frame.as_ref(),
                        true,
                        &self.preview_container_sizes.studio_program,
                        ratio,
                    ));
                return band.child(columns);
            }
            return match &self.preview_frame {
                // A live frame replaces the placeholder entirely so the band
                // layout stays stable between captures. The frame keeps a
                // black stage and hugs the scene at its natural aspect ratio
                // instead of stretching across the band.
                Some(frame) => band
                    .child(Self::container_probe(&self.preview_container_sizes.program))
                    .child(self.capture_frame(
                        "preview-frame",
                        Some(frame),
                        "",
                        &self.preview_container_sizes.program,
                        ratio,
                    )),
                // Documented placeholder: gpui 0.2.2 `img()` accepts
                // `Arc<Image>` bytes directly, so this surfaces the libobs
                // capture state instead of a blank surface while the stream
                // of frames warms up.
                None => band
                    .child(Self::container_probe(&self.preview_container_sizes.program))
                    .child(self.capture_frame(
                        "preview-frame",
                        None,
                        "OBS preview: waiting for first frame (150ms capture loop)",
                        &self.preview_container_sizes.program,
                        ratio,
                    )),
            };
        }
        #[cfg(not(feature = "obs-backend"))]
        {
            band.child(self.demo.clone())
        }
    }

    /// The bordered `.panel` section from the Svelte layout: fixed 240px
    /// band (Wide), 1px border, 12px radius, 20px padding, 24px margin, and
    /// equal-width columns with a 16px gap. Medium/Compact keep the
    /// documented ordering and scroll instead of a fixed height.
    fn lower_panels(&self, cx: &mut Context<'_, Self>, mode: LayoutMode) -> gpui::Div {
        let [first, second, third] = mode.panel_order();
        let cells = match mode {
            LayoutMode::Wide => div()
                .flex()
                .flex_row()
                .gap_4()
                .child(div().flex_1().min_w_0().child(self.panel(cx, first)))
                .child(div().flex_1().min_w_0().child(self.panel(cx, second)))
                .child(div().flex_1().min_w_0().child(self.panel(cx, third))),
            LayoutMode::Medium => div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_4()
                        .child(div().flex_1().min_w_0().child(self.panel(cx, first)))
                        .child(div().flex_1().min_w_0().child(self.panel(cx, second))),
                )
                .child(div().min_h(px(180.)).child(self.panel(cx, third))),
            LayoutMode::Compact => div()
                .flex()
                .flex_col()
                .gap_4()
                .child(self.panel(cx, first))
                .child(self.panel(cx, second))
                .child(div().min_h(px(180.)).child(self.panel(cx, third))),
        };
        let band = match mode {
            LayoutMode::Wide => div().h(px(240.)),
            LayoutMode::Medium => div().max_h(px(250.)).h(px(420.)),
            LayoutMode::Compact => div().h(px(560.)),
        };
        band.border_1()
            .border_color(self.theme.border)
            .rounded_lg()
            .p_5()
            .m_6()
            .overflow_hidden()
            .child(cells)
    }

    fn panel(&self, cx: &mut Context<'_, Self>, panel: PanelKind) -> gpui::Div {
        match panel {
            PanelKind::Scenes => div().child(self.scenes.clone()),
            PanelKind::Sources => div().child(self.sources.clone()),
            PanelKind::Tools => div().child(self.tools(cx)),
        }
    }

    fn modal_content(
        &self,
        cx: &mut Context<'_, Self>,
    ) -> Option<(ModalConfig, Vec<SharedString>, Option<gpui::AnyElement>)> {
        match &self.state.active_modal {
            ActiveModal::None => None,
            ActiveModal::Settings => None,
            ActiveModal::Plugins => None,
            ActiveModal::History => {
                // Live undo/redo history from the workspace log.
                let mut lines = Vec::new();
                if self.state.history.undo_stack.is_empty() {
                    lines.push("Nothing to undo yet.".into());
                } else {
                    for (index, entry) in self.state.history.undo_stack.iter().enumerate().rev() {
                        lines.push(SharedString::from(format!(
                            "#{} undo: {}",
                            index + 1,
                            entry.label
                        )));
                    }
                }
                if !self.state.history.redo_stack.is_empty() {
                    lines.push("— redo —".into());
                    for entry in self.state.history.redo_stack.iter().rev() {
                        lines.push(SharedString::from(format!("redo: {}", entry.label)));
                    }
                }
                let footer = self.modal_cancel_footer(cx);
                Some((
                    ModalConfig {
                        show_close: false,
                        ..ModalConfig::standard("Action History", ModalSize::Medium)
                    },
                    lines,
                    Some(footer),
                ))
            }
            ActiveModal::Templates => {
                // Mirrors the Svelte "Templates availability" dialog.
                let footer = div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .p_3()
                    .child(
                        div()
                            .id("templates-ok")
                            .cursor_pointer()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(self.theme.accent)
                            .text_color(self.theme.text_strong)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.close_active_modal(cx);
                            }))
                            .child("OK"),
                    )
                    .into_any_element();
                Some((
                    ModalConfig::standard("Templates", ModalSize::Small),
                    vec!["Future will be available on newer versions.".into()],
                    Some(footer),
                ))
            }
            ActiveModal::AddScene => None,
            ActiveModal::AddSource => None,
            // The properties window is an entity-hosted modal rendered in
            // the overlay tree, so the card content stays empty here.
            ActiveModal::SourceProperties(_) => None,
            ActiveModal::AudioMixer => None,
            ActiveModal::Transitions => None,
            ActiveModal::SourceEditor(_) => None,
            ActiveModal::AudioMixerAdvanced(_) | ActiveModal::GraphicPlanner => None,
            ActiveModal::QuickAction { .. } => None,
        }
    }

    /// Working Cancel footer shared by the workspace-rendered modals; the
    /// structural header Close stays hidden wherever a footer Cancel exists.
    fn modal_cancel_footer(&self, cx: &mut Context<'_, Self>) -> gpui::AnyElement {
        div()
            .flex()
            .flex_row()
            .justify_end()
            .p_3()
            .child(
                div()
                    .id("workspace-modal-cancel")
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(self.theme.surface_active)
                    .text_color(self.theme.text)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.close_active_modal(cx);
                    }))
                    .child("Cancel"),
            )
            .into_any_element()
    }
}

/// Decoding of `data:image/png;base64,...` frames lives in
/// `services::obs::decode_png_data_uri` so the auxiliary preview and
/// interact windows share it with the workspace loop.

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let mode = LayoutMode::for_width(f32::from(window.viewport_size().width));

        if !self.state.booted {
            // Boot splash: port of the Svelte startup screen — brand mark in
            // the accent color, status line, animated spinner ring.
            let spin =
                gpui::Transformation::rotate(gpui::radians(self.state.splash_spin.to_radians()));
            return div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .bg(rgb(0x080c12))
                .font_family("Space Grotesk")
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_2p5()
                        .child(
                            svg()
                                .path("brand/mark.svg")
                                .w(px(64.))
                                .h(px(64.))
                                .text_color(self.theme.accent),
                        )
                        .child(
                            div()
                                .text_color(self.theme.text_strong)
                                .text_size(px(21.6))
                                .font_weight(gpui::FontWeight::BOLD)
                                .child("RevoStream"),
                        )
                        .child(
                            div()
                                .text_color(self.theme.text_muted)
                                .text_size(px(14.4))
                                .child(SharedString::from(self.state.status.clone())),
                        )
                        .child(
                            svg()
                                .path("icons/spinner.svg")
                                .w(px(16.))
                                .h(px(16.))
                                .mt_1p5()
                                .text_color(self.theme.accent)
                                .with_transformation(spin),
                        ),
                );
        }

        let mut root = div()
            .relative()
            .flex()
            .flex_col()
            .gap_2()
            .size_full()
            .bg(self.theme.background)
            .text_color(self.theme.text)
            .font_family("Space Grotesk")
            .child(self.header.clone())
            .child(self.preview_surface(cx))
            .child(self.lower_panels(cx, mode));

        if let Some((config, lines, footer)) = self.modal_content(cx) {
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
                        // Occlusion keeps the modal exclusive: without it the
                        // panels underneath stay hoverable, so closing the
                        // overlay on mouse-down lets the mouse-up click land
                        // on a panel row (for example the Tools Transitions
                        // button appearing to open after Add Source).
                        .occlude()
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
                            footer,
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

        if self.state.active_modal == ActiveModal::Plugins {
            if let Some(entity) = self.plugins_modal.clone() {
                root = root.child(
                    div()
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom_0()
                        .left_0()
                        .bg(self.theme.overlay)
                        .occlude()
                        .child(entity),
                );
            }
        }
        if self.state.active_modal == ActiveModal::AddScene {
            if let Some(entity) = self.add_scene_modal.clone() {
                root = root.child(
                    div()
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom_0()
                        .left_0()
                        .bg(self.theme.overlay)
                        .occlude()
                        .child(entity),
                );
            }
        }
        if self.state.active_modal == ActiveModal::AddSource {
            if let Some(entity) = self.add_source_modal.clone() {
                root = root.child(
                    div()
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom_0()
                        .left_0()
                        .bg(self.theme.overlay)
                        .occlude()
                        .child(entity),
                );
            }
        }
        if self.state.active_modal == ActiveModal::AudioMixer {
            // The footer Cancel is the dismissal path; the header Close is
            // dropped as a duplicate.
            root = root.child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .left_0()
                    .bg(self.theme.overlay)
                    .occlude()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_6()
                    .child(modal_card(
                        &ModalConfig {
                            show_close: false,
                            ..ModalConfig::standard("Audio Mixer", ModalSize::Large)
                        },
                        self.theme,
                        self.tools_mixer.clone(),
                        Some(self.modal_cancel_footer(cx)),
                        None,
                    )),
            );
        }
        if self.state.active_modal == ActiveModal::Transitions {
            root = root.child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .left_0()
                    .bg(self.theme.overlay)
                    .occlude()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_6()
                    .child(modal_card(
                        &ModalConfig {
                            show_close: false,
                            ..ModalConfig::standard("Transitions", ModalSize::Large)
                        },
                        self.theme,
                        self.tools_transitions.clone(),
                        Some(self.modal_cancel_footer(cx)),
                        None,
                    )),
            );
        }

        if self.mixer_menu.read(cx).open {
            root = root.child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .left_0()
                    .occlude()
                    .child(self.mixer_menu.clone()),
            );
        }

        if self.context_menu.read(cx).open {
            root = root.child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .left_0()
                    .occlude()
                    .child(self.context_menu.clone()),
            );
        }

        if self.mixer_advanced.read(cx).open_id.as_deref().is_some() {
            root = root.child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .left_0()
                    .bg(self.theme.overlay)
                    .occlude()
                    .child(self.mixer_advanced.clone()),
            );
        }
        match &self.state.active_modal {
            ActiveModal::SourceProperties(_) => {
                if let Some(entity) = self.source_properties_modal.clone() {
                    root = root.child(
                        div()
                            .absolute()
                            .top_0()
                            .right_0()
                            .bottom_0()
                            .left_0()
                            .bg(self.theme.overlay)
                            .occlude()
                            .child(entity),
                    );
                }
            }
            ActiveModal::QuickAction { .. } => {
                if let Some(entity) = self.quick_action_modal.clone() {
                    root = root.child(
                        div()
                            .absolute()
                            .top_0()
                            .right_0()
                            .bottom_0()
                            .left_0()
                            .bg(self.theme.overlay)
                            .occlude()
                            .child(entity),
                    );
                }
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
                        .occlude()
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

/// Registers the bundled UI font (Space Grotesk) with the text system so
/// the shell can set `font_family` like the Svelte build did.
fn register_ui_fonts(cx: &mut App) {
    let fonts: Vec<std::borrow::Cow<'static, [u8]>> = crate::assets::EmbeddedAssets
        .list("fonts")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|path| crate::assets::EmbeddedAssets.load(&path).ok().flatten())
        .collect();
    if !fonts.is_empty() {
        if let Err(error) = cx.text_system().add_fonts(fonts) {
            eprintln!("failed to register bundled fonts: {error}");
        }
    }
}

pub fn run_native_app() {
    Application::new()
        .with_assets(crate::assets::EmbeddedAssets)
        .run(|cx: &mut App| {
            register_ui_fonts(cx);
            let bounds = Bounds::centered(None, size(px(1280.), px(720.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(1080.), px(600.))),
                    // Name the window like the Svelte build did; gpui 0.2.2
                    // has no window-icon API, so the mark.svg brand lives in
                    // the header and the desktop entry supplies the icon.
                    titlebar: Some(TitlebarOptions {
                        title: Some("RevoStream".into()),
                        ..TitlebarOptions::default()
                    }),
                    app_id: Some("RevoStream".into()),
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
            Some(AuxiliaryWindowKind::Preview { source_id: None })
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
    fn right_click_requests_open_the_context_menu_without_backend_commands() {
        let mut state = WorkspaceState::default();
        let target = ContextMenuTarget::Scene {
            name: "Main Camera".into(),
            locked: false,
        };
        state.route(ComponentEvent::OpenContextMenu {
            target: target.clone(),
            position: (40.0, 60.0),
        });
        assert_eq!(state.requested_menu.take(), Some((target, (40.0, 60.0))));
        assert_eq!(state.requested_menu.take(), None);
        assert_eq!(state.last_command, None);
    }

    #[test]
    fn context_menu_scene_actions_route_into_typed_backend_commands() {
        let mut state = WorkspaceState::default();
        assert_eq!(
            state.route(ComponentEvent::SetSceneLock {
                name: "Main Camera".into(),
                locked: true,
            }),
            Some(BackendCommand::SetSceneLock {
                name: "Main Camera".into(),
                locked: true,
            })
        );
        assert_eq!(
            state.route(ComponentEvent::RemoveScene("Be Right Back".into())),
            Some(BackendCommand::RemoveScene("Be Right Back".into()))
        );
        assert_eq!(
            state.route(ComponentEvent::RemoveSource("camera".into())),
            Some(BackendCommand::RemoveSource("camera".into()))
        );
        assert_eq!(
            state.last_command,
            Some(BackendCommand::RemoveSource("camera".into()))
        );
    }

    #[test]
    fn mock_service_executes_scene_lock_removal_and_source_removal() {
        let service = BackendService::new(4, MockBackendRuntime::default).unwrap();
        service
            .execute(BackendCommand::SetSceneLock {
                name: "Main Camera".into(),
                locked: true,
            })
            .unwrap();
        let snapshot = service.snapshot().unwrap();
        assert!(snapshot.locked_scenes.contains(&"Main Camera".into()));

        service
            .execute(BackendCommand::RemoveSource("camera".into()))
            .unwrap();
        service
            .execute(BackendCommand::RemoveScene("Starting Soon".into()))
            .unwrap();
        let snapshot = service.snapshot().unwrap();
        assert!(!snapshot.sources.contains(&"camera".into()));
        assert!(!snapshot.visible_sources.contains(&"camera".into()));
        assert_eq!(
            snapshot
                .scenes
                .iter()
                .position(|scene| scene == "Starting Soon"),
            None
        );
        // Removing the selected scene falls back to the first remaining one.
        service
            .execute(BackendCommand::RemoveScene("Main Camera".into()))
            .unwrap();
        let snapshot = service.snapshot().unwrap();
        assert_eq!(snapshot.selected_scene.as_deref(), Some("Be Right Back"));
        service.shutdown().unwrap();
    }

    #[test]
    fn mock_service_creates_scenes_and_sources() {
        let service = BackendService::new(4, MockBackendRuntime::default).unwrap();
        service
            .execute(BackendCommand::AddScene("Interview".into()))
            .unwrap();
        let snapshot = service.snapshot().unwrap();
        assert!(snapshot.scenes.contains(&"Interview".into()));
        // Creating a scene switches to it, mirroring obs create_scene.
        assert_eq!(snapshot.selected_scene.as_deref(), Some("Interview"));

        service
            .execute(BackendCommand::AddSource {
                name: "Alerts overlay".into(),
                source_type: "browser_source".into(),
            })
            .unwrap();
        service
            .execute(BackendCommand::AddSource {
                name: "Alerts overlay".into(),
                source_type: "browser_source".into(),
            })
            .unwrap();
        let snapshot = service.snapshot().unwrap();
        let created: Vec<&revo_core::service::SourceSnapshotItem> = snapshot
            .source_items
            .iter()
            .filter(|source| source.name == "Alerts overlay")
            .collect();
        assert_eq!(created.len(), 2);
        assert_eq!(created[0].id, "alerts-overlay");
        assert_eq!(created[1].id, "alerts-overlay-2");
        assert!(snapshot
            .sources
            .iter()
            .all(|id| id != "camera" || snapshot.sources.iter().any(|other| other == id)));

        // The add-source modal tiles read the available types.
        assert!(snapshot
            .available_source_types
            .iter()
            .any(|item| item.id == "browser_source" && item.label == "Browser"));

        service
            .execute(BackendCommand::AddScene("".into()))
            .expect_err("empty scene names are rejected");
        service.shutdown().unwrap();
    }

    #[test]
    fn mock_service_tracks_plugin_profile_states() {
        let service = BackendService::new(4, MockBackendRuntime::default).unwrap();
        service
            .execute(BackendCommand::SetPluginProfile {
                profile_name: "streaming".into(),
                modules: vec!["obs-websocket".into(), "obs-vst".into()],
            })
            .unwrap();
        let snapshot = service.snapshot().unwrap();
        let state = snapshot
            .plugin_profile_states
            .iter()
            .find(|state| state.profile_name == "streaming")
            .expect("saved profile state in snapshot");
        assert_eq!(
            state.enabled_modules,
            vec!["obs-websocket".to_owned(), "obs-vst".to_owned()]
        );

        service
            .execute(BackendCommand::SetActivePluginProfile("streaming".into()))
            .unwrap();
        let snapshot = service.snapshot().unwrap();
        assert_eq!(snapshot.active_plugin_profile.as_deref(), Some("streaming"));
        assert_eq!(
            snapshot.enabled_plugin_modules,
            vec!["obs-websocket".to_owned(), "obs-vst".to_owned()]
        );
        service.shutdown().unwrap();
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
                    audio_capable: true,
                    locked: false,
                },
                SourceItem {
                    id: "lower-third".into(),
                    name: "Lower Third".into(),
                    source_type: "text_ft2_source_v2".into(),
                    visible: false,
                    audio_capable: false,
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
        // The mock runtime carries source items now, so the mock shell
        // renders real source rows (type icon, actions) like the obs
        // backend does.
        assert!(snapshot
            .source_items
            .iter()
            .any(|source| source.id == "camera" && source.source_type == "v4l2_input"));
        assert!(snapshot.locked_sources.contains(&"browser-chat".into()));
        assert!(snapshot_scene_items(&snapshot).iter().all(|scene| {
            (snapshot.selected_scene.as_deref() == Some(scene.name.as_str())) == scene.active
        }));
    }
}
