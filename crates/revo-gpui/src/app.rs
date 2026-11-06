use gpui::{
    div, prelude::*, px, size, App, Application, Bounds, Context, Entity, IntoElement, Render,
    SharedString, Subscription, Window, WindowBounds, WindowOptions,
};
use revo_core::service::{BackendCommand, BackendService};

use crate::{
    actions::{ComponentEvent, QuickSourceAction},
    components::{
        home::{
            DemoMode, HeaderBar, HeaderBarState, SceneItem, ScenesPanel, SourceItem, SourcesPanel,
        },
        modal_shell::{modal_shell, ModalConfig, ModalSize},
    },
    services::MockBackendRuntime,
    theme::RevoTheme,
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
    QuickAction {
        source_id: String,
        action: QuickSourceAction,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceState {
    pub active_modal: ActiveModal,
    pub last_command: Option<BackendCommand>,
    pub status: String,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            active_modal: ActiveModal::None,
            last_command: None,
            status: "Native mock backend ready".into(),
        }
    }
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

        self.status = format!("Mock command: {command:?}");
        self.last_command = Some(command.clone());
        Some(command)
    }

    pub fn close_modal(&mut self) {
        self.active_modal = ActiveModal::None;
    }
}

pub struct Workspace {
    pub header: Entity<HeaderBar>,
    pub scenes: Entity<ScenesPanel>,
    pub sources: Entity<SourcesPanel>,
    pub demo: Entity<DemoMode>,
    pub state: WorkspaceState,
    backend: BackendService<MockBackendRuntime>,
    theme: RevoTheme,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new_mock(cx: &mut Context<'_, Self>) -> Self {
        let theme = RevoTheme::future();
        let backend = BackendService::new(32, MockBackendRuntime::default)
            .expect("failed to start mock backend service");
        let header = cx.new(|_| HeaderBar {
            state: HeaderBarState {
                demo_mode: true,
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
        ];

        Self {
            header,
            scenes,
            sources,
            demo,
            state: WorkspaceState::default(),
            backend,
            theme,
            _subscriptions: subscriptions,
        }
    }

    fn route_event(&mut self, event: ComponentEvent, cx: &mut Context<'_, Self>) {
        self.update_mock_header(&event, cx);
        if let Some(command) = self.state.route(event) {
            if let Err(error) = self.backend.try_dispatch(command) {
                self.state.status = format!("Backend error: {error}");
            }
        }
        cx.notify();
    }

    fn update_mock_header(&self, event: &ComponentEvent, cx: &mut Context<'_, Self>) {
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

    fn tools(&self) -> gpui::Div {
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
            .child("Audio Mixer")
            .child("Transitions")
            .child("Plugins")
            .child(
                div()
                    .text_color(self.theme.text_muted)
                    .child(self.state.status.clone()),
            )
    }

    fn lower_panels(&self, mode: LayoutMode) -> gpui::Div {
        let [first, second, third] = mode.panel_order();
        match mode {
            LayoutMode::Wide => div()
                .flex()
                .flex_row()
                .gap_2()
                .child(div().flex_1().child(self.panel(first)))
                .child(div().flex_1().child(self.panel(second)))
                .child(div().flex_1().child(self.panel(third))),
            LayoutMode::Medium => div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .child(div().flex_1().child(self.panel(first)))
                        .child(div().flex_1().child(self.panel(second))),
                )
                .child(self.panel(third)),
            LayoutMode::Compact => div()
                .flex()
                .flex_col()
                .gap_2()
                .child(self.panel(first))
                .child(self.panel(second))
                .child(self.panel(third)),
        }
    }

    fn panel(&self, panel: PanelKind) -> gpui::Div {
        match panel {
            PanelKind::Scenes => div().child(self.scenes.clone()),
            PanelKind::Sources => div().child(self.sources.clone()),
            PanelKind::Tools => div().child(self.tools()),
        }
    }

    fn modal_content(&self) -> Option<(ModalConfig, Vec<SharedString>)> {
        match &self.state.active_modal {
            ActiveModal::None => None,
            ActiveModal::Settings => Some((
                ModalConfig::standard("Settings", ModalSize::Workspace),
                vec!["Native settings sections are ready for Phase 2 services.".into()],
            )),
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
            ActiveModal::QuickAction { source_id, action } => Some((
                ModalConfig::quick(format!("Quick {action:?}")),
                vec![SharedString::from(format!("Source: {source_id}"))],
            )),
        }
    }
}

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
            .child(div().flex_1().min_h(px(200.)).child(self.demo.clone()))
            .child(self.lower_panels(mode));

        if let Some((config, lines)) = self.modal_content() {
            root = root.child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .bottom_0()
                    .left_0()
                    .child(modal_shell(&config, self.theme, lines))
                    .child(
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
                            .child("Close")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.close_modal();
                                cx.notify();
                            })),
                    ),
            );
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
            |_, cx| cx.new(Workspace::new_mock),
        )
        .expect("failed to open RevoStream native window");
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
