pub(crate) fn list_external_source_types(
	state: tauri::State<crate::ObsState>,
) -> Result<Vec<crate::SourceTypeItem>, String> {
	let runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Ok(vec![]);
	}

	let mut out: Vec<crate::SourceTypeItem> = Vec::new();
	let mut idx: usize = 0;

	unsafe {
		loop {
			let mut id_ptr: *const std::os::raw::c_char = std::ptr::null();
			let ok = revo_lib::obs::obs_enum_input_types(
				idx,
				&mut id_ptr as *mut *const std::os::raw::c_char,
			);
			if !ok {
				break;
			}

			if !id_ptr.is_null() {
				let id = std::ffi::CStr::from_ptr(id_ptr).to_string_lossy().to_string();
				if !id.trim().is_empty() {
					let display_ptr = revo_lib::obs::obs_source_get_display_name(id_ptr);
					let label = if display_ptr.is_null() {
						id.clone()
					} else {
						std::ffi::CStr::from_ptr(display_ptr)
							.to_string_lossy()
							.to_string()
					};

					out.push(crate::SourceTypeItem { id, label });
				}
			}

			idx += 1;
		}
	}

	out.sort_by(|a, b| a.label.cmp(&b.label).then(a.id.cmp(&b.id)));
	out.dedup_by(|a, b| a.id == b.id);
	Ok(out)
}

pub(crate) fn list_sources(
	state: tauri::State<crate::ObsState>,
) -> Result<Vec<crate::SourceInfo>, String> {
	let runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	Ok(collect_sources(&runtime))
}

pub(crate) fn collect_sources(runtime: &crate::ObsRuntime) -> Vec<crate::SourceInfo> {
	if !runtime.initialized {
		return vec![];
	}
	let scene = match crate::current_scene(runtime) {
		Ok(scene) => scene,
		Err(_) => return vec![],
	};
	unsafe {
		let mut list = Vec::new();
		let mut items: Vec<*mut revo_lib::obs::obs_scene_item> = Vec::new();
		revo_lib::obs::obs_scene_enum_items(
			scene.scene,
			Some(crate::scenes::scene_items::collect_scene_items_cb),
			&mut items as *mut _ as *mut std::os::raw::c_void,
		);

		let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
		for item in items {
			if item.is_null() {
				continue;
			}
			let source = revo_lib::obs::obs_sceneitem_get_source(item);
			if source.is_null() {
				continue;
			}

			let name_ptr = revo_lib::obs::obs_source_get_name(source);
			let name = if name_ptr.is_null() {
				"".to_string()
			} else {
				std::ffi::CStr::from_ptr(name_ptr).to_string_lossy().to_string()
			};

			let type_ptr = revo_lib::obs::obs_source_get_id(source);
			let source_type = if type_ptr.is_null() {
				"Unknown".to_string()
			} else {
				std::ffi::CStr::from_ptr(type_ptr).to_string_lossy().to_string()
			};

			let id = if item == scene.item_accent {
				"accent".to_string()
			} else if item == scene.item_text {
				"title".to_string()
			} else if let Some((custom_id, _)) = scene.custom_items.iter().find(|(_, v)| **v == item)
			{
				custom_id.clone()
			} else if !name.is_empty() {
				name.clone()
			} else {
				format!("item-{:p}", item)
			};

			if seen.contains(&id) {
				continue;
			}
			seen.insert(id.clone());

			let settings = revo_lib::obs::obs_source_get_settings(source);
			let mut params = std::collections::HashMap::new();
			if !settings.is_null() {
				crate::extract_source_params(settings, &mut params);
				revo_lib::obs::obs_data_release(settings);
			}

			if let Some(color1) = params.get("color1").and_then(|v| v.parse::<u32>().ok()) {
				let hex = crate::abgr_to_hex(color1);
				params.insert("color1".to_string(), hex.clone());
				params.entry("color".to_string()).or_insert(hex);
			} else if let Some(color) = params.get("color").and_then(|v| v.parse::<u32>().ok()) {
				let hex = crate::abgr_to_hex(color);
				params.insert("color".to_string(), hex);
			}

			let mut pos: revo_lib::obs::vec2 = std::mem::zeroed();
			let mut scale: revo_lib::obs::vec2 = std::mem::zeroed();
			revo_lib::obs::obs_sceneitem_get_pos(item, &mut pos as *mut _);
			revo_lib::obs::obs_sceneitem_get_scale(item, &mut scale as *mut _);
			params.insert(
				"pos_x".to_string(),
				pos.__bindgen_anon_1.__bindgen_anon_1.x.to_string(),
			);
			params.insert(
				"pos_y".to_string(),
				pos.__bindgen_anon_1.__bindgen_anon_1.y.to_string(),
			);
			let base_w = revo_lib::obs::obs_source_get_width(source) as f32;
			let base_h = revo_lib::obs::obs_source_get_height(source) as f32;
			if base_w > 0.0 && base_h > 0.0 {
				params.insert(
					"item_width".to_string(),
					(base_w * scale.__bindgen_anon_1.__bindgen_anon_1.x).to_string(),
				);
				params.insert(
					"item_height".to_string(),
					(base_h * scale.__bindgen_anon_1.__bindgen_anon_1.y).to_string(),
				);
			}

			params.insert(
				"scale_x".to_string(),
				scale.__bindgen_anon_1.__bindgen_anon_1.x.to_string(),
			);
			params.insert(
				"scale_y".to_string(),
				scale.__bindgen_anon_1.__bindgen_anon_1.y.to_string(),
			);

			list.push(crate::SourceInfo {
				id,
				name: if name.is_empty() {
					"Source".to_string()
				} else {
					name
				},
				visible: revo_lib::obs::obs_sceneitem_visible(item),
				source_type,
				params,
			});
		}

		list
	}
}

pub(crate) fn remove_source(
	state: tauri::State<crate::ObsState>,
	id: String,
) -> Result<String, String> {
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Err("OBS not initialized".to_string());
	}
	let scene = crate::current_scene_mut(&mut runtime)?;
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
		revo_lib::obs::obs_sceneitem_remove(item_ptr);
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

pub(crate) fn get_source_settings(
	state: tauri::State<crate::ObsState>,
	id: String,
) -> Result<serde_json::Value, String> {
	let runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Err("OBS not initialized".to_string());
	}
	let scene = crate::current_scene(&runtime)?;
	let item = crate::resolve_scene_item(scene, id.as_str())
		.ok_or_else(|| "unknown source id".to_string())?;
	if item.is_null() {
		return Err("source not available".to_string());
	}
	unsafe {
		let source = revo_lib::obs::obs_sceneitem_get_source(item);
		if source.is_null() {
			return Err("source not available".to_string());
		}
		let name_ptr = revo_lib::obs::obs_source_get_name(source);
		let name = if name_ptr.is_null() {
			"".to_string()
		} else {
			std::ffi::CStr::from_ptr(name_ptr).to_string_lossy().to_string()
		};
		let type_ptr = revo_lib::obs::obs_source_get_id(source);
		let source_type = if type_ptr.is_null() {
			"Unknown".to_string()
		} else {
			std::ffi::CStr::from_ptr(type_ptr).to_string_lossy().to_string()
		};
		let settings = revo_lib::obs::obs_source_get_settings(source);
		if settings.is_null() {
			return Err("failed to get source settings".to_string());
		}
		let mut params = std::collections::HashMap::new();
		crate::extract_source_params(settings, &mut params);

		let mut source_properties: Vec<crate::SourcePropertySpec> = Vec::new();
		revo_lib::obs::obs_source_update_properties(source);
		let properties = revo_lib::obs::obs_source_properties(source as *const _);
		if !properties.is_null() {
			crate::refresh_source_properties_with_settings(properties, settings);
			crate::collect_source_property_specs(properties, &mut source_properties);
			revo_lib::obs::obs_properties_destroy(properties);
		}

		revo_lib::obs::obs_data_release(settings);

		for spec in &source_properties {
			if spec.kind == "color" {
				if let Some(raw) = params.get(&spec.key).cloned() {
					if let Ok(v) = raw.parse::<u32>() {
						params.insert(spec.key.clone(), crate::abgr_to_hex(v));
					} else if let Ok(v) = raw.parse::<i64>() {
						params.insert(spec.key.clone(), crate::abgr_to_hex(v as u32));
					}
				}
			}
		}

		let mut pos: revo_lib::obs::vec2 = std::mem::zeroed();
		let mut scale: revo_lib::obs::vec2 = std::mem::zeroed();
		revo_lib::obs::obs_sceneitem_get_pos(item, &mut pos as *mut _);
		revo_lib::obs::obs_sceneitem_get_scale(item, &mut scale as *mut _);
		params.insert(
			"pos_x".to_string(),
			pos.__bindgen_anon_1.__bindgen_anon_1.x.to_string(),
		);
		params.insert(
			"pos_y".to_string(),
			pos.__bindgen_anon_1.__bindgen_anon_1.y.to_string(),
		);
		let base_w = revo_lib::obs::obs_source_get_width(source) as f32;
		let base_h = revo_lib::obs::obs_source_get_height(source) as f32;
		if base_w > 0.0 && base_h > 0.0 {
			params.insert(
				"item_width".to_string(),
				(base_w * scale.__bindgen_anon_1.__bindgen_anon_1.x).to_string(),
			);
			params.insert(
				"item_height".to_string(),
				(base_h * scale.__bindgen_anon_1.__bindgen_anon_1.y).to_string(),
			);
		} else {
			params.insert(
				"scale_x".to_string(),
				scale.__bindgen_anon_1.__bindgen_anon_1.x.to_string(),
			);
			params.insert(
				"scale_y".to_string(),
				scale.__bindgen_anon_1.__bindgen_anon_1.y.to_string(),
			);
		}
		let result = serde_json::json!({
			"name": name,
			"source_type": source_type,
			"params": params,
			"source_properties": source_properties
		});
		Ok(result)
	}
}

pub(crate) fn set_source_visible(
	state: tauri::State<crate::ObsState>,
	id: String,
	visible: bool,
) -> Result<(), String> {
	let runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Err("OBS not initialized".to_string());
	}
	let scene = crate::current_scene(&runtime)?;
	let item = crate::resolve_scene_item(scene, id.as_str())
		.ok_or_else(|| "unknown source id".to_string())?;
	if item.is_null() {
		return Err("source not available".to_string());
	}
	unsafe {
		revo_lib::obs::obs_sceneitem_set_visible(item, visible);
	}
	crate::push_debug_log_entry(
		"obs_set_source_visible".to_string(),
		Some(serde_json::json!({ "id": id, "visible": visible })),
	);
	Ok(())
}

pub(crate) fn move_source(
	state: tauri::State<crate::ObsState>,
	id: String,
	direction: String,
) -> Result<(), String> {
	let runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Err("OBS not initialized".to_string());
	}
	let scene = crate::current_scene(&runtime)?;
	let item = crate::resolve_scene_item(scene, id.as_str())
		.ok_or_else(|| "unknown source id".to_string())?;
	if item.is_null() {
		return Err("source not available".to_string());
	}
	unsafe {
		let movement = match direction.as_str() {
			"up" => revo_lib::obs::obs_order_movement_OBS_ORDER_MOVE_UP,
			"down" => revo_lib::obs::obs_order_movement_OBS_ORDER_MOVE_DOWN,
			"top" => revo_lib::obs::obs_order_movement_OBS_ORDER_MOVE_TOP,
			"bottom" => revo_lib::obs::obs_order_movement_OBS_ORDER_MOVE_BOTTOM,
			_ => return Err("invalid direction".to_string()),
		};
		revo_lib::obs::obs_sceneitem_set_order(item, movement);
	}
	Ok(())
}

pub(crate) fn reorder_source(
	state: tauri::State<crate::ObsState>,
	id: String,
	to_index: usize,
) -> Result<(), String> {
	let runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Err("OBS not initialized".to_string());
	}
	let scene = crate::current_scene(&runtime)?;
	if scene.scene.is_null() {
		return Err("scene unavailable".to_string());
	}

	let mut items: Vec<*mut revo_lib::obs::obs_scene_item> = Vec::new();
	unsafe {
		revo_lib::obs::obs_scene_enum_items(
			scene.scene,
			Some(crate::scenes::scene_items::collect_scene_items_cb),
			&mut items as *mut _ as *mut std::os::raw::c_void,
		);
	}
	let mut ordered_items: Vec<*mut revo_lib::obs::obs_scene_item> = Vec::new();
	for item in items {
		if !item.is_null() {
			ordered_items.push(item);
		}
	}
	let mut ids: Vec<String> = Vec::new();
	for item in ordered_items.iter() {
		if let Some(item_id) = crate::scene_item_id(scene, *item) {
			ids.push(item_id);
		} else {
			ids.push(format!("item-{:p}", item));
		}
	}
	let from_index = ids
		.iter()
		.position(|item_id| item_id == &id)
		.ok_or_else(|| "unknown source id".to_string())?;

	let item = ordered_items.remove(from_index);
	let mut index = if to_index > from_index {
		to_index.saturating_sub(1)
	} else {
		to_index
	};
	index = std::cmp::min(index, ordered_items.len());
	ordered_items.insert(index, item);

	unsafe {
		let ok = revo_lib::obs::obs_scene_reorder_items(
			scene.scene,
			ordered_items.as_mut_ptr(),
			ordered_items.len(),
		);
		if !ok {
			return Err("failed to reorder sources".to_string());
		}
	}
	Ok(())
}

pub(crate) fn create_source_in_scene(
	runtime: &mut crate::ObsRuntime,
	create: &crate::SourceCreate,
) -> Result<String, String> {
	fn input_type_exists(type_id: &str) -> bool {
		let mut idx: usize = 0;
		unsafe {
			loop {
				let mut id_ptr: *const std::os::raw::c_char = std::ptr::null();
				let ok = revo_lib::obs::obs_enum_input_types(
					idx,
					&mut id_ptr as *mut *const std::os::raw::c_char,
				);
				if !ok {
					break;
				}
				if !id_ptr.is_null() {
					let id = std::ffi::CStr::from_ptr(id_ptr).to_string_lossy();
					if id.as_ref() == type_id {
						return true;
					}
				}
				idx += 1;
			}
		}
		false
	}

	fn resolve_creatable_source_type(requested: &str) -> String {
		let req = requested.trim();
		if req.is_empty() {
			return String::new();
		}
		if input_type_exists(req) {
			return req.to_string();
		}

		match req {
			"window_capture" if input_type_exists("xcomposite_input") => {
				"xcomposite_input".to_string()
			}
			"xcomposite_input" if input_type_exists("window_capture") => {
				"window_capture".to_string()
			}
			_ => req.to_string(),
		}
	}

	let scene = crate::current_scene_mut(runtime)?;
	let id = create.id.trim();
	if id.is_empty() {
		return Err("source id required".to_string());
	}
	if let Some(existing) = crate::resolve_scene_item(scene, id) {
		if !existing.is_null() {
			return Err("source id already exists".to_string());
		}
	}

	let name = if create.name.trim().is_empty() {
		id
	} else {
		create.name.as_str()
	};
	let requested_type = create.source_type.trim();
	if requested_type.is_empty() {
		return Err("source type required".to_string());
	}
	let create_type = resolve_creatable_source_type(requested_type);

	unsafe {
		let settings = revo_lib::obs::obs_data_create();
		crate::apply_source_params(settings, create_type.as_str(), &create.params, None);

		let source_id = std::ffi::CString::new(create_type.as_str())
			.map_err(|_| "source type invalid".to_string())?;
		let source_name =
			std::ffi::CString::new(name).map_err(|_| "source name invalid".to_string())?;
		let source = revo_lib::obs::obs_source_create(
			source_id.as_ptr(),
			source_name.as_ptr(),
			settings,
			std::ptr::null_mut(),
		);
		if !settings.is_null() {
			revo_lib::obs::obs_data_release(settings);
		}
		if source.is_null() {
			return Err(format!("failed to create source '{}'", requested_type));
		}

		let item = revo_lib::obs::obs_scene_add(scene.scene, source);
		if item.is_null() {
			revo_lib::obs::obs_source_release(source);
			return Err("failed to add source to scene".to_string());
		}
		revo_lib::obs::obs_sceneitem_set_visible(item, true);
		crate::apply_scene_item_transform(item, source, &create.params);
		revo_lib::obs::obs_source_release(source);
		scene.custom_items.insert(id.to_string(), item);
	}

	Ok(format!("created {id}"))
}

pub(crate) fn create_source(
	state: tauri::State<crate::ObsState>,
	create: crate::SourceCreate,
) -> Result<String, String> {
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Err("OBS not initialized".to_string());
	}
	create_source_in_scene(&mut runtime, &create)
}

fn parse_boolish(value: &str) -> bool {
	matches!(
		value.trim().to_ascii_lowercase().as_str(),
		"1" | "true" | "yes" | "on"
	)
}

fn apply_audio_runtime_params(
	source: *mut revo_lib::obs::obs_source_t,
	params: &std::collections::HashMap<String, String>,
) {
	if source.is_null() {
		return;
	}

	unsafe {
		if let Some(volume_db) = params.get("volume_db").and_then(|v| v.parse::<f32>().ok()) {
			let linear = 10_f32.powf(volume_db / 20.0).max(0.0);
			revo_lib::obs::obs_source_set_volume(source, linear);
		} else if let Some(volume_percent) = params.get("volume_percent").and_then(|v| v.parse::<f32>().ok()) {
			let linear = (volume_percent / 100.0).max(0.0);
			revo_lib::obs::obs_source_set_volume(source, linear);
		}

		if let Some(monitoring) = params.get("monitoring") {
			let monitoring_type = match monitoring.trim().to_ascii_lowercase().as_str() {
				"monitor_only" | "monitor" => 1,
				"on" | "monitor_and_output" | "output" => 2,
				_ => 0,
			};
			revo_lib::obs::obs_source_set_monitoring_type(source, monitoring_type);
		}

		if let Some(tracks_raw) = params.get("audio_tracks") {
			let mut mixers_mask: u32 = 0;
			for token in tracks_raw.split(',').map(|t| t.trim()).filter(|t| !t.is_empty()) {
				if let Ok(track_index) = token.parse::<u32>() {
					if (1..=6).contains(&track_index) {
						mixers_mask |= 1_u32 << (track_index - 1);
					}
				}
			}
			if mixers_mask == 0 {
				mixers_mask = 1;
			}
			revo_lib::obs::obs_source_set_audio_mixers(source, mixers_mask);
		}

		if let Some(muted) = params.get("muted") {
			revo_lib::obs::obs_source_set_muted(source, parse_boolish(muted));
		}
	}
}

pub(crate) fn update_source(
	state: tauri::State<crate::ObsState>,
	update: crate::SourceUpdate,
) -> Result<String, String> {
	crate::push_debug_log_entry(
		"obs_update_source".to_string(),
		Some(serde_json::json!({
			"id": update.id,
			"name": update.name,
			"type": update.source_type,
			"params": update.params
		})),
	);
	let runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		crate::push_debug_log_entry("obs_update_source:error not-initialized".to_string(), None);
		return Err("OBS not initialized".to_string());
	}
	let scene = crate::current_scene(&runtime)?;
	let item = crate::resolve_scene_item(scene, update.id.as_str())
		.ok_or_else(|| "unknown source id".to_string())?;
	if item.is_null() {
		crate::push_debug_log_entry(
			"obs_update_source:error source-not-available".to_string(),
			Some(serde_json::json!({ "id": update.id })),
		);
		return Err("source not available".to_string());
	}
	unsafe {
		let source = revo_lib::obs::obs_sceneitem_get_source(item);
		if source.is_null() {
			return Err("source not available".to_string());
		}
		if !update.name.trim().is_empty() {
			if let Ok(name_c) = std::ffi::CString::new(update.name.as_bytes()) {
				revo_lib::obs::obs_source_set_name(source, name_c.as_ptr());
			}
		}

		let settings = revo_lib::obs::obs_source_get_settings(source);
		if settings.is_null() {
			return Err("failed to get source settings".to_string());
		}

		let type_ptr = revo_lib::obs::obs_source_get_id(source);
		let actual_type = if type_ptr.is_null() {
			"Unknown".to_string()
		} else {
			std::ffi::CStr::from_ptr(type_ptr)
				.to_string_lossy()
				.to_string()
		};
		let chosen_type = if update.source_type.trim().is_empty() || update.source_type == "Unknown" {
			actual_type.as_str()
		} else {
			update.source_type.as_str()
		};
		let is_ffmpeg_source = chosen_type.trim().eq_ignore_ascii_case("ffmpeg_source");
		let was_visible = revo_lib::obs::obs_sceneitem_visible(item);
		let editable_list_keys = crate::source_editable_list_keys(source);
		crate::apply_source_params(settings, chosen_type, &update.params, Some(&editable_list_keys));

		revo_lib::obs::obs_source_update(source, settings);
		revo_lib::obs::obs_data_release(settings);
		crate::apply_scene_item_transform(item, source, &update.params);
		apply_audio_runtime_params(source, &update.params);
		if !is_ffmpeg_source {
			revo_lib::obs::obs_sceneitem_set_visible(item, false);
			revo_lib::obs::obs_sceneitem_set_visible(item, was_visible);
		}
	}
	Ok("updated".to_string())
}

fn resolve_filter_source_ids(kind: &str) -> &'static [&'static str] {
	match kind.trim().to_ascii_lowercase().as_str() {
		"color_correction" => &["color_filter_v2", "color_filter"],
		"chroma_key" => &["chroma_key_filter_v2", "chroma_key_filter"],
		"crop_pad" => &["crop_filter"],
		"gain" => &["gain_filter"],
		"sharpness" => &["sharpness_filter"],
		"scroll" => &["scroll_filter"],
		_ => &[],
	}
}

unsafe extern "C" fn collect_filter_names_cb(
	_parent: *mut revo_lib::obs::obs_source_t,
	child: *mut revo_lib::obs::obs_source_t,
	param: *mut std::os::raw::c_void,
) {
	if param.is_null() || child.is_null() {
		return;
	}
	let out = &mut *(param as *mut Vec<String>);
	let name_ptr = revo_lib::obs::obs_source_get_name(child);
	if name_ptr.is_null() {
		return;
	}
	let name = std::ffi::CStr::from_ptr(name_ptr).to_string_lossy().to_string();
	if !name.trim().is_empty() {
		out.push(name);
	}
}

pub(crate) fn set_source_filters(
	state: tauri::State<crate::ObsState>,
	source_id: String,
	filters: Vec<crate::SourceFilterItem>,
) -> Result<String, String> {
	let runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Err("OBS not initialized".to_string());
	}

	let scene = crate::current_scene(&runtime)?;
	let item = crate::resolve_scene_item(scene, source_id.as_str())
		.ok_or_else(|| "unknown source id".to_string())?;
	if item.is_null() {
		return Err("source not available".to_string());
	}

	unsafe {
		let source = revo_lib::obs::obs_sceneitem_get_source(item);
		if source.is_null() {
			return Err("source not available".to_string());
		}

		let mut desired_names: std::collections::HashSet<String> = std::collections::HashSet::new();

		for (index, filter) in filters.iter().enumerate() {
			let filter_name = if filter.name.trim().is_empty() {
				format!("Filter {}", index + 1)
			} else {
				filter.name.trim().to_string()
			};
			desired_names.insert(filter_name.clone());

			let filter_source_ids = resolve_filter_source_ids(filter.kind.as_str());
			if filter_source_ids.is_empty() {
				continue;
			}
			let primary_filter_source_id = filter_source_ids[0];

			let filter_name_c = std::ffi::CString::new(filter_name.as_str())
				.map_err(|_| "invalid filter name".to_string())?;
			let existing_filter = revo_lib::obs::obs_source_get_filter_by_name(source, filter_name_c.as_ptr());

			if !existing_filter.is_null() {
				let settings = revo_lib::obs::obs_source_get_settings(existing_filter);
				if !settings.is_null() {
					crate::apply_source_params(settings, primary_filter_source_id, &filter.params, None);
					revo_lib::obs::obs_source_update(existing_filter, settings);
					revo_lib::obs::obs_data_release(settings);
				}
				revo_lib::obs::obs_source_set_enabled(existing_filter, filter.enabled);
				revo_lib::obs::obs_source_release(existing_filter);
				continue;
			}
			let settings = revo_lib::obs::obs_data_create();
			if settings.is_null() {
				continue;
			}
			crate::apply_source_params(settings, primary_filter_source_id, &filter.params, None);

			let mut filter_source: *mut revo_lib::obs::obs_source_t = std::ptr::null_mut();
			for source_id in filter_source_ids {
				let Ok(filter_id_c) = std::ffi::CString::new(*source_id) else {
					continue;
				};
				filter_source = revo_lib::obs::obs_source_create(
					filter_id_c.as_ptr(),
					filter_name_c.as_ptr(),
					settings,
					std::ptr::null_mut(),
				);
				if !filter_source.is_null() {
					break;
				}
			}
			revo_lib::obs::obs_data_release(settings);

			if filter_source.is_null() {
				continue;
			}

			revo_lib::obs::obs_source_filter_add(source, filter_source);
			revo_lib::obs::obs_source_set_enabled(filter_source, filter.enabled);
			revo_lib::obs::obs_source_release(filter_source);
		}

		let mut existing_names: Vec<String> = Vec::new();
		revo_lib::obs::obs_source_enum_filters(
			source,
			Some(collect_filter_names_cb),
			&mut existing_names as *mut _ as *mut std::os::raw::c_void,
		);

		for name in existing_names {
			if desired_names.contains(&name) {
				continue;
			}
			let Ok(name_c) = std::ffi::CString::new(name.as_str()) else {
				continue;
			};
			let existing = revo_lib::obs::obs_source_get_filter_by_name(source, name_c.as_ptr());
			if existing.is_null() {
				continue;
			}
			revo_lib::obs::obs_source_filter_remove(source, existing);
			revo_lib::obs::obs_source_release(existing);
		}
	}

	Ok("filters-updated".to_string())
}

pub(crate) fn open_source_interaction(
	app: tauri::AppHandle,
	state: tauri::State<crate::ObsState>,
	id: String,
) -> Result<String, String> {
	let runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Err("OBS is not initialized".to_string());
	}

	let scene = crate::current_scene(&runtime)?;
	let item = crate::resolve_scene_item(scene, &id)
		.ok_or_else(|| format!("source '{}' not found", id))?;

	let mut source_name = id.clone();
	let mut browser_url: Option<String> = None;

	unsafe {
		let source = revo_lib::obs::obs_sceneitem_get_source(item);
		if source.is_null() {
			return Err("source pointer is null".to_string());
		}

		let source_type_ptr = revo_lib::obs::obs_source_get_id(source);
		let source_type = if source_type_ptr.is_null() {
			String::new()
		} else {
			std::ffi::CStr::from_ptr(source_type_ptr)
				.to_string_lossy()
				.to_string()
		};

		if source_type != "browser_source" {
			return Err(format!(
				"source '{}' is type '{}', expected 'browser_source'",
				id, source_type
			));
		}

		let name_ptr = revo_lib::obs::obs_source_get_name(source);
		if !name_ptr.is_null() {
			source_name = std::ffi::CStr::from_ptr(name_ptr)
				.to_string_lossy()
				.to_string();
		}

		let settings = revo_lib::obs::obs_source_get_settings(source);
		if !settings.is_null() {
			let key = std::ffi::CString::new("url").map_err(|_| "invalid key".to_string())?;
			let url_ptr = revo_lib::obs::obs_data_get_string(settings, key.as_ptr());
			if !url_ptr.is_null() {
				let url = std::ffi::CStr::from_ptr(url_ptr).to_string_lossy().to_string();
				if !url.trim().is_empty() {
					browser_url = Some(url);
				}
			}
			revo_lib::obs::obs_data_release(settings);
		}
	}

	if let Some(url) = browser_url {
		let parsed = tauri::Url::parse(url.trim())
			.map_err(|e| format!("invalid browser source url: {e}"))?;
		let label = format!("browser-interact-{}", uuid::Uuid::new_v4());
		tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::External(parsed))
			.title(format!("Interact - {}", source_name))
			.inner_size(1200.0, 760.0)
			.resizable(true)
			.build()
			.map_err(|e| format!("failed to open browser interaction window: {e}"))?;
	}

	Ok("Interaction window opened".to_string())
}
