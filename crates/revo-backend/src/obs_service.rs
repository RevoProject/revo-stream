//! Typed command backend over the real libobs runtime.
//!
//! [`ObsBackendRuntime`] implements [`revo_core::service::BackendRuntime`]:
//! it maps every [`BackendCommand`] onto revo-backend operation modules and
//! builds [`BackendSnapshot`]s from the live runtime state. It runs on
//! whatever worker owns it (the GUI [`BackendService`] worker thread or the
//! native `revo-engine` process) and never touches a GUI framework.
//!
//! libobs owns the authoritative scene, source, output, and transition
//! state. Selection, mute, volume, and lock flags have no libobs equivalent
//! yet, so this adapter holds them until those features migrate.

use base64::Engine as _;
use std::collections::{HashMap, HashSet};

use revo_core::models::{PluginProfile, PluginProfileState};
use revo_core::service::{
    BackendCommand, BackendError, BackendRuntime, BackendSnapshot, MoveDirection, PlacedSource,
    SourceSnapshotItem,
};

use crate::runtime::lifecycle::ObsPlatform;
use crate::runtime::preview::ScreenshotCapture;
use crate::runtime::{recording, streaming};
use crate::scenes::scene_ops;
use crate::sources::source_ops;
use crate::ObsRuntime;

pub const PREVIEW_CAPTURE_INTERVAL_MS: u64 = 150;

/// Platform hooks without a GUI framework: libobs startup/shutdown run
/// directly on the calling thread. Used by the native engine process; on
/// macOS the engine wires its own main-thread dispatch before enabling the
/// release path (macOS stays experimental).
#[derive(Default)]
pub struct NativeObsPlatform;

impl ObsPlatform for NativeObsPlatform {
    fn startup(&self, locale: &std::ffi::CStr, config: &std::ffi::CStr) -> Result<(), String> {
        // SAFETY: The caller serializes the libobs lifecycle, and both C
        // strings remain valid for the duration of obs_startup.
        if unsafe {
            revo_lib::obs::obs_startup(locale.as_ptr(), config.as_ptr(), std::ptr::null_mut())
        } {
            Ok(())
        } else {
            Err("obs_startup failed".to_string())
        }
    }

    fn shutdown(&self) -> Result<(), String> {
        // SAFETY: The caller established that the libobs core is active, and
        // teardown runs on the same serialized lifecycle.
        unsafe { revo_lib::obs::obs_shutdown() };
        Ok(())
    }
}

/// [`revo_core::service::BackendRuntime`] implementation over the real
/// revo-backend ObsRuntime.
pub struct ObsBackendRuntime {
    pub(crate) runtime: ObsRuntime,
    platform: Box<dyn ObsPlatform + Send>,
    root_dir: Option<String>,
    last_error: Option<String>,
    /// Persisted settings as loaded at construction and updated by
    /// `SaveSettings`; surfaced through the snapshot for the settings UI.
    settings: Option<revo_core::models::AppSettings>,
    // UI-mode and selection state kept on the adapter.
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
    /// Plugin module enablement and profile state. libobs owns the module
    /// list; the adapter tracks per-profile enablement and the active
    /// profile until profile persistence is fully migrated.
    plugin_modules: HashMap<String, bool>,
    plugin_profiles: Vec<PluginProfileState>,
    active_plugin_profile: Option<String>,
    /// Live Settings option catalog (encoders/formats/devices) refreshed on
    /// `StartObs`; surfaced verbatim through every snapshot (plan §6–8).
    select_options: Option<revo_core::service::SnapshotSelectOptions>,
}

impl ObsBackendRuntime {
    pub fn new(root_dir: Option<String>, platform: Box<dyn ObsPlatform + Send>) -> Self {
        Self {
            runtime: ObsRuntime::default(),
            platform,
            root_dir,
            last_error: None,
            settings: crate::settings::core::settings_get().ok(),
            selected_source: None,
            muted_sources: Vec::new(),
            source_volumes: Vec::new(),
            volume_locked_sources: Vec::new(),
            locked_sources: Vec::new(),
            transition_kind: "cut".to_string(),
            transition_duration_ms: 300,
            studio_mode: false,
            realtime_refresh: true,
            preview_resolution_requests: 0,
            plugin_modules: HashMap::new(),
            plugin_profiles: Vec::new(),
            active_plugin_profile:
                crate::settings::plugins_profiles::plugin_profile_active_name_internal().ok(),
            select_options: None,
        }
    }

    /// Recomputes the Settings catalogs from libobs + the audio system and
    /// caches them on the adapter. Called after successful OBS startup;
    /// failures fall back to the static defaults so the dropdowns are never
    /// empty. Device enumeration runs `pactl` once here (not per snapshot).
    pub(crate) fn refresh_select_options(&mut self) {
        let defaults = crate::settings::encoder_options::defaults_obs_settings_select_options();
        let merged = crate::settings::encoder_options::get_settings_select_options(defaults)
            .unwrap_or_else(|_| {
                crate::settings::encoder_options::defaults_obs_settings_select_options()
            });
        let output_devices =
            crate::devices::audio::list_pulse_devices("output").unwrap_or_default();
        let input_devices = crate::devices::audio::list_pulse_devices("input").unwrap_or_default();
        self.select_options = Some(revo_core::service::SnapshotSelectOptions::from_parts(
            &merged,
            &output_devices,
            &input_devices,
        ));
    }

    /// Rebuilds the session module map so every discovered module maps to
    /// its enabled flag for the given enabled list.
    fn sync_module_map(&mut self, enabled_modules: &[String]) {
        let available =
            crate::settings::plugins_profiles::list_runtime_plugins_internal().unwrap_or_default();
        for plugin in available {
            let enabled = enabled_modules
                .iter()
                .any(|module| *module == plugin.module_name);
            self.plugin_modules.insert(plugin.module_name, enabled);
        }
    }

    /// Enabled module names for the snapshot: explicit session toggles when
    /// any exist, otherwise every discovered module (the Svelte UI defaults
    /// to all-enabled until a profile is applied).
    fn enabled_modules_snapshot(&self) -> Vec<String> {
        if self.plugin_modules.is_empty() {
            return crate::settings::plugins_profiles::list_runtime_plugins_internal()
                .unwrap_or_default()
                .into_iter()
                .map(|plugin| plugin.module_name)
                .collect();
        }
        let mut enabled: Vec<String> = self
            .plugin_modules
            .iter()
            .filter(|(_, enabled)| **enabled)
            .map(|(module, _)| module.clone())
            .collect();
        enabled.sort();
        enabled
    }

    /// Captures a preview frame as a `data:image/png;base64,...` URI.
    ///
    /// Without a `source_id` the current scene composite is captured (the
    /// workspace preview loop); with one, the matching source is captured
    /// individually (the preview auxiliary window). Runs on the owner's
    /// worker (libobs capture with the runtime lock held) — never on a GUI
    /// main thread.
    pub fn capture_preview_png(&mut self, source_id: Option<&str>) -> Result<String, BackendError> {
        let capture = crate::runtime::preview::capture_screenshot(
            &mut self.runtime,
            None,
            None,
            source_id.map(str::to_owned),
            0,
        )
        .map_err(BackendError::Runtime)?;
        match capture {
            ScreenshotCapture::Serve(frame) => Ok(frame),
            ScreenshotCapture::Encode(captured) => {
                crate::runtime::preview::encode_screenshot(captured.frame)
                    .map_err(BackendError::Runtime)
            }
        }
    }

    /// Opens a persistent headless-Chromium CDP session for one browser
    /// source, resolving its interaction settings through the runtime lock.
    pub fn interact_open(&mut self, source_id: &str) -> Result<(), BackendError> {
        let (url, css, js, user_agent, width, height) =
            source_ops::read_interaction_source(&self.runtime, source_id)
                .map_err(BackendError::Runtime)?;
        crate::sources::browser_interact::ensure_session(
            source_id,
            &url,
            &user_agent,
            &css,
            &js,
            (width.clamp(64, 4096) as u32, height.clamp(64, 4096) as u32),
        )
        .map_err(BackendError::Runtime)
    }

    /// Fetches the newest browser-source frame as a data-URI string; `None`
    /// while the session has not produced a frame yet. Blocks on the
    /// session's screencast wait — worker threads only.
    pub fn interact_frame(&mut self, source_id: &str) -> Result<Option<String>, BackendError> {
        if !self.runtime.initialized {
            return Err(BackendError::Runtime("OBS not initialized".into()));
        }
        let png = crate::sources::browser_interact::frame_png(source_id)
            .map_err(BackendError::Runtime)?;
        Ok(Some(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        )))
    }

    /// Forwards one typed mouse interaction to the source's CDP session.
    pub fn interact_mouse(
        &mut self,
        source_id: &str,
        kind: &str,
        x: f64,
        y: f64,
        button: &str,
        buttons: i64,
        click_count: i64,
        delta_x: f64,
        delta_y: f64,
    ) -> Result<(), BackendError> {
        if !self.runtime.initialized {
            return Err(BackendError::Runtime("OBS not initialized".into()));
        }
        crate::sources::browser_interact::dispatch_mouse(
            source_id,
            kind,
            x,
            y,
            button,
            buttons,
            click_count,
            delta_x,
            delta_y,
        )
        .map_err(BackendError::Runtime)
    }

    /// Forwards one typed key interaction to the source's CDP session.
    pub fn interact_key(
        &mut self,
        source_id: &str,
        kind: &str,
        key: &str,
        code: &str,
        key_code: i64,
        text: &str,
    ) -> Result<(), BackendError> {
        if !self.runtime.initialized {
            return Err(BackendError::Runtime("OBS not initialized".into()));
        }
        crate::sources::browser_interact::dispatch_key(source_id, kind, key, code, key_code, text)
            .map_err(BackendError::Runtime)
    }

    /// Re-runs the session's custom CSS/JS injection.
    pub fn interact_reapply(&mut self, source_id: &str) -> Result<(), BackendError> {
        if !self.runtime.initialized {
            return Err(BackendError::Runtime("OBS not initialized".into()));
        }
        crate::sources::browser_interact::reapply_code(source_id).map_err(BackendError::Runtime)
    }

    /// Closes the source's CDP session and its headless browser.
    /// Fire-and-forget: errors cannot reach the UI once the window is gone.
    pub fn interact_close(&mut self, source_id: &str) {
        if self.runtime.initialized {
            crate::sources::browser_interact::close_session(source_id);
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

        let sources = crate::collect_sources(runtime);
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
            .and_then(|value| transform_value_to_placed_sources(&value).ok())
            .unwrap_or_default();

        let audio_levels = crate::devices::levels::get_levels()
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
            settings: self.settings.clone(),
            plugins: crate::settings::plugins_profiles::list_runtime_plugins_internal()
                .unwrap_or_default(),
            plugin_profiles: crate::settings::plugins_profiles::plugin_profiles_list()
                .unwrap_or_default(),
            plugin_profile_states: crate::settings::plugins_profiles::plugin_profiles_list()
                .unwrap_or_default()
                .iter()
                .filter_map(|name| {
                    crate::settings::plugins_profiles::plugin_profile_get_internal(name)
                        .ok()
                        .map(|profile| crate::models::PluginProfileState {
                            profile_name: name.clone(),
                            enabled_modules: profile.enabled_modules,
                        })
                })
                .collect(),
            available_source_types: if runtime.initialized {
                source_ops::list_external_source_types(runtime).unwrap_or_default()
            } else {
                Vec::new()
            },
            active_plugin_profile: self.active_plugin_profile.clone(),
            enabled_plugin_modules: self.enabled_modules_snapshot(),
            select_options: self.select_options.clone(),
        })
    }

    fn shutdown(&mut self) -> Result<(), BackendError> {
        let result =
            crate::runtime::init::shutdown_with_platform(&mut self.runtime, &*self.platform);
        if let Err(message) = &result {
            self.last_error = Some(message.clone());
        } else {
            self.last_error = None;
            // The catalogs describe loaded modules; drop them with the
            // runtime so a later snapshot never serves a stale list.
            self.select_options = None;
        }
        Ok(())
    }
}

impl ObsBackendRuntime {
    /// Applies one command against the real backend. Every variant maps onto a
    /// revo-backend call; undo/redo history (not yet migrated off the Tauri
    /// command journal) is recorded as a no-op.
    pub(crate) fn apply(&mut self, command: BackendCommand) -> Result<String, String> {
        match command {
            // Undo/redo history has no libobs backing yet; the Tauri adapter
            // treats both commands the same way (recorded + no-op pending the
            // command-journal migration).
            BackendCommand::Undo | BackendCommand::Redo => Ok("history not migrated yet".into()),
            BackendCommand::SaveSettings { settings } => {
                let saved = crate::settings::core::settings_save(settings.clone());
                match saved {
                    Ok(message) => {
                        self.settings = Some(settings);
                        Ok(message)
                    }
                    Err(message) => Err(message),
                }
            }
            BackendCommand::SetStudioMode(enabled) => {
                self.studio_mode = enabled;
                Ok(format!("studio mode: {enabled}"))
            }
            BackendCommand::SetRealtimeRefresh(enabled) => {
                self.realtime_refresh = enabled;
                Ok(format!("realtime refresh: {enabled}"))
            }
            BackendCommand::RefreshSelectOptions => {
                if !self.runtime.initialized {
                    return Err("OBS not initialized".into());
                }
                self.refresh_select_options();
                Ok("select options refreshed".into())
            }
            BackendCommand::StartObs => {
                let started = crate::runtime::init::start_with_platform(
                    &mut self.runtime,
                    &*self.platform,
                    self.root_dir.clone(),
                );
                if started.is_ok() {
                    // Publish the live Settings catalogs once libobs is up
                    // (plan §6–8): encoders come from the loaded modules and
                    // the audio catalog from pulse/alsa.
                    self.refresh_select_options();
                }
                started
            }
            BackendCommand::StopObs => {
                self.muted_sources.clear();
                self.source_volumes.clear();
                self.select_options = None;
                crate::runtime::init::shutdown_with_platform(&mut self.runtime, &*self.platform)
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
                scene_ops::reorder_scene(&mut self.runtime, name.clone(), to_index)
                    .map(|()| format!("scene reordered: {name} -> {to_index}"))
            }
            BackendCommand::SetSceneLock { name, locked } => {
                scene_ops::set_scene_lock(&mut self.runtime, name, locked)
            }
            BackendCommand::RemoveScene(name) => scene_ops::remove_scene(&mut self.runtime, name),
            BackendCommand::AddScene(name) => scene_ops::create_scene(&mut self.runtime, name),
            BackendCommand::AddSource { name, source_type } => {
                let slug = slugify_source_name(&name);
                let existing: HashSet<String> = crate::collect_sources(&self.runtime)
                    .iter()
                    .map(|source| source.id.clone())
                    .collect();
                let mut id = slug.clone();
                let mut counter = 1;
                while existing.contains(&id) {
                    counter += 1;
                    id = format!("{slug}-{counter}");
                }
                source_ops::create_source(
                    &mut self.runtime,
                    revo_core::models::SourceCreate {
                        id,
                        name,
                        source_type,
                        params: HashMap::new(),
                    },
                )
            }
            BackendCommand::SelectSource(id) => {
                self.selected_source = Some(id.clone());
                Ok(format!("selected source: {id}"))
            }
            BackendCommand::ToggleSourceVisibility(id) => {
                let visible = crate::collect_sources(&self.runtime)
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
                source_ops::move_source(&self.runtime, id.clone(), direction.to_string())
                    .map(|()| format!("source moved: {id} {direction}"))
            }
            BackendCommand::SetSourceFilters { id, filters } => {
                source_ops::set_source_filters(&self.runtime, id.clone(), filters)
                    .map(|message| format!("source filters updated: {id}: {message}"))
            }
            BackendCommand::UpdateSource { update } => {
                source_ops::update_source(&self.runtime, update)
                    .map(|message| format!("source updated: {message}"))
            }
            BackendCommand::RemoveSource(id) => {
                source_ops::remove_source(&mut self.runtime, id.clone()).map(|message| {
                    self.locked_sources.retain(|source| source != &id);
                    self.muted_sources.retain(|source| source != &id);
                    self.source_volumes.retain(|(source, _)| source != &id);
                    self.volume_locked_sources.retain(|source| source != &id);
                    if self.selected_source.as_deref() == Some(id.as_str()) {
                        self.selected_source = None;
                    }
                    message
                })
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
            BackendCommand::SetTransitionParams(params) => {
                scene_ops::set_program_transition_params(&mut self.runtime, &params)
            }
            BackendCommand::SetSourceTransform {
                id,
                x,
                y,
                width,
                height,
                rotation,
            } => apply_transform_command(self, &id, x, y, width, height, rotation),
            BackendCommand::SetPluginProfile {
                profile_name,
                modules,
            } => {
                let available: HashSet<String> =
                    crate::settings::plugins_profiles::list_runtime_plugins_internal()?
                        .into_iter()
                        .map(|plugin| plugin.module_name)
                        .collect();
                let normalized = revo_core::plugins::normalize_enabled_modules(modules, &available);
                let profile = PluginProfile {
                    enabled_modules: normalized.clone(),
                };
                crate::settings::plugins_profiles::plugin_profile_save_internal(
                    &profile_name,
                    &profile,
                )?;
                match self
                    .plugin_profiles
                    .iter_mut()
                    .find(|existing| existing.profile_name == profile_name)
                {
                    Some(existing) => existing.enabled_modules = normalized.clone(),
                    None => self.plugin_profiles.push(PluginProfileState {
                        profile_name: profile_name.clone(),
                        enabled_modules: normalized.clone(),
                    }),
                }
                self.sync_module_map(&normalized);
                Ok(format!("saved plugin profile '{profile_name}'"))
            }
            BackendCommand::SetActivePluginProfile(name) => {
                let profile =
                    crate::settings::plugins_profiles::plugin_profile_get_internal(&name)?;
                self.active_plugin_profile = Some(name);
                self.sync_module_map(&profile.enabled_modules);
                Ok(format!(
                    "activated plugin profile '{}'",
                    self.active_plugin_profile.as_deref().unwrap_or_default()
                ))
            }
            BackendCommand::ExportRevoSceneCollection { path } => {
                let json = crate::scenes::collection_io::export_scene_collection(&self.runtime)?;
                crate::scenes::collection_io::export_scene_collection_to_file(json, path, None)
            }
            BackendCommand::ImportRevoSceneCollection { path } => {
                let json = std::fs::read_to_string(&path)
                    .map_err(|e| format!("failed to read '{path}': {e}"))?;
                crate::scenes::collection_io::import_scene_collection(&mut self.runtime, json)
            }
            BackendCommand::ExportObsSceneCollection { path } => {
                let json =
                    crate::scenes::collection_io::export_scene_collection_obs(&self.runtime)?;
                crate::scenes::collection_io::export_scene_collection_to_file(json, path, None)
            }
            BackendCommand::ImportObsSceneCollection { path } => {
                let json = std::fs::read_to_string(&path)
                    .map_err(|e| format!("failed to read '{path}': {e}"))?;
                crate::scenes::collection_io::import_scene_collection(&mut self.runtime, json)
            }
            BackendCommand::ExtractDefaultTheme { root_dir } => {
                let root = if root_dir.trim().is_empty() {
                    crate::settings::core::resolve_root_dir(None)
                        .map(|path| path.to_string_lossy().to_string())
                        .map_err(|error| error)?
                } else {
                    root_dir
                };
                crate::settings::themes::themes_extract_default(
                    Some(root),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }
            BackendCommand::ExportProfile { path } => {
                crate::settings::profile_obs::export_profile_obs_to_file(path)
            }
            BackendCommand::ImportProfile { path } => {
                let json = std::fs::read_to_string(&path)
                    .map_err(|e| format!("failed to read '{path}': {e}"))?;
                crate::settings::profile_obs::import_profile_obs(json)
            }
        }
    }

    fn resolve_record_path(&self) -> String {
        crate::settings::core::settings_get()
            .ok()
            .map(|settings| settings.record_path)
            .filter(|path| !path.trim().is_empty())
            .or_else(|| Some(revo_core::settings::default_record_path()))
            .unwrap_or_default()
    }

    fn resolve_stream_url(&mut self) -> String {
        match crate::settings::core::settings_get() {
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
        .or_else(|| {
            json.get(key)
                .and_then(|value| value.as_i64())
                .map(|value| value as i32)
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn movable() -> ObsBackendRuntime {
        ObsBackendRuntime::new(None, Box::new(NativeObsPlatform))
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

    #[test]
    fn plugin_profile_commands_persist_and_apply() {
        let mut backend = movable();
        backend
            .apply(BackendCommand::SetPluginProfile {
                profile_name: "native-test".into(),
                modules: vec!["obs-websocket".into()],
            })
            .expect("profile save should succeed");
        let snapshot = backend.snapshot().expect("snapshot should build");
        assert!(snapshot
            .plugin_profiles
            .iter()
            .any(|name| name == "native-test"));
        // Module enablement normalizes against the discovered module list;
        // the test runtime discovers nothing, so the persisted profile
        // stores an empty set. The profile tabs read exactly what the
        // backend persisted (the mock test covers the full module
        // round-trip without normalization).
        let saved_state = snapshot
            .plugin_profile_states
            .iter()
            .find(|state| state.profile_name == "native-test")
            .expect("saved profile state in snapshot");
        assert_eq!(saved_state.enabled_modules, Vec::<String>::new());

        backend
            .apply(BackendCommand::SetActivePluginProfile("native-test".into()))
            .expect("profile activation should succeed");
        let snapshot = backend.snapshot().expect("snapshot should build");
        assert_eq!(
            snapshot.active_plugin_profile.as_deref(),
            Some("native-test")
        );
    }
}
