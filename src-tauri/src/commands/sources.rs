use crate::commands::scenes::{current_scene, current_scene_mut};
use crate::obs::obs_sys as obs;
use crate::state::{ObsState, SceneState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_void;
use tauri::State;

#[derive(Serialize)]
pub struct SourceInfo {
    pub id: String,
    pub name: String,
    pub visible: bool,
    pub source_type: String,
}

#[derive(Deserialize)]
pub struct SourceUpdate {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub params: HashMap<String, String>,
}

#[derive(Deserialize)]
pub struct SourceCreate {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub params: HashMap<String, String>,
}

fn resolve_scene_item(scene: &SceneState, id: &str) -> Option<*mut obs::obs_scene_item> {
    match id {
        "accent" => Some(scene.item_accent),
        "title" => Some(scene.item_text),
        _ => scene
            .custom_items
            .get(id)
            .copied()
            .or_else(|| find_scene_item_by_name(scene, id)),
    }
}

fn find_scene_item_by_name(scene: &SceneState, name: &str) -> Option<*mut obs::obs_scene_item> {
    if scene.scene.is_null() {
        return None;
    }
    let mut items: Vec<*mut obs::obs_scene_item> = Vec::new();
    unsafe {
        obs::obs_scene_enum_items(scene.scene, Some(collect_scene_items_cb), &mut items as *mut _ as *mut c_void);
    }
    for item in items {
        if item.is_null() {
            continue;
        }
        unsafe {
            let source = obs::obs_sceneitem_get_source(item);
            if source.is_null() {
                continue;
            }
            let name_ptr = obs::obs_source_get_name(source);
            if !name_ptr.is_null() {
                let sname = CStr::from_ptr(name_ptr).to_string_lossy();
                if sname.eq_ignore_ascii_case(name) {
                    return Some(item);
                }
            }
        }
    }
    None
}

#[tauri::command]
pub fn obs_list_sources(state: State<ObsState>) -> Result<Vec<SourceInfo>, String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Ok(vec![]);
    }
    let scene = match current_scene(&runtime) {
        Ok(scene) => scene,
        Err(_) => return Ok(vec![]),
    };
    unsafe {
        let mut list = Vec::new();
        let mut items: Vec<*mut obs::obs_scene_item> = Vec::new();
        obs::obs_scene_enum_items(scene.scene, Some(collect_scene_items_cb), &mut items as *mut _ as *mut c_void);

        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for item in items {
            if item.is_null() {
                continue;
            }
            let source = obs::obs_sceneitem_get_source(item);
            if source.is_null() {
                continue;
            }

            let name_ptr = obs::obs_source_get_name(source);
            let name = if name_ptr.is_null() {
                "".to_string()
            } else {
                CStr::from_ptr(name_ptr).to_string_lossy().to_string()
            };

            let type_ptr = obs::obs_source_get_id(source);
            let source_type = if type_ptr.is_null() {
                "Unknown".to_string()
            } else {
                CStr::from_ptr(type_ptr).to_string_lossy().to_string()
            };

            let id = if item == scene.item_accent {
                "accent".to_string()
            } else if item == scene.item_text {
                "title".to_string()
            } else if let Some((custom_id, _)) = scene.custom_items.iter().find(|(_, v)| **v == item) {
                custom_id.clone()
            } else if !name.is_empty() {
                name.clone()
            } else {
                format!("item-{item:p}")
            };

            if seen.contains(&id) {
                continue;
            }
            seen.insert(id.clone());

            list.push(SourceInfo {
                id,
                name: if name.is_empty() { "Source".to_string() } else { name },
                visible: obs::obs_sceneitem_visible(item),
                source_type,
            });
        }

        Ok(list)
    }
}

#[tauri::command]
pub fn obs_get_source_settings(state: State<ObsState>, id: String) -> Result<serde_json::Value, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    let scene = current_scene(&runtime)?;
    let item = resolve_scene_item(scene, id.as_str()).ok_or_else(|| "unknown source id".to_string())?;
    if item.is_null() {
        return Err("source not available".to_string());
    }
    unsafe {
        let source = obs::obs_sceneitem_get_source(item);
        if source.is_null() {
            return Err("source not available".to_string());
        }
        let name_ptr = obs::obs_source_get_name(source);
        let name = if name_ptr.is_null() {
            "".to_string()
        } else {
            CStr::from_ptr(name_ptr).to_string_lossy().to_string()
        };
        let type_ptr = obs::obs_source_get_id(source);
        let source_type = if type_ptr.is_null() {
            "Unknown".to_string()
        } else {
            CStr::from_ptr(type_ptr).to_string_lossy().to_string()
        };
        let settings = obs::obs_source_get_settings(source);
        if settings.is_null() {
            return Err("failed to get source settings".to_string());
        }
        let mut params = HashMap::new();
        extract_source_params(settings, &mut params);
        obs::obs_data_release(settings);
        let result = serde_json::json!({
            "name": name,
            "source_type": source_type,
            "params": params
        });
        Ok(result)
    }
}

#[tauri::command]
pub fn obs_create_source(state: State<ObsState>, create: SourceCreate) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    let scene = current_scene_mut(&mut runtime)?;
    let id = create.id.trim();
    if id.is_empty() {
        return Err("source id required".to_string());
    }
    if let Some(existing) = resolve_scene_item(scene, id) {
        if !existing.is_null() {
            return Err("source id already exists".to_string());
        }
    }

    let name = if create.name.trim().is_empty() { id } else { create.name.as_str() };

    unsafe {
        let settings = obs::obs_data_create();
        apply_source_params(settings, create.source_type.as_str(), &create.params);

        let source_id = CString::new(create.source_type.as_str()).map_err(|_| "source type invalid".to_string())?;
        let source_name = CString::new(name).map_err(|_| "source name invalid".to_string())?;
        let source = obs::obs_source_create(source_id.as_ptr(), source_name.as_ptr(), settings, std::ptr::null_mut());
        if !settings.is_null() {
            obs::obs_data_release(settings);
        }
        if source.is_null() {
            return Err("failed to create source".to_string());
        }

        let item = obs::obs_scene_add(scene.scene, source);
        if item.is_null() {
            obs::obs_source_release(source);
            return Err("failed to add source to scene".to_string());
        }
        obs::obs_sceneitem_set_visible(item, true);
        obs::obs_source_release(source);
        scene.custom_items.insert(id.to_string(), item);
    }

    Ok(format!("created {id}"))
}

fn parse_color_abgr(value: &str) -> Option<u32> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as u32;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as u32;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as u32;
    Some((0xFF << 24) | (b << 16) | (g << 8) | r)
}

fn abgr_to_hex(abgr: u32) -> String {
    let r = (abgr & 0xFF) as u8;
    let g = ((abgr >> 8) & 0xFF) as u8;
    let b = ((abgr >> 16) & 0xFF) as u8;
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

#[tauri::command]
pub fn obs_update_source(state: State<ObsState>, update: SourceUpdate) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    let scene = current_scene(&runtime)?;
    let item = resolve_scene_item(scene, update.id.as_str()).ok_or_else(|| "unknown source id".to_string())?;
    if item.is_null() {
        return Err("source not available".to_string());
    }
    unsafe {
        let source = obs::obs_sceneitem_get_source(item);
        if source.is_null() {
            return Err("source not available".to_string());
        }
        if !update.name.trim().is_empty() {
            if let Ok(name_c) = CString::new(update.name.as_bytes()) {
                obs::obs_source_set_name(source, name_c.as_ptr());
            }
        }

        let settings = obs::obs_source_get_settings(source);
        if settings.is_null() {
            return Err("failed to get source settings".to_string());
        }

        let type_ptr = obs::obs_source_get_id(source);
        let actual_type = if type_ptr.is_null() {
            "Unknown".to_string()
        } else {
            CStr::from_ptr(type_ptr).to_string_lossy().to_string()
        };
        let chosen_type = if update.source_type.trim().is_empty() || update.source_type == "Unknown" {
            actual_type.as_str()
        } else {
            update.source_type.as_str()
        };
        apply_source_params(settings, chosen_type, &update.params);

        obs::obs_source_update(source, settings);
        obs::obs_data_release(settings);
        obs::obs_sceneitem_set_visible(item, false);
        obs::obs_sceneitem_set_visible(item, true);
    }
    Ok("updated".to_string())
}

#[tauri::command]
pub fn obs_set_source_visible(state: State<ObsState>, id: String, visible: bool) -> Result<(), String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    let scene = current_scene(&runtime)?;
    let item = resolve_scene_item(scene, id.as_str()).ok_or_else(|| "unknown source id".to_string())?;
    if item.is_null() {
        return Err("source not available".to_string());
    }
    unsafe {
        obs::obs_sceneitem_set_visible(item, visible);
    }
    Ok(())
}

#[tauri::command]
pub fn obs_move_source(state: State<ObsState>, id: String, direction: String) -> Result<(), String> {
    let runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    let scene = current_scene(&runtime)?;
    let item = resolve_scene_item(scene, id.as_str()).ok_or_else(|| "unknown source id".to_string())?;
    if item.is_null() {
        return Err("source not available".to_string());
    }
    unsafe {
        let movement = match direction.as_str() {
            "up" => obs::obs_order_movement_OBS_ORDER_MOVE_UP,
            "down" => obs::obs_order_movement_OBS_ORDER_MOVE_DOWN,
            "top" => obs::obs_order_movement_OBS_ORDER_MOVE_TOP,
            "bottom" => obs::obs_order_movement_OBS_ORDER_MOVE_BOTTOM,
            _ => return Err("invalid direction".to_string()),
        };
        obs::obs_sceneitem_set_order(item, movement);
    }
    Ok(())
}

#[tauri::command]
pub fn obs_remove_source(state: State<ObsState>, id: String) -> Result<String, String> {
    let mut runtime = state.runtime.lock().map_err(|_| "state poisoned".to_string())?;
    if !runtime.initialized {
        return Err("OBS not initialized".to_string());
    }
    let scene = current_scene_mut(&mut runtime)?;
    let (item_ptr, is_custom) = match id.as_str() {
        "accent" => (scene.item_accent, false),
        "title" => (scene.item_text, false),
        _ => match scene.custom_items.remove(&id) {
            Some(item) => (item, true),
            None => return Err("unknown source id".to_string()),
        },
    };
    if item_ptr.is_null() {
        return Err("source not available".to_string());
    }
    unsafe {
        obs::obs_sceneitem_remove(item_ptr);
    }
    if !is_custom {
        match id.as_str() {
            "accent" => scene.item_accent = std::ptr::null_mut(),
            "title" => scene.item_text = std::ptr::null_mut(),
            _ => {}
        }
    }
    Ok(format!("removed {id}"))
}

fn extract_source_params(settings: *mut obs::obs_data, params: &mut HashMap<String, String>) {
    if settings.is_null() {
        return;
    }
    unsafe {
        let int_keys = ["color", "color1", "width", "height", "font_size"];
        let string_keys = ["text", "file", "local_file", "url", "font_face", "device", "device_id", "window"];
        for &key in &int_keys {
            if let Ok(key_c) = CString::new(key) {
                if obs::obs_data_has_user_value(settings, key_c.as_ptr()) {
                    let val = obs::obs_data_get_int(settings, key_c.as_ptr());
                    params.insert(key.to_string(), val.to_string());
                }
            }
        }
        for &key in &string_keys {
            if let Ok(key_c) = CString::new(key) {
                if obs::obs_data_has_user_value(settings, key_c.as_ptr()) {
                    let val_ptr = obs::obs_data_get_string(settings, key_c.as_ptr());
                    if !val_ptr.is_null() {
                        let val = CStr::from_ptr(val_ptr).to_string_lossy().to_string();
                        params.insert(key.to_string(), val);
                    }
                }
            }
        }
    }
}

fn apply_source_params(settings: *mut obs::obs_data, source_type: &str, params: &HashMap<String, String>) {
    if settings.is_null() {
        return;
    }
    unsafe {
        match source_type {
            "color_source" | "color_source_v2" => {
                if let Some(color) = params.get("color").and_then(|v| parse_color_abgr(v)) {
                    let key = CString::new("color").unwrap();
                    obs::obs_data_set_int(settings, key.as_ptr(), color as i64);
                }
                if let Some(width) = params.get("width").and_then(|v| v.parse::<i64>().ok()) {
                    let key = CString::new("width").unwrap();
                    obs::obs_data_set_int(settings, key.as_ptr(), width);
                }
                if let Some(height) = params.get("height").and_then(|v| v.parse::<i64>().ok()) {
                    let key = CString::new("height").unwrap();
                    obs::obs_data_set_int(settings, key.as_ptr(), height);
                }
            }
            "text_ft2_source" | "text_ft2_source_v2" => {
                if let Some(text) = params.get("text") {
                    let key = CString::new("text").unwrap();
                    let val = CString::new(text.as_str()).unwrap();
                    obs::obs_data_set_string(settings, key.as_ptr(), val.as_ptr());
                }
                if let Some(color) = params
                    .get("color1")
                    .or_else(|| params.get("color"))
                    .and_then(|v| parse_color_abgr(v))
                {
                    let key = CString::new("color1").unwrap();
                    obs::obs_data_set_int(settings, key.as_ptr(), color as i64);
                }
                if params.contains_key("font_size") || params.contains_key("font_face") {
                    let font_key = CString::new("font").unwrap();
                    let font_settings = obs::obs_data_create();
                    if !font_settings.is_null() {
                        let face_key = CString::new("face").unwrap();
                        let face_val = CString::new(
                            params
                                .get("font_face")
                                .map(|v| v.as_str())
                                .unwrap_or("DejaVu Sans"),
                        )
                        .unwrap();
                        obs::obs_data_set_string(font_settings, face_key.as_ptr(), face_val.as_ptr());
                        if let Some(size) = params.get("font_size").and_then(|v| v.parse::<i64>().ok()) {
                            let size_key = CString::new("size").unwrap();
                            obs::obs_data_set_int(font_settings, size_key.as_ptr(), size);
                        }
                        obs::obs_data_set_obj(settings, font_key.as_ptr(), font_settings);
                        obs::obs_data_release(font_settings);
                    }
                }

                for (key, value) in params {
                    let normalized_key = key
                        .chars()
                        .filter(|c| c.is_ascii_alphanumeric())
                        .collect::<String>()
                        .to_ascii_lowercase();

                    if normalized_key == "text"
                        || normalized_key == "color"
                        || normalized_key == "color1"
                        || normalized_key == "font"
                        || normalized_key == "fontface"
                        || normalized_key == "fontsize"
                    {
                        continue;
                    }

                    if let Ok(key_c) = CString::new(key.as_str()) {
                        let normalized = value.trim().to_ascii_lowercase();
                        let is_color_like = normalized_key.contains("color");

                        if is_color_like {
                            if let Some(color) = parse_color_abgr(value) {
                                obs::obs_data_set_int(settings, key_c.as_ptr(), color as i64);
                                continue;
                            }
                        }

                        if normalized == "true" || normalized == "false" {
                            obs::obs_data_set_bool(settings, key_c.as_ptr(), normalized == "true");
                        } else if let Ok(int_val) = value.parse::<i64>() {
                            obs::obs_data_set_int(settings, key_c.as_ptr(), int_val);
                        } else if let Ok(float_val) = value.parse::<f64>() {
                            obs::obs_data_set_double(settings, key_c.as_ptr(), float_val);
                        } else if let Ok(val_c) = CString::new(value.as_str()) {
                            obs::obs_data_set_string(settings, key_c.as_ptr(), val_c.as_ptr());
                        }
                    }
                }
            }
            "image_source" => {
                if let Some(file) = params.get("file") {
                    let key = CString::new("file").unwrap();
                    let val = CString::new(file.as_str()).unwrap();
                    obs::obs_data_set_string(settings, key.as_ptr(), val.as_ptr());
                }
            }
            "ffmpeg_source" => {
                if let Some(file) = params.get("file") {
                    let key = CString::new("local_file").unwrap();
                    let val = CString::new(file.as_str()).unwrap();
                    obs::obs_data_set_string(settings, key.as_ptr(), val.as_ptr());
                }
            }
            "browser_source" => {
                if let Some(url) = params.get("url") {
                    let key = CString::new("url").unwrap();
                    let val = CString::new(url.as_str()).unwrap();
                    obs::obs_data_set_string(settings, key.as_ptr(), val.as_ptr());
                }
                if let Some(width) = params.get("width").and_then(|v| v.parse::<i64>().ok()) {
                    let key = CString::new("width").unwrap();
                    obs::obs_data_set_int(settings, key.as_ptr(), width);
                }
                if let Some(height) = params.get("height").and_then(|v| v.parse::<i64>().ok()) {
                    let key = CString::new("height").unwrap();
                    obs::obs_data_set_int(settings, key.as_ptr(), height);
                }
            }
            "window_capture" | "xcomposite_input" => {
                if let Some(window) = params.get("window").or_else(|| params.get("capture_window")) {
                    let val = CString::new(window.as_str()).unwrap();

                    let key_window = CString::new("window").unwrap();
                    obs::obs_data_set_string(settings, key_window.as_ptr(), val.as_ptr());

                    let key_capture_window = CString::new("capture_window").unwrap();
                    obs::obs_data_set_string(settings, key_capture_window.as_ptr(), val.as_ptr());
                }
            }
            "pulse_input_capture" | "pulse_output_capture" => {
                if let Some(device) = params.get("device") {
                    let key = CString::new("device_id").unwrap();
                    let val = CString::new(device.as_str()).unwrap();
                    obs::obs_data_set_string(settings, key.as_ptr(), val.as_ptr());
                }
            }
            _ => {
                for (key, value) in params {
                    if let Ok(key_c) = CString::new(key.as_str()) {
                        if let Ok(int_val) = value.parse::<i64>() {
                            obs::obs_data_set_int(settings, key_c.as_ptr(), int_val);
                        } else if let Ok(val_c) = CString::new(value.as_str()) {
                            obs::obs_data_set_string(settings, key_c.as_ptr(), val_c.as_ptr());
                        }
                    }
                }
            }
        }
    }
}

unsafe extern "C" fn collect_scene_items_cb(
    _scene: *mut obs::obs_scene,
    item: *mut obs::obs_scene_item,
    param: *mut c_void,
) -> bool {
    if !param.is_null() {
        let list = &mut *(param as *mut Vec<*mut obs::obs_scene_item>);
        list.push(item);
    }
    true
}
