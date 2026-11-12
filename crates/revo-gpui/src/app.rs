use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use gpui::{
    div, prelude::*, px, rgb, size, svg, App, Application, AssetSource, Bounds, Context, Entity,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Render, SharedString,
    Subscription, TitlebarOptions, Window, WindowBounds, WindowHandle, WindowOptions,
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
        AuxiliaryWindowKind, ComponentEvent, ContextMenuAction, ContextMenuTarget, DropdownField,
        QuickSourceAction,
    },
    components::{
        color_picker::{ColorPickerEvent, ColorPickerPopup},
        context_menu::{ContextMenuEvent, WorkspaceContextMenu},
        dropdown_picker::{DropdownPickerEvent, DropdownPickerPopup},
        font_picker::{FontPickerEvent, FontPickerPopup},
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
    /// Lifecycle confirmation dialog awaiting Accept/Cancel (plan §12).
    Confirm,
}

/// One gated lifecycle action: the backend command plus the originating
/// event (for the optimistic header flip on Accept) and the dialog copy.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingConfirm {
    /// Backend command dispatched on Accept; `None` for UI-only actions
    /// (the official-website confirmation, plan §24).
    pub command: Option<BackendCommand>,
    pub event: ComponentEvent,
    pub title: SharedString,
    pub message: SharedString,
    pub accept_label: SharedString,
    /// Optional non-default cancel label ("No" instead of "Cancel", §24).
    pub cancel_label: Option<SharedString>,
    /// History entry computed at gate time from the pre-dispatch snapshot;
    /// recorded on Accept so undo/redo keeps working behind the dialog
    /// (plan §15.4).
    pub history: Option<crate::history::HistoryEntry>,
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
    /// A lifecycle action held behind the confirmation dialog (plan §12).
    pub pending_confirm: Option<PendingConfirm>,
    /// The engine-published Settings option catalog (plan §6–8), kept on
    /// the workspace so a newly opened Settings modal seeds its dropdowns.
    pub select_options: Option<revo_core::service::SnapshotSelectOptions>,
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
            pending_confirm: None,
            select_options: None,
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
    /// Reads a `ui_profile` boolean ("true"/"false" strings or a JSON bool).
    pub fn ui_flag(&self, key: &str) -> bool {
        self.settings
            .as_ref()
            .and_then(|s| s.ui_profile.as_ref())
            .and_then(|p| p.get(key))
            .map(|value| match value.as_bool() {
                Some(flag) => flag,
                None => value
                    .as_str()
                    .map(|text| text.eq_ignore_ascii_case("true"))
                    .unwrap_or(false),
            })
            .unwrap_or(false)
    }

    /// Plan §12: a lifecycle event gated by its Settings > General toggle
    /// turns into a confirmation request instead of dispatching.
    pub fn confirm_request(&self, event: &ComponentEvent) -> Option<PendingConfirm> {
        let (command, title, message, accept_label) = match event {
            ComponentEvent::StartStreaming => {
                if !self.ui_flag("confirm_start_stream") {
                    return None;
                }
                (
                    BackendCommand::StartStreaming,
                    "Start streaming?",
                    "Start streaming to the configured service?".into(),
                    "Start streaming",
                )
            }
            ComponentEvent::StopStreaming => {
                if !self.ui_flag("confirm_stop_stream") {
                    return None;
                }
                (
                    BackendCommand::StopStreaming,
                    "Stop streaming?",
                    "Stop the current broadcast stream?".into(),
                    "Stop streaming",
                )
            }
            ComponentEvent::StartRecording => {
                if !self.ui_flag("confirm_start_record") {
                    return None;
                }
                let target = self
                    .settings
                    .as_ref()
                    .map(|s| s.record_path.clone())
                    .unwrap_or_else(|| "the default output folder".into());
                (
                    BackendCommand::StartRecording,
                    "Start recording?",
                    format!("Record to {target}?").into(),
                    "Start recording",
                )
            }
            ComponentEvent::StopRecording => {
                if !self.ui_flag("confirm_stop_record") {
                    return None;
                }
                (
                    BackendCommand::StopRecording,
                    "Stop recording?",
                    "Stop the current recording?".into(),
                    "Stop recording",
                )
            }
            ComponentEvent::RemoveScene(name) => {
                if !self.ui_flag("confirm_delete") {
                    return None;
                }
                (
                    BackendCommand::RemoveScene(name.clone()),
                    "Delete scene?",
                    format!("Delete scene \"{name}\"? The change stays undoable from History.")
                        .into(),
                    "Delete scene",
                )
            }
            ComponentEvent::RemoveSource(id) => {
                if !self.ui_flag("confirm_delete") {
                    return None;
                }
                (
                    BackendCommand::RemoveSource(id.clone()),
                    "Delete source?",
                    format!("Delete source \"{id}\"? The change stays undoable from History.")
                        .into(),
                    "Delete source",
                )
            }
            _ => return None,
        };
        Some(PendingConfirm {
            command: Some(command),
            event: event.clone(),
            title: title.into(),
            message,
            accept_label: accept_label.into(),
            cancel_label: None,
            history: None,
        })
    }

    pub fn route(&mut self, event: ComponentEvent) -> Option<BackendCommand> {
        let command = match event {
            ComponentEvent::OpenSettings => {
                self.active_modal = ActiveModal::Settings;
                return None;
            }
            ComponentEvent::OpenOfficialWebsite => {
                // Plan §24: clicking the brand asks first; the browser only
                // opens on an explicit Yes.
                if self.pending_confirm.is_some() {
                    return None;
                }
                self.pending_confirm = Some(PendingConfirm {
                    command: None,
                    event: event.clone(),
                    title: "Open official website?".into(),
                    message: format!("Open {OFFICIAL_WEBSITE_URL} in your browser?").into(),
                    accept_label: "Yes".into(),
                    cancel_label: Some("No".into()),
                    history: None,
                });
                self.active_modal = ActiveModal::Confirm;
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
            // Picker requests never reach this arm: the Workspace intercepts
            // them in `route_event` before `route` (they carry no modal or
            // backend state). Kept here only for exhaustiveness.
            ComponentEvent::OpenColorPicker { .. }
            | ComponentEvent::OpenFontPicker { .. }
            | ComponentEvent::OpenDropdown { .. } => {
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
        self.select_options = snapshot.select_options.clone();
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
            MixerEvent::ContextMenuRequested { id, position: _ } => {
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
            TransitionEvent::Apply {
                kind,
                duration_ms,
                params,
            } => {
                vec![
                    BackendCommand::SetTransitionKind(kind),
                    BackendCommand::SetTransitionDuration(duration_ms),
                    BackendCommand::SetTransitionParams(params),
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

/// Official product page opened after the brand click is confirmed
/// (plan §24).
pub const OFFICIAL_WEBSITE_URL: &str = "https://github.com/revoproject/revo-stream";

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
    pub color_picker: Entity<ColorPickerPopup>,
    pub font_picker: Entity<FontPickerPopup>,
    pub dropdown_picker: Entity<DropdownPickerPopup>,
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
    /// Basic options captured in the add-source modal; the next snapshot
    /// resolves the created source's id and applies them via UpdateSource.
    pending_source_params: Option<(String, std::collections::BTreeMap<String, String>)>,
    /// The interface style already applied from `ui_profile` (Svelte Look
    /// section); `None` until the first snapshot carries the profile.
    applied_theme_style: Option<String>,
    /// Source settings fetched once per source when the first snapshots
    /// arrive and kept warm so the editor and quick actions open with an
    /// instant prefill (every open still revalidates in the background).
    source_settings_cache: std::collections::HashMap<String, revo_core::models::SourceSettingsData>,
    /// Source ids whose settings warm-up has already been requested.
    settings_warmed: std::collections::HashSet<String>,
    /// Draggable panel-band height (px); the preview band takes the rest.
    /// Grabbing the divider above Scenes/Sources/Tools resizes it.
    panel_band_height: f32,
    /// Y position where the divider drag started; `None` while not dragging.
    divider_drag_start: Option<f32>,
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
            thumbnails: Default::default(),
            thumbnail_cursor: 0,
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
        let color_picker = cx.new(|_| ColorPickerPopup {
            theme,
            ..ColorPickerPopup::default()
        });
        let font_picker = cx.new(|cx| FontPickerPopup::new(theme, cx));
        let dropdown_picker = cx.new(|_| DropdownPickerPopup {
            theme,
            ..DropdownPickerPopup::default()
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
            cx.subscribe(
                &tools_transitions,
                |this, _, event: &TransitionEvent, cx| {
                    this.handle_transition_event(event.clone(), cx);
                },
            ),
            cx.subscribe(&tools_transitions, |this, _, event: &ComponentEvent, cx| {
                if matches!(
                    event,
                    ComponentEvent::OpenColorPicker { .. }
                        | ComponentEvent::OpenFontPicker { .. }
                        | ComponentEvent::OpenDropdown { .. }
                ) {
                    this.route_event(event.clone(), cx);
                }
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
            cx.subscribe(&color_picker, |this, _, event, cx| {
                this.handle_color_picker_event(event.clone(), cx);
            }),
            cx.subscribe(&font_picker, |this, _, event, cx| {
                this.handle_font_picker_event(event.clone(), cx);
            }),
            cx.subscribe(&dropdown_picker, |this, _, event, cx| {
                this.handle_dropdown_picker_event(event.clone(), cx);
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
            color_picker,
            font_picker,
            dropdown_picker,
            settings_modal: None,
            plugins_modal: None,
            add_scene_modal: None,
            add_source_modal: None,
            quick_action_modal: None,
            source_properties_modal: None,
            pending_properties_source: None,
            pending_history_entry: None,
            pending_source_params: None,
            applied_theme_style: None,
            source_settings_cache: std::collections::HashMap::new(),
            settings_warmed: std::collections::HashSet::new(),
            panel_band_height: 240.0,
            divider_drag_start: None,
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
            mixer.sync_from_snapshot(
                &snapshot.source_items,
                &snapshot.source_volumes,
                &snapshot.muted_sources,
                &snapshot.volume_locked_sources,
                &snapshot.audio_levels,
            );
            cx.notify();
        });
        self.header.update(cx, |header, cx| {
            header.state.recording = snapshot.recording;
            header.state.streaming = snapshot.streaming;
            header.state.studio_mode = snapshot.studio_mode;
            cx.notify();
        });
        self.state.apply_snapshot(&snapshot);
        // Keep an open Settings modal on the live catalog (plan §6–8).
        if let Some(modal) = self.settings_modal.clone() {
            modal.update(cx, |modal, cx| {
                modal.set_select_options(snapshot.select_options.clone(), cx);
            });
        }
        self.warm_source_settings(&snapshot, cx);
        self.resolve_pending_properties(&snapshot, cx);
        self.apply_theme_style(&snapshot, cx);
        cx.notify();
    }

    /// Warms the source-settings cache once per source (settings are loaded
    /// at startup instead of paying a cold engine round-trip on first open).
    /// Ids with no settings are dropped from the warmed set so a later
    /// snapshot retries them; history restores invalidate everything.
    fn warm_source_settings(&mut self, snapshot: &BackendSnapshot, cx: &mut Context<'_, Self>) {
        for item in &snapshot.source_items {
            if !self.settings_warmed.insert(item.id.clone()) {
                continue;
            }
            let backend = self.backend.clone();
            let id = item.id.clone();
            cx.spawn(async move |workspace, cx| {
                let fetch_id = id.clone();
                let data = cx
                    .background_executor()
                    .spawn(
                        async move { crate::services::fetch_source_settings(&backend, &fetch_id) },
                    )
                    .await;
                let _ = workspace.update(cx, |this, _| match data {
                    Some(data) => {
                        this.source_settings_cache.insert(id, data);
                    }
                    None => {
                        this.settings_warmed.remove(&id);
                    }
                });
            })
            .detach();
        }
    }

    /// Clicking a history row replays every step up to it in one batch.
    fn restore_history(&mut self, undo: bool, depth: usize, cx: &mut Context<'_, Self>) {
        let result = if undo {
            self.state
                .history
                .restore_to_undo(self.state.history.undo_stack.len() - depth)
        } else {
            self.state
                .history
                .restore_to_redo(self.state.history.redo_stack.len() - depth)
        };
        match result {
            Some((label, commands)) => {
                self.state.status = label;
                // Any source's settings may change; re-warm after the next
                // snapshot lands.
                self.source_settings_cache.clear();
                self.settings_warmed.clear();
                self.dispatch(commands, cx);
            }
            None => {
                self.state.status = "History entry no longer available".into();
                cx.notify();
            }
        }
    }

    /// The General-section OBS status line (Svelte parity).
    fn obs_status_text(&self) -> String {
        if self.state.booted {
            "Running OBS (revo-lib)".into()
        } else {
            "Waiting for Connect".into()
        }
    }

    /// Applies the saved interface style (Svelte Look section) to the shell
    /// and the live panels; theme objects on views are plain fields, so the
    /// switch is a re-seed.
    fn apply_theme_style(&mut self, snapshot: &BackendSnapshot, cx: &mut Context<'_, Self>) {
        let Some(profile) = snapshot
            .settings
            .as_ref()
            .and_then(|s| s.ui_profile.as_ref())
        else {
            return;
        };
        let Some(style) = profile.get("interface_style").and_then(|v| v.as_str()) else {
            return;
        };
        let wants_classic = style.eq_ignore_ascii_case("classic");
        if self.applied_theme_style.as_deref() == Some(style) {
            return;
        }
        self.applied_theme_style = Some(style.to_owned());
        let theme = if wants_classic {
            RevoTheme::classic()
        } else {
            RevoTheme::future()
        };
        self.apply_theme_to_views(theme, cx);
    }

    /// Re-seeds the shared palette on every view that renders with it
    /// (plan §4.1): the workspace `self.theme` is the single source of
    /// truth, so panels, popups and any open modal switch palettes together
    /// instead of keeping the palette they were constructed with.
    fn apply_theme_to_views(&mut self, theme: RevoTheme, cx: &mut Context<'_, Self>) {
        self.theme = theme;
        macro_rules! seed {
            ($entity:expr) => {
                $entity.update(cx, |view, cx| {
                    view.theme = theme;
                    cx.notify();
                });
            };
        }
        seed!(&self.header);
        seed!(&self.scenes);
        seed!(&self.sources);
        seed!(&self.tools_mixer);
        seed!(&self.tools_transitions);
        seed!(&self.mixer_menu);
        seed!(&self.mixer_advanced);
        seed!(&self.planner);
        seed!(&self.context_menu);
        seed!(&self.color_picker);
        seed!(&self.font_picker);
        seed!(&self.dropdown_picker);
        if let Some(modal) = &self.settings_modal {
            seed!(modal);
        }
        if let Some(modal) = &self.plugins_modal {
            seed!(modal);
        }
        if let Some(modal) = &self.add_scene_modal {
            seed!(modal);
        }
        if let Some(modal) = &self.add_source_modal {
            seed!(modal);
        }
        if let Some(modal) = &self.quick_action_modal {
            seed!(modal);
        }
        if let Some(modal) = &self.source_properties_modal {
            seed!(modal);
        }
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
        // The same resolution finalizes the deferred undo entry and applies
        // the add-source modal's basic options.
        if let Some(mut entry) = self.pending_history_entry.take() {
            if entry.undo.is_empty() {
                if let Some(id) = &match_id {
                    entry.undo = vec![BackendCommand::RemoveSource(id.clone())];
                    self.state.history.record(entry);
                }
            }
        }
        if let Some((_pending_name, options)) = self.pending_source_params.take() {
            if let Some(id) = &match_id {
                self.dispatch(
                    [BackendCommand::UpdateSource {
                        update: revo_core::models::SourceUpdate {
                            id: id.clone(),
                            name: String::new(),
                            source_type: String::new(),
                            params: options
                                .iter()
                                .map(|(key, value)| (key.clone(), value.clone()))
                                .collect(),
                        },
                    }],
                    cx,
                );
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
        let _backend = self.backend.clone();
        let mut tick: u64 = 0;
        cx.spawn(async move |workspace, cx| loop {
            tick = tick.wrapping_add(1);
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
                    // Plan §16: keep the last good frame when a capture
                    // misses; assigning None on every failed tick produced
                    // the per-refresh blink.
                    if program.is_some() {
                        workspace.preview_frame = program;
                    }
                    if studio_preview.is_some() {
                        workspace.studio_preview_frame = studio_preview;
                    }
                    cx.notify();
                })
                .is_ok();
            if !alive {
                return;
            }
            // Scene thumbnails: one slow round-robin capture per cycle so the
            // panel previews never contend with the live preview loop.
            if tick % SCENE_THUMBNAIL_EVERY_TICKS == 0 {
                let (backend, scene) = match workspace.update(cx, |workspace, cx| {
                    let target = workspace
                        .scenes
                        .update(cx, |panel, _| panel.next_thumbnail_target());
                    (workspace.backend.clone(), target)
                }) {
                    Ok((backend, Some(scene))) => (backend, scene),
                    _ => continue,
                };
                let scene_name = scene.clone();
                let png = cx
                    .background_executor()
                    .spawn(async move {
                        crate::services::obs::capture_source_png(&backend, &scene_name).ok()
                    })
                    .await;
                let Some(png) = png else {
                    continue;
                };
                let image = std::sync::Arc::new(GpuiImage::from_bytes(ImageFormat::Png, png));
                let _ = workspace.update(cx, |workspace, cx| {
                    workspace.scenes.update(cx, |panel, cx| {
                        panel.set_thumbnail(&scene, image);
                        cx.notify();
                    });
                });
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
            MixerEvent::ContextMenuRequested { id, position } => {
                self.mixer_menu.update(cx, |menu, cx| {
                    menu.open_menu(&id, position);
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
        // Plan §12: gated lifecycle events open the confirmation dialog
        // BEFORE the optimistic header flip — nothing may look "live" while
        // the dialog waits. Accepting dispatches the held command.
        if let Some(mut pending) = self.state.confirm_request(&event) {
            // History must be computed from the pre-dispatch snapshot, like
            // the ungated path does further below.
            pending.history = self.history_entry_for(&event, cx);
            self.state.pending_confirm = Some(pending);
            self.state.active_modal = ActiveModal::Confirm;
            cx.notify();
            return;
        }
        self.update_header_flags(&event, cx);
        // Popup pickers open before the state router: they carry no modal or
        // backend state, and their replies target the host component
        // directly.
        match &event {
            ComponentEvent::OpenColorPicker {
                field,
                current,
                position,
            } => {
                self.color_picker.update(cx, |picker, cx| {
                    picker.open(field.clone(), current.clone(), *position);
                    cx.notify();
                });
                cx.notify();
                return;
            }
            ComponentEvent::OpenFontPicker {
                field,
                current,
                position,
            } => {
                let fonts_loaded = self.font_picker.read(cx).fonts_loaded;
                self.font_picker.update(cx, |picker, cx| {
                    picker.open(field.clone(), current.clone(), *position);
                    cx.notify();
                });
                if !fonts_loaded {
                    self.spawn_font_fetch(cx);
                }
                cx.notify();
                return;
            }
            ComponentEvent::OpenDropdown {
                field,
                options,
                current,
                position,
            } => {
                self.dropdown_picker.update(cx, |picker, cx| {
                    picker.open(field.clone(), options.clone(), current.clone(), *position);
                    cx.notify();
                });
                cx.notify();
                return;
            }
            _ => {}
        }
        // Undo/redo never reach the backend stubs; they replay the
        // workspace-side history log instead (Svelte parity).
        match &event {
            ComponentEvent::Undo => {
                if let Some((label, commands)) = self.state.history.undo() {
                    self.state.status = format!("Undo: {label}");
                    self.dispatch(commands, cx);
                    // Plan §15.3: undo may rename/rebuild sources; drop the
                    // warmed property caches so panels re-seed from fresh
                    // engine state instead of stale drafts.
                    self.source_settings_cache.clear();
                    self.settings_warmed.clear();
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
                    self.source_settings_cache.clear();
                    self.settings_warmed.clear();
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
        if self.state.active_modal == ActiveModal::Settings {
            if let Some(entity) = &self.settings_modal {
                let status = self.obs_status_text();
                entity.update(cx, |modal, cx| {
                    if modal.obs_status != status {
                        modal.obs_status = status;
                        cx.notify();
                    }
                });
            }
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

    /// Carries a confirmed picker color back to the host component by its
    /// [`ColorField`] key; every host exposes a small typed setter.
    fn handle_color_picker_event(&mut self, event: ColorPickerEvent, cx: &mut Context<'_, Self>) {
        let ColorPickerEvent::Picked { field, color } = event else {
            return;
        };
        match field {
            crate::actions::ColorField::QuickAction { source_id } => {
                if let Some(entity) = &self.quick_action_modal {
                    let matches = entity.read(cx).source_id == source_id;
                    if matches {
                        entity.update(cx, |modal, cx| modal.apply_color(&color, cx));
                    }
                }
            }
            crate::actions::ColorField::SourceParam { source_id, key } => {
                if let Some(entity) = &self.source_properties_modal {
                    if entity.read(cx).source_id == source_id {
                        entity.update(cx, |modal, cx| modal.set_param_value(&key, &color, cx));
                    }
                }
            }
            crate::actions::ColorField::FilterParam {
                source_id,
                filter_name,
                key,
            } => {
                if let Some(entity) = &self.source_properties_modal {
                    if entity.read(cx).source_id == source_id {
                        entity.update(cx, |modal, cx| {
                            modal.set_filter_param(&filter_name, &key, &color, cx)
                        });
                    }
                }
            }
            crate::actions::ColorField::TransitionParam { transition_id, key } => {
                self.tools_transitions.update(cx, |panel, cx| {
                    panel.update_transition_param(&transition_id, &key, color.clone());
                    cx.notify();
                });
            }
            crate::actions::ColorField::AddSource { key } => {
                if let Some(entity) = &self.add_source_modal {
                    entity.update(cx, |modal, cx| modal.set_option_value(&key, &color, cx));
                }
            }
        }
        cx.notify();
    }

    /// Carries a picked font family back to the host component by its
    /// [`FontField`] key.
    fn handle_font_picker_event(&mut self, event: FontPickerEvent, cx: &mut Context<'_, Self>) {
        let FontPickerEvent::Picked { field, font } = event else {
            return;
        };
        match field {
            crate::actions::FontField::SourceFont { source_id } => {
                if let Some(entity) = &self.source_properties_modal {
                    if entity.read(cx).source_id == source_id {
                        entity.update(cx, |modal, cx| {
                            modal.set_param_value("font_face", &font, cx)
                        });
                    }
                }
            }
            crate::actions::FontField::AddSource => {
                if let Some(entity) = &self.add_source_modal {
                    entity.update(cx, |modal, cx| {
                        modal.set_option_value("font_face", &font, cx)
                    });
                }
            }
        }
        cx.notify();
    }

    /// Carries a picked dropdown value back to the host component by its
    /// [`DropdownField`] key.
    fn handle_dropdown_picker_event(
        &mut self,
        event: DropdownPickerEvent,
        cx: &mut Context<'_, Self>,
    ) {
        let DropdownPickerEvent::Picked { field, value } = event else {
            return;
        };
        match field {
            DropdownField::SourceParam { source_id, key } => {
                if let Some(entity) = &self.source_properties_modal {
                    if entity.read(cx).source_id == source_id {
                        entity.update(cx, |modal, cx| modal.set_param_value(&key, &value, cx));
                    }
                }
            }
            DropdownField::FilterParam {
                source_id,
                filter_name,
                key,
            } => {
                if let Some(entity) = &self.source_properties_modal {
                    if entity.read(cx).source_id == source_id {
                        entity.update(cx, |modal, cx| {
                            modal.set_filter_param(&filter_name, &key, &value, cx)
                        });
                    }
                }
            }
            DropdownField::AddSourceOption { key } => {
                if let Some(entity) = &self.add_source_modal {
                    entity.update(cx, |modal, cx| modal.set_option_value(&key, &value, cx));
                }
            }
            DropdownField::AddFilterKind { source_id } => {
                if let Some(entity) = &self.source_properties_modal {
                    if entity.read(cx).source_id == source_id {
                        entity.update(cx, |modal, cx| modal.set_new_filter_kind(&value, cx));
                    }
                }
            }
            DropdownField::TransitionKind { transition_id } => {
                if transition_id == "__new__" {
                    if let Some(kind) =
                        crate::components::modules::transitions::TransitionKind::from_value(&value)
                    {
                        self.tools_transitions.update(cx, |panel, cx| {
                            panel.set_new_kind(kind);
                            cx.notify();
                        });
                    }
                } else {
                    self.tools_transitions.update(cx, |panel, cx| {
                        panel.set_transition_kind_value(&transition_id, &value, cx);
                    });
                }
            }
            DropdownField::TransitionParam { transition_id, key } => {
                self.tools_transitions.update(cx, |panel, cx| {
                    panel.update_transition_param(&transition_id, &key, value.clone());
                    cx.notify();
                });
            }
        }
        cx.notify();
    }

    /// One background engine round trip enumerating system fonts; mock
    /// builds resolve the static list immediately. Repeat opens reuse the
    /// cached list, so this runs once per session.
    fn spawn_font_fetch(&mut self, cx: &mut Context<'_, Self>) {
        let backend = self.backend.clone();
        cx.spawn(async move |workspace, cx| {
            let fonts = cx
                .background_executor()
                .spawn(async move { crate::services::fetch_system_fonts(&backend) })
                .await;
            let _ = workspace.update(cx, |this, cx| {
                this.font_picker.update(cx, |picker, cx| {
                    picker.load_fonts(fonts);
                    cx.notify();
                });
            });
        })
        .detach();
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
        let obs_status = self.obs_status_text();
        let catalog = self.state.select_options.clone();
        let theme = self.theme;
        let entity = cx.new(|cx| {
            let mut modal = SettingsModal::with_status(settings, obs_status, theme, cx);
            // Seed the engine catalog so encoders/devices/resolutions render
            // as real dropdowns from the first frame (plan §6–8).
            modal.set_select_options(catalog, cx);
            modal
        });
        let subscription = cx.subscribe(&entity, |this, _, event, cx| match event {
            SettingsEvent::Saved(settings) => {
                this.state.settings = Some(settings.clone());
                if let Err(error) = this.backend.try_dispatch(BackendCommand::SaveSettings {
                    settings: settings.clone(),
                }) {
                    this.state.status = format!("Backend error: {error}");
                }
                // Re-read the live catalogs (modules/settings may have
                // changed) before pulling the next snapshot.
                let _ = this
                    .backend
                    .try_dispatch(BackendCommand::RefreshSelectOptions);
                this.close_settings_modal(cx);
                this.refresh_snapshot(cx);
            }
            SettingsEvent::Cancelled => this.close_settings_modal(cx),
            SettingsEvent::ExportRevoSceneCollection { path } => {
                if let Err(error) = this
                    .backend
                    .try_dispatch(BackendCommand::ExportRevoSceneCollection { path: path.clone() })
                {
                    this.state.status = format!("Backend error: {error}");
                }
            }
            SettingsEvent::ImportRevoSceneCollection { path } => {
                if let Err(error) = this
                    .backend
                    .try_dispatch(BackendCommand::ImportRevoSceneCollection { path: path.clone() })
                {
                    this.state.status = format!("Backend error: {error}");
                }
            }
            SettingsEvent::ExportObsSceneCollection { path } => {
                if let Err(error) = this
                    .backend
                    .try_dispatch(BackendCommand::ExportObsSceneCollection { path: path.clone() })
                {
                    this.state.status = format!("Backend error: {error}");
                }
            }
            SettingsEvent::ImportObsSceneCollection { path } => {
                if let Err(error) = this
                    .backend
                    .try_dispatch(BackendCommand::ImportObsSceneCollection { path: path.clone() })
                {
                    this.state.status = format!("Backend error: {error}");
                }
            }
            SettingsEvent::ExtractDefaultTheme { root_dir } => {
                if let Err(error) = this
                    .backend
                    .try_dispatch(BackendCommand::ExtractDefaultTheme {
                        root_dir: root_dir.clone(),
                    })
                {
                    this.state.status = format!("Backend error: {error}");
                } else {
                    this.state.status = format!("Default theme extracted under {root_dir}");
                }
            }
            SettingsEvent::ExportProfile { path } => {
                if let Err(error) = this
                    .backend
                    .try_dispatch(BackendCommand::ExportProfile { path: path.clone() })
                {
                    this.state.status = format!("Backend error: {error}");
                } else {
                    this.state.status = format!("Profile exported to {path}");
                }
            }
            SettingsEvent::ImportProfile { path } => {
                if let Err(error) = this
                    .backend
                    .try_dispatch(BackendCommand::ImportProfile { path: path.clone() })
                {
                    this.state.status = format!("Backend error: {error}");
                } else {
                    this.state.status = format!("Profile imported from {path}");
                }
            }
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
        let theme = self.theme;
        let entity = cx.new(|cx| PluginsModal::new(draft, theme, cx));
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
        let theme = self.theme;
        let entity = cx.new(|cx| AddSceneModal::new(existing_names, theme, cx));
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
        let theme = self.theme;
        let entity = cx.new(|cx| {
            AddSourceModal::new(
                self.state.available_source_types.clone(),
                existing_names,
                theme,
                cx,
            )
        });
        let subscription = cx.subscribe(&entity, |this, _, event, cx| match event {
            AddSourceEvent::Created {
                name,
                source_type,
                options,
            } => {
                this.state.status = format!("Source '{name}' created");
                if let Err(error) = this.backend.try_dispatch(BackendCommand::AddSource {
                    name: name.clone(),
                    source_type: source_type.clone(),
                }) {
                    this.state.status = format!("Backend error: {error}");
                }
                // Basic per-type options apply right after creation (Svelte
                // parity); the properties window does NOT open anymore — the
                // next snapshot resolves the created source's id.
                if !options.is_empty() {
                    this.pending_source_params = Some((name.clone(), options.clone()));
                }
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
        // The modal also emits shared ComponentEvents (color/font picker
        // opens); route them through the workspace router.
        let picker_subscription = cx.subscribe(&entity, |this, _, event, cx| {
            if matches!(
                event,
                ComponentEvent::OpenColorPicker { .. }
                    | ComponentEvent::OpenFontPicker { .. }
                    | ComponentEvent::OpenDropdown { .. }
            ) {
                this.route_event(event.clone(), cx);
            }
        });
        self._subscriptions.push(picker_subscription);
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
        self.open_source_properties_modal_inner(source_id, editor, cx);
        if editor {
            // A warmed cache (fetched at startup) fills the editor instantly;
            // the background fetch below revalidates it.
            if let Some(cached) = self.source_settings_cache.get(source_id).cloned() {
                if let Some(entity) = self.source_properties_modal.clone() {
                    entity.update(cx, |modal, cx| modal.load_settings(cached, cx));
                }
            }
            // Fetch the filter list too for the Filters tab.
            let filter_backend = self.backend.clone();
            let filter_source = source_id.to_owned();
            cx.spawn(async move |workspace, cx| {
                let filters = cx
                    .background_executor()
                    .spawn(async move {
                        crate::services::fetch_source_filters(&filter_backend, &filter_source)
                    })
                    .await;
                let _ = workspace.update(cx, |this, cx| {
                    if let Some(entity) = this.source_properties_modal.clone() {
                        entity.update(cx, |modal, cx| modal.load_filters(filters, cx));
                    }
                });
            })
            .detach();
            // Fetch the engine's full settings (typed property specs) and
            // seed the editor; mock builds keep the transform-only view.
            let backend = self.backend.clone();
            let source_id = source_id.to_owned();
            let cache_id = source_id.clone();
            cx.spawn(async move |workspace, cx| {
                let settings = cx
                    .background_executor()
                    .spawn(
                        async move { crate::services::fetch_source_settings(&backend, &source_id) },
                    )
                    .await;
                let _ = workspace.update(cx, |this, cx| {
                    if let Some(data) = &settings {
                        this.source_settings_cache.insert(cache_id, data.clone());
                    }
                    if let Some(entity) = this.source_properties_modal.clone() {
                        entity.update(cx, |modal, cx| modal.refresh_settings(settings, cx));
                    }
                });
            })
            .detach();
        }
    }

    fn open_source_properties_modal_inner(
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
        let theme = self.theme;
        let entity = cx.new(|cx| {
            if editor {
                SourcePropertiesModal::new_editor(
                    item.id.clone(),
                    item.name.clone(),
                    item.source_type.clone(),
                    transform,
                    canvas,
                    theme,
                    cx,
                )
            } else {
                SourcePropertiesModal::new(
                    item.id.clone(),
                    item.name.clone(),
                    item.source_type.clone(),
                    transform,
                    canvas,
                    theme,
                    cx,
                )
            }
        });
        let subscription = cx.subscribe(&entity, |this, _, event: &SourcePropertiesEvent, cx| {
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
                SourcePropertiesEvent::AppliedFull {
                    id,
                    name,
                    params,
                    previous_params,
                    transform,
                    previous_transform,
                } => {
                    if let Some(previous) = previous_transform {
                        this.state.history.record(crate::history::HistoryEntry {
                            label: format!("Transform '{id}'"),
                            undo: vec![BackendCommand::SetSourceTransform {
                                id: id.clone(),
                                x: previous[0],
                                y: previous[1],
                                width: previous[2],
                                height: previous[3],
                                rotation: previous[4],
                            }],
                            redo: vec![BackendCommand::SetSourceTransform {
                                id: id.clone(),
                                x: transform[0],
                                y: transform[1],
                                width: transform[2],
                                height: transform[3],
                                rotation: transform[4],
                            }],
                        });
                    }
                    if !previous_params.is_empty() {
                        let to_map = |entries: &[(String, String)]| {
                            entries
                                .iter()
                                .cloned()
                                .collect::<std::collections::HashMap<_, _>>()
                        };
                        this.state.history.record(crate::history::HistoryEntry {
                            label: format!("Edit properties of '{id}'"),
                            undo: vec![BackendCommand::UpdateSource {
                                update: revo_core::models::SourceUpdate {
                                    id: id.clone(),
                                    name: String::new(),
                                    source_type: String::new(),
                                    params: to_map(&previous_params),
                                },
                            }],
                            redo: vec![BackendCommand::UpdateSource {
                                update: revo_core::models::SourceUpdate {
                                    id: id.clone(),
                                    name: String::new(),
                                    source_type: String::new(),
                                    params: to_map(&params),
                                },
                            }],
                        });
                    }
                    this.state.status = format!("Source '{name}' updated");
                    this.source_settings_cache.remove(&id);
                    this.dispatch(
                        [
                            BackendCommand::UpdateSource {
                                update: revo_core::models::SourceUpdate {
                                    id: id.clone(),
                                    name,
                                    source_type: String::new(),
                                    params: params.iter().cloned().collect(),
                                },
                            },
                            BackendCommand::SetSourceTransform {
                                id,
                                x: transform[0],
                                y: transform[1],
                                width: transform[2],
                                height: transform[3],
                                rotation: transform[4],
                            },
                        ],
                        cx,
                    );
                }
                SourcePropertiesEvent::AppliedFilters {
                    id,
                    filters,
                    previous,
                } => {
                    if !previous.is_empty() {
                        this.state.history.record(crate::history::HistoryEntry {
                            label: format!("Edit filters of '{id}'"),
                            undo: vec![BackendCommand::SetSourceFilters {
                                id: id.clone(),
                                filters: previous.clone(),
                            }],
                            redo: vec![BackendCommand::SetSourceFilters {
                                id: id.clone(),
                                filters: filters.clone(),
                            }],
                        });
                    }
                    this.state.status = format!("Filters updated ({})", filters.len());
                    this.dispatch([BackendCommand::SetSourceFilters { id, filters }], cx);
                }
                SourcePropertiesEvent::OpenEditor { id } => {
                    // Route first: the editor replaces the properties entity,
                    // so nothing is dropped while its own event is emitting.
                    this.route_event(ComponentEvent::EditSource(id), cx);
                }
                SourcePropertiesEvent::FetchFilterSettings { filter_name, kind } => {
                    // One background round trip for the expanded filter's
                    // typed settings; mock builds resolve to None and the
                    // accordion reports them unavailable.
                    let fetch_source = this
                        .source_properties_modal
                        .as_ref()
                        .map(|entity| entity.read(cx).source_id.clone())
                        .unwrap_or_default();
                    let (name, kind) = (filter_name.clone(), kind);
                    let fetch_name = name.clone();
                    let backend = this.backend.clone();
                    cx.spawn(async move |workspace, cx| {
                        let settings = cx
                            .background_executor()
                            .spawn(async move {
                                crate::services::fetch_filter_settings(
                                    &backend,
                                    &fetch_source,
                                    &fetch_name,
                                    &kind,
                                )
                            })
                            .await;
                        let _ = workspace.update(cx, |this, cx| {
                            if let Some(entity) = this.source_properties_modal.clone() {
                                entity.update(cx, |modal, cx| {
                                    modal.load_filter_settings(name, settings, cx)
                                });
                            }
                        });
                    })
                    .detach();
                }
                SourcePropertiesEvent::Cancelled => this.close_source_properties_modal(cx),
            }
        });
        self._subscriptions.push(subscription);
        // The modal also emits shared ComponentEvents (color/font picker
        // opens); route them through the workspace router.
        let picker_subscription = cx.subscribe(&entity, |this, _, event, cx| {
            if matches!(
                event,
                ComponentEvent::OpenColorPicker { .. }
                    | ComponentEvent::OpenFontPicker { .. }
                    | ComponentEvent::OpenDropdown { .. }
            ) {
                this.route_event(event.clone(), cx);
            }
        });
        self._subscriptions.push(picker_subscription);
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
        let theme = self.theme;
        let entity = cx
            .new(|cx| QuickActionModal::new(item.id.clone(), item.name.clone(), action, theme, cx));
        if action == QuickSourceAction::SelectDevice {
            // Device drafts start from the backend catalog plus the source's
            // current selection (Svelte openQuickDeviceModal parity).
            if let Some(current) = self.source_settings_cache.get(&item.id).and_then(|data| {
                crate::services::device_param_keys(&item.source_type)
                    .iter()
                    .find_map(|key| data.params.get(*key).cloned())
            }) {
                if let Some(entity) = self.quick_action_modal.clone() {
                    entity.update(cx, |modal, cx| {
                        modal.prefill_devices(current, Vec::new(), cx)
                    });
                }
            }
            let backend = self.backend.clone();
            let fetch_id = item.id.clone();
            let fetch_type = item.source_type.clone();
            cx.spawn(async move |workspace, cx| {
                let (current, options) = cx
                    .background_executor()
                    .spawn(async move {
                        let keys = crate::services::device_param_keys(&fetch_type);
                        let current = crate::services::fetch_source_settings(&backend, &fetch_id)
                            .and_then(|data| {
                                keys.iter().find_map(|key| data.params.get(*key).cloned())
                            })
                            .unwrap_or_default();
                        let options =
                            crate::services::fetch_source_devices(&backend, &fetch_type, &fetch_id);
                        (current, options)
                    })
                    .await;
                let _ = workspace.update(cx, |this, cx| {
                    if let Some(entity) = this.quick_action_modal.clone() {
                        entity.update(cx, |modal, cx| {
                            modal.prefill_devices(current, options, cx);
                        });
                    }
                });
            })
            .detach();
        } else if let Some(key) = crate::components::quick_modal::param_key(action) {
            // Preload the current value so edits start from what the source
            // already shows — instantly from the warm cache, then revalidated.
            if let Some(current) = self
                .source_settings_cache
                .get(&item.id)
                .and_then(|data| data.params.get(key).cloned())
            {
                if let Some(entity) = self.quick_action_modal.clone() {
                    entity.update(cx, |modal, cx| modal.prefill(current, cx));
                }
            }
            let backend = self.backend.clone();
            let fetch_id = item.id.clone();
            let fetch_key = key.to_string();
            cx.spawn(async move |workspace, cx| {
                let current = cx
                    .background_executor()
                    .spawn(async move {
                        crate::services::fetch_source_settings(&backend, &fetch_id)
                            .and_then(|data| data.params.get(&fetch_key).cloned())
                    })
                    .await;
                let _ = workspace.update(cx, |this, cx| {
                    if let Some(entity) = this.quick_action_modal.clone() {
                        entity.update(cx, |modal, cx| {
                            modal.prefill(current.unwrap_or_default(), cx)
                        });
                    }
                });
            })
            .detach();
        }
        let subscription = cx.subscribe(&entity, |this, _, event: &QuickActionEvent, cx| {
            let event = event.clone();
            match event {
                QuickActionEvent::Applied { id, action, value } => {
                    let keys: Vec<String> = if action == QuickSourceAction::SelectDevice {
                        let source_type = this
                            .sources
                            .read(cx)
                            .sources
                            .iter()
                            .find(|source| source.id == id)
                            .map(|source| source.source_type.clone())
                            .unwrap_or_default();
                        crate::services::device_param_keys(&source_type)
                            .iter()
                            .map(|key| key.to_string())
                            .collect()
                    } else if let Some(key) = crate::components::quick_modal::param_key(action) {
                        vec![key.to_string()]
                    } else {
                        this.state.status = "Quick action has no settings key".into();
                        cx.notify();
                        return;
                    };
                    // Fetch the current settings first so the undo entry can
                    // restore the previous values (Svelte
                    // recordSourceSettingsHistory parity). Mock builds fetch
                    // nothing and apply without a history entry.
                    let (id, value) = (id.clone(), value.clone());
                    let backend = this.backend.clone();
                    cx.spawn(async move |workspace, cx| {
                        let (fetch_id, fetch_keys) = (id.clone(), keys.clone());
                        let previous = cx
                            .background_executor()
                            .spawn(async move {
                                crate::services::fetch_source_settings(&backend, &fetch_id)
                                    .map(|data| {
                                        fetch_keys
                                            .iter()
                                            .filter_map(|key| {
                                                data.params
                                                    .get(key.as_str())
                                                    .map(|value| (key.clone(), value.clone()))
                                            })
                                            .collect::<Vec<(String, String)>>()
                                    })
                                    .unwrap_or_default()
                            })
                            .await;
                        let _ = workspace.update(cx, |this, cx| {
                            this.apply_quick_action_params(id, keys, value, previous, cx);
                        });
                    })
                    .detach();
                }
                QuickActionEvent::Interact { id: _ } => {
                    this.close_quick_action_modal(cx);
                    this.state.requested_window = Some(AuxiliaryWindowKind::BrowserInteraction);
                }
                QuickActionEvent::Cancelled => this.close_quick_action_modal(cx),
            }
        });
        self._subscriptions.push(subscription);
        // The modal also emits shared ComponentEvents (color-picker opens);
        // route them through the workspace router.
        let picker_subscription = cx.subscribe(&entity, |this, _, event, cx| {
            if matches!(
                event,
                ComponentEvent::OpenColorPicker { .. }
                    | ComponentEvent::OpenFontPicker { .. }
                    | ComponentEvent::OpenDropdown { .. }
            ) {
                this.route_event(event.clone(), cx);
            }
        });
        self._subscriptions.push(picker_subscription);
        self.quick_action_modal = Some(entity);
        cx.notify();
    }

    /// Applies one quick-action settings edit across all target keys (device
    /// picks mirror `device`/`device_id` for audio sources) and records the
    /// undo entry when the previous values are known.
    fn apply_quick_action_params(
        &mut self,
        id: String,
        keys: Vec<String>,
        value: String,
        previous: Vec<(String, String)>,
        cx: &mut Context<'_, Self>,
    ) {
        self.source_settings_cache.remove(&id);
        let mut params = std::collections::HashMap::new();
        for key in &keys {
            params.insert(key.clone(), value.clone());
        }
        if !previous.is_empty() {
            let mut undo_params = std::collections::HashMap::new();
            for (key, value) in previous {
                undo_params.insert(key, value);
            }
            self.state.history.record(crate::history::HistoryEntry {
                label: format!("Edit {} of '{id}'", keys.join("/")),
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
                        params: params.clone(),
                    },
                }],
            });
        }
        self.state.status = format!("Quick action applied ({})", keys.join("/"));
        self.dispatch(
            [BackendCommand::UpdateSource {
                update: revo_core::models::SourceUpdate {
                    id,
                    name: String::new(),
                    source_type: String::new(),
                    params,
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
            ActiveModal::Confirm => {
                // Dismissal = cancel: drop the held command.
                self.state.pending_confirm = None;
                self.state.close_modal();
                cx.notify();
            }
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

    /// Accept on the confirmation dialog: dispatch the held command and run
    /// the same optimistic header flip the ungated path performs (plan §12).
    /// Opens a URL in the user's browser without ever waiting on the
    /// child process (foreground-executor rule, plan §24).
    fn open_external(&mut self, url: &str) {
        let spawned = if cfg!(target_os = "windows") {
            std::process::Command::new("cmd")
                .args(["/C", "start", "", url])
                .spawn()
        } else if cfg!(target_os = "macos") {
            std::process::Command::new("open").arg(url).spawn()
        } else {
            std::process::Command::new("xdg-open").arg(url).spawn()
        };
        self.state.status = match spawned {
            Ok(_) => format!("Opened {url} in the browser"),
            Err(_) => "No browser launcher is available on this system".into(),
        };
    }

    fn accept_pending_confirm(&mut self, cx: &mut Context<'_, Self>) {
        let Some(pending) = self.state.pending_confirm.take() else {
            return;
        };
        self.state.close_modal();
        let event = pending.event.clone();
        if let Some(command) = pending.command.clone() {
            self.dispatch([command], cx);
        }
        if matches!(event, ComponentEvent::OpenOfficialWebsite) {
            self.open_external(OFFICIAL_WEBSITE_URL);
        }
        if let Some(entry) = pending.history {
            self.state.history.record(entry);
        }
        self.update_header_flags(&event, cx);
        cx.notify();
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
            // Plan §22: the Preview and Debug Console window entries are
            // removed from the Tools menu; the auxiliary window code stays
            // reachable for the adapter/parity path.
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
    /// The drag handle above the Scenes/Sources/Tools band: dragging it
    /// resizes the band and the preview takes the remaining space.
    fn panel_divider(&self, cx: &mut Context<'_, Self>) -> gpui::Stateful<gpui::Div> {
        div()
            .id("panel-divider")
            .cursor_row_resize()
            .h(px(6.))
            .mx_6()
            .rounded_sm()
            .hover(|style| style.bg(self.theme.border_selected))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.divider_drag_start = Some(f32::from(event.position.y));
                    cx.notify();
                }),
            )
    }

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
            // The user-adjustable height (divider drag) replaces the fixed
            // per-mode heights; preview keeps the remaining space.
            LayoutMode::Wide => div().h(px(self.panel_band_height)),
            LayoutMode::Medium => div()
                .max_h(px(self.panel_band_height + 60.))
                .h(px(self.panel_band_height + 180.)),
            LayoutMode::Compact => div().h(px(self.panel_band_height + 320.)),
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
    ) -> Option<(ModalConfig, Vec<gpui::AnyElement>, Option<gpui::AnyElement>)> {
        match &self.state.active_modal {
            ActiveModal::None => None,
            ActiveModal::Settings => None,
            ActiveModal::Plugins => None,
            // The confirmation dialog owns a dedicated layer (custom
            // Accept/Cancel footer), not the generic content modal.
            ActiveModal::Confirm => None,
            ActiveModal::History => {
                // Interactive undo/redo history: one readable row per entry
                // with a divider between the stacks; clicking an entry
                // restores the state up to it.
                let mut rows = div().flex().flex_col().gap_1().min_w(px(420.));
                if self.state.history.undo_stack.is_empty()
                    && self.state.history.redo_stack.is_empty()
                {
                    rows = rows.child(
                        div()
                            .py_3()
                            .text_color(self.theme.text_muted)
                            .child("Nothing to undo yet."),
                    );
                }
                let undo_len = self.state.history.undo_stack.len();
                for (index, entry) in self.state.history.undo_stack.iter().enumerate().rev() {
                    let depth = undo_len - index;
                    rows = rows.child(
                        div()
                            .id(SharedString::from(format!("history-undo-{index}")))
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .py_2()
                            .rounded_md()
                            .bg(self.theme.surface_elevated)
                            .border_1()
                            .border_color(self.theme.border)
                            .hover(|style| style.bg(self.theme.surface_selected))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.restore_history(true, depth, cx);
                            }))
                            .child(
                                div()
                                    .px_1()
                                    .text_color(self.theme.text_muted)
                                    .text_size(px(11.))
                                    .child(format!("-{}", depth)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .truncate()
                                    .text_color(self.theme.text)
                                    .child(SharedString::from(entry.label.clone())),
                            ),
                    );
                }
                if !self.state.history.redo_stack.is_empty() {
                    rows = rows.child(div().my_1().h(px(1.)).w_full().bg(self.theme.border_strong));
                    let redo_len = self.state.history.redo_stack.len();
                    for (index, entry) in self.state.history.redo_stack.iter().enumerate().rev() {
                        let depth = redo_len - index;
                        rows = rows.child(
                            div()
                                .id(SharedString::from(format!("history-redo-{index}")))
                                .cursor_pointer()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .px_2()
                                .py_2()
                                .rounded_md()
                                .bg(self.theme.surface_elevated)
                                .border_1()
                                .border_color(self.theme.border)
                                .hover(|style| style.bg(self.theme.surface_selected))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.restore_history(false, depth, cx);
                                }))
                                .child(
                                    div()
                                        .px_1()
                                        .text_color(self.theme.text_muted)
                                        .text_size(px(11.))
                                        .child(format!("+{}", depth)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .truncate()
                                        .text_color(self.theme.text_muted)
                                        .child(SharedString::from(entry.label.clone())),
                                ),
                        );
                    }
                }
                let footer = self.modal_cancel_footer(cx);
                Some((
                    ModalConfig {
                        show_close: false,
                        ..ModalConfig::standard("Action History", ModalSize::Medium)
                    },
                    vec![rows.into_any_element()],
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
                    vec![div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child("Templates")
                        .child(
                            div()
                                .text_color(self.theme.text_muted)
                                .child("Template management will be available in future versions."),
                        )
                        .into_any_element()],
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

/// Scene thumbnails refresh one panel preview per 16 preview ticks
/// (~2.4 s round-robin) — deliberately slower than the live loop.
#[cfg(feature = "obs-backend")]
const SCENE_THUMBNAIL_EVERY_TICKS: u64 = 16;

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
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if let Some(start) = this.divider_drag_start {
                    // Incremental deltas keep the drag stable at fast moves.
                    let delta = f32::from(event.position.y) - start;
                    let max = (f32::from(window.viewport_size().height) - 280.0).max(160.0);
                    this.panel_band_height = (this.panel_band_height - delta).clamp(120.0, max);
                    this.divider_drag_start = Some(f32::from(event.position.y));
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.divider_drag_start.is_some() {
                        this.divider_drag_start = None;
                        cx.notify();
                    }
                }),
            )
            .font_family("Space Grotesk")
            .child(self.header.clone())
            .child(self.preview_surface(cx))
            .child(self.panel_divider(cx))
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
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(30.))
                        .h(px(30.))
                        .rounded_md()
                        .bg(self.theme.surface_active)
                        // Icon close (plan §2.3): hover turns the control
                        // danger-red so it reads as dismissal.
                        .hover(|style| style.bg(self.theme.danger))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.close_active_modal(cx);
                        }))
                        .child(crate::components::icon::icon(self.theme.text, "close", 14.)),
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
                            // Same exclusivity as the other modal layers: the
                            // panels behind an open Settings modal must never
                            // receive clicks.
                            .occlude()
                            .child(entity),
                    );
                }
            }
            ActiveModal::Confirm => {
                // Plan §12: hold the lifecycle command until an explicit
                // Accept; backdrop/Cancel/Esc all drop it.
                if let Some(pending) = self.state.pending_confirm.clone() {
                    let theme = self.theme;
                    let footer = div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap_2()
                        .p_3()
                        .child(
                            div()
                                .id("confirm-modal-cancel")
                                .cursor_pointer()
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .bg(theme.surface_active)
                                .text_color(theme.text)
                                .hover(|style| style.bg(theme.surface_selected))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.close_active_modal(cx);
                                }))
                                .child(
                                    pending
                                        .cancel_label
                                        .clone()
                                        .unwrap_or_else(|| "Cancel".into()),
                                ),
                        )
                        .child(
                            div()
                                .id("confirm-modal-accept")
                                .cursor_pointer()
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .bg(theme.accent)
                                .text_color(theme.text_strong)
                                .hover(|style| style.bg(theme.accent_hover))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.accept_pending_confirm(cx);
                                }))
                                .child(pending.accept_label.clone()),
                        )
                        .into_any_element();
                    let card = modal_card(
                        &ModalConfig {
                            show_close: false,
                            ..ModalConfig::confirmation(
                                pending.title.clone(),
                                pending.accept_label.clone(),
                            )
                        },
                        theme,
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(pending.message.clone()),
                        Some(footer),
                        None,
                    );
                    root = root
                        .child(
                            div()
                                .id("confirm-modal-backdrop")
                                .absolute()
                                .top_0()
                                .right_0()
                                .bottom_0()
                                .left_0()
                                .bg(self.theme.overlay)
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
                                .id("confirm-modal-layer")
                                .absolute()
                                .top_0()
                                .right_0()
                                .bottom_0()
                                .left_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(card),
                        );
                }
            }
            _ => {}
        }

        // The pickers are the topmost layers: they open over whichever
        // surface requested them (panels and full-window modals alike), so
        // they must paint after every modal layer above.
        if self.color_picker.read(cx).open {
            root = root.child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .left_0()
                    .occlude()
                    .child(self.color_picker.clone()),
            );
        }

        if self.font_picker.read(cx).open {
            root = root.child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .left_0()
                    .occlude()
                    .child(self.font_picker.clone()),
            );
        }

        if self.dropdown_picker.read(cx).open {
            root = root.child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .left_0()
                    .occlude()
                    .child(self.dropdown_picker.clone()),
            );
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
    use gpui::TestAppContext;
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
            params: Default::default(),
        });
        assert_eq!(
            commands,
            vec![
                BackendCommand::SetTransitionKind("fade".into()),
                BackendCommand::SetTransitionDuration(5),
                BackendCommand::SetTransitionParams(Default::default()),
            ]
        );
        assert_eq!(
            state.last_command,
            Some(BackendCommand::SetTransitionParams(Default::default()))
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

    #[test]
    fn confirm_toggles_gate_lifecycle_requests() {
        use serde_json::json;
        let mut state = WorkspaceState::default();
        // Unset toggles never gate (legacy direct dispatch).
        assert!(state
            .confirm_request(&ComponentEvent::StartStreaming)
            .is_none());
        state.settings = Some(revo_core::settings::default_settings());
        state.settings.as_mut().unwrap().ui_profile = Some(json!({
            "confirm_start_stream": "true",
            "confirm_stop_record": true,
        }));
        let pending = state
            .confirm_request(&ComponentEvent::StartStreaming)
            .expect("string toggle gates");
        assert_eq!(pending.command, Some(BackendCommand::StartStreaming));
        assert_eq!(pending.event, ComponentEvent::StartStreaming);
        // JSON bool form gates too; untouched lifecycle keeps dispatching.
        assert!(state
            .confirm_request(&ComponentEvent::StopRecording)
            .is_some());
        assert!(state
            .confirm_request(&ComponentEvent::StartRecording)
            .is_none());
        // Non-lifecycle events are never gated.
        assert!(state
            .confirm_request(&ComponentEvent::OpenSettings)
            .is_none());
        // Deletes stay ungated until confirm_delete is on (plan §15.4).
        assert!(state
            .confirm_request(&ComponentEvent::RemoveScene("A".into()))
            .is_none());
        state.settings.as_mut().unwrap().ui_profile = Some(json!({
            "confirm_start_stream": "true",
            "confirm_stop_record": true,
            "confirm_delete": "true",
        }));
        let delete = state
            .confirm_request(&ComponentEvent::RemoveScene("Studio".into()))
            .expect("delete gated");
        assert_eq!(
            delete.command,
            Some(BackendCommand::RemoveScene("Studio".into()))
        );
        assert!(delete.message.contains("Studio"));
        assert!(state
            .confirm_request(&ComponentEvent::RemoveSource("mic".into()))
            .is_some());
    }

    #[gpui::test]
    fn gated_start_streaming_holds_dispatch_until_accept(cx: &mut TestAppContext) {
        use serde_json::json;
        let (workspace, cx) = cx.add_window_view(|_window, cx| Workspace::new(cx));
        workspace.update(cx, |w, _| {
            w.state.settings = Some(revo_core::settings::default_settings());
            w.state.settings.as_mut().unwrap().ui_profile =
                Some(json!({ "confirm_start_stream": "true" }));
        });
        // Gated: no command reaches the backend, the dialog opens instead.
        workspace.update(cx, |w, cx| {
            w.route_event(ComponentEvent::StartStreaming, cx);
        });
        workspace.update(cx, |w, _| {
            assert_eq!(w.state.active_modal, ActiveModal::Confirm);
            assert!(w.state.pending_confirm.is_some());
            assert_eq!(w.state.last_command, None);
        });
        // Accept dispatches the held command exactly once and closes.
        workspace.update(cx, |w, cx| w.accept_pending_confirm(cx));
        workspace.update(cx, |w, _| {
            assert_eq!(w.state.active_modal, ActiveModal::None);
            assert!(w.state.pending_confirm.is_none());
            assert_eq!(w.state.last_command, Some(BackendCommand::StartStreaming));
        });
    }

    #[gpui::test]
    fn undo_restores_removed_scene_in_scenes_panel(cx: &mut TestAppContext) {
        // Plan §15.3: undo must re-dispatch and re-seed the panels from the
        // snapshot, not just update the status line.
        let (workspace, cx) = cx.add_window_view(|_window, cx| Workspace::new(cx));
        cx.run_until_parked();
        workspace.update(cx, |w, cx| {
            w.route_event(ComponentEvent::RemoveScene("Main Camera".into()), cx)
        });
        cx.run_until_parked();
        workspace.update(cx, |w, cx| {
            let names: Vec<String> = w
                .scenes
                .read(cx)
                .scenes
                .iter()
                .map(|s| s.name.clone())
                .collect();
            assert!(
                !names.iter().any(|n| n == "Main Camera"),
                "scene should be gone after remove: {names:?}"
            );
        });
        workspace.update(cx, |w, cx| w.route_event(ComponentEvent::Undo, cx));
        cx.run_until_parked();
        workspace.update(cx, |w, cx| {
            let names: Vec<String> = w
                .scenes
                .read(cx)
                .scenes
                .iter()
                .map(|s| s.name.clone())
                .collect();
            assert!(
                names.iter().any(|n| n == "Main Camera"),
                "undo must restore the scene in the panel: {names:?}"
            );
        });
    }

    #[gpui::test]
    fn cancelling_the_confirm_dialog_drops_the_pending_command(cx: &mut TestAppContext) {
        use serde_json::json;
        let (workspace, cx) = cx.add_window_view(|_window, cx| Workspace::new(cx));
        workspace.update(cx, |w, _| {
            w.state.settings = Some(revo_core::settings::default_settings());
            w.state.settings.as_mut().unwrap().ui_profile =
                Some(json!({ "confirm_start_record": "true" }));
        });
        workspace.update(cx, |w, cx| {
            w.route_event(ComponentEvent::StartRecording, cx);
        });
        workspace.update(cx, |w, cx| w.close_active_modal(cx));
        workspace.update(cx, |w, _| {
            assert_eq!(w.state.active_modal, ActiveModal::None);
            assert!(w.state.pending_confirm.is_none());
            assert_eq!(w.state.last_command, None);
        });
    }

    #[test]
    fn brand_click_asks_before_opening_the_website() {
        // Plan §24: no browser access without an explicit Yes.
        let mut state = WorkspaceState::default();
        state.route(ComponentEvent::OpenOfficialWebsite);
        assert_eq!(state.last_command, None);
        let pending = state.pending_confirm.clone().expect("confirm opens");
        assert_eq!(pending.command, None);
        assert_eq!(pending.accept_label.as_ref(), "Yes");
        assert_eq!(
            pending.cancel_label.as_deref().map(|l| l.as_ref()),
            Some("No")
        );
        assert!(pending.message.contains(OFFICIAL_WEBSITE_URL));
        assert_eq!(state.active_modal, ActiveModal::Confirm);
        // A second click while the dialog is up must not stack dialogs.
        state.route(ComponentEvent::OpenOfficialWebsite);
        assert_eq!(
            state.pending_confirm.as_ref().map(|p| p.title.clone()),
            Some(pending.title)
        );
    }

    #[gpui::test]
    fn classic_style_opens_modals_on_the_workspace_palette(cx: &mut TestAppContext) {
        // Plan §4.1: every modal must render with the app theme, not a
        // palette hard-coded in its constructor.
        use serde_json::json;
        let (workspace, cx) = cx.add_window_view(|_window, cx| Workspace::new(cx));
        let classic_snapshot = {
            let mut snapshot = revo_core::service::BackendSnapshot::default();
            snapshot.settings = Some({
                let mut settings = revo_core::settings::default_settings();
                settings.ui_profile = Some(json!({ "interface_style": "classic" }));
                settings
            });
            snapshot
        };
        workspace.update(cx, |w, cx| w.apply_theme_style(&classic_snapshot, cx));
        workspace.update(cx, |w, _| assert_eq!(w.theme, RevoTheme::classic()));
        // A modal opened afterwards inherits the classic palette.
        workspace.update(cx, |w, cx| w.open_settings_modal(cx));
        workspace.update(cx, |w, cx| {
            let modal = w.settings_modal.clone().expect("settings modal");
            assert_eq!(modal.read(cx).theme, RevoTheme::classic());
        });
        // A live switch back to future re-seeds the open modal too.
        let mut future_snapshot = classic_snapshot.clone();
        future_snapshot.settings.as_mut().unwrap().ui_profile =
            Some(json!({ "interface_style": "future" }));
        workspace.update(cx, |w, cx| w.apply_theme_style(&future_snapshot, cx));
        workspace.update(cx, |w, cx| {
            let modal = w.settings_modal.clone().expect("settings modal");
            assert_eq!(modal.read(cx).theme, RevoTheme::future());
        });
    }
}
