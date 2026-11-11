pub use revo_core::service::MoveDirection;

/// Which editing field a dropdown-picker result belongs to. Routing identity
/// for [`ComponentEvent::OpenDropdown`]: the workspace carries the picked
/// option value back to the host component through this key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DropdownField {
    /// One typed list param in the source editor's General tab.
    SourceParam { source_id: String, key: String },
    /// One typed list param in the source editor's per-filter accordion.
    FilterParam {
        source_id: String,
        filter_name: String,
        key: String,
    },
    /// One basic option of the add-source modal (including `__mode`).
    AddSourceOption { key: String },
    /// The add-filter kind selector in the source editor.
    AddFilterKind { source_id: String },
    /// The selected transition's kind (re-blends default params).
    TransitionKind { transition_id: String },
    /// One typed param of the selected transition draft.
    TransitionParam { transition_id: String, key: String },
}

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
    /// Header brand click: the workspace asks before opening the official
    /// website in the browser (plan §24).
    OpenOfficialWebsite,
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
    /// Open the shared color-picker popup for one editing field; UI-only.
    OpenColorPicker {
        field: ColorField,
        /// Current `#RRGGBB` value the picker starts from.
        current: String,
        /// Window-space anchor (px) the popup opens under.
        position: (f32, f32),
    },
    /// Open the shared system-font-picker popup for one editing field;
    /// UI-only.
    OpenFontPicker {
        field: FontField,
        current: String,
        position: (f32, f32),
    },
    /// Open the shared dropdown-list popup for one editing field; UI-only.
    /// `options` are `(value, label)` pairs.
    OpenDropdown {
        field: DropdownField,
        options: Vec<(String, String)>,
        current: String,
        position: (f32, f32),
    },
}

/// Which editing field a color-picker result belongs to. Routing identity
/// for [`ComponentEvent::OpenColorPicker`]: the workspace carries the pick
/// back to the host component through this key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColorField {
    /// The quick-action modal's color draft for one source.
    QuickAction { source_id: String },
    /// One typed param row in the source editor's General tab.
    SourceParam { source_id: String, key: String },
    /// One filter param in the source editor's per-filter settings.
    FilterParam {
        source_id: String,
        filter_name: String,
        key: String,
    },
    /// One param of the selected transition draft.
    TransitionParam { transition_id: String, key: String },
    /// One basic option of the add-source modal.
    AddSource { key: String },
}

/// Which editing field a font-picker result belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FontField {
    /// The source editor's Font row (text sources).
    SourceFont { source_id: String },
    /// The add-source modal's font option.
    AddSource,
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
