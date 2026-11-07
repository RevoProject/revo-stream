//! Framework-neutral libobs backend extracted from the Tauri adapter.
//!
//! This crate owns the raw libobs runtime state, scenes/sources/settings
//! operation modules, logging, and platform-neutral device listing. GUI
//! adapters (currently the Tauri app) wrap these items behind their own
//! command surface.

pub mod devices;
pub mod engine;
pub mod logging;
pub mod models;
pub mod obs_service;
pub mod permissions;
pub mod runtime;
pub mod scenes;
pub mod settings;
pub mod sources;
pub mod utils;

pub use models::*;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use revo_lib::obs;
use serde_json::Value as JsonValue;

pub struct ObsRuntime {
    pub initialized: bool,
    pub core_started: bool,
    pub scenes: HashMap<String, SceneState>,
    pub current_scene: Option<String>,
    pub locked_scenes: HashSet<String>,
    pub scene_order: Vec<String>,
    pub output_record: *mut obs::obs_output,
    pub record_video_encoder: *mut obs::obs_encoder,
    pub record_audio_encoder: *mut obs::obs_encoder,
    pub output_stream: *mut obs::obs_output,
    pub stream_service: *mut obs::obs_service,
    pub stream_video_encoder: *mut obs::obs_encoder,
    pub stream_audio_encoder: *mut obs::obs_encoder,
    pub preview_view: *mut obs::obs_view,
    pub preview_texrender: *mut obs::gs_texrender_t,
    /// When true, scene-level preview captures serve the retained last good
    /// frame instead of rendering. Used by the Add Source workflow so source
    /// creation does not contend with preview rendering. Recording and
    /// streaming outputs are unaffected.
    pub preview_freeze: bool,
    pub preview_freeze_reason: Option<String>,
    pub preview_freeze_requests: u64,
    pub preview_freeze_since: Option<std::time::Instant>,
    /// Last successfully captured preview frame (data URI). Returned when a
    /// fresh capture fails, so the UI keeps a good frame instead of an error
    /// or a wrongly-rendered main/output texture.
    pub last_preview_frame: Option<String>,
    pub last_preview_frame_generation: u64,
    pub last_preview_frame_at: Option<std::time::Instant>,
    pub last_preview_error_at: Option<std::time::Instant>,
    pub preview_frame_seq: u64,
    pub last_preview_frame_seq: u64,
    pub scene_generation: u64,
    pub last_record_path: Option<PathBuf>,
    pub video_encoder_preference: VideoEncoderPreference,
    pub scene_resolution: String,
    pub planner_init: Option<PlannerInit>,
    pub transition_source: *mut obs::obs_source,
    pub program_transition_kind: String,
    pub program_transition_duration_ms: u32,
}

impl Default for ObsRuntime {
    fn default() -> Self {
        Self {
            initialized: false,
            core_started: false,
            scenes: HashMap::new(),
            current_scene: None,
            locked_scenes: HashSet::new(),
            scene_order: Vec::new(),
            output_record: std::ptr::null_mut(),
            record_video_encoder: std::ptr::null_mut(),
            record_audio_encoder: std::ptr::null_mut(),
            output_stream: std::ptr::null_mut(),
            stream_service: std::ptr::null_mut(),
            stream_video_encoder: std::ptr::null_mut(),
            stream_audio_encoder: std::ptr::null_mut(),
            preview_view: std::ptr::null_mut(),
            preview_texrender: std::ptr::null_mut(),
            preview_freeze: false,
            preview_freeze_reason: None,
            preview_freeze_requests: 0,
            preview_freeze_since: None,
            last_preview_frame: None,
            last_preview_frame_generation: 0,
            last_preview_frame_at: None,
            last_preview_error_at: None,
            preview_frame_seq: 0,
            last_preview_frame_seq: 0,
            scene_generation: 0,
            last_record_path: None,
            video_encoder_preference: VideoEncoderPreference::default(),
            scene_resolution: "1920x1080".to_string(),
            planner_init: None,
            transition_source: std::ptr::null_mut(),
            program_transition_kind: "cut".to_string(),
            program_transition_duration_ms: 300,
        }
    }
}

pub struct SceneState {
    pub name: String,
    pub scene: *mut obs::obs_scene,
    pub scene_source: *mut obs::obs_source,
    pub item_accent: *mut obs::obs_scene_item,
    pub item_text: *mut obs::obs_scene_item,
    pub custom_items: HashMap<String, *mut obs::obs_scene_item>,
}

impl SceneState {
    pub fn new(
        name: String,
        scene: *mut obs::obs_scene,
        scene_source: *mut obs::obs_source,
    ) -> Self {
        SceneState {
            name,
            scene,
            scene_source,
            item_accent: std::ptr::null_mut(),
            item_text: std::ptr::null_mut(),
            custom_items: HashMap::new(),
        }
    }
}

pub struct ObsState {
    pub runtime: Mutex<ObsRuntime>,
}

#[derive(Clone, Default, serde::Serialize)]
pub struct PreviewStats {
    pub requests: u64,
    pub frozen_returns: u64,
    pub stale_returns: u64,
    pub captures_ok: u64,
    pub captures_err: u64,
    pub encoded_ok: u64,
    pub encoded_err: u64,
    pub stale_discards: u64,
    pub lock_wait_us_max: u64,
    pub capture_us_max: u64,
    pub encode_us_max: u64,
    pub last_error: String,
}

fn preview_stats() -> &'static Mutex<PreviewStats> {
    static PREVIEW_STATS: std::sync::OnceLock<Mutex<PreviewStats>> = std::sync::OnceLock::new();
    PREVIEW_STATS.get_or_init(|| Mutex::new(PreviewStats::default()))
}

pub fn with_preview_stats<T>(f: impl FnOnce(&mut PreviewStats) -> T) -> T {
    let mut guard = preview_stats()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

pub fn clone_preview_stats() -> PreviewStats {
    with_preview_stats(|stats| stats.clone())
}

// Raw OBS pointers are not thread-safe by default. We guard all access with a Mutex
// and only touch them on the command thread.
unsafe impl Send for ObsRuntime {}
unsafe impl Sync for ObsRuntime {}

pub fn init_debug_log_file() -> Result<PathBuf, String> {
    logging::debug::init_debug_log_file()
}

pub fn push_debug_log_entry(action: String, detail: Option<JsonValue>) {
    logging::debug::push_debug_log_entry(action, detail)
}

pub fn resolve_scene_item(scene: &SceneState, id: &str) -> Option<*mut obs::obs_scene_item> {
    scenes::scene_items::resolve_scene_item(scene, id)
}

pub fn scene_item_id(scene: &SceneState, item: *mut obs::obs_scene_item) -> Option<String> {
    scenes::scene_items::scene_item_id(scene, item)
}

pub fn current_scene(runtime: &ObsRuntime) -> Result<&SceneState, String> {
    scenes::scene_state::current_scene(runtime)
}

pub fn current_scene_mut(runtime: &mut ObsRuntime) -> Result<&mut SceneState, String> {
    scenes::scene_state::current_scene_mut(runtime)
}

pub fn set_current_scene_internal(runtime: &mut ObsRuntime, name: &str) -> Result<(), String> {
    scenes::scene_state::set_current_scene_internal(runtime, name)
}

pub fn extract_source_params(settings: *mut obs::obs_data, params: &mut HashMap<String, String>) {
    sources::helpers::extract_source_params(settings, params)
}

pub fn collect_source_property_specs(
    props: *mut obs::obs_properties_t,
    out: &mut Vec<SourcePropertySpec>,
) {
    sources::helpers::collect_source_property_specs(props, out)
}

pub fn refresh_source_properties_with_settings(
    props: *mut obs::obs_properties_t,
    settings: *mut obs::obs_data_t,
) {
    sources::helpers::refresh_source_properties_with_settings(props, settings)
}

pub fn source_editable_list_keys(source: *mut obs::obs_source_t) -> HashSet<String> {
    sources::helpers::source_editable_list_keys(source)
}

pub fn live_scene_resolution() -> Option<String> {
    sources::helpers::live_scene_resolution()
}

pub fn apply_scene_item_transform(
    item: *mut obs::obs_scene_item,
    source: *mut obs::obs_source,
    params: &HashMap<String, String>,
) {
    sources::helpers::apply_scene_item_transform(item, source, params)
}

pub fn apply_source_params(
    settings: *mut obs::obs_data,
    source_type: &str,
    params: &HashMap<String, String>,
    editable_list_keys: Option<&HashSet<String>>,
) {
    sources::helpers::apply_source_params(settings, source_type, params, editable_list_keys)
}

pub fn collect_sources(runtime: &ObsRuntime) -> Vec<SourceInfo> {
    sources::source_ops::collect_sources(runtime)
}

pub fn create_source_in_scene(
    runtime: &mut ObsRuntime,
    create: &SourceCreate,
) -> Result<String, String> {
    sources::source_ops::create_source_in_scene(runtime, create)
}

pub fn abgr_to_hex(abgr: u32) -> String {
    sources::helpers::abgr_to_hex(abgr)
}
