use revo_core::models::{
    PluginInfo, PluginWindowCapabilities, SourcePropertySpec, SourceSettingsData, SourceTypeItem,
};
use revo_core::service::{
    clamp_volume_percent, upsert_source_list, BackendCommand, BackendError, BackendRuntime,
    BackendSnapshot, MoveDirection, PlacedSource,
};
use std::collections::HashMap;

pub struct MockBackendRuntime {
    snapshot: BackendSnapshot,
    /// Saved plugin profiles: profile name -> enabled modules. Mirrors the
    /// file-backed profiles the real backend keeps on disk.
    plugin_modules: HashMap<String, Vec<String>>,
    /// Typed per-source params applied through `UpdateSource`; merged over
    /// the type defaults `source_settings` reports (engine parity: the
    /// engine stores nested obs_data and flattens it into string params).
    source_params: HashMap<String, HashMap<String, String>>,
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
                source_items: vec![
                    revo_core::service::SourceSnapshotItem {
                        id: "camera".into(),
                        name: "Camera".into(),
                        source_type: "v4l2_input".into(),
                        visible: true,
                        audio_capable: true,
                    },
                    revo_core::service::SourceSnapshotItem {
                        id: "lower-third".into(),
                        name: "Lower third".into(),
                        source_type: "text_ft2_source_v2".into(),
                        visible: true,
                        audio_capable: false,
                    },
                    revo_core::service::SourceSnapshotItem {
                        id: "browser-chat".into(),
                        name: "Chat overlay".into(),
                        source_type: "browser_source".into(),
                        visible: true,
                        audio_capable: false,
                    },
                ],
                locked_sources: vec!["browser-chat".into()],
                settings: Some(revo_core::settings::default_settings()),
                plugins: vec![
                    PluginInfo {
                        name: "WebSocket".into(),
                        file_name: "obs-websocket.so".into(),
                        module_name: "obs-websocket".into(),
                        window_capabilities: PluginWindowCapabilities {
                            can_list: true,
                            can_open: false,
                            reason: "Native plugin windows require a frontend bridge".into(),
                        },
                    },
                    PluginInfo {
                        name: "VLC Video Source".into(),
                        file_name: "vlc-video.so".into(),
                        module_name: "vlc-video".into(),
                        window_capabilities: PluginWindowCapabilities {
                            can_list: true,
                            can_open: false,
                            reason: "Native plugin windows require a frontend bridge".into(),
                        },
                    },
                ],
                plugin_profiles: vec!["default".into()],
                active_plugin_profile: Some("default".into()),
                enabled_plugin_modules: Vec::new(),
                plugin_profile_states: vec![revo_core::models::PluginProfileState {
                    profile_name: "default".into(),
                    enabled_modules: Vec::new(),
                }],
                available_source_types: vec![
                    revo_core::models::SourceTypeItem {
                        id: "color_source".into(),
                        label: "Color source".into(),
                        audio_capable: false,
                    },
                    revo_core::models::SourceTypeItem {
                        id: "text_ft2_source_v2".into(),
                        label: "Text (FreeType 2)".into(),
                        audio_capable: false,
                    },
                    revo_core::models::SourceTypeItem {
                        id: "image_source".into(),
                        label: "Image".into(),
                        audio_capable: false,
                    },
                    revo_core::models::SourceTypeItem {
                        id: "browser_source".into(),
                        label: "Browser".into(),
                        audio_capable: false,
                    },
                    revo_core::models::SourceTypeItem {
                        id: "monitor_capture".into(),
                        label: "Display capture".into(),
                        audio_capable: true,
                    },
                ],
                ..BackendSnapshot::default()
            },
            plugin_modules: HashMap::from([("default".to_string(), Vec::new())]),
            source_params: HashMap::new(),
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
            BackendCommand::SetPluginProfile {
                profile_name,
                modules,
            } => {
                self.plugin_modules
                    .insert(profile_name.clone(), modules.clone());
                if !self.snapshot.plugin_profiles.contains(&profile_name) {
                    self.snapshot.plugin_profiles.push(profile_name.clone());
                }
                self.snapshot
                    .plugin_profile_states
                    .retain(|state| state.profile_name != profile_name);
                self.snapshot
                    .plugin_profile_states
                    .push(revo_core::models::PluginProfileState {
                        profile_name: profile_name.clone(),
                        enabled_modules: modules.clone(),
                    });
                self.snapshot
                    .plugin_profile_states
                    .sort_by(|a, b| a.profile_name.cmp(&b.profile_name));
                if self.snapshot.active_plugin_profile.as_deref() == Some(profile_name.as_str()) {
                    self.snapshot.enabled_plugin_modules = modules;
                }
            }
            BackendCommand::SetActivePluginProfile(name) => {
                self.snapshot.active_plugin_profile = Some(name.clone());
                self.snapshot.enabled_plugin_modules =
                    self.plugin_modules.get(&name).cloned().unwrap_or_default();
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
            BackendCommand::SetSceneLock { name, locked } => {
                upsert_source_list(&mut self.snapshot.locked_scenes, &name, locked);
            }
            BackendCommand::RemoveScene(name) => {
                self.snapshot.scenes.retain(|scene| *scene != name);
                self.snapshot.locked_scenes.retain(|scene| *scene != name);
                if self.snapshot.selected_scene.as_deref() == Some(name.as_str()) {
                    self.snapshot.selected_source = None;
                    self.snapshot.selected_scene = self.snapshot.scenes.first().cloned();
                }
            }
            BackendCommand::AddScene(name) => {
                let name = name.trim().to_string();
                if name.is_empty() {
                    return Err(BackendError::Runtime("scene name required".into()));
                }
                if self.snapshot.scenes.contains(&name) {
                    return Err(BackendError::Runtime("scene already exists".into()));
                }
                self.snapshot.scenes.push(name.clone());
                self.snapshot.selected_scene = Some(name);
            }
            BackendCommand::AddSource { name, source_type } => {
                let name = name.trim().to_string();
                if name.is_empty() {
                    return Err(BackendError::Runtime("source name required".into()));
                }
                let slug = slugify_source_name(&name);
                let mut id = slug.clone();
                let mut counter = 1;
                while self
                    .snapshot
                    .source_items
                    .iter()
                    .any(|source| source.id == id)
                {
                    counter += 1;
                    id = format!("{slug}-{counter}");
                }
                self.snapshot.sources.push(id.clone());
                self.snapshot.visible_sources.push(id.clone());
                self.snapshot
                    .source_items
                    .push(revo_core::service::SourceSnapshotItem {
                        id,
                        name,
                        source_type,
                        visible: true,
                        audio_capable: false,
                    });
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
            BackendCommand::RemoveSource(id) => {
                self.snapshot.sources.retain(|source| *source != id);
                self.snapshot.visible_sources.retain(|source| *source != id);
                self.snapshot.locked_sources.retain(|source| *source != id);
                self.snapshot.source_items.retain(|source| source.id != id);
                if self.snapshot.selected_source.as_deref() == Some(id.as_str()) {
                    self.snapshot.selected_source = None;
                }
                self.source_params.remove(&id);
            }
            BackendCommand::UpdateSource { update } => {
                if !update.name.trim().is_empty() {
                    self.snapshot.sources = self
                        .snapshot
                        .sources
                        .iter()
                        .map(|source| {
                            if *source == update.id {
                                update.name.clone()
                            } else {
                                source.clone()
                            }
                        })
                        .collect();
                }
                if let Some(source) = self
                    .snapshot
                    .source_items
                    .iter_mut()
                    .find(|source| source.id == update.id)
                {
                    if !update.name.trim().is_empty() {
                        source.name = update.name.clone();
                    }
                    if !update.source_type.trim().is_empty() {
                        source.source_type = update.source_type.clone();
                    }
                }
                if !update.params.is_empty() {
                    let entry = self.source_params.entry(update.id.clone()).or_default();
                    for (key, value) in update.params {
                        entry.insert(key, value);
                    }
                }
            }
            BackendCommand::SetTransitionKind(kind) => {
                self.snapshot.transition_kind = Some(kind);
            }
            BackendCommand::SetSourceFilters {
                id: _id,
                filters: _,
            } => {
                // Filters have no mock representation.
            }
            BackendCommand::SetTransitionParams(_params) => {
                // Per-kind transition params have no mock representation.
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
            BackendCommand::ExportRevoSceneCollection { .. } => {}
            BackendCommand::ImportRevoSceneCollection { .. } => {}
            BackendCommand::ExportObsSceneCollection { .. } => {}
            BackendCommand::ImportObsSceneCollection { .. } => {}
        }
        Ok(())
    }

    fn snapshot(&self) -> Result<BackendSnapshot, BackendError> {
        Ok(self.snapshot.clone())
    }
}

impl MockBackendRuntime {
    /// Builds one source's full settings payload: identity, the typed
    /// property specs for its source type, and defaults merged with any
    /// params applied through `UpdateSource`. `None` for unknown ids.
    /// Mirrors the engine's flattened string-param shape so the native
    /// editor renders the same typeable fields (font face/size, text,
    /// URL, colors) on the mock backend.
    pub fn source_settings(&self, id: &str) -> Result<Option<SourceSettingsData>, BackendError> {
        let Some(item) = self
            .snapshot
            .source_items
            .iter()
            .find(|source| source.id == id)
        else {
            return Ok(None);
        };
        let mut params: std::collections::BTreeMap<String, String> =
            mock_source_params(&item.source_type).into_iter().collect();
        if let Some(stored) = self.source_params.get(id) {
            for (key, value) in stored {
                params.insert(key.clone(), value.clone());
            }
        }
        Ok(Some(SourceSettingsData {
            id: item.id.clone(),
            name: item.name.clone(),
            source_type: item.source_type.clone(),
            params,
            source_properties: mock_source_specs(&item.source_type),
        }))
    }

    /// Device catalog for capture sources (Svelte `listQuickDeviceOptions`
    /// parity shape): static pulse/video/window entries so the "Select
    /// device" picker is fully usable on the mock backend without a real
    /// host. Returns `(id, label)` items; an unknown type yields an empty
    /// list and the UI falls back to the source's property options.
    pub fn source_devices(&self, source_type: &str) -> Result<Vec<SourceTypeItem>, BackendError> {
        let t = source_type.to_ascii_lowercase();
        let items: Vec<(&str, &str, bool)> = if t == "pulse_input_capture"
            || t.contains("audio_input")
            || t.contains("audio_capture")
            || t.contains("audio_line")
        {
            vec![
                ("default", "Default audio input", true),
                (
                    "alsa_input.usb-Camera_Microphone",
                    "Camera Microphone (alsa_input.usb-Camera_Microphone)",
                    true,
                ),
                (
                    "alsa_input.pci-HDA_Line",
                    "Line In (alsa_input.pci-HDA_Line)",
                    true,
                ),
            ]
        } else if t == "pulse_output_capture" || t.contains("audio_output") {
            vec![
                ("default", "Default audio output", true),
                (
                    "alsa_output.pci-HDA_Analog_Stereo.monitor",
                    "Speakers (monitor)",
                    true,
                ),
            ]
        } else if t.contains("video") || t.contains("v4l2") {
            vec![
                ("/dev/video0", "/dev/video0 (Integrated Webcam)", false),
                ("/dev/video1", "/dev/video1 (USB Capture)", false),
            ]
        } else if t.contains("window") || t.contains("xcomposite") {
            vec![
                ("active", "Active window", false),
                ("revo-stream", "RevoStream window", false),
            ]
        } else if t.contains("xshm")
            || t.contains("display")
            || t.contains("screen")
            || t.contains("monitor_capture")
        {
            vec![
                (":0.0", "Display :0.0", false),
                (":0.1", "Secondary display :0.1", false),
            ]
        } else {
            Vec::new()
        };
        Ok(items
            .into_iter()
            .map(|(id, label, audio_capable)| SourceTypeItem {
                id: id.to_string(),
                label: label.to_string(),
                audio_capable,
            })
            .collect())
    }
}

/// Typed property specs per source type; matches the libobs property
/// shapes the engine forwards (the nested text-source `font` property is
/// rebuilt by the editor into the typeable `font_face`/`font_size` pair).
fn mock_source_specs(source_type: &str) -> Vec<SourcePropertySpec> {
    let spec = |key: &str, label: &str, kind: &str| SourcePropertySpec {
        key: key.to_string(),
        label: label.to_string(),
        kind: kind.to_string(),
        hint: String::new(),
        options: Vec::new(),
    };
    match source_type {
        "text_ft2_source_v2" => vec![
            spec("font", "Font", "text"),
            spec("text", "Text", "text"),
            spec("text_file", "Text file (UTF-8)", "path"),
            spec("color1", "Color", "color"),
        ],
        "browser_source" => vec![
            spec("url", "URL", "text"),
            spec("width", "Width", "int"),
            spec("height", "Height", "int"),
        ],
        "color_source" => vec![
            spec("color1", "Color", "color"),
            spec("width", "Width", "int"),
            spec("height", "Height", "int"),
        ],
        "image_source" => vec![spec("file", "Image file", "path")],
        _ => Vec::new(),
    }
}

/// Default flat params per source type, in the engine's flattened shape
/// (nested font objects arrive as `font_face`/`font_size` strings).
fn mock_source_params(source_type: &str) -> Vec<(String, String)> {
    match source_type {
        "text_ft2_source_v2" => vec![
            ("text".into(), "Lower third".into()),
            ("font_face".into(), "DejaVu Sans".into()),
            ("font_size".into(), "48".into()),
            ("color1".into(), "#FFFFFFFF".into()),
        ],
        "browser_source" => vec![
            ("url".into(), "https://chatterino.example/overlay".into()),
            ("width".into(), "800".into()),
            ("height".into(), "600".into()),
        ],
        "color_source" => vec![
            ("color1".into(), "#FF00FF00".into()),
            ("width".into(), "400".into()),
            ("height".into(), "225".into()),
        ],
        "v4l2_input" => vec![
            ("device_id".into(), "/dev/video0".into()),
            ("device".into(), "/dev/video0".into()),
        ],
        "monitor_capture" => vec![("device".into(), ":0.0".into())],
        _ => Vec::new(),
    }
}

/// URL-style slug for generated source ids: lowercase, non-alphanumeric
/// characters collapse to dashes.
fn slugify_source_name(name: &str) -> String {
    let slug: String = name
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "source".to_string()
    } else {
        trimmed.to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_settings_serves_typed_defaults_for_known_sources() {
        let runtime = MockBackendRuntime::default();
        let settings = runtime
            .source_settings("lower-third")
            .expect("mock settings")
            .expect("seeded source has settings");
        assert_eq!(settings.id, "lower-third");
        assert_eq!(settings.source_type, "text_ft2_source_v2");
        assert!(
            settings
                .source_properties
                .iter()
                .any(|spec| spec.key == "font"),
            "the font spec lets the editor rebuild the typeable font pair"
        );
        assert_eq!(
            settings.params.get("font_face").map(String::as_str),
            Some("DejaVu Sans")
        );
        assert_eq!(
            settings.params.get("font_size").map(String::as_str),
            Some("48")
        );
        assert_eq!(
            settings.params.get("text").map(String::as_str),
            Some("Lower third")
        );
    }

    #[test]
    fn source_settings_is_none_for_unknown_sources() {
        let runtime = MockBackendRuntime::default();
        assert!(runtime.source_settings("missing").expect("call").is_none());
    }

    #[test]
    fn update_source_merges_params_the_editor_can_reopen() {
        let mut runtime = MockBackendRuntime::default();
        let mut params = HashMap::new();
        params.insert("font_face".to_string(), "Liberation Sans".to_string());
        params.insert("font_size".to_string(), "96".to_string());
        runtime
            .execute(BackendCommand::UpdateSource {
                update: revo_core::models::SourceUpdate {
                    id: "lower-third".into(),
                    name: "Lower third 2".into(),
                    source_type: String::new(),
                    params,
                },
            })
            .expect("update applies");
        let settings = runtime
            .source_settings("lower-third")
            .expect("mock settings")
            .expect("source still present");
        assert_eq!(settings.name, "Lower third 2");
        assert_eq!(
            settings.params.get("font_face").map(String::as_str),
            Some("Liberation Sans")
        );
        assert_eq!(
            settings.params.get("font_size").map(String::as_str),
            Some("96")
        );
        // Untouched defaults keep their values.
        assert_eq!(
            settings.params.get("text").map(String::as_str),
            Some("Lower third")
        );
    }

    #[test]
    fn history_undo_restores_the_previous_params() {
        let mut runtime = MockBackendRuntime::default();
        let mut edited = HashMap::new();
        edited.insert("font_size".to_string(), "72".to_string());
        runtime
            .execute(BackendCommand::UpdateSource {
                update: revo_core::models::SourceUpdate {
                    id: "lower-third".into(),
                    name: String::new(),
                    source_type: String::new(),
                    params: edited,
                },
            })
            .expect("edit applies");
        let mut previous = HashMap::new();
        previous.insert("font_size".to_string(), "48".to_string());
        runtime
            .execute(BackendCommand::UpdateSource {
                update: revo_core::models::SourceUpdate {
                    id: "lower-third".into(),
                    name: String::new(),
                    source_type: String::new(),
                    params: previous,
                },
            })
            .expect("undo applies");
        let settings = runtime
            .source_settings("lower-third")
            .expect("mock settings")
            .expect("source still present");
        assert_eq!(
            settings.params.get("font_size").map(String::as_str),
            Some("48")
        );
    }

    #[test]
    fn removed_sources_drop_their_stored_params() {
        let mut runtime = MockBackendRuntime::default();
        runtime
            .execute(BackendCommand::RemoveSource("lower-third".into()))
            .expect("remove applies");
        assert!(runtime
            .source_settings("lower-third")
            .expect("call")
            .is_none());
    }

    #[test]
    fn source_devices_serves_typed_catalogs_per_source_type() {
        let runtime = MockBackendRuntime::default();
        let video = runtime
            .source_devices("v4l2_input")
            .expect("video catalog serves");
        assert!(
            video.iter().any(|item| item.id == "/dev/video0"),
            "video capture types enumerate /dev/video* items"
        );
        let audio = runtime
            .source_devices("pulse_input_capture")
            .expect("audio catalog serves");
        assert!(
            audio.iter().all(|item| item.audio_capable),
            "audio catalogs are marked audio-capable"
        );
        assert!(
            runtime
                .source_devices("text_ft2_source_v2")
                .expect("plain types never error")
                .is_empty(),
            "non-capture types yield an empty catalog (the UI falls back)"
        );
    }

    #[test]
    fn capture_sources_carry_a_current_device_param() {
        let runtime = MockBackendRuntime::default();
        let settings = runtime
            .source_settings("camera")
            .expect("mock settings")
            .expect("seeded camera has settings");
        assert_eq!(
            settings.params.get("device_id").map(String::as_str),
            Some("/dev/video0"),
            "the quick device picker prefills from a stored device"
        );
    }
}
