use std::ffi::CString;
use crate::obs::obs_sys as obs;
use crate::state::{ObsRuntime, ObsState, SceneState};
use tauri::State;
use serde::Serialize;
use std::ptr;

#[derive(Serialize)]
pub struct SceneInfo {
    pub name: String,
    pub active: bool,
    pub locked: bool,
}

pub fn current_scene(runtime: &ObsRuntime) -> Result<&SceneState, String> {
    let current = runtime.current_scene.as_ref().ok_or_else(|| "no active scene".to_string())?;
    runtime
        .scenes
        .get(current)
        .ok_or_else(|| "active scene missing".to_string())
}

pub fn current_scene_mut(runtime: &mut ObsRuntime) -> Result<&mut SceneState, String> {
    let current = runtime.current_scene.clone().ok_or_else(|| "no active scene".to_string())?;
    runtime
        .scenes
        .get_mut(&current)
        .ok_or_else(|| "active scene missing".to_string())
}

pub fn set_current_scene_internal(runtime: &mut ObsRuntime, name: &str) -> Result<(), String> {
    let next = runtime
        .scenes
        .get(name)
        .ok_or_else(|| "scene not found".to_string())?;
    unsafe {
        obs::obs_set_output_source(0, next.scene_source);
        obs::obs_source_inc_showing(next.scene_source);
        if !runtime.preview_view.is_null() {
            obs::obs_view_set_source(runtime.preview_view, 0, next.scene_source);
        }
    }
    runtime.current_scene = Some(name.to_string());
    Ok(())
}

#[tauri::command]
pub fn obs_list_scenes(state: State<ObsState>) -> Result<Vec<SceneInfo>, String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Ok(vec![]);
    }
    let current = runtime.current_scene.clone().unwrap_or_default();
    let mut list = Vec::new();
    for (name, _) in runtime.scenes.iter() {
        list.push(SceneInfo {
            name: name.clone(),
            active: name == &current,
            locked: runtime.locked_scenes.contains(name),
        });
    }
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(list)
}

#[tauri::command]
pub fn obs_set_current_scene(state: State<ObsState>, name: String) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("scene name required".to_string());
    }
    set_current_scene_internal(&mut runtime, trimmed)?;
    Ok(format!("active scene: {trimmed}"))
}

#[tauri::command]
pub fn obs_create_scene(state: State<ObsState>, name: String) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("scene name required".to_string());
    }
    if runtime.scenes.contains_key(trimmed) {
        return Err("scene already exists".to_string());
    }
    let scene_name = CString::new(trimmed).map_err(|_| "scene name invalid".to_string())?;
    let scene = unsafe { obs::obs_scene_create(scene_name.as_ptr()) };
    if scene.is_null() {
        return Err("failed to create scene".to_string());
    }
    let scene_source = unsafe { obs::obs_scene_get_source(scene) };
    let state = SceneState::new(trimmed.to_string(), scene, scene_source);
    runtime.scenes.insert(trimmed.to_string(), state);
    set_current_scene_internal(&mut runtime, trimmed)?;
    Ok(format!("created {trimmed}"))
}

#[tauri::command]
pub fn obs_rename_scene(state: State<ObsState>, old_name: String, new_name: String) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    let old_trim = old_name.trim();
    let new_trim = new_name.trim();
    if old_trim.is_empty() || new_trim.is_empty() {
        return Err("scene name required".to_string());
    }
    if runtime.locked_scenes.contains(old_trim) {
        return Err("scene is locked".to_string());
    }
    if runtime.scenes.contains_key(new_trim) {
        return Err("scene name already exists".to_string());
    }
    let mut state = runtime
        .scenes
        .remove(old_trim)
        .ok_or_else(|| "scene not found".to_string())?;
    state.name = new_trim.to_string();
    unsafe {
        if !state.scene_source.is_null() {
            if let Ok(name_c) = CString::new(new_trim) {
                obs::obs_source_set_name(state.scene_source, name_c.as_ptr());
            }
        }
    }
    runtime.scenes.insert(new_trim.to_string(), state);
    if runtime.current_scene.as_deref() == Some(old_trim) {
        runtime.current_scene = Some(new_trim.to_string());
    }
    if runtime.locked_scenes.remove(old_trim) {
        runtime.locked_scenes.insert(new_trim.to_string());
    }
    Ok(format!("renamed {old_trim} to {new_trim}"))
}

#[tauri::command]
pub fn obs_remove_scene(state: State<ObsState>, name: String) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("scene name required".to_string());
    }
    if runtime.locked_scenes.contains(trimmed) {
        return Err("scene is locked".to_string());
    }
    if runtime.scenes.len() <= 1 {
        return Err("cannot remove last scene".to_string());
    }
    let state = runtime
        .scenes
        .remove(trimmed)
        .ok_or_else(|| "scene not found".to_string())?;
    unsafe {
        if !state.scene_source.is_null() {
            obs::obs_source_dec_showing(state.scene_source);
        }
        if !state.scene.is_null() {
            obs::obs_scene_enum_items(state.scene, Some(remove_scene_item_cb), ptr::null_mut());
            obs::obs_scene_release(state.scene);
        }
    }

    if runtime.current_scene.as_deref() == Some(trimmed) {
        if let Some((next_name, _)) = runtime.scenes.iter().next() {
            let next = next_name.clone();
            set_current_scene_internal(&mut runtime, &next)?;
        } else {
            runtime.current_scene = None;
        }
    }
    runtime.locked_scenes.remove(trimmed);
    Ok(format!("removed {trimmed}"))
}

#[tauri::command]
pub fn obs_set_scene_lock(state: State<ObsState>, name: String, locked: bool) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("scene name required".to_string());
    }
    if !runtime.scenes.contains_key(trimmed) {
        return Err("scene not found".to_string());
    }
    if locked {
        runtime.locked_scenes.insert(trimmed.to_string());
    } else {
        runtime.locked_scenes.remove(trimmed);
    }
    Ok(format!("scene {trimmed} lock={locked}"))
}

pub fn ensure_scene(runtime: &mut ObsRuntime) -> Result<(), String> {
    if !runtime.scenes.is_empty() {
        return Ok(());
    }

    let scene_name = CString::new("revo_scene").map_err(|_| "scene name".to_string())?;
    let scene = unsafe { obs::obs_scene_create(scene_name.as_ptr()) };
    if scene.is_null() {
        return Err("failed to create scene".to_string());
    }

    let accent = create_color_source("revo_accent", 0xFF1E90FF, 420, 220);
    let text = create_text_source("RevoStream", 32);

    unsafe {
        let accent_item = obs::obs_scene_add(scene, accent);
        if !accent_item.is_null() {
            let pos = obs::vec2 {
                __bindgen_anon_1: obs::vec2__bindgen_ty_1 {
                    __bindgen_anon_1: obs::vec2__bindgen_ty_1__bindgen_ty_1 { x: 60.0, y: 60.0 },
                },
            };
            obs::obs_sceneitem_set_pos(accent_item, &pos);
            obs::obs_sceneitem_set_visible(accent_item, true);
        }
        let text_item = obs::obs_scene_add(scene, text);
        if !text_item.is_null() {
            let pos = obs::vec2 {
                __bindgen_anon_1: obs::vec2__bindgen_ty_1 {
                    __bindgen_anon_1: obs::vec2__bindgen_ty_1__bindgen_ty_1 { x: 640.0, y: 360.0 },
                },
            };
            obs::obs_sceneitem_set_pos(text_item, &pos);
            obs::obs_sceneitem_set_alignment(text_item, obs::OBS_ALIGN_CENTER);
            obs::obs_sceneitem_set_visible(text_item, true);
        }

        if !accent.is_null() {
            obs::obs_source_release(accent);
        }
        if !text.is_null() {
            obs::obs_source_release(text);
        }

        let scene_source = obs::obs_scene_get_source(scene);
        obs::obs_set_output_source(0, scene_source);
        obs::obs_source_inc_showing(scene_source);

        let mut state = SceneState::new("revo_scene".to_string(), scene, scene_source);
        state.item_accent = accent_item;
        state.item_text = text_item;
        runtime.scenes.insert(state.name.clone(), state);
        runtime.current_scene = Some("revo_scene".to_string());
    }

    Ok(())
}

pub fn cleanup_scene(runtime: &mut ObsRuntime) {
    unsafe {
        if !runtime.preview_texrender.is_null() {
            obs::gs_texrender_destroy(runtime.preview_texrender);
        }
        if !runtime.preview_view.is_null() {
            obs::obs_view_destroy(runtime.preview_view);
        }
        obs::obs_set_output_source(0, ptr::null_mut());
        for state in runtime.scenes.values() {
            if !state.scene_source.is_null() {
                obs::obs_source_dec_showing(state.scene_source);
            }
            if !state.scene.is_null() {
                obs::obs_scene_enum_items(state.scene, Some(remove_scene_item_cb), ptr::null_mut());
                obs::obs_scene_release(state.scene);
            }
        }
    }
    runtime.scenes.clear();
    runtime.current_scene = None;
    runtime.locked_scenes.clear();
    runtime.preview_view = ptr::null_mut();
    runtime.preview_texrender = ptr::null_mut();
    runtime.last_record_path = None;
}

fn create_color_source(name: &str, abgr: u32, width: i64, height: i64) -> *mut obs::obs_source {
    unsafe {
        let settings = obs::obs_data_create();
        if !settings.is_null() {
            let key = CString::new("color").unwrap();
            obs::obs_data_set_int(settings, key.as_ptr(), abgr as i64);

            let width_key = CString::new("width").unwrap();
            obs::obs_data_set_int(settings, width_key.as_ptr(), width);
            let height_key = CString::new("height").unwrap();
            obs::obs_data_set_int(settings, height_key.as_ptr(), height);
        }
        let id = CString::new("color_source").unwrap();
        let name = CString::new(name).unwrap();
        let source = obs::obs_source_create(id.as_ptr(), name.as_ptr(), settings, ptr::null_mut());
        if !settings.is_null() {
            obs::obs_data_release(settings);
        }
        source
    }
}

fn create_text_source(text: &str, size: i64) -> *mut obs::obs_source {
    unsafe {
        let settings = obs::obs_data_create();
        if !settings.is_null() {
            let text_key = CString::new("text").unwrap();
            let text_val = CString::new(text).unwrap();
            obs::obs_data_set_string(settings, text_key.as_ptr(), text_val.as_ptr());

            let color_key = CString::new("color1").unwrap();
            obs::obs_data_set_int(settings, color_key.as_ptr(), 0xFFFFFFFFu32 as i64);

            let font_key = CString::new("font").unwrap();
            let font_settings = obs::obs_data_create();
            if !font_settings.is_null() {
                let face_key = CString::new("face").unwrap();
                let face_val = CString::new("DejaVu Sans").unwrap();
                obs::obs_data_set_string(font_settings, face_key.as_ptr(), face_val.as_ptr());

                let size_key = CString::new("size").unwrap();
                obs::obs_data_set_int(font_settings, size_key.as_ptr(), size);
                obs::obs_data_set_obj(settings, font_key.as_ptr(), font_settings);
                obs::obs_data_release(font_settings);
            }
        }
        let id = CString::new("text_ft2_source").unwrap();
        let name = CString::new("revo_text").unwrap();
        let source = obs::obs_source_create(id.as_ptr(), name.as_ptr(), settings, ptr::null_mut());
        if !settings.is_null() {
            obs::obs_data_release(settings);
        }
        source
    }
}

unsafe extern "C" fn remove_scene_item_cb(_scene: *mut obs::obs_scene, item: *mut obs::obs_scene_item, _param: *mut std::os::raw::c_void) -> bool {
    if !item.is_null() {
        obs::obs_sceneitem_remove(item);
    }
    true
}
