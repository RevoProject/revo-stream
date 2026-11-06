pub use revo_core::service::MoveDirection;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComponentEvent {
    OpenSettings,
    OpenHistory,
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
    AddSource,
    SelectSource(String),
    ToggleSourceVisibility(String),
    ToggleSourceLock(String),
    MoveSource {
        id: String,
        direction: MoveDirection,
    },
    EditSource(String),
    QuickSourceAction {
        id: String,
        action: QuickSourceAction,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModalEvent {
    Close,
    Cancel,
    Confirm,
    SubmitText(String),
    SelectValue(String),
    ConfirmChoice(bool),
}
