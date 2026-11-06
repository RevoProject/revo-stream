//! Real libobs backend wiring for the native GPUI shell, gated behind the
//! `obs-backend` feature so default builds (and `scripts/core/verify.sh`)
//! stay green without libobs/openssl host packages.

use std::collections::HashMap;

use revo_backend::runtime::lifecycle::ObsPlatform;
use revo_backend::runtime::preview::{encode_screenshot, ScreenshotCapture};
use revo_backend::runtime::{recording, streaming};
use revo_backend::scenes::scene_ops;
use revo_backend::sources::source_ops;
use revo_backend::ObsRuntime;
use revo_core::service::{
    BackendCommand, BackendError, BackendRuntime, BackendSnapshot, MoveDirection, PlacedSource,
    SourceSnapshotItem,
};
use revo_lib::obs;

pub const PREVIEW_CAPTURE_INTERVAL_MS: u64 = 150;

/// libobs platform hooks for the native GPUI application.
///
/// GPUI owns the process main thread, so unlike the Tauri adapter there is no
/// separate helper application handle to dispatch onto the platform event
/// loop. Startup and shutdown therefore call libobs directly.
pub struct GpuiObsPlatform;

impl GpuiObsPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GpuiObsPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl ObsPlatform for GpuiObsPlatform {
    fn startup(&self, locale: &std::ffi::CStr, config: &std::ffi::CStr) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            // The Tauri adapter dispatches obs_startup onto the macOS main
            // dispatch queue (HIToolbox/TSM hotkey init asserts it). GPUI
            // owns the main thread here and direct dispatch is not wired up;
            // TODO(main-thread): verify main-thread dispatch strategy with
            // GPUI before enabling macOS.
            // SAFETY: The caller serializes the libobs lifecycle, and both C
            // strings remain valid for the duration of obs_startup.
            if unsafe { obs::obs_startup(locale.as_ptr(), config.as_ptr(), std::ptr::null_mut()) } {
                Ok(())
            } else {
                Err("obs_startup failed".to_string())
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            // SAFETY: The caller serializes the libobs lifecycle, and both C
            // strings remain valid for the duration of obs_startup.
            if unsafe { obs::obs_startup(locale.as_ptr(), config.as_ptr(), std::ptr::null_mut()) } {
                Ok(())
            } else {
                Err("obs_startup failed".to_string())
            }
        }
    }

    fn shutdown(&self) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            // TODO(main-thread): see startup() — obs_shutdown touches
            // HIToolbox on macOS and must run on the main thread.
            // SAFETY: The caller established that the libobs core is active,
            // and teardown runs while the lifecycle is serialized.
            unsafe { obs::obs_shutdown() };
            Ok(())
        }
        #[cfg(not(target_os = "macos"))]
        {
            // SAFETY: The caller serializes lifecycle access and only invokes
            // shutdown for a successfully started libobs core.
            unsafe { obs::obs_shutdown() };
            Ok(())
        }
    }
}

/// [`revo_core::service::BackendRuntime`] implementation over the real
/// revo-backend ObsRuntime. It runs exclusively on its BackendService worker
/// thread; GPUI code only talks to it through typed service requests.
pub struct ObsBackendRuntime {
    runtime: ObsRuntime,
    platform: GpuiObsPlatform,
    root_dir: Option<String>,
    last_error: Option<String>,
    // UI-mode and selection state kept on the adapter. libobs owns the
    // authoritative scene, source, output, and transition state; locks and
    // volume-lock flags have no libobs equivalent yet, so the adapter holds
    // them until scene locking is migrated.
    selected_source: Option<String>,
    muted_sources: Vec<String>,
    source_volumes: Vec<(String, u32)>,
    volume_locked_sources: Vec<String>,
    locked_sources: Vec<String>,
    transition_kind: String,
    transition_duration_ms: u32,
    studio_mode: bool,
    realtime_refresh: bool,
    preview_resolution_requests: u64,
}

impl ObsBackendRuntime {
    pub fn new(root_dir: Option<String>) -> Self {
        Self {
            runtime: ObsRuntime::default(),
            platform: GpuiObsPlatform::new(),
            root_dir,
            last_error: None,
            selected_source: None,
            muted_sources: Vec::new(),
            source_volumes: Vec::new(),
            volume_locked_sources: Vec::new(),
            locked_sources: Vec::new(),
            transition_kind: "cut".to_string(),
            transition_duration_ms: 300,
            studio_mode: false,
            realtime_refresh: false,
            preview_resolution_requests: 0,
        }
    }
}

impl BackendRuntime for ObsBackendRuntime {
    fn execute(&mut self, command: BackendCommand) -> Result<(), BackendError> {
        let result = self.apply(command);
        if let Err(message) = &result {
            self.last_error = Some(message.clone());
        } else {
            self.last_error = None;
        }
        Ok(())
    }

    fn snapshot(&self) -> Result<BackendSnapshot, BackendError> {
        let runtime = &self.runtime;
        let mut scenes = runtime.scene_order.clone();
        for name in runtime.scenes.keys() {
            if !scenes.iter().any(|scene| scene == name) {
                scenes.push(name.clone());
            }
        }

        let sources = revo_backend::collect_sources(runtime);
        let source_items: Vec<SourceSnapshotItem> = sources
            .iter()
            .map(|source| SourceSnapshotItem {
                id: source.id.clone(),
                name: source.name.clone(),
                source_type: source.source_type.clone(),
                visible: source.visible,
                audio_capable: source.audio_capable,
            })
            .collect();
        let visible_sources = sources
            .iter()
            .filter(|source| source.visible)
            .map(|source| source.id.clone())
            .collect();
        let source_ids = sources.iter().map(|source| source.id.clone()).collect();

        let source_transforms = source_ops::get_sceneitem_transforms(runtime)
            .ok()
            .and_then(|value| transform_value_to_placed_sources(&value))
            .unwrap_or_default();

        let audio_levels = revo_backend::devices::levels::get_levels()
            .lock()
            .map(|levels| {
                let mut levels: Vec<(String, u32)> = levels
                    .iter()
                    .map(|(id, level)| (id.clone(), level.round().clamp(0.0, 100.0) as u32))
                    .collect();
                levels.sort_by(|a, b| a.0.cmp(&b.0));
                levels
            })
            .unwrap_or_default();

        Ok(BackendSnapshot {
            obs_running: runtime.initialized,
            recording: !runtime.output_record.is_null(),
            streaming: !runtime.output_stream.is_null(),
            studio_mode: self.studio_mode,
            realtime_refresh: self.realtime_refresh,
            preview_resolution_requests: self.preview_resolution_requests,
            selected_scene: runtime.current_scene.clone(),
            selected_source: self.selected_source.clone(),
            scenes,
            locked_scenes: runtime.locked_scenes.iter().cloned().collect(),
            sources: source_ids,
            source_items,
            visible_sources,
            locked_sources: self.locked_sources.clone(),
            source_volumes: self.source_volumes.clone(),
            muted_sources: self.muted_sources.clone(),
            volume_locked_sources: self.volume_locked_sources.clone(),
            transition_kind: Some(if runtime.initialized {
                runtime.program_transition_kind.clone()
            } else {
                self.transition_kind.clone()
            }),
            transition_duration_ms: if runtime.initialized {
                runtime.program_transition_duration_ms
            } else {
                self.transition_duration_ms
            },
            source_transforms,
            audio_levels,
            last_error: self.last_error.clone(),
        })
    }

    fn shutdown(&mut self) -> Result<(), BackendError> {
        let result =
            revo_backend::runtime::init::shutdown_with_platform(&mut self.runtime, &self.platform);
        if let Err(message) = &result {
            self.last_error = Some(message.clone());
        } else {
            self.last_error = None;
        }
        Ok(())
    }
}

impl ObsBackendRuntime {
    /// Applies one command against the real backend. Every variant maps onto a
    /// revo-backend call; undo/redo history (not yet migrated off the Tauri
    /// command journal) is recorded as a no-op.
    fn apply(&mut self, command: BackendCommand) -> Result<String, String> {
        match command {
            // Undo/redo history has no libobs backing yet; the Tauri adapter
            // treats both commands the same way (recorded + no-op pending the
            // command-journal migration).
            BackendCommand::Undo | BackendCommand::Redo => Ok("history not migrated yet".into()),
            BackendCommand::SetStudioMode(enabled) => {
                self.studio_mode = enabled;
                Ok(format!("studio mode: {enabled}"))
            }
            BackendCommand::SetRealtimeRefresh(enabled) => {
                self.realtime_refresh = enabled;
                Ok(format!("realtime refresh: {enabled}"))
            }
            BackendCommand::StartObs => revo_backend::runtime::init::start_with_platform(
                &mut self.runtime,
                &self.platform,
                self.root_dir.clone(),
            ),
            BackendCommand::StopObs => {
                self.muted_sources.clear();
                self.source_volumes.clear();
                revo_backend::runtime::init::shutdown_with_platform(
                    &mut self.runtime,
                    &self.platform,
                )
            }
            BackendCommand::StartRecording => {
                let path = self.resolve_record_path();
                recording::start_recording(&mut self.runtime, path)
            }
            BackendCommand::StopRecording => recording::stop_recording(&mut self.runtime),
            BackendCommand::StartStreaming => {
                let url = self.resolve_stream_url();
                streaming::start_streaming(&mut self.runtime, url)
            }
            BackendCommand::StopStreaming => streaming::stop_streaming(&mut self.runtime),
            BackendCommand::ForcePreviewResolution => {
                self.preview_resolution_requests += 1;
                Ok("preview resolution refresh requested".into())
            }
            BackendCommand::SelectScene(name) => {
                scene_ops::set_current_scene(&mut self.runtime, name)
            }
            BackendCommand::RenameScene { from, to } => {
                scene_ops::rename_scene(&mut self.runtime, from, to)
            }
            BackendCommand::ReorderScene { name, to_index } => {
                scene_ops::reorder_scene(&mut self.runtime, name, to_index)
                    .map(|()| format!("scene reordered: {name} -> {to_index}"))
            }
            BackendCommand::SelectSource(id) => {
                self.selected_source = Some(id.clone());
                Ok(format!("selected source: {id}"))
            }
            BackendCommand::ToggleSourceVisibility(id) => {
                let visible = revo_backend::collect_sources(&self.runtime)
                    .iter()
                    .find(|source| source.id == id)
                    .map(|source| source.visible)
                    .ok_or_else(|| format!("unknown source id: {id}"))?;
                source_ops::set_source_visible(&self.runtime, id.clone(), !visible)
                    .map(|()| format!("visibility: {id} -> {}", !visible))
            }
            BackendCommand::ToggleSourceLock(id) => {
                // libobs has no scene-item lock concept yet; the lock lives on
                // the adapter until scene locking is migrated.
                let locked = self.locked_sources.contains(&id);
                revo_core::service::upsert_source_list(&mut self.locked_sources, &id, !locked);
                Ok(format!("source lock: {id} -> {}", !locked))
            }
            BackendCommand::MoveSource { id, direction } => {
                let direction = move_direction_label(direction);
                source_ops::move_source(&self.runtime, id, direction)
                    .map(|()| format!("source moved: {id} {direction}"))
            }
            BackendCommand::SetSourceVolume { id, percent } => {
                let volume = revo_core::service::clamp_volume_percent(percent);
                update_source_runtime(
                    self,
                    SourceUpdateBuilder::new(id.clone())
                        .param("volume_percent".to_string(), volume.to_string())
                        .update()?,
                )
                .map(|message| format!("source volume: {id} -> {volume}% ({message})"))
            }
            BackendCommand::SetSourceMuted { id, muted } => {
                revo_core::service::upsert_source_list(&mut self.muted_sources, &id, muted);
                update_source_runtime(
                    self,
                    SourceUpdateBuilder::new(id.clone())
                        .param("muted".to_string(), muted.to_string())
                        .update()?,
                )
                .map(|message| format!("source muted: {id} -> {muted} ({message})"))
            }
            BackendCommand::SetSourceVolumeLocked { id, locked } => {
                revo_core::service::upsert_source_list(
                    &mut self.volume_locked_sources,
                    &id,
                    locked,
                );
                Ok(format!("source volume lock: {id} -> {locked}"))
            }
            BackendCommand::SetTransitionKind(kind) => {
                self.transition_kind = kind.clone();
                let kind = kind.clone();
                if self.runtime.initialized {
                    let duration = self.transition_duration_ms;
                    scene_ops::set_program_transition(&mut self.runtime, kind, Some(duration))
                } else {
                    Ok(format!("transition kind: {kind} (offline)"))
                }
            }
            BackendCommand::SetTransitionDuration(duration_ms) => {
                self.transition_duration_ms = duration_ms;
                if self.runtime.initialized {
                    let kind = self.transition_kind.clone();
                    scene_ops::set_program_transition(&mut self.runtime, kind, Some(duration_ms))
                } else {
                    Ok(format!("transition duration: {duration_ms}ms (offline)"))
                }
            }
            BackendCommand::SetSourceTransform {
                id,
                x,
                y,
                width,
                height,
                rotation,
            } => apply_transform_command(self, &id, x, y, width, height, rotation),
        }
    }

    fn resolve_record_path(&self) -> String {
        revo_backend::settings::core::settings_get()
            .ok()
            .map(|settings| settings.record_path)
            .filter(|path| !path.trim().is_empty())
            .or_else(|| Some(revo_core::settings::default_record_path()))
            .unwrap_or_default()
    }

    fn resolve_stream_url(&self) -> String {
        match revo_backend::settings::core::settings_get() {
            Ok(settings) => {
                let url = settings.stream_url.trim();
                let key = settings.stream_key.trim();
                if key.is_empty() {
                    settings.stream_url.clone()
                } else if url.is_empty() {
                    key.to_string()
                } else if url.ends_with('/') {
                    format!("{url}{key}")
                } else {
                    format!("{url}/{key}")
                }
            }
            Err(error) => {
                self.last_error = Some(error.clone());
                String::new()
            }
        }
    }
}

fn update_source_runtime(
    backend: &mut ObsBackendRuntime,
    update: revo_core::models::SourceUpdate,
) -> Result<String, String> {
    source_ops::update_source(&backend.runtime, update)
}

fn move_direction_label(direction: MoveDirection) -> &'static str {
    match direction {
        MoveDirection::Up => "up",
        MoveDirection::Down => "down",
        MoveDirection::Top => "top",
        MoveDirection::Bottom => "bottom",
    }
}

/// Applies one SetSourceTransform command: `pos`/`rot` move through
/// `apply_sceneitem_transform` directly and pixel width/height are turned
/// into item scale against the source's current base size read from
/// `get_sceneitem_transforms` (kept exact even when dimensions drift).
fn apply_transform_command(
    backend: &mut ObsBackendRuntime,
    id: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    rotation: i32,
) -> Result<String, String> {
    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("pos_x".to_string(), x.to_string());
    params.insert("pos_y".to_string(), y.to_string());
    params.insert("rot".to_string(), rotation.to_string());

    if let Ok(transforms) = source_ops::get_sceneitem_transforms(&backend.runtime) {
        if let Some(entry) = transforms.get(id) {
            let base_width = get_i32(entry, "base_width");
            let base_height = get_i32(entry, "base_height");
            if base_width > 0 && base_height > 0 && width > 0 && height > 0 {
                let scale_x = (width as f32) / (base_width as f32);
                let scale_y = (height as f32) / (base_height as f32);
                params.insert("scale_x".to_string(), scale_x.to_string());
                params.insert("scale_y".to_string(), scale_y.to_string());
            }
        }
    }

    source_ops::apply_sceneitem_transform(&backend.runtime, id.to_string(), params)
}

fn get_i32(json: &serde_json::Value, key: &str) -> i32 {
    json.get(key)
        .and_then(|value| value.as_str())
        .and_then(|raw| raw.trim().parse::<i32>().ok())
        .or_else(|| json.get(key).and_then(|value| value.as_i64()))
        .map(|value| value as i32)
        .unwrap_or(0)
}

fn transform_value_to_placed_sources(json: &serde_json::Value) -> Result<Vec<PlacedSource>, ()> {
    let object = json.as_object().ok_or(())?;
    let mut out = Vec::new();
    for (id, value) in object {
        let x = get_i32(value, "pos_x");
        let y = get_i32(value, "pos_y");
        let width = get_i32(value, "item_width");
        let height = get_i32(value, "item_height");
        let rotation = get_i32(value, "rot");
        if width == 0 && height == 0 && x == 0 && y == 0 && rotation == 0 {
            continue;
        }
        out.push(PlacedSource {
            id: id.clone(),
            x,
            y,
            width,
            height,
            rotation,
        });
    }
    Ok(out)
}

/// Helper to assemble a typed `SourceUpdate`.
struct SourceUpdateBuilder {
    id: String,
    params: HashMap<String, String>,
}

impl SourceUpdateBuilder {
    fn new(id: String) -> Self {
        Self {
            id,
            params: HashMap::new(),
        }
    }

    fn param(mut self, key: String, value: String) -> Self {
        self.params.insert(key, value);
        self
    }

    fn update(self) -> Result<revo_core::models::SourceUpdate, String> {
        Ok(revo_core::models::SourceUpdate {
            id: self.id,
            name: String::new(),
            source_type: String::new(),
            params: self.params,
        })
    }
}

/// Captures the current scene preview frame as a PNG data-URI string.
///
/// Runs through [`revo_core::service::BackendService::call`] on the backend
/// worker thread: the libobs capture runs with the runtime lock held on a
/// background thread and the PNG encoding completes without ever blocking the
/// GPUI foreground executor.
///
/// Returns the `data:image/png;base64,...` frame, or an error for the mock
/// backend which has no libobs surface to capture.
pub fn capture_preview_png(backend: &super::AnyBackend) -> Result<String, BackendError> {
    match backend {
        super::AnyBackend::Obs(service) => service.call(|backend: &mut ObsBackendRuntime| {
            let capture = revo_backend::runtime::preview::capture_screenshot(
                &mut backend.runtime,
                None,
                None,
                None,
                0,
            )?;
            match capture {
                ScreenshotCapture::Serve(frame) => Ok(frame),
                ScreenshotCapture::Encode(captured) => {
                    revo_backend::runtime::preview::encode_screenshot(captured.frame)
                }
            }
        }),
        super::AnyBackend::Mock(_) => Err(BackendError::Runtime(
            "preview capture requires the obs backend".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn movable() -> ObsBackendRuntime {
        ObsBackendRuntime::new(None)
    }

    #[test]
    fn new_runtime_defaults_track_libobs_reference_values() {
        let runtime = movable();
        assert_eq!(runtime.transition_kind, "cut");
        assert_eq!(runtime.transition_duration_ms, 300);
        assert!(!runtime.runtime.initialized);
        assert_eq!(runtime.last_error, None);
    }

    #[test]
    fn transform_value_converts_into_placed_scene_items() {
        let json = serde_json::json!({
            "accent": {
                "pos_x": "60",
                "pos_y": "30",
                "scale_x": "0.5",
                "rot": "0",
                "base_width": "640",
                "item_width": "320",
                "item_height": "180"
            },
            "title": {
                "pos_x": "0",
                "pos_y": "0",
                "rot": "0",
                "item_width": "0",
                "item_height": "0"
            }
        });
        let placed = transform_value_to_placed_sources(&json).unwrap();
        assert_eq!(
            placed,
            vec![PlacedSource {
                id: "accent".into(),
                x: 60,
                y: 30,
                width: 320,
                height: 180,
                rotation: 0,
            }]
        );
    }

    #[test]
    fn move_direction_labels_map_to_svelte_order_strings() {
        assert_eq!(move_direction_label(MoveDirection::Up), "up");
        assert_eq!(move_direction_label(MoveDirection::Down), "down");
        assert_eq!(move_direction_label(MoveDirection::Top), "top");
        assert_eq!(move_direction_label(MoveDirection::Bottom), "bottom");
    }
}
