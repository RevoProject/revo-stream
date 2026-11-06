pub mod ui;
pub mod runtime;

pub use revo_backend::{devices, logging, models, permissions, scenes, settings, sources, utils};
pub use revo_backend::{
    ObsRuntime, SceneState, ObsState, PreviewStats, abgr_to_hex, apply_scene_item_transform,
    apply_source_params, clone_preview_stats, collect_source_property_specs, collect_sources,
    create_source_in_scene, current_scene, current_scene_mut, extract_source_params,
    init_debug_log_file, live_scene_resolution, push_debug_log_entry, refresh_source_properties_with_settings,
    resolve_scene_item, scene_item_id, set_current_scene_internal, source_editable_list_keys,
    with_preview_stats,
};

pub(crate) use models::*;

use std::collections::HashMap;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
// (no threading imports needed here)

use uuid::Uuid;

use serde_json::Value as JsonValue;

struct AppUiState {
    minimize_to_tray: AtomicBool,
}


#[tauri::command(async)]
fn obs_get_settings_select_options(state: tauri::State<ObsState>) -> Result<ObsSettingsSelectOptions, String> {
    let defaults = settings::encoder_options::defaults_obs_settings_select_options();
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Ok(defaults);
    }
    drop(runtime);
    settings::encoder_options::get_settings_select_options(defaults)
}

#[tauri::command(async)]
fn obs_list_pulse_devices(kind: String) -> Result<Vec<AudioDevice>, String> {
    devices::audio::list_pulse_devices(&kind)
}

#[tauri::command(async)]
fn obs_list_alsa_devices() -> Result<Vec<AudioDevice>, String> {
    devices::audio::list_alsa_devices()
}

#[tauri::command(async)]
fn obs_list_window_picker_items() -> Result<Vec<SourceTypeItem>, String> {
    sources::window_picker::list_window_picker_items()
}

#[tauri::command(async)]
fn obs_list_system_fonts() -> Result<Vec<SourceTypeItem>, String> {
    sources::fonts::list_system_fonts()
}

#[tauri::command(async)]
fn obs_list_video_device_picker_items() -> Result<Vec<SourceTypeItem>, String> {
    sources::video_devices::list_video_device_picker_items()
}

// Runs off the UI thread: enumerating input types may contend with the
// preview loop on the runtime lock and must not block the main thread.
#[tauri::command(async)]
fn obs_list_external_source_types(state: tauri::State<ObsState>) -> Result<Vec<SourceTypeItem>, String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Ok(vec![]);
    }
    sources::source_ops::list_external_source_types(&runtime)
}

fn resolve_scene_item(scene: &SceneState, id: &str) -> Option<*mut obs::obs_scene_item> {
    scenes::scene_items::resolve_scene_item(scene, id)
}

fn scene_item_id(scene: &SceneState, item: *mut obs::obs_scene_item) -> Option<String> {
    scenes::scene_items::scene_item_id(scene, item)
}

fn current_scene(runtime: &ObsRuntime) -> Result<&SceneState, String> {
    scenes::scene_state::current_scene(runtime)
}

fn current_scene_mut(runtime: &mut ObsRuntime) -> Result<&mut SceneState, String> {
    scenes::scene_state::current_scene_mut(runtime)
}

fn set_current_scene_internal(runtime: &mut ObsRuntime, name: &str) -> Result<(), String> {
    scenes::scene_state::set_current_scene_internal(runtime, name)
}

#[tauri::command(async)]
fn obs_list_scenes(state: tauri::State<ObsState>) -> Result<Vec<SceneInfo>, String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Ok(vec![]);
    }
    scenes::scene_ops::list_scenes(&runtime)
}

#[tauri::command(async)]
fn obs_set_program_transition(
    state: tauri::State<ObsState>,
    kind: String,
    duration_ms: Option<u32>,
) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    scenes::scene_ops::set_program_transition(&mut runtime, kind, duration_ms)
}

#[tauri::command(async)]
fn obs_set_current_scene(state: tauri::State<ObsState>, name: String) -> Result<String, String> {
    let mut runtime = crate::logging::mutex_probe::probe_lock(&state.runtime, "runtime.set_current_scene")
        .map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    scenes::scene_ops::set_current_scene(&mut runtime, name)
}

#[tauri::command]
fn obs_create_scene(state: tauri::State<ObsState>, name: String) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    scenes::scene_ops::create_scene(&mut runtime, name)
}

#[tauri::command]
fn obs_rename_scene(state: tauri::State<ObsState>, old_name: String, new_name: String) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    scenes::scene_ops::rename_scene(&mut runtime, old_name, new_name)
}

#[tauri::command]
fn obs_remove_scene(state: tauri::State<ObsState>, name: String) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    scenes::scene_ops::remove_scene(&mut runtime, name)
}

#[tauri::command]
fn obs_set_scene_lock(state: tauri::State<ObsState>, name: String, locked: bool) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    scenes::scene_ops::set_scene_lock(&mut runtime, name, locked)
}

#[tauri::command]
fn open_preview_window(app: tauri::AppHandle, source: Option<String>) -> Result<(), String> {
    let url = match &source {
        Some(name) => format!("preview?source={}", url_encode(name)),
        None => "preview".to_string(),
    };
    tauri::WebviewWindowBuilder::new(&app, "revo-preview", tauri::WebviewUrl::App(url.into()))
        .title("Preview - RevoStream")
        .inner_size(960.0, 540.0)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn url_encode(input: &str) -> String {
    let mut result = String::new();
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => result.push(byte as char),
            b' ' => result.push_str("%20"),
            _ => result.push_str(&format!("%{:02X}", byte)),
        }
    }
    result
}

#[tauri::command]
fn open_graphic_planner(app: tauri::AppHandle) -> Result<(), String> {
    ui::dock::open_graphic_planner(app)
}

#[tauri::command]
fn open_browser_dock(app: tauri::AppHandle, url: Option<String>) -> Result<String, String> {
    ui::dock::open_browser_dock(app, url)
}

#[tauri::command]
fn browser_dock_state(app: tauri::AppHandle) -> Result<BrowserDockState, String> {
    ui::dock::browser_dock_state(app)
}

#[tauri::command]
fn close_browser_dock(app: tauri::AppHandle) -> Result<(), String> {
    ui::dock::close_browser_dock(app)
}

#[tauri::command]
fn open_debug_console(app: tauri::AppHandle) -> Result<(), String> {
    ui::dock::open_debug_console(app)
}

#[tauri::command]
fn app_close_all_windows(app: tauri::AppHandle, state: tauri::State<ObsState>) -> Result<(), String> {
    use tauri::Manager;

    if let Err(err) = runtime::init::shutdown(app.clone(), state) {
        eprintln!("[shutdown] failed before closing application windows: {err}");
    }

    let windows: Vec<_> = app.webview_windows().into_iter().map(|(_, w)| w).collect();
    for window in windows {
        let _ = window.close();
    }
    Ok(())
}

fn restore_main_window(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn app_set_minimize_to_tray(
    app: tauri::AppHandle,
    state: tauri::State<AppUiState>,
    enabled: bool,
) -> Result<(), String> {
    state.minimize_to_tray.store(enabled, Ordering::Relaxed);
    if let Some(tray) = app.tray_by_id("revo-main-tray") {
        tray.set_visible(enabled).map_err(|error| error.to_string())?;
    }
    if !enabled {
        restore_main_window(&app);
    }
    Ok(())
}

#[tauri::command]
fn app_set_builtin_browser_enabled(
    state: tauri::State<ObsState>,
    enabled: bool,
) -> Result<(), String> {
    if enabled && sources::browser::chromium_binary().is_none() {
        sources::browser::set_enabled(false);
        return Err("Built-in Browser requires Chromium, Chrome, Brave or Edge".to_string());
    }
    let initialized = state
        .runtime
        .lock()
        .map_err(|_| "state poisoned".to_string())?
        .initialized;
    apply_builtin_browser_enabled(enabled, initialized)
}

// Browser registration and session teardown must run outside the runtime
// lock, so the command passes only the observed `initialized` flag.
fn apply_builtin_browser_enabled(enabled: bool, initialized: bool) -> Result<(), String> {
    sources::browser::set_enabled(enabled);
    if enabled && initialized {
        if let Err(error) = sources::browser::register() {
            sources::browser::set_enabled(false);
            return Err(error);
        }
    }
    if !enabled {
        sources::browser_interact::close_all();
    }
    Ok(())
}

#[tauri::command]
fn cef_bridge_info() -> Result<CefBridgeInfo, String> {
    ui::cef::bridge_info()
}

#[tauri::command]
async fn cef_dock_render_frame(url: String, width: u32, height: u32) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || ui::cef::dock_render_frame(url, width, height))
        .await
        .map_err(|err| format!("browser capture task failed: {err}"))?
}

#[tauri::command]
fn debug_console_log(
    action: String,
    detail: Option<JsonValue>,
) -> Result<(), String> {
    logging::debug::debug_console_log(action, detail)
}

#[tauri::command]
fn debug_console_history() -> Result<Vec<LibObsActionEvent>, String> {
    logging::debug::debug_console_history()
}

#[tauri::command]
fn debug_console_clear() -> Result<(), String> {
    logging::debug::debug_console_clear()
}

#[tauri::command]
fn debug_trace_last() -> Result<JsonValue, String> {
    let (step, age_ms) = logging::trace::last()
        .unwrap_or_else(|| ("(no trace recorded)".to_string(), 0));
    Ok(serde_json::json!({
        "step": step,
        "age_ms": age_ms,
        "scopes": logging::trace::snapshot(),
    }))
}

#[tauri::command(async)]
fn debug_preview_stats(
    state: tauri::State<ObsState>,
    ui: Option<JsonValue>,
) -> Result<JsonValue, String> {
    let stats = clone_preview_stats();
    let (runtime_lock, runtime) = match state.runtime.try_lock() {
        Ok(runtime) => (
            "ok",
            Some(serde_json::json!({
                "preview_freeze": runtime.preview_freeze,
                "preview_freeze_reason": runtime.preview_freeze_reason,
                "preview_freeze_requests": runtime.preview_freeze_requests,
                "preview_freeze_age_ms": runtime.preview_freeze_since.map(|at| at.elapsed().as_millis() as u64),
                "has_retained_frame": runtime.last_preview_frame.is_some(),
                "last_preview_frame_generation": runtime.last_preview_frame_generation,
                "last_frame_age_ms": runtime.last_preview_frame_at.map(|at| at.elapsed().as_millis() as u64),
                "last_error_age_ms": runtime.last_preview_error_at.map(|at| at.elapsed().as_millis() as u64),
                "preview_frame_seq": runtime.preview_frame_seq,
                "last_preview_frame_seq": runtime.last_preview_frame_seq,
                "scene_generation": runtime.scene_generation,
                "current_scene": runtime.current_scene,
                "transition_active": !runtime.transition_source.is_null(),
                "program_transition_kind": runtime.program_transition_kind,
                "program_transition_duration_ms": runtime.program_transition_duration_ms,
                "initialized": runtime.initialized,
            })),
        ),
        Err(std::sync::TryLockError::WouldBlock) => ("busy", None),
        Err(std::sync::TryLockError::Poisoned(_)) => return Err("state poisoned".to_string()),
    };
    let (trace_step, trace_age_ms) = logging::trace::last()
        .unwrap_or_else(|| ("(no trace recorded)".to_string(), 0));
    Ok(serde_json::json!({
        "requests": stats.requests,
        "frozen_returns": stats.frozen_returns,
        "stale_returns": stats.stale_returns,
        "captures_ok": stats.captures_ok,
        "captures_err": stats.captures_err,
        "encoded_ok": stats.encoded_ok,
        "encoded_err": stats.encoded_err,
        "stale_discards": stats.stale_discards,
        "lock_wait_us_max": stats.lock_wait_us_max,
        "capture_us_max": stats.capture_us_max,
        "encode_us_max": stats.encode_us_max,
        "last_error": stats.last_error,
        "runtime_lock": runtime_lock,
        "runtime": runtime,
        "trace": { "step": trace_step, "age_ms": trace_age_ms },
        "mutex_probe": logging::mutex_probe::snapshot(),
        "ui": ui,
    }))
}

#[tauri::command(async)]
fn debug_interact_stats() -> Result<JsonValue, String> {
    Ok(sources::browser_interact::debug_stats())
}

fn extract_source_params(settings: *mut obs::obs_data, params: &mut HashMap<String, String>) {
    sources::helpers::extract_source_params(settings, params)
}

fn collect_source_property_specs(
    props: *mut obs::obs_properties_t,
    out: &mut Vec<SourcePropertySpec>,
) {
    sources::helpers::collect_source_property_specs(props, out)
}

fn refresh_source_properties_with_settings(
    props: *mut obs::obs_properties_t,
    settings: *mut obs::obs_data_t,
) {
    sources::helpers::refresh_source_properties_with_settings(props, settings)
}

fn source_editable_list_keys(source: *mut obs::obs_source_t) -> HashSet<String> {
    sources::helpers::source_editable_list_keys(source)
}

fn live_scene_resolution() -> Option<String> {
    sources::helpers::live_scene_resolution()
}

fn apply_scene_item_transform(item: *mut obs::obs_scene_item, source: *mut obs::obs_source, params: &HashMap<String, String>) {
    sources::helpers::apply_scene_item_transform(item, source, params)
}

fn apply_source_params(
    settings: *mut obs::obs_data,
    source_type: &str,
    params: &HashMap<String, String>,
    editable_list_keys: Option<&HashSet<String>>,
) {
    sources::helpers::apply_source_params(settings, source_type, params, editable_list_keys)
}

#[tauri::command]
fn obs_export_scene_collection(state: tauri::State<ObsState>) -> Result<String, String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    scenes::collection_io::export_scene_collection(&runtime)
}

#[tauri::command]
fn obs_autosave_collection(state: tauri::State<ObsState>) -> Result<String, String> {
    let json = {
        let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
        if !runtime.initialized {
            return Err("OBS not initialized".to_string());
        }
        scenes::collection_io::export_scene_collection(&runtime)?
    };
    scenes::collection_io::autosave_scene_collection(json)
}

#[tauri::command]
fn obs_export_scene_collection_obs(state: tauri::State<ObsState>) -> Result<String, String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    scenes::collection_io::export_scene_collection_obs(&runtime)
}

#[tauri::command]
fn obs_export_scene_collection_obs_to_file(
    state: tauri::State<ObsState>,
    path: String,
    ui_json: Option<String>,
) -> Result<String, String> {
    let json = {
        let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
        if !runtime.initialized {
            return Err("OBS not initialized".to_string());
        }
        scenes::collection_io::export_scene_collection_obs(&runtime)?
    };
    scenes::collection_io::export_scene_collection_obs_to_file(json, path, ui_json)
}

#[tauri::command]
fn obs_export_scene_collection_to_file(
    state: tauri::State<ObsState>,
    path: String,
    ui_json: Option<String>,
) -> Result<String, String> {
    let json = {
        let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
        if !runtime.initialized {
            return Err("OBS not initialized".to_string());
        }
        scenes::collection_io::export_scene_collection(&runtime)?
    };
    scenes::collection_io::export_scene_collection_to_file(json, path, ui_json)
}

#[tauri::command]
fn obs_import_scene_collection(state: tauri::State<ObsState>, json: String) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    scenes::collection_io::import_scene_collection(&mut runtime, json)
}

#[tauri::command(async)]
fn load_rtmp_services_json() -> Result<String, String> {
    settings::core::load_rtmp_services_json()
}

#[tauri::command(async)]
fn settings_get() -> Result<AppSettings, String> {
    settings::core::settings_get()
}

#[tauri::command(async)]
fn settings_save(settings: AppSettings) -> Result<String, String> {
    settings::core::settings_save(settings)
}

#[tauri::command]
fn obs_export_profile_obs() -> Result<String, String> {
    settings::profile_obs::export_profile_obs()
}

#[tauri::command]
fn obs_export_profile_obs_to_file(path: String) -> Result<String, String> {
    settings::profile_obs::export_profile_obs_to_file(path)
}

#[tauri::command]
fn obs_import_profile_obs(json: String) -> Result<String, String> {
    settings::profile_obs::import_profile_obs(json)
}

#[tauri::command]
fn plugins_list() -> Result<Vec<PluginInfo>, String> {
    settings::plugins_profiles::plugins_list()
}

#[tauri::command]
fn plugin_profiles_list() -> Result<Vec<String>, String> {
    settings::plugins_profiles::plugin_profiles_list()
}

#[tauri::command]
fn plugin_profile_get_active() -> Result<String, String> {
    settings::plugins_profiles::plugin_profile_get_active()
}

#[tauri::command]
fn plugin_profile_set_active(name: String) -> Result<String, String> {
    settings::plugins_profiles::plugin_profile_set_active(name)
}

#[tauri::command]
fn plugin_profile_get(name: String) -> Result<PluginProfileState, String> {
    settings::plugins_profiles::plugin_profile_get(name)
}

#[tauri::command]
fn plugin_profile_save(name: String, enabled_modules: Vec<String>) -> Result<String, String> {
    settings::plugins_profiles::plugin_profile_save(name, enabled_modules)
}

#[tauri::command]
fn profiles_list() -> Result<Vec<String>, String> {
    settings::plugins_profiles::profiles_list()
}

#[tauri::command]
fn profiles_create(name: String) -> Result<String, String> {
    settings::plugins_profiles::profiles_create(name)
}

#[tauri::command]
fn profiles_activate(name: String) -> Result<AppSettings, String> {
    settings::plugins_profiles::profiles_activate(name)
}

#[tauri::command]
fn themes_list(root_dir: Option<String>) -> Result<Vec<ThemeInfo>, String> {
    settings::themes::themes_list(root_dir)
}

#[tauri::command]
fn themes_get_css(theme_id: String, root_dir: Option<String>) -> Result<String, String> {
    settings::themes::themes_get_css(theme_id, root_dir)
}

#[tauri::command]
fn themes_extract_default(
    root_dir: Option<String>,
    bg: Option<String>,
    surface: Option<String>,
    surface_2: Option<String>,
    surface_3: Option<String>,
    border: Option<String>,
    border_strong: Option<String>,
    text: Option<String>,
    text_muted: Option<String>,
    accent: Option<String>,
    accent_strong: Option<String>,
    success: Option<String>,
    danger: Option<String>,
    warning: Option<String>,
) -> Result<String, String> {
    settings::themes::themes_extract_default(
        root_dir,
        bg,
        surface,
        surface_2,
        surface_3,
        border,
        border_strong,
        text,
        text_muted,
        accent,
        accent_strong,
        success,
        danger,
        warning,
    )
}

#[tauri::command]
fn transitions_validate_sequence_dir(path: String) -> Result<String, String> {
    settings::themes::transitions_validate_sequence_dir(path)
}

#[tauri::command]
fn themes_import(
    root_dir: Option<String>,
    theme_id: String,
    name: Option<String>,
    author: Option<String>,
    version: Option<String>,
    css: String,
) -> Result<ThemeInfo, String> {
    settings::themes::themes_import(root_dir, theme_id, name, author, version, css)
}

#[tauri::command]
fn themes_import_archive(
    root_dir: Option<String>,
    file_name: String,
    data: Vec<u8>,
) -> Result<ThemeInfo, String> {
    settings::themes::themes_import_archive(root_dir, file_name, data)
}

#[tauri::command(async)]
fn obs_start(
    app: tauri::AppHandle,
    state: tauri::State<ObsState>,
    root_dir: Option<String>,
) -> Result<String, String> {
    runtime::init::start(app, state, root_dir)
}

#[tauri::command(async)]
fn obs_shutdown(app: tauri::AppHandle, state: tauri::State<ObsState>) -> Result<String, String> {
    runtime::init::shutdown(app, state)
}

#[tauri::command]
fn obs_is_initialized(state: tauri::State<ObsState>) -> Result<bool, String> {
    runtime::lifecycle::is_initialized(state)
}

#[tauri::command]
fn obs_version() -> Result<String, String> {
    runtime::lifecycle::version()
}

#[tauri::command(async)]
fn obs_list_sources(state: tauri::State<ObsState>) -> Result<Vec<SourceInfo>, String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    sources::source_ops::list_sources(&runtime)
}

#[tauri::command(async)]
fn obs_fit_source_to_canvas(
    state: tauri::State<ObsState>,
    source_id: String,
) -> Result<String, String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    sources::source_ops::fit_source_to_canvas(&runtime, source_id)
}

#[tauri::command(async)]
fn obs_stretch_source_to_canvas(
    state: tauri::State<ObsState>,
    source_id: String,
) -> Result<String, String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    sources::source_ops::stretch_source_to_canvas(&runtime, source_id)
}

#[tauri::command(async)]
fn obs_get_sceneitem_transforms(state: tauri::State<ObsState>) -> Result<JsonValue, String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    sources::source_ops::get_sceneitem_transforms(&runtime)
}

#[tauri::command(async)]
fn obs_apply_sceneitem_transform(
    state: tauri::State<ObsState>,
    id: String,
    transform: std::collections::HashMap<String, String>,
) -> Result<String, String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    sources::source_ops::apply_sceneitem_transform(&runtime, id, transform)
}

fn collect_sources(runtime: &ObsRuntime) -> Vec<SourceInfo> {
    sources::source_ops::collect_sources(runtime)
}

#[tauri::command]
fn obs_set_graphic_planner_init(state: tauri::State<ObsState>, init: PlannerInitInput) -> Result<(), String> {
    let planner_init = runtime::preview::build_planner_init(init);
    let mut runtime = crate::logging::mutex_probe::probe_lock(&state.runtime, "runtime.take_screenshot")
        .map_err(|_| "state poisoned".to_string())?;
    runtime::preview::set_graphic_planner_init(&mut runtime, planner_init)
}

#[tauri::command]
fn obs_get_graphic_planner_init(state: tauri::State<ObsState>) -> Result<PlannerInit, String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    runtime::preview::get_graphic_planner_init(&runtime)
}

// Polled by the preview loop and audio mixer; keep off the main thread.
#[tauri::command(async)]
fn obs_get_current_scene_resolution(state: tauri::State<ObsState>) -> Result<String, String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    runtime::preview::get_current_scene_resolution(&runtime)
}

#[tauri::command]
fn obs_remove_source(state: tauri::State<ObsState>, id: String) -> Result<String, String> {
    let mut runtime = crate::logging::mutex_probe::probe_lock(&state.runtime, "runtime.remove_source")
        .map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    sources::source_ops::remove_source(&mut runtime, id)
}

#[tauri::command]
fn obs_get_source_settings(state: tauri::State<ObsState>, id: String) -> Result<serde_json::Value, String> {
    crate::logging::trace::step("source-settings", "get:enter");
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    sources::source_ops::get_source_settings(&runtime, id)
}

#[tauri::command]
fn obs_interact_frame(source_id: String) -> Result<String, String> {
    use base64::Engine as _;
    let png = sources::browser_interact::frame_png(&source_id)?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    ))
}

#[tauri::command]
fn obs_interact_mouse(
    source_id: String,
    kind: String,
    x: f64,
    y: f64,
    button: Option<String>,
    buttons: Option<i64>,
    click_count: Option<i64>,
    delta_x: Option<f64>,
    delta_y: Option<f64>,
) -> Result<(), String> {
    sources::browser_interact::dispatch_mouse(
        &source_id,
        &kind,
        x,
        y,
        button.as_deref().unwrap_or("left"),
        buttons.unwrap_or(0),
        click_count.unwrap_or(0),
        delta_x.unwrap_or(0.0),
        delta_y.unwrap_or(0.0),
    )
}

#[tauri::command]
fn obs_interact_key(
    source_id: String,
    kind: String,
    key: String,
    code: String,
    key_code: i64,
    text: Option<String>,
) -> Result<(), String> {
    sources::browser_interact::dispatch_key(
        &source_id,
        &kind,
        &key,
        &code,
        key_code,
        text.as_deref().unwrap_or(""),
    )
}

#[tauri::command]
fn obs_interact_reapply(source_id: String) -> Result<(), String> {
    sources::browser_interact::reapply_code(&source_id)
}

#[tauri::command]
fn obs_interact_close(source_id: String) {
    sources::browser_interact::close_session(&source_id);
}

#[tauri::command]
fn obs_open_source_interaction(
    app: tauri::AppHandle,
    state: tauri::State<ObsState>,
    id: String,
    open_window: Option<bool>,
) -> Result<String, String> {
    let values = {
        let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
        if !runtime.initialized {
            return Err("OBS not initialized".to_string());
        }
        sources::source_ops::read_interaction_source(&runtime, &id)?
    };
    sources::source_ops::open_source_interaction(app, id, open_window.unwrap_or(true), values)
}

fn create_source_in_scene(runtime: &mut ObsRuntime, create: &SourceCreate) -> Result<String, String> {
    sources::source_ops::create_source_in_scene(runtime, create)
}

// Runs off the UI thread: `obs_source_create` performs heavy per-type
// initialization (FFmpeg probing, DXGI duplicator setup, browser workers)
// and must never execute on the main thread.
#[tauri::command(async)]
fn obs_create_source(state: tauri::State<ObsState>, create: SourceCreate) -> Result<String, String> {
    let mut runtime = crate::logging::mutex_probe::probe_lock(&state.runtime, "runtime.create_source")
        .map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    sources::source_ops::create_source(&mut runtime, create)
}

fn abgr_to_hex(abgr: u32) -> String {
    sources::helpers::abgr_to_hex(abgr)
}

// Runs off the UI thread: source updates apply settings and transforms and
// can contend with the preview loop on the runtime lock.
#[tauri::command(async)]
fn obs_update_source(state: tauri::State<ObsState>, update: SourceUpdate) -> Result<String, String> {
    crate::push_debug_log_entry(
        "obs_update_source".to_string(),
        Some(serde_json::json!({
            "id": update.id,
            "name": update.name,
            "type": update.source_type,
            "params": update.params
        })),
    );
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        crate::push_debug_log_entry("obs_update_source:error not-initialized".to_string(), None);
        return Err("OBS not initialized".to_string());
    }
    sources::source_ops::update_source(&runtime, update)
}

#[tauri::command]
fn obs_set_source_filters(
    state: tauri::State<ObsState>,
    source_id: String,
    filters: Vec<SourceFilterItem>,
) -> Result<String, String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    sources::source_ops::set_source_filters(&runtime, source_id, filters)
}

#[tauri::command]
fn obs_set_source_visible(state: tauri::State<ObsState>, id: String, visible: bool) -> Result<(), String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    sources::source_ops::set_source_visible(&runtime, id, visible)
}

#[tauri::command]
fn obs_move_source(state: tauri::State<ObsState>, id: String, direction: String) -> Result<(), String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    sources::source_ops::move_source(&runtime, id, direction)
}

#[tauri::command]
fn obs_reorder_source(state: tauri::State<ObsState>, id: String, to_index: usize) -> Result<(), String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    sources::source_ops::reorder_source(&runtime, id, to_index)
}

#[tauri::command]
fn obs_reorder_scene(state: tauri::State<ObsState>, name: String, to_index: usize) -> Result<(), String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    scenes::scene_ops::reorder_scene(&mut runtime, name, to_index)
}

#[tauri::command]
fn obs_start_recording(state: tauri::State<ObsState>, output_path: String) -> Result<String, String> {
    eprintln!("[record] start_recording...");
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    runtime::recording::start_recording(&mut runtime, output_path)
}

#[tauri::command]
fn obs_stop_recording(state: tauri::State<ObsState>) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    runtime::recording::stop_recording(&mut runtime)
}

#[tauri::command]
fn obs_start_streaming(state: tauri::State<ObsState>, stream_url: String) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    runtime::streaming::start_streaming(&mut runtime, stream_url)
}

#[tauri::command]
fn obs_stop_streaming(state: tauri::State<ObsState>) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    runtime::streaming::stop_streaming(&mut runtime)
}

#[tauri::command]
fn obs_set_encoder_preference(
    state: tauri::State<ObsState>,
    preference: String,
) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    runtime::encoders::set_encoder_preference(&mut runtime, preference)
}

// Polled every 100ms by the audio mixer; keep off the main thread.
#[tauri::command(async)]
fn obs_get_audio_levels() -> Result<HashMap<String, f64>, String> {
    let levels = devices::levels::get_levels()
        .lock()
        .map_err(|e| format!("poisoned: {e}"))?;
    Ok(levels.clone())
}

#[tauri::command]
fn obs_get_source_size(
    state: tauri::State<ObsState>,
    source_id: String,
) -> Result<(u32, u32), String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    runtime::preview::get_source_size(&runtime, source_id)
}

#[tauri::command(async)]
fn obs_take_screenshot(
    state: tauri::State<ObsState>,
    width: Option<u32>,
    height: Option<u32>,
    source_id: Option<String>,
) -> Result<String, String> {
    let lock_started = std::time::Instant::now();
    let mut runtime = crate::logging::mutex_probe::probe_lock(&state.runtime, "runtime.take_screenshot")
        .map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    let captured = match runtime::preview::capture_screenshot(
        &mut runtime,
        width,
        height,
        source_id,
        lock_started.elapsed().as_micros() as u64,
    )? {
        runtime::preview::ScreenshotCapture::Serve(frame) => return Ok(frame),
        runtime::preview::ScreenshotCapture::Encode(captured) => captured,
    };

    // PNG encode + base64 are pure CPU work (~15ms at 1080p). Run them with
    // the runtime lock released so other Tauri commands are not starved by
    // the preview loop.
    drop(runtime);
    let encoded = runtime::preview::encode_screenshot(captured.frame);

    let mut generation_invalidated = false;
    if let Ok(mut runtime) = crate::logging::mutex_probe::probe_lock(&state.runtime, "runtime.preview_retain") {
        generation_invalidated = runtime::preview::publish_screenshot(
            &mut runtime,
            captured.capture_seq,
            captured.capture_generation,
            captured.capture_scene_output,
            &encoded,
        );
    }
    crate::logging::trace::step("preview-loop", "request:done");
    if generation_invalidated {
        return Err("preview frame invalidated by scene change".to_string());
    }

    encoded
}

#[tauri::command(async)]
fn obs_list_reusable_sources(
    state: tauri::State<ObsState>,
    source_type: String,
) -> Result<Vec<ReusableSourceInfo>, String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    sources::source_ops::list_reusable_sources(&runtime, source_type)
}

#[tauri::command(async)]
fn obs_add_existing_source(
    state: tauri::State<ObsState>,
    id: String,
    source_name: String,
) -> Result<String, String> {
    let mut runtime = crate::logging::mutex_probe::probe_lock(&state.runtime, "runtime.add_existing_source")
        .map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    sources::source_ops::add_existing_source(&mut runtime, id, source_name)
}

#[tauri::command(async)]
fn obs_take_source_thumbnail(
    state: tauri::State<ObsState>,
    source_name: String,
    width: Option<u32>,
    height: Option<u32>,
) -> Result<String, String> {
    let mut runtime = crate::logging::mutex_probe::probe_lock(&state.runtime, "runtime.source_thumbnail")
        .map_err(|_| "state poisoned".to_string())?;
    let captured = runtime::preview::capture_source_thumbnail(&mut runtime, &source_name, width, height);
    drop(runtime);
    runtime::preview::encode_preview_frame(captured?)
}

/// Freeze/unfreeze scene-level preview rendering. While frozen,
/// `obs_take_screenshot` returns the retained last good frame without any
/// render, GPU or encode work. Single-source screenshots and active
/// recording/streaming outputs are never affected.
#[tauri::command(async)]
fn obs_preview_set_freeze(
    state: tauri::State<ObsState>,
    freeze: bool,
    reason: Option<String>,
) -> Result<(), String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    runtime::preview::set_preview_freeze(&mut runtime, freeze, reason)
}

fn parse_cli_paths(args: impl IntoIterator<Item = String>) -> Result<Vec<(&'static str, String)>, String> {
    let args: Vec<String> = args.into_iter().collect();
    let mut paths = Vec::new();
    let mut index = 1;
    while index < args.len() {
        let argument = &args[index];
        let (name, inline_value) = argument
            .split_once('=')
            .map_or((argument.as_str(), None), |(name, value)| (name, Some(value)));
        let variable = match name {
            "--revo-root" => "REVO_ROOT",
            "--obs-path" => "REVO_OBS_PATH",
            _ => {
                index += 1;
                continue;
            }
        };
        let value = if let Some(value) = inline_value {
            value.to_string()
        } else {
            index += 1;
            args.get(index)
                .filter(|value| !value.starts_with('-'))
                .cloned()
                .ok_or_else(|| format!("{name} requires a path"))?
        };
        if value.trim().is_empty() {
            return Err(format!("{name} requires a non-empty path"));
        }
        paths.push((variable, value));
        index += 1;
    }
    Ok(paths)
}

fn configure_cli_paths() -> Result<(), String> {
    for (variable, value) in parse_cli_paths(std::env::args())? {
        std::env::set_var(variable, value);
    }
    Ok(())
}

#[cfg(test)]
mod cli_path_tests {
    use super::parse_cli_paths;

    #[test]
    fn cli_flags_accept_equals_and_separate_values() {
        let values = parse_cli_paths([
            "revo-ui".into(),
            "--revo-root".into(),
            "/tmp/revo".into(),
            "--obs-path=/opt/obs".into(),
        ])
        .unwrap();
        assert_eq!(values, vec![("REVO_ROOT", "/tmp/revo".into()), ("REVO_OBS_PATH", "/opt/obs".into())]);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(error) = configure_cli_paths() {
        eprintln!("[startup] {error}");
    }
    tauri::Builder::default()
        .setup(|app| {
            use tauri::Manager;

            if let Ok(cwd) = env::current_dir() {
                crate::utils::fs::set_startup_cwd(cwd);
            }
            crate::utils::fs::set_app_paths(app.path().resource_dir()?, app.path().app_data_dir()?);

            if let Err(err) = init_debug_log_file() {
                eprintln!("debug log file init warning: {err}");
            }

            if let Err(err) = settings::core::ensure_runtime_data_dirs() {
                eprintln!("startup data dir init warning: {err}");
            }

            let initial_minimize_to_tray = settings::core::settings_get()
                .ok()
                .and_then(|settings| settings.minimize_to_tray)
                .unwrap_or(false);
            app.state::<AppUiState>()
                .minimize_to_tray
                .store(initial_minimize_to_tray, Ordering::Relaxed);

            let show_item = tauri::menu::MenuItem::with_id(
                app,
                "tray-show",
                "Show RevoStream",
                true,
                None::<&str>,
            )?;
            let quit_item = tauri::menu::MenuItem::with_id(
                app,
                "tray-quit",
                "Quit",
                true,
                None::<&str>,
            )?;
            let tray_menu = tauri::menu::Menu::with_items(app, &[&show_item, &quit_item])?;
            let mut tray_builder = tauri::tray::TrayIconBuilder::with_id("revo-main-tray")
                .tooltip("RevoStream")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        restore_main_window(tray.app_handle());
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "tray-show" => restore_main_window(app),
                    "tray-quit" => {
                        let state = app.state::<ObsState>();
                        if let Err(error) = runtime::init::shutdown(app.clone(), state) {
                            eprintln!("[shutdown] failed from tray menu: {error}");
                        }
                        app.exit(0);
                    }
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }
            // libappindicator-sys panics inside its constructor when the system
            // tray library is missing (libayatana-appindicator3/appindicator3).
            // A missing tray must degrade to "no tray icon", never kill the app.
            let tray_build = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                tray_builder.build(app)
            }));
            match tray_build {
                Ok(Ok(tray)) => {
                    if let Err(error) = tray.set_visible(initial_minimize_to_tray) {
                        eprintln!("[tray] set_visible failed: {error}");
                    }
                }
                Ok(Err(error)) => {
                    eprintln!("[tray] unavailable: {error}");
                }
                Err(_) => {
                    eprintln!("[tray] unavailable: libappindicator failed to load; continuing without tray icon");
                }
            }

            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }
            // --- MAIN WINDOW WITH POPUP SUPPORT ---
            let app_handle = app.handle().clone();
            if app.get_webview_window("main").is_none() {
                let _main = tauri::WebviewWindowBuilder::new(
                    &app_handle,
                    "main",
                    tauri::WebviewUrl::App("index.html".into()),
                )
                .title("RevoStream")
                .min_inner_size(1080.0, 600.0)
                .inner_size(1280.0, 720.0)
                .devtools(cfg!(debug_assertions))
                .on_new_window({
                    let app_handle = app_handle.clone();
                    move |url: tauri::Url, features| {
                        let url_str = url.to_string();
                        if url_str.ends_with("source-transform") {
                            let label = format!("popup-{}", Uuid::new_v4());
                            let builder = tauri::WebviewWindowBuilder::new(
                                &app_handle,
                                &label,
                                tauri::WebviewUrl::App("source-transform".into()),
                            )
                            .title("RevoStream - Graphic planner")
                            .min_inner_size(680.0, 620.0)
                            .inner_size(900.0, 700.0)
                            .devtools(cfg!(debug_assertions))
                            .window_features(features);
                            let window = builder.build().unwrap();
                            tauri::webview::NewWindowResponse::Create { window }
                        } else {
                            tauri::webview::NewWindowResponse::Deny
                        }
                    }
                })
                .build()?;
            }

            if let Some(main_window) = app.get_webview_window("main") {
                let app_handle_for_close = app_handle.clone();
                let main_window_for_events = main_window.clone();
                main_window.on_window_event(move |event| {
                    if matches!(event, tauri::WindowEvent::Resized(_))
                        && app_handle_for_close
                            .state::<AppUiState>()
                            .minimize_to_tray
                            .load(Ordering::Relaxed)
                        && main_window_for_events.is_minimized().unwrap_or(false)
                    {
                        let _ = main_window_for_events.hide();
                    }
                    if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                        let state = app_handle_for_close.state::<ObsState>();
                        if let Err(err) = runtime::init::shutdown(app_handle_for_close.clone(), state) {
                            eprintln!("[shutdown] failed during main window close: {err}");
                        }

                        #[cfg(debug_assertions)]
                        {
                            for (_, window) in app_handle_for_close.webview_windows() {
                                window.close_devtools();
                            }
                        }

                        for (label, window) in app_handle_for_close.webview_windows() {
                            if label != "main" {
                                let _ = window.close();
                            }
                        }
                    }
                });
            }
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(ObsState {
            runtime: Mutex::new(ObsRuntime::default()),
        })
        .manage(AppUiState {
            minimize_to_tray: AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            obs_start,
            obs_shutdown,
            obs_is_initialized,
            obs_version,
            obs_list_scenes,
            obs_set_current_scene,
            obs_set_program_transition,
            obs_interact_frame,
            obs_interact_mouse,
            obs_interact_key,
            obs_interact_reapply,
            obs_interact_close,
            obs_create_scene,
            obs_rename_scene,
            obs_remove_scene,
            obs_set_scene_lock,
            obs_export_scene_collection,
            obs_autosave_collection,
            obs_export_scene_collection_to_file,
            obs_export_scene_collection_obs,
            obs_export_scene_collection_obs_to_file,
            obs_import_scene_collection,
            obs_set_scene_resolution,
            obs_get_current_scene_resolution,
            obs_list_sources,
            obs_fit_source_to_canvas,
            obs_stretch_source_to_canvas,
            obs_get_sceneitem_transforms,
            obs_apply_sceneitem_transform,
            obs_list_external_source_types,
            obs_get_graphic_planner_init,
            obs_set_graphic_planner_init,
            obs_list_pulse_devices,
            permissions::macos_permissions_status,
            permissions::macos_request_screen_capture,
            permissions::macos_open_privacy_pane,
            obs_list_alsa_devices,
            obs_get_settings_select_options,
            obs_list_window_picker_items,
            obs_list_system_fonts,
            obs_list_video_device_picker_items,
            obs_set_source_visible,
            obs_remove_source,
            obs_create_source,
            obs_move_source,
            obs_reorder_source,
            obs_update_source,
            obs_set_source_filters,
            obs_get_source_settings,
            obs_open_source_interaction,
            obs_start_recording,
            obs_stop_recording,
            obs_start_streaming,
            obs_stop_streaming,
            obs_set_encoder_preference,
            obs_get_source_size,
            obs_take_screenshot,
            obs_list_reusable_sources,
            obs_add_existing_source,
            obs_take_source_thumbnail,
            obs_preview_set_freeze,
            obs_get_audio_levels,
            obs_reorder_scene,
            settings_get,
            settings_save,
            profiles_list,
            profiles_create,
            profiles_activate,
            plugins_list,
            plugin_profiles_list,
            plugin_profile_get_active,
            plugin_profile_set_active,
            plugin_profile_get,
            plugin_profile_save,
            themes_list,
            themes_get_css,
            themes_extract_default,
            themes_import,
            themes_import_archive,
            transitions_validate_sequence_dir,
            obs_export_profile_obs,
            obs_export_profile_obs_to_file,
            obs_import_profile_obs,
            load_rtmp_services_json,
            open_preview_window,
            open_graphic_planner,
            open_browser_dock,
            browser_dock_state,
            close_browser_dock,
            app_close_all_windows,
            app_set_minimize_to_tray,
            app_set_builtin_browser_enabled,
            cef_bridge_info,
            cef_dock_render_frame,
            open_debug_console,
            debug_console_log,
            debug_console_history,
            debug_console_clear,
            debug_preview_stats,
            debug_interact_stats,
            debug_trace_last
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn obs_set_scene_resolution(state: tauri::State<ObsState>, resolution: String) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    settings::video_settings::set_scene_resolution(&mut runtime, resolution)
}
