pub(crate) fn reset_scenes(runtime: &mut crate::ObsRuntime) {
	let names: Vec<String> = runtime.scenes.keys().cloned().collect();
	for name in names {
		if let Some(state) = runtime.scenes.remove(&name) {
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
		}
	}
	runtime.current_scene = None;
	runtime.locked_scenes.clear();
	runtime.scene_order.clear();
}

fn export_filter_kind_from_source_id(source_id: &str) -> String {
	match source_id.trim().to_ascii_lowercase().as_str() {
		"color_filter_v2" | "color_filter" => "color_correction".to_string(),
		"chroma_key_filter_v2" | "chroma_key_filter" => "chroma_key".to_string(),
		"crop_filter" => "crop_pad".to_string(),
		"gain_filter" => "gain".to_string(),
		"sharpness_filter" => "sharpness".to_string(),
		"scroll_filter" => "scroll".to_string(),
		other => other.to_string(),
	}
}

fn import_filter_source_ids(kind: &str) -> &'static [&'static str] {
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

unsafe extern "C" fn collect_source_filters_export_cb(
	_parent: *mut revo_lib::obs::obs_source_t,
	child: *mut revo_lib::obs::obs_source_t,
	param: *mut std::os::raw::c_void,
) {
	if child.is_null() || param.is_null() {
		return;
	}

	let out = &mut *(param as *mut Vec<serde_json::Value>);
	let name_ptr = revo_lib::obs::obs_source_get_name(child);
	if name_ptr.is_null() {
		return;
	}
	let filter_name = std::ffi::CStr::from_ptr(name_ptr).to_string_lossy().to_string();
	if filter_name.trim().is_empty() {
		return;
	}

	let id_ptr = revo_lib::obs::obs_source_get_id(child);
	let filter_source_id = if id_ptr.is_null() {
		"custom".to_string()
	} else {
		std::ffi::CStr::from_ptr(id_ptr).to_string_lossy().to_string()
	};

	let mut params = std::collections::HashMap::new();
	let settings = revo_lib::obs::obs_source_get_settings(child);
	if !settings.is_null() {
		crate::extract_source_params(settings, &mut params);
		revo_lib::obs::obs_data_release(settings);
	}

	out.push(serde_json::json!({
		"name": filter_name,
		"kind": export_filter_kind_from_source_id(&filter_source_id),
		"enabled": revo_lib::obs::obs_source_enabled(child),
		"params": params
	}));
}

unsafe fn apply_imported_filters_to_source(
	source: *mut revo_lib::obs::obs_source_t,
	raw_filters: &[serde_json::Value],
) {
	if source.is_null() {
		return;
	}

	for (index, filter_val) in raw_filters.iter().enumerate() {
		let Some(filter_obj) = filter_val.as_object() else {
			continue;
		};

		let filter_name = filter_obj
			.get("name")
			.and_then(|v| v.as_str())
			.map(|v| v.trim())
			.filter(|v| !v.is_empty())
			.map(|v| v.to_string())
			.unwrap_or_else(|| format!("Filter {}", index + 1));

		let filter_kind = filter_obj
			.get("kind")
			.and_then(|v| v.as_str())
			.unwrap_or("custom");

		let enabled = filter_obj
			.get("enabled")
			.and_then(|v| v.as_bool())
			.unwrap_or(true);

		let filter_source_ids = import_filter_source_ids(filter_kind);
		if filter_source_ids.is_empty() {
			continue;
		}
		let primary_filter_source_id = filter_source_ids[0];

		let mut params = std::collections::HashMap::new();
		if let Some(settings_obj) = filter_obj.get("params").and_then(|v| v.as_object()) {
			for (k, v) in settings_obj {
				if let Some(s) = v.as_str() {
					params.insert(k.clone(), s.to_string());
				} else if let Some(i) = v.as_i64() {
					params.insert(k.clone(), i.to_string());
				} else if let Some(f) = v.as_f64() {
					params.insert(k.clone(), f.to_string());
				} else if let Some(b) = v.as_bool() {
					params.insert(k.clone(), b.to_string());
				}
			}
		}

		let Ok(filter_name_c) = std::ffi::CString::new(filter_name.as_str()) else {
			continue;
		};

		let existing = revo_lib::obs::obs_source_get_filter_by_name(source, filter_name_c.as_ptr());
		if !existing.is_null() {
			let settings = revo_lib::obs::obs_source_get_settings(existing);
			if !settings.is_null() {
				crate::apply_source_params(settings, primary_filter_source_id, &params, None);
				revo_lib::obs::obs_source_update(existing, settings);
				revo_lib::obs::obs_data_release(settings);
			}
			revo_lib::obs::obs_source_set_enabled(existing, enabled);
			revo_lib::obs::obs_source_release(existing);
			continue;
		}

		let settings = revo_lib::obs::obs_data_create();
		if settings.is_null() {
			continue;
		}
		crate::apply_source_params(settings, primary_filter_source_id, &params, None);

		let mut created_filter: *mut revo_lib::obs::obs_source_t = std::ptr::null_mut();
		for source_id in filter_source_ids {
			let Ok(filter_id_c) = std::ffi::CString::new(*source_id) else {
				continue;
			};
			created_filter = revo_lib::obs::obs_source_create(
				filter_id_c.as_ptr(),
				filter_name_c.as_ptr(),
				settings,
				std::ptr::null_mut(),
			);
			if !created_filter.is_null() {
				break;
			}
		}
		revo_lib::obs::obs_data_release(settings);

		if created_filter.is_null() {
			continue;
		}

		revo_lib::obs::obs_source_filter_add(source, created_filter);
		revo_lib::obs::obs_source_set_enabled(created_filter, enabled);
		revo_lib::obs::obs_source_release(created_filter);
	}
}

pub(crate) fn export_scene_collection(
	state: tauri::State<crate::ObsState>,
) -> Result<String, String> {
	let runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Err("OBS not initialized".to_string());
	}
	let mut scenes_json = Vec::new();

	let names: Vec<String> = if runtime.scene_order.is_empty() {
		runtime.scenes.keys().cloned().collect()
	} else {
		runtime.scene_order.clone()
	};

	for name in names.iter() {
		let scene = match runtime.scenes.get(name) {
			Some(scene) => scene,
			None => continue,
		};
		if scene.scene.is_null() {
			continue;
		}
		let mut items: Vec<*mut revo_lib::obs::obs_scene_item> = Vec::new();
		unsafe {
			revo_lib::obs::obs_scene_enum_items(
				scene.scene,
				Some(crate::scenes::scene_items::collect_scene_items_cb),
				&mut items as *mut _ as *mut std::os::raw::c_void,
			);
		}
		let mut sources_json = Vec::new();
		for item in items {
			if item.is_null() {
				continue;
			}
			unsafe {
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
				let settings = revo_lib::obs::obs_source_get_settings(source);
				let mut params = std::collections::HashMap::new();
				crate::extract_source_params(settings, &mut params);
				if !settings.is_null() {
					revo_lib::obs::obs_data_release(settings);
				}
				let mut filters_json: Vec<serde_json::Value> = Vec::new();
				revo_lib::obs::obs_source_enum_filters(
					source,
					Some(collect_source_filters_export_cb),
					&mut filters_json as *mut _ as *mut std::os::raw::c_void,
				);
				let mut pos: revo_lib::obs::vec2 = std::mem::zeroed();
				let mut scale: revo_lib::obs::vec2 = std::mem::zeroed();
				revo_lib::obs::obs_sceneitem_get_pos(item, &mut pos as *mut _);
				revo_lib::obs::obs_sceneitem_get_scale(item, &mut scale as *mut _);
				let base_w = revo_lib::obs::obs_source_get_width(source) as f32;
				let base_h = revo_lib::obs::obs_source_get_height(source) as f32;
				let size = if base_w > 0.0 && base_h > 0.0 {
					serde_json::json!({
						"width": base_w * scale.__bindgen_anon_1.__bindgen_anon_1.x,
						"height": base_h * scale.__bindgen_anon_1.__bindgen_anon_1.y
					})
				} else {
					serde_json::Value::Null
				};
				sources_json.push(serde_json::json!({
					"name": name,
					"id": source_type,
					"settings": params,
					"filters": filters_json,
					"visible": revo_lib::obs::obs_sceneitem_visible(item),
					"transform": {
						"pos": {"x": pos.__bindgen_anon_1.__bindgen_anon_1.x, "y": pos.__bindgen_anon_1.__bindgen_anon_1.y},
						"scale": {"x": scale.__bindgen_anon_1.__bindgen_anon_1.x, "y": scale.__bindgen_anon_1.__bindgen_anon_1.y},
						"size": size
					}
				}));
			}
		}
		scenes_json.push(serde_json::json!({
			"name": name,
			"sources": sources_json
		}));
	}

	let doc = serde_json::json!({
		"name": "RevoStream",
		"scenes": scenes_json
	});
	serde_json::to_string_pretty(&doc).map_err(|e| format!("export failed: {e}"))
}

pub(crate) fn export_scene_collection_obs(
	state: tauri::State<crate::ObsState>,
) -> Result<String, String> {
	let runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Err("OBS not initialized".to_string());
	}

	let mut scene_names: Vec<String> = runtime.scene_order.clone();
	if scene_names.is_empty() {
		scene_names = runtime.scenes.keys().cloned().collect();
	}
	let current_scene = runtime
		.current_scene
		.clone()
		.or_else(|| scene_names.first().cloned())
		.unwrap_or_else(|| "Scene".to_string());

	unsafe {
		let sources = revo_lib::obs::obs_save_sources();
		if sources.is_null() {
			return Err("failed to save sources".to_string());
		}

		let doc = revo_lib::obs::obs_data_create();
		if doc.is_null() {
			revo_lib::obs::obs_data_array_release(sources);
			return Err("failed to create obs_data".to_string());
		}

		let scene_order = revo_lib::obs::obs_data_array_create();
		if scene_order.is_null() {
			revo_lib::obs::obs_data_array_release(sources);
			revo_lib::obs::obs_data_release(doc);
			return Err("failed to create scene_order".to_string());
		}

		for name in &scene_names {
			let obj = revo_lib::obs::obs_data_create();
			if obj.is_null() {
				continue;
			}
			let name_key = std::ffi::CString::new("name").unwrap();
			let name_val = std::ffi::CString::new(name.as_str())
				.map_err(|_| "scene name invalid".to_string())?;
			revo_lib::obs::obs_data_set_string(obj, name_key.as_ptr(), name_val.as_ptr());
			revo_lib::obs::obs_data_array_push_back(scene_order, obj);
			revo_lib::obs::obs_data_release(obj);
		}

		let sources_key = std::ffi::CString::new("sources").unwrap();
		revo_lib::obs::obs_data_set_array(doc, sources_key.as_ptr(), sources);

		let scene_order_key = std::ffi::CString::new("scene_order").unwrap();
		revo_lib::obs::obs_data_set_array(doc, scene_order_key.as_ptr(), scene_order);

		let current_key = std::ffi::CString::new("current_scene").unwrap();
		let current_val = std::ffi::CString::new(current_scene.clone())
			.map_err(|_| "current scene invalid".to_string())?;
		revo_lib::obs::obs_data_set_string(doc, current_key.as_ptr(), current_val.as_ptr());

		let current_prog_key = std::ffi::CString::new("current_program_scene").unwrap();
		let current_prog_val = std::ffi::CString::new(current_scene.clone())
			.map_err(|_| "current scene invalid".to_string())?;
		revo_lib::obs::obs_data_set_string(doc, current_prog_key.as_ptr(), current_prog_val.as_ptr());

		let name_key = std::ffi::CString::new("name").unwrap();
		let name_val = std::ffi::CString::new("RevoStream").unwrap();
		revo_lib::obs::obs_data_set_string(doc, name_key.as_ptr(), name_val.as_ptr());

		let version_key = std::ffi::CString::new("version").unwrap();
		revo_lib::obs::obs_data_set_int(doc, version_key.as_ptr(), 1);

		let json_ptr = revo_lib::obs::obs_data_get_json_pretty(doc);
		let json = if json_ptr.is_null() {
			Err("failed to serialize obs data".to_string())
		} else {
			Ok(std::ffi::CStr::from_ptr(json_ptr).to_string_lossy().to_string())
		};

		revo_lib::obs::obs_data_array_release(scene_order);
		revo_lib::obs::obs_data_array_release(sources);
		revo_lib::obs::obs_data_release(doc);

		json
	}
}

pub(crate) fn export_scene_collection_obs_to_file(
	state: tauri::State<crate::ObsState>,
	path: String,
	ui_json: Option<String>,
) -> Result<String, String> {
	let mut exported: serde_json::Value = serde_json::from_str(&export_scene_collection_obs(state)?)
		.map_err(|e| format!("failed to parse exported scene collection: {e}"))?;
	if let Some(raw_ui) = ui_json {
		let trimmed = raw_ui.trim();
		if !trimmed.is_empty() {
			let ui_value: serde_json::Value = serde_json::from_str(trimmed)
				.map_err(|e| format!("invalid ui_json payload: {e}"))?;
			if let Some(obj) = exported.as_object_mut() {
				obj.insert("revo_ui".to_string(), ui_value);
			}
		}
	}
	let json = serde_json::to_string_pretty(&exported)
		.map_err(|e| format!("failed to serialize scene collection: {e}"))?;
	let target = std::path::PathBuf::from(path);
	if let Some(parent) = target.parent() {
		std::fs::create_dir_all(parent).map_err(|e| format!("failed to create dir: {e}"))?;
	}
	std::fs::write(&target, json).map_err(|e| format!("failed to write file: {e}"))?;
	Ok(format!("OBS scenes exported to {}", target.to_string_lossy()))
}

pub(crate) fn export_scene_collection_to_file(
	state: tauri::State<crate::ObsState>,
	path: String,
	ui_json: Option<String>,
) -> Result<String, String> {
	let mut exported: serde_json::Value = serde_json::from_str(&export_scene_collection(state)?)
		.map_err(|e| format!("failed to parse exported scene collection: {e}"))?;
	if let Some(raw_ui) = ui_json {
		let trimmed = raw_ui.trim();
		if !trimmed.is_empty() {
			let ui_value: serde_json::Value = serde_json::from_str(trimmed)
				.map_err(|e| format!("invalid ui_json payload: {e}"))?;
			if let Some(obj) = exported.as_object_mut() {
				obj.insert("revo_ui".to_string(), ui_value);
			}
		}
	}
	let json = serde_json::to_string_pretty(&exported)
		.map_err(|e| format!("failed to serialize scene collection: {e}"))?;
	let target = std::path::PathBuf::from(path);
	if let Some(parent) = target.parent() {
		std::fs::create_dir_all(parent).map_err(|e| format!("failed to create dir: {e}"))?;
	}
	std::fs::write(&target, json).map_err(|e| format!("failed to write file: {e}"))?;
	Ok(format!("Scenes exported to {}", target.to_string_lossy()))
}

pub(crate) fn import_scene_collection(
	state: tauri::State<crate::ObsState>,
	json: String,
) -> Result<String, String> {
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Err("OBS not initialized".to_string());
	}
	let doc: serde_json::Value =
		serde_json::from_str(&json).map_err(|e| format!("invalid JSON: {e}"))?;
	let mut scenes: Vec<serde_json::Value> = Vec::new();

	let mut source_catalog: std::collections::HashMap<String, (String, serde_json::Value)> =
		std::collections::HashMap::new();
	if let Some(arr) = doc.get("sources").and_then(|v| v.as_array()) {
		for src in arr {
			let id = src.get("id").and_then(|v| v.as_str()).unwrap_or("");
			let name = src.get("name").and_then(|v| v.as_str()).unwrap_or("");
			let settings = src.get("settings").cloned().unwrap_or(serde_json::Value::Null);
			if !name.is_empty() {
				source_catalog.insert(name.to_string(), (id.to_string(), settings));
			}
			if id == "scene" {
				let scene_name = if name.is_empty() { "Scene" } else { name };
				let items = src
					.get("settings")
					.and_then(|s| s.get("items"))
					.and_then(|v| v.as_array())
					.cloned()
					.unwrap_or_default();
				scenes.push(serde_json::json!({
					"name": scene_name,
					"sources": items
				}));
			}
		}
	}

	if scenes.is_empty() {
		if let Some(arr) = doc.get("scenes").and_then(|v| v.as_array()) {
			scenes = arr.clone();
		} else if let Some(arr) = doc.as_array() {
			scenes = arr.clone();
		}
	}

	if scenes.is_empty() {
		return Err("missing scenes array".to_string());
	}

	reset_scenes(&mut runtime);
	runtime.scene_order.clear();

	let mut first_scene_name: Option<String> = None;
	let mut created_any = false;
	let mut skipped = 0;
	for scene_val in scenes {
		let scene_name = scene_val
			.get("name")
			.and_then(|v| v.as_str())
			.unwrap_or("Scene")
			.to_string();

		if first_scene_name.is_none() {
			first_scene_name = Some(scene_name.clone());
		}

		let scene_name_c = std::ffi::CString::new(scene_name.as_str())
			.map_err(|_| "scene name invalid".to_string())?;
		let scene_ptr = unsafe { revo_lib::obs::obs_scene_create(scene_name_c.as_ptr()) };
		if scene_ptr.is_null() {
			return Err("failed to create scene".to_string());
		}
		let scene_source = unsafe { revo_lib::obs::obs_scene_get_source(scene_ptr) };
		let state = crate::SceneState::new(scene_name.clone(), scene_ptr, scene_source);
		runtime.scenes.insert(scene_name.clone(), state);
		runtime.scene_order.push(scene_name.clone());

		let sources = scene_val
			.get("sources")
			.and_then(|v| v.as_array())
			.cloned()
			.or_else(|| scene_val.get("items").and_then(|v| v.as_array()).cloned())
			.unwrap_or_default();
		for src in sources {
			let item_name = src
				.get("name")
				.or_else(|| src.get("source_name"))
				.and_then(|v| v.as_str())
				.unwrap_or("Source")
				.to_string();

			let (source_type, settings) =
				if let Some((id, settings)) = source_catalog.get(&item_name) {
					(id.clone(), settings.clone())
				} else {
					let source_type = src
						.get("id")
						.and_then(|v| v.as_str())
						.or_else(|| src.get("source_type").and_then(|v| v.as_str()))
						.or_else(|| src.get("type").and_then(|v| v.as_str()))
						.unwrap_or("color_source")
						.to_string();
					let settings = src
						.get("settings")
						.cloned()
						.or_else(|| src.get("params").cloned())
						.unwrap_or(serde_json::Value::Null);
					(source_type, settings)
				};
			let source_type = match source_type.as_str() {
				"color_source" => "color_source_v2".to_string(),
				"text_ft2_source" => "text_ft2_source_v2".to_string(),
				"text_gdiplus" => "text_ft2_source_v2".to_string(),
				other => other.to_string(),
			};

			let mut params = std::collections::HashMap::new();
			if let serde_json::Value::Object(map) = settings {
				for (k, v) in map {
					if let Some(s) = v.as_str() {
						params.insert(k, s.to_string());
					} else if let Some(n) = v.as_i64() {
						params.insert(k, n.to_string());
					} else if let Some(n) = v.as_f64() {
						params.insert(k, n.to_string());
					} else if let Some(b) = v.as_bool() {
						params.insert(k, b.to_string());
					}
				}
			}

			if let Some(pos) = src.get("pos") {
				if let Some(x) = pos.get("x").and_then(|v| v.as_f64()) {
					params.insert("pos_x".to_string(), x.to_string());
				}
				if let Some(y) = pos.get("y").and_then(|v| v.as_f64()) {
					params.insert("pos_y".to_string(), y.to_string());
				}
			}
			if let Some(scale) = src.get("scale") {
				if let Some(x) = scale.get("x").and_then(|v| v.as_f64()) {
					params.insert("scale_x".to_string(), x.to_string());
				}
				if let Some(y) = scale.get("y").and_then(|v| v.as_f64()) {
					params.insert("scale_y".to_string(), y.to_string());
				}
			}
			if let Some(bounds) = src.get("bounds") {
				if let Some(x) = bounds.get("x").and_then(|v| v.as_f64()) {
					if x > 0.0 {
						params.insert("item_width".to_string(), x.to_string());
					}
				}
				if let Some(y) = bounds.get("y").and_then(|v| v.as_f64()) {
					if y > 0.0 {
						params.insert("item_height".to_string(), y.to_string());
					}
				}
			}
			if let Some(transform) = src.get("transform") {
				if let Some(pos) = transform.get("pos") {
					if let Some(x) = pos.get("x").and_then(|v| v.as_f64()) {
						params.insert("pos_x".to_string(), x.to_string());
					}
					if let Some(y) = pos.get("y").and_then(|v| v.as_f64()) {
						params.insert("pos_y".to_string(), y.to_string());
					}
				}
				if let Some(size) = transform.get("size") {
					if let Some(w) = size.get("width").and_then(|v| v.as_f64()) {
						params.insert("item_width".to_string(), w.to_string());
					}
					if let Some(h) = size.get("height").and_then(|v| v.as_f64()) {
						params.insert("item_height".to_string(), h.to_string());
					}
				}
				if let Some(scale) = transform.get("scale") {
					if let Some(x) = scale.get("x").and_then(|v| v.as_f64()) {
						params.insert("scale_x".to_string(), x.to_string());
					}
					if let Some(y) = scale.get("y").and_then(|v| v.as_f64()) {
						params.insert("scale_y".to_string(), y.to_string());
					}
				}
			}

			let name = item_name;
			let id = name.clone();
			let create = crate::SourceCreate {
				id,
				name,
				source_type,
				params,
			};
			crate::set_current_scene_internal(&mut runtime, scene_name.as_str())?;
			match crate::create_source_in_scene(&mut runtime, &create) {
				Ok(_) => {
					created_any = true;
					let imported_filters = src
						.get("filters")
						.and_then(|v| v.as_array())
						.cloned()
						.unwrap_or_default();
					if !imported_filters.is_empty() {
						let current_scene = crate::current_scene(&runtime)?;
						if let Some(item) = crate::resolve_scene_item(current_scene, create.id.as_str()) {
							if !item.is_null() {
								unsafe {
									let source = revo_lib::obs::obs_sceneitem_get_source(item);
									apply_imported_filters_to_source(source, &imported_filters);
								}
							}
						}
					}
				}
				Err(_) => skipped += 1,
			}
		}
	}

	if let Some(first) = first_scene_name {
		let _ = crate::set_current_scene_internal(&mut runtime, first.as_str());
	}

	if !created_any {
		return Err("no sources imported (unsupported types?)".to_string());
	}
	if skipped > 0 {
		return Ok(format!("scene collection imported with {skipped} skipped sources"));
	}
	Ok("scene collection imported".to_string())
}
