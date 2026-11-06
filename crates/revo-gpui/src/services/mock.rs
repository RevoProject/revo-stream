use revo_core::service::{
    BackendCommand, BackendError, BackendRuntime, BackendSnapshot, MoveDirection,
};

pub struct MockBackendRuntime {
    snapshot: BackendSnapshot,
}

impl Default for MockBackendRuntime {
    fn default() -> Self {
        Self {
            snapshot: BackendSnapshot {
                selected_scene: Some("Main Camera".into()),
                selected_source: Some("lower-third".into()),
                scenes: vec![
                    "Starting Soon".into(),
                    "Main Camera".into(),
                    "Be Right Back".into(),
                ],
                sources: vec!["camera".into(), "lower-third".into(), "browser-chat".into()],
                visible_sources: vec!["camera".into(), "lower-third".into(), "browser-chat".into()],
                locked_sources: vec!["browser-chat".into()],
                ..BackendSnapshot::default()
            },
        }
    }
}

impl BackendRuntime for MockBackendRuntime {
    fn execute(&mut self, command: BackendCommand) -> Result<(), BackendError> {
        match command {
            BackendCommand::Undo | BackendCommand::Redo => {}
            BackendCommand::SetStudioMode(enabled) => self.snapshot.studio_mode = enabled,
            BackendCommand::SetRealtimeRefresh(enabled) => self.snapshot.realtime_refresh = enabled,
            BackendCommand::StartObs => self.snapshot.obs_running = true,
            BackendCommand::StopObs => {
                self.snapshot.obs_running = false;
                self.snapshot.recording = false;
                self.snapshot.streaming = false;
            }
            BackendCommand::StartRecording => self.snapshot.recording = true,
            BackendCommand::StopRecording => self.snapshot.recording = false,
            BackendCommand::StartStreaming => self.snapshot.streaming = true,
            BackendCommand::StopStreaming => self.snapshot.streaming = false,
            BackendCommand::ForcePreviewResolution => {
                self.snapshot.preview_resolution_requests += 1
            }
            BackendCommand::SelectScene(name) => self.snapshot.selected_scene = Some(name),
            BackendCommand::RenameScene { from, to } => {
                if let Some(scene) = self
                    .snapshot
                    .scenes
                    .iter_mut()
                    .find(|scene| **scene == from)
                {
                    *scene = to.clone();
                }
                if self.snapshot.selected_scene.as_deref() == Some(from.as_str()) {
                    self.snapshot.selected_scene = Some(to);
                }
            }
            BackendCommand::ReorderScene { name, to_index } => {
                reorder(&mut self.snapshot.scenes, &name, to_index);
            }
            BackendCommand::SelectSource(id) => self.snapshot.selected_source = Some(id),
            BackendCommand::ToggleSourceVisibility(id) => {
                toggle_membership(&mut self.snapshot.visible_sources, id)
            }
            BackendCommand::ToggleSourceLock(id) => {
                toggle_membership(&mut self.snapshot.locked_sources, id)
            }
            BackendCommand::MoveSource { id, direction } => {
                if let Some(index) = self
                    .snapshot
                    .sources
                    .iter()
                    .position(|source| source == &id)
                {
                    let last = self.snapshot.sources.len().saturating_sub(1);
                    let to_index = match direction {
                        MoveDirection::Up => (index + 1).min(last),
                        MoveDirection::Down => index.saturating_sub(1),
                        MoveDirection::Top => last,
                        MoveDirection::Bottom => 0,
                    };
                    reorder(&mut self.snapshot.sources, &id, to_index);
                }
            }
        }
        Ok(())
    }

    fn snapshot(&self) -> Result<BackendSnapshot, BackendError> {
        Ok(self.snapshot.clone())
    }
}

fn reorder(items: &mut Vec<String>, item: &str, to_index: usize) {
    if let Some(from_index) = items.iter().position(|candidate| candidate == item) {
        let item = items.remove(from_index);
        let to_index = to_index.min(items.len());
        items.insert(to_index, item);
    }
}

fn toggle_membership(items: &mut Vec<String>, item: String) {
    if let Some(index) = items.iter().position(|candidate| candidate == &item) {
        items.remove(index);
    } else {
        items.push(item);
    }
}
