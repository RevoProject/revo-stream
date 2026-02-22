pub(crate) fn list_scenes(
	state: tauri::State<crate::ObsState>,
) -> Result<Vec<crate::SceneInfo>, String> {
	let runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Ok(vec![]);
	}

	let current = runtime.current_scene.clone().unwrap_or_default();
	let mut list = Vec::new();
	for name in runtime.scene_order.iter() {
		if !runtime.scenes.contains_key(name) {
			continue;
		}
		list.push(crate::SceneInfo {
			name: name.clone(),
			active: name == &current,
			locked: runtime.locked_scenes.contains(name),
		});
	}
	for name in runtime.scenes.keys() {
		if runtime.scene_order.iter().any(|n| n == name) {
			continue;
		}
		list.push(crate::SceneInfo {
			name: name.clone(),
			active: name == &current,
			locked: runtime.locked_scenes.contains(name),
		});
	}
	Ok(list)
}

pub(crate) fn set_current_scene(
	state: tauri::State<crate::ObsState>,
	name: String,
) -> Result<String, String> {
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Err("OBS not initialized".to_string());
	}
	let trimmed = name.trim();
	if trimmed.is_empty() {
		return Err("scene name required".to_string());
	}
	crate::set_current_scene_internal(&mut runtime, trimmed)?;
	Ok(format!("active scene: {trimmed}"))
}

pub(crate) fn create_scene(
	state: tauri::State<crate::ObsState>,
	name: String,
) -> Result<String, String> {
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
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

	let scene_name =
		std::ffi::CString::new(trimmed).map_err(|_| "scene name invalid".to_string())?;
	let scene = unsafe { revo_lib::obs::obs_scene_create(scene_name.as_ptr()) };
	if scene.is_null() {
		return Err("failed to create scene".to_string());
	}
	let scene_source = unsafe { revo_lib::obs::obs_scene_get_source(scene) };
	let state = crate::SceneState::new(trimmed.to_string(), scene, scene_source);
	runtime.scenes.insert(trimmed.to_string(), state);
	if !runtime.scene_order.iter().any(|n| n == trimmed) {
		runtime.scene_order.push(trimmed.to_string());
	}
	crate::set_current_scene_internal(&mut runtime, trimmed)?;
	Ok(format!("created {trimmed}"))
}

pub(crate) fn rename_scene(
	state: tauri::State<crate::ObsState>,
	old_name: String,
	new_name: String,
) -> Result<String, String> {
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
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
			if let Ok(name_c) = std::ffi::CString::new(new_trim) {
				revo_lib::obs::obs_source_set_name(state.scene_source, name_c.as_ptr());
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
	for name in runtime.scene_order.iter_mut() {
		if name == old_trim {
			*name = new_trim.to_string();
		}
	}
	Ok(format!("renamed {old_trim} to {new_trim}"))
}

pub(crate) fn remove_scene(
	state: tauri::State<crate::ObsState>,
	name: String,
) -> Result<String, String> {
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
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
			revo_lib::obs::obs_source_dec_showing(state.scene_source);
		}
		if !state.scene.is_null() {
			revo_lib::obs::obs_scene_enum_items(
				state.scene,
				Some(crate::scenes::scene_items::remove_scene_item_cb),
				std::ptr::null_mut(),
			);
			revo_lib::obs::obs_scene_release(state.scene);
		}
	}

	if runtime.current_scene.as_deref() == Some(trimmed) {
		if let Some((next_name, _)) = runtime.scenes.iter().next() {
			let next = next_name.clone();
			crate::set_current_scene_internal(&mut runtime, &next)?;
		} else {
			runtime.current_scene = None;
		}
	}
	runtime.locked_scenes.remove(trimmed);
	runtime.scene_order.retain(|n| n != trimmed);
	Ok(format!("removed {trimmed}"))
}

pub(crate) fn set_scene_lock(
	state: tauri::State<crate::ObsState>,
	name: String,
	locked: bool,
) -> Result<String, String> {
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
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

pub(crate) fn reorder_scene(
	state: tauri::State<crate::ObsState>,
	name: String,
	to_index: usize,
) -> Result<(), String> {
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Err("OBS not initialized".to_string());
	}
	if !runtime.scenes.contains_key(name.as_str()) {
		return Err("scene not found".to_string());
	}
	let from_index = runtime
		.scene_order
		.iter()
		.position(|n| n == &name)
		.ok_or_else(|| "scene not found".to_string())?;
	let scene_name = runtime.scene_order.remove(from_index);
	let mut index = if to_index > from_index {
		to_index.saturating_sub(1)
	} else {
		to_index
	};
	index = std::cmp::min(index, runtime.scene_order.len());
	runtime.scene_order.insert(index, scene_name);
	Ok(())
}
