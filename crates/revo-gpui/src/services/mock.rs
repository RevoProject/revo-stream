use revo_core::service::{
    clamp_volume_percent, upsert_source_list, BackendCommand, BackendError, BackendRuntime,
    BackendSnapshot, MoveDirection, PlacedSource,
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
                settings: Some(revo_core::settings::default_settings()),
                ..BackendSnapshot::default()
            },
        }
    }
}

impl BackendRuntime for MockBackendRuntime {
    fn execute(&mut self, command: BackendCommand) -> Result<(), BackendError> {
        match command {
            BackendCommand::Undo | BackendCommand::Redo => {}
            BackendCommand::SaveSettings { settings } => {
                self.snapshot.settings = Some(settings);
            }
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
            BackendCommand::SetSourceVolume { id, percent } => {
                let volume = clamp_volume_percent(percent);
                if let Some(entry) = self
                    .snapshot
                    .source_volumes
                    .iter_mut()
                    .find(|(source, _)| source == &id)
                {
                    entry.1 = volume;
                } else {
                    self.snapshot.source_volumes.push((id, volume));
                }
            }
            BackendCommand::SetSourceMuted { id, muted } => {
                upsert_source_list(&mut self.snapshot.muted_sources, &id, muted);
            }
            BackendCommand::SetSourceVolumeLocked { id, locked } => {
                upsert_source_list(&mut self.snapshot.volume_locked_sources, &id, locked);
            }
            BackendCommand::SetTransitionKind(kind) => {
                self.snapshot.transition_kind = Some(kind);
            }
            BackendCommand::SetTransitionDuration(duration_ms) => {
                self.snapshot.transition_duration_ms = duration_ms;
            }
            BackendCommand::SetSourceTransform {
                id,
                x,
                y,
                width,
                height,
                rotation,
            } => {
                let placed = PlacedSource {
                    id,
                    x,
                    y,
                    width,
                    height,
                    rotation,
                };
                if let Some(entry) = self
                    .snapshot
                    .source_transforms
                    .iter_mut()
                    .find(|source| source.id == placed.id)
                {
                    *entry = placed;
                } else {
                    self.snapshot.source_transforms.push(placed);
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
