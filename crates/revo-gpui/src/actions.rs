pub use revo_core::service::MoveDirection;

/// Which auxiliary (separate native window) surface a Tools-panel request
/// wants. Opened once per kind; repeated requests focus the existing window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuxiliaryWindowKind {
    /// Preview window; `source_id` selects a scene or source to capture
    /// (`None` = the current scene composite).
    Preview {
        source_id: Option<String>,
    },
    DebugConsole,
    BrowserInteraction,
}

/// Typed event vocabulary shared by every migrated GPUI panel. `Eq` is not
/// derived because context-menu requests carry cursor positions (`f32`).
#[derive(Clone, Debug, PartialEq)]
pub enum ComponentEvent {
    OpenSettings,
    OpenHistory,
    OpenPlugins,
    OpenTemplates,
    OpenGraphicPlanner,
    OpenPreviewWindow,
    OpenDebugConsoleWindow,
    OpenBrowserInteractionWindow,
    Undo,
    Redo,
    SetStudioMode(bool),
    SetRealtimeRefresh(bool),
    StartObs,
    StopObs,
    StartRecording,
    StopRecording,
    StartStreaming,
    StopStreaming,
    ForcePreviewResolution,
    AddScene,
    SelectScene(String),
    RenameScene {
        from: String,
        to: String,
    },
    ReorderScene {
        name: String,
        to_index: usize,
    },
    SetSceneLock {
        name: String,
        locked: bool,
    },
    RemoveScene(String),
    AddSource,
    SelectSource(String),
    ToggleSourceVisibility(String),
    ToggleSourceLock(String),
    MoveSource {
        id: String,
        direction: MoveDirection,
    },
    RemoveSource(String),
    EditSource(String),
    QuickSourceAction {
        id: String,
        action: QuickSourceAction,
    },
    /// Open the audio mixer as a modal (Svelte parity: showAudioMixerModal).
    OpenMixer,
    /// Open the preview window for a scene or source (Svelte
    /// openScenePreviewInWindow / openPreviewInWindow).
    OpenPreviewInWindow {
        target_id: Option<String>,
    },
    /// Open the transitions editor as a modal (showTransitionsModal).
    OpenTransitions,
    /// Right-click context-menu request from a panel row or the preview
    /// surface; UI-only, never dispatches a backend command.
    OpenContextMenu {
        target: ContextMenuTarget,
        /// Window-space cursor position (px) the menu opens at.
        position: (f32, f32),
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickSourceAction {
    Interact,
    EditText,
    ChangeColor,
    SelectFile,
    SelectDevice,
}

/// What a right-click context menu was opened on. The target carries the
/// state snapshot the menu entries were built from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextMenuTarget {
    Scene {
        name: String,
        locked: bool,
    },
    Source {
        id: String,
        name: String,
        visible: bool,
        locked: bool,
    },
    Preview,
}

/// One typed context-menu choice, mirroring the legacy OBS right-click
/// workflow per surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextMenuAction {
    Select,
    ToggleLock,
    Move(MoveDirection),
    Remove,
    Edit,
    QuickAction,
    ToggleVisibility,
    ForcePreviewResolution,
    ToggleStudioMode,
    PreviewInWindow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModalEvent {
    Close,
    Cancel,
    Confirm,
    SubmitText(String),
    SelectValue(String),
    ConfirmChoice(bool),
}
