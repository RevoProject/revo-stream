pub(crate) fn extract_source_params(
	settings: *mut revo_lib::obs::obs_data,
	params: &mut std::collections::HashMap<String, String>,
) {
	if settings.is_null() {
		return;
	}
	unsafe {
		let mut item = revo_lib::obs::obs_data_first(settings);
		while !item.is_null() {
			let key_ptr = revo_lib::obs::obs_data_item_get_name(item);
			if !key_ptr.is_null() {
				let key = std::ffi::CStr::from_ptr(key_ptr).to_string_lossy().to_string();
				if !key.is_empty() {
					let value = match revo_lib::obs::obs_data_item_gettype(item) {
						t if t == revo_lib::obs::obs_data_type_OBS_DATA_STRING => {
							let ptr = revo_lib::obs::obs_data_item_get_string(item);
							if ptr.is_null() {
								String::new()
							} else {
								std::ffi::CStr::from_ptr(ptr).to_string_lossy().to_string()
							}
						}
						t if t == revo_lib::obs::obs_data_type_OBS_DATA_NUMBER => {
							if revo_lib::obs::obs_data_item_numtype(item)
								== revo_lib::obs::obs_data_number_type_OBS_DATA_NUM_DOUBLE
							{
								revo_lib::obs::obs_data_item_get_double(item).to_string()
							} else {
								revo_lib::obs::obs_data_item_get_int(item).to_string()
							}
						}
						t if t == revo_lib::obs::obs_data_type_OBS_DATA_BOOLEAN => {
							if revo_lib::obs::obs_data_item_get_bool(item) {
								"true".to_string()
							} else {
								"false".to_string()
							}
						}
						t if t == revo_lib::obs::obs_data_type_OBS_DATA_ARRAY => {
							let arr = revo_lib::obs::obs_data_item_get_array(item);
							if arr.is_null() {
								String::new()
							} else {
								let out = obs_data_array_to_multiline(arr);
								revo_lib::obs::obs_data_array_release(arr);
								out
							}
						}
						_ => String::new(),
					};
					if !value.is_empty() {
						params.insert(key, value);
					}
				}
			}

			if !revo_lib::obs::obs_data_item_next(&mut item as *mut _) {
				break;
			}
		}
		if !item.is_null() {
			revo_lib::obs::obs_data_item_release(&mut item as *mut _);
		}

		if let Ok(font_key) = std::ffi::CString::new("font") {
			let font_obj = revo_lib::obs::obs_data_get_obj(settings, font_key.as_ptr());
			if !font_obj.is_null() {
				if let Ok(face_key) = std::ffi::CString::new("face") {
					if revo_lib::obs::obs_data_has_user_value(font_obj, face_key.as_ptr()) {
						let face_ptr = revo_lib::obs::obs_data_get_string(font_obj, face_key.as_ptr());
						if !face_ptr.is_null() {
							let face = std::ffi::CStr::from_ptr(face_ptr).to_string_lossy().to_string();
							params.insert("font_face".to_string(), face);
						}
					}
				}
				if let Ok(size_key) = std::ffi::CString::new("size") {
					if revo_lib::obs::obs_data_has_user_value(font_obj, size_key.as_ptr()) {
						let size = revo_lib::obs::obs_data_get_int(font_obj, size_key.as_ptr());
						params.insert("font_size".to_string(), size.to_string());
					}
				}
				revo_lib::obs::obs_data_release(font_obj);
			}
		}
	}
}

pub(crate) fn obs_data_object_primary_value(obj: *mut revo_lib::obs::obs_data_t) -> Option<String> {
	if obj.is_null() {
		return None;
	}

	unsafe {
		let mut preferred: Option<String> = None;
		let mut fallback: Option<String> = None;
		let mut item = revo_lib::obs::obs_data_first(obj);

		while !item.is_null() {
			let name_ptr = revo_lib::obs::obs_data_item_get_name(item);
			let key = if name_ptr.is_null() {
				String::new()
			} else {
				std::ffi::CStr::from_ptr(name_ptr).to_string_lossy().to_string()
			};

			let value = match revo_lib::obs::obs_data_item_gettype(item) {
				t if t == revo_lib::obs::obs_data_type_OBS_DATA_STRING => {
					let ptr = revo_lib::obs::obs_data_item_get_string(item);
					if ptr.is_null() {
						None
					} else {
						Some(std::ffi::CStr::from_ptr(ptr).to_string_lossy().to_string())
					}
				}
				t if t == revo_lib::obs::obs_data_type_OBS_DATA_NUMBER => {
					if revo_lib::obs::obs_data_item_numtype(item)
						== revo_lib::obs::obs_data_number_type_OBS_DATA_NUM_DOUBLE
					{
						Some(revo_lib::obs::obs_data_item_get_double(item).to_string())
					} else {
						Some(revo_lib::obs::obs_data_item_get_int(item).to_string())
					}
				}
				t if t == revo_lib::obs::obs_data_type_OBS_DATA_BOOLEAN => Some(if revo_lib::obs::obs_data_item_get_bool(item) {
					"true".to_string()
				} else {
					"false".to_string()
				}),
				_ => None,
			};

			if let Some(v) = value {
				if !v.trim().is_empty() {
					let normalized = key
						.chars()
						.filter(|c| c.is_ascii_alphanumeric())
						.collect::<String>()
						.to_ascii_lowercase();
					if normalized == "value"
						|| normalized == "path"
						|| normalized == "url"
						|| normalized == "file"
						|| normalized == "localfile"
					{
						preferred = Some(v);
					} else if fallback.is_none() {
						fallback = Some(v);
					}
				}
			}

			if preferred.is_some() {
				break;
			}

			if !revo_lib::obs::obs_data_item_next(&mut item as *mut _) {
				break;
			}
		}

		if !item.is_null() {
			revo_lib::obs::obs_data_item_release(&mut item as *mut _);
		}

		preferred.or(fallback)
	}
}

pub(crate) fn obs_data_array_to_multiline(arr: *mut revo_lib::obs::obs_data_array_t) -> String {
	if arr.is_null() {
		return String::new();
	}

	let mut lines: Vec<String> = Vec::new();
	unsafe {
		let count = revo_lib::obs::obs_data_array_count(arr);
		for i in 0..count {
			let obj = revo_lib::obs::obs_data_array_item(arr, i);
			if obj.is_null() {
				continue;
			}

			if let Some(v) = obs_data_object_primary_value(obj) {
				if !v.trim().is_empty() {
					lines.push(v);
				}
			} else {
				let json_ptr = revo_lib::obs::obs_data_get_json(obj);
				if !json_ptr.is_null() {
					let json = std::ffi::CStr::from_ptr(json_ptr).to_string_lossy().to_string();
					if !json.trim().is_empty() {
						lines.push(json);
					}
				}
			}

			revo_lib::obs::obs_data_release(obj);
		}
	}

	lines.join("\n")
}

pub(crate) fn parse_list_entries(raw: &str) -> Vec<String> {
	let trimmed = raw.trim();
	if trimmed.is_empty() {
		return Vec::new();
	}

	if trimmed.starts_with('[') {
		if let Ok(items) = serde_json::from_str::<Vec<String>>(trimmed) {
			return items
				.into_iter()
				.map(|s| s.trim().to_string())
				.filter(|s| !s.is_empty())
				.collect();
		}
	}

	trimmed
		.lines()
		.map(|line| line.trim().to_string())
		.filter(|line| !line.is_empty())
		.collect()
}

pub(crate) fn build_obs_string_array(entries: &[String]) -> *mut revo_lib::obs::obs_data_array_t {
	unsafe {
		let arr = revo_lib::obs::obs_data_array_create();
		if arr.is_null() {
			return std::ptr::null_mut();
		}

		for entry in entries {
			let obj = revo_lib::obs::obs_data_create();
			if obj.is_null() {
				continue;
			}
			if let (Ok(value_key), Ok(value_val)) = (
				std::ffi::CString::new("value"),
				std::ffi::CString::new(entry.as_str()),
			) {
				revo_lib::obs::obs_data_set_string(obj, value_key.as_ptr(), value_val.as_ptr());
				let _ = revo_lib::obs::obs_data_array_push_back(arr, obj);
			}
			revo_lib::obs::obs_data_release(obj);
		}

		arr
	}
}

pub(crate) fn collect_source_property_specs(
	props: *mut revo_lib::obs::obs_properties_t,
	out: &mut Vec<crate::SourcePropertySpec>,
) {
	if props.is_null() {
		return;
	}

	unsafe {
		let mut prop = revo_lib::obs::obs_properties_first(props);
		while !prop.is_null() {
			let prop_type = revo_lib::obs::obs_property_get_type(prop);

			if prop_type == revo_lib::obs::obs_property_type_OBS_PROPERTY_GROUP {
				let group = revo_lib::obs::obs_property_group_content(prop);
				if !group.is_null() {
					collect_source_property_specs(group, out);
				}
			} else if prop_type != revo_lib::obs::obs_property_type_OBS_PROPERTY_BUTTON
				&& prop_type != revo_lib::obs::obs_property_type_OBS_PROPERTY_INVALID
			{
				let key_ptr = revo_lib::obs::obs_property_name(prop);
				let mut key = if key_ptr.is_null() {
					String::new()
				} else {
					std::ffi::CStr::from_ptr(key_ptr).to_string_lossy().to_string()
				};

				let label_ptr = revo_lib::obs::obs_property_description(prop);
				let label = if !label_ptr.is_null() {
					std::ffi::CStr::from_ptr(label_ptr).to_string_lossy().to_string()
				} else {
					key.clone()
				};

				let long_desc_ptr = revo_lib::obs::obs_property_long_description(prop);
				let hint = if !long_desc_ptr.is_null() {
					std::ffi::CStr::from_ptr(long_desc_ptr).to_string_lossy().to_string()
				} else {
					String::new()
				};

				let mut options: Vec<crate::SourcePropertyOption> = Vec::new();
				let kind = if prop_type == revo_lib::obs::obs_property_type_OBS_PROPERTY_BOOL {
					"bool".to_string()
				} else if prop_type == revo_lib::obs::obs_property_type_OBS_PROPERTY_INT {
					"int".to_string()
				} else if prop_type == revo_lib::obs::obs_property_type_OBS_PROPERTY_FLOAT {
					"float".to_string()
				} else if prop_type == revo_lib::obs::obs_property_type_OBS_PROPERTY_COLOR
					|| prop_type == revo_lib::obs::obs_property_type_OBS_PROPERTY_COLOR_ALPHA
				{
					"color".to_string()
				} else if prop_type == revo_lib::obs::obs_property_type_OBS_PROPERTY_PATH {
					"path".to_string()
				} else if prop_type == revo_lib::obs::obs_property_type_OBS_PROPERTY_EDITABLE_LIST {
					"editable_list".to_string()
				} else if prop_type == revo_lib::obs::obs_property_type_OBS_PROPERTY_LIST {
					let format = revo_lib::obs::obs_property_list_format(prop);
					let count = revo_lib::obs::obs_property_list_item_count(prop);
					for i in 0..count {
						let name_ptr = revo_lib::obs::obs_property_list_item_name(prop, i);
						let label_item = if name_ptr.is_null() {
							String::new()
						} else {
							std::ffi::CStr::from_ptr(name_ptr).to_string_lossy().to_string()
						};
						let value_item = if format
							== revo_lib::obs::obs_combo_format_OBS_COMBO_FORMAT_STRING
						{
							let value_ptr = revo_lib::obs::obs_property_list_item_string(prop, i);
							if value_ptr.is_null() {
								String::new()
							} else {
								std::ffi::CStr::from_ptr(value_ptr).to_string_lossy().to_string()
							}
						} else if format
							== revo_lib::obs::obs_combo_format_OBS_COMBO_FORMAT_FLOAT
						{
							revo_lib::obs::obs_property_list_item_float(prop, i).to_string()
						} else if format
							== revo_lib::obs::obs_combo_format_OBS_COMBO_FORMAT_BOOL
						{
							if revo_lib::obs::obs_property_list_item_bool(prop, i) {
								"true".to_string()
							} else {
								"false".to_string()
							}
						} else {
							revo_lib::obs::obs_property_list_item_int(prop, i).to_string()
						};

						options.push(crate::SourcePropertyOption {
							value: value_item,
							label: if label_item.is_empty() {
								key.clone()
							} else {
								label_item
							},
						});
					}
					"list".to_string()
				} else if prop_type == revo_lib::obs::obs_property_type_OBS_PROPERTY_TEXT
					&& revo_lib::obs::obs_property_text_type(prop)
						== revo_lib::obs::obs_text_type_OBS_TEXT_INFO
				{
					"info".to_string()
				} else {
					"text".to_string()
				};

				if kind == "info" && key.is_empty() {
					key = format!("__info_{}", out.len());
				}

				if !key.is_empty() {
					out.push(crate::SourcePropertySpec {
						key,
						label,
						kind,
						hint,
						options,
					});
				}
			}

			if !revo_lib::obs::obs_property_next(&mut prop as *mut _) {
				break;
			}
		}
	}
}

pub(crate) fn refresh_source_properties_with_settings(
	props: *mut revo_lib::obs::obs_properties_t,
	settings: *mut revo_lib::obs::obs_data_t,
) {
	if props.is_null() || settings.is_null() {
		return;
	}

	unsafe {
		let mut prop = revo_lib::obs::obs_properties_first(props);
		while !prop.is_null() {
			let prop_type = revo_lib::obs::obs_property_get_type(prop);
			if prop_type == revo_lib::obs::obs_property_type_OBS_PROPERTY_GROUP {
				let group = revo_lib::obs::obs_property_group_content(prop);
				if !group.is_null() {
					refresh_source_properties_with_settings(group, settings);
				}
			} else {
				let _ = revo_lib::obs::obs_property_modified(prop, settings);
			}

			if !revo_lib::obs::obs_property_next(&mut prop as *mut _) {
				break;
			}
		}
	}
}

pub(crate) fn collect_editable_list_keys_from_properties(
	props: *mut revo_lib::obs::obs_properties_t,
	out: &mut std::collections::HashSet<String>,
) {
	if props.is_null() {
		return;
	}

	unsafe {
		let mut prop = revo_lib::obs::obs_properties_first(props);
		while !prop.is_null() {
			let prop_type = revo_lib::obs::obs_property_get_type(prop);
			if prop_type == revo_lib::obs::obs_property_type_OBS_PROPERTY_GROUP {
				let group = revo_lib::obs::obs_property_group_content(prop);
				if !group.is_null() {
					collect_editable_list_keys_from_properties(group, out);
				}
			} else if prop_type == revo_lib::obs::obs_property_type_OBS_PROPERTY_EDITABLE_LIST {
				let key_ptr = revo_lib::obs::obs_property_name(prop);
				if !key_ptr.is_null() {
					let key = std::ffi::CStr::from_ptr(key_ptr).to_string_lossy().to_string();
					if !key.is_empty() {
						out.insert(key);
					}
				}
			}

			if !revo_lib::obs::obs_property_next(&mut prop as *mut _) {
				break;
			}
		}
	}
}

pub(crate) fn source_editable_list_keys(
	source: *mut revo_lib::obs::obs_source_t,
) -> std::collections::HashSet<String> {
	let mut keys = std::collections::HashSet::new();
	if source.is_null() {
		return keys;
	}

	unsafe {
		let props = revo_lib::obs::obs_source_properties(source as *const _);
		if !props.is_null() {
			collect_editable_list_keys_from_properties(props, &mut keys);
			revo_lib::obs::obs_properties_destroy(props);
		}
	}
	keys
}

pub(crate) fn parse_f32_param(
	params: &std::collections::HashMap<String, String>,
	key: &str,
) -> Option<f32> {
	params.get(key)?.trim().parse::<f32>().ok()
}

pub(crate) fn parse_i32_param(
	params: &std::collections::HashMap<String, String>,
	key: &str,
) -> Option<i32> {
	let raw = params.get(key)?.trim();
	if let Ok(v) = raw.parse::<i32>() {
		return Some(v);
	}
	raw.parse::<f32>().ok().map(|v| v.round() as i32)
}

pub(crate) fn live_scene_resolution() -> Option<String> {
	unsafe {
		let mut ovi: revo_lib::obs::obs_video_info = std::mem::zeroed();
		if !revo_lib::obs::obs_get_video_info(&mut ovi as *mut _) {
			return None;
		}
		if ovi.base_width == 0 || ovi.base_height == 0 {
			return None;
		}
		Some(format!("{}x{}", ovi.base_width, ovi.base_height))
	}
}

pub(crate) fn apply_scene_item_transform(
	item: *mut revo_lib::obs::obs_scene_item,
	source: *mut revo_lib::obs::obs_source,
	params: &std::collections::HashMap<String, String>,
) {
	if item.is_null() || source.is_null() {
		return;
	}
	unsafe {
		let mut pos: revo_lib::obs::vec2 = std::mem::zeroed();
		revo_lib::obs::obs_sceneitem_get_pos(item, &mut pos as *mut _);
		if let Some(x) = parse_f32_param(params, "pos_x") {
			pos.__bindgen_anon_1.__bindgen_anon_1.x = x;
		}
		if let Some(y) = parse_f32_param(params, "pos_y") {
			pos.__bindgen_anon_1.__bindgen_anon_1.y = y;
		}
		revo_lib::obs::obs_sceneitem_set_pos(item, &pos as *const _);

		let base_w = revo_lib::obs::obs_source_get_width(source) as f32;
		let base_h = revo_lib::obs::obs_source_get_height(source) as f32;
		let mut scale: revo_lib::obs::vec2 = std::mem::zeroed();
		revo_lib::obs::obs_sceneitem_get_scale(item, &mut scale as *mut _);

		if let (Some(w), Some(h)) = (
			parse_f32_param(params, "item_width"),
			parse_f32_param(params, "item_height"),
		) {
			if base_w > 0.0 && base_h > 0.0 {
				scale.__bindgen_anon_1.__bindgen_anon_1.x = w / base_w;
				scale.__bindgen_anon_1.__bindgen_anon_1.y = h / base_h;
				revo_lib::obs::obs_sceneitem_set_scale(item, &scale as *const _);
			} else {
				if let Some(sx) = parse_f32_param(params, "scale_x") {
					scale.__bindgen_anon_1.__bindgen_anon_1.x = sx;
				}
				if let Some(sy) = parse_f32_param(params, "scale_y") {
					scale.__bindgen_anon_1.__bindgen_anon_1.y = sy;
				}
				revo_lib::obs::obs_sceneitem_set_scale(item, &scale as *const _);
			}
		} else {
			if let Some(sx) = parse_f32_param(params, "scale_x") {
				scale.__bindgen_anon_1.__bindgen_anon_1.x = sx;
			}
			if let Some(sy) = parse_f32_param(params, "scale_y") {
				scale.__bindgen_anon_1.__bindgen_anon_1.y = sy;
			}
			revo_lib::obs::obs_sceneitem_set_scale(item, &scale as *const _);
		}

		if let Some(rot) = parse_f32_param(params, "rot").or_else(|| parse_f32_param(params, "rotation")) {
			revo_lib::obs::obs_sceneitem_set_rot(item, rot);
		}

		let mut crop: revo_lib::obs::obs_sceneitem_crop = std::mem::zeroed();
		revo_lib::obs::obs_sceneitem_get_crop(item, &mut crop as *mut _);
		if let Some(left) = parse_i32_param(params, "crop_left") {
			crop.left = left.max(0);
		}
		if let Some(top) = parse_i32_param(params, "crop_top") {
			crop.top = top.max(0);
		}
		if let Some(right) = parse_i32_param(params, "crop_right") {
			crop.right = right.max(0);
		}
		if let Some(bottom) = parse_i32_param(params, "crop_bottom") {
			crop.bottom = bottom.max(0);
		}
		revo_lib::obs::obs_sceneitem_set_crop(item, &crop as *const _);
	}
}

pub(crate) fn parse_color_abgr(value: &str) -> Option<u32> {
	let hex = value.trim().trim_start_matches('#');
	if hex.len() != 6 {
		return None;
	}
	let r = u8::from_str_radix(&hex[0..2], 16).ok()? as u32;
	let g = u8::from_str_radix(&hex[2..4], 16).ok()? as u32;
	let b = u8::from_str_radix(&hex[4..6], 16).ok()? as u32;
	Some((0xFF << 24) | (b << 16) | (g << 8) | r)
}

pub(crate) fn abgr_to_hex(abgr: u32) -> String {
	let r = (abgr & 0xFF) as u8;
	let g = ((abgr >> 8) & 0xFF) as u8;
	let b = ((abgr >> 16) & 0xFF) as u8;
	format!("#{:02x}{:02x}{:02x}", r, g, b)
}

pub(crate) fn apply_source_params(
	settings: *mut revo_lib::obs::obs_data,
	source_type: &str,
	params: &std::collections::HashMap<String, String>,
	editable_list_keys: Option<&std::collections::HashSet<String>>,
) {
	if settings.is_null() {
		return;
	}
	unsafe {
		match source_type {
			"color_source" | "color_source_v2" => {
				if let Some(color) = params.get("color").and_then(|v| parse_color_abgr(v)) {
					let key = std::ffi::CString::new("color").unwrap();
					revo_lib::obs::obs_data_set_int(settings, key.as_ptr(), color as i64);
				}
				if let Some(width) = params.get("width").and_then(|v| v.parse::<i64>().ok()) {
					let key = std::ffi::CString::new("width").unwrap();
					revo_lib::obs::obs_data_set_int(settings, key.as_ptr(), width);
				}
				if let Some(height) = params.get("height").and_then(|v| v.parse::<i64>().ok()) {
					let key = std::ffi::CString::new("height").unwrap();
					revo_lib::obs::obs_data_set_int(settings, key.as_ptr(), height);
				}
			}
			"text_ft2_source" | "text_ft2_source_v2" => {
				if let Some(text) = params.get("text") {
					let key = std::ffi::CString::new("text").unwrap();
					let val = std::ffi::CString::new(text.as_str()).unwrap();
					revo_lib::obs::obs_data_set_string(settings, key.as_ptr(), val.as_ptr());
				}
				if let Some(color) = params
					.get("color1")
					.or_else(|| params.get("color"))
					.and_then(|v| parse_color_abgr(v))
				{
					let key = std::ffi::CString::new("color1").unwrap();
					revo_lib::obs::obs_data_set_int(settings, key.as_ptr(), color as i64);
				}
				if params.contains_key("font_size")
					|| params.contains_key("font_face")
					|| params.contains_key("size")
					|| params.contains_key("face")
				{
					let font_key = std::ffi::CString::new("font").unwrap();

					let mut current_face = "DejaVu Sans".to_string();
					let mut current_size: i64 = 24;
					let current_font = revo_lib::obs::obs_data_get_obj(settings, font_key.as_ptr());
					if !current_font.is_null() {
						if let Ok(face_key) = std::ffi::CString::new("face") {
							let face_ptr = revo_lib::obs::obs_data_get_string(current_font, face_key.as_ptr());
							if !face_ptr.is_null() {
								current_face = std::ffi::CStr::from_ptr(face_ptr).to_string_lossy().to_string();
							}
						}
						if let Ok(size_key) = std::ffi::CString::new("size") {
							let size = revo_lib::obs::obs_data_get_int(current_font, size_key.as_ptr());
							if size > 0 {
								current_size = size;
							}
						}
						revo_lib::obs::obs_data_release(current_font);
					}

					let font_settings = revo_lib::obs::obs_data_create();
					if !font_settings.is_null() {
						let face_key = std::ffi::CString::new("face").unwrap();
						let face_val = std::ffi::CString::new(
							params
								.get("font_face")
								.or_else(|| params.get("face"))
								.map(|v| v.as_str())
								.unwrap_or(current_face.as_str()),
						)
						.unwrap();
						revo_lib::obs::obs_data_set_string(
							font_settings,
							face_key.as_ptr(),
							face_val.as_ptr(),
						);
						if let Some(size) = params
							.get("font_size")
							.or_else(|| params.get("size"))
							.or_else(|| params.get("fontsize"))
							.and_then(|v| v.parse::<i64>().ok())
						{
							let size_key = std::ffi::CString::new("size").unwrap();
							revo_lib::obs::obs_data_set_int(font_settings, size_key.as_ptr(), size);
						} else {
							let size_key = std::ffi::CString::new("size").unwrap();
							revo_lib::obs::obs_data_set_int(font_settings, size_key.as_ptr(), current_size);
						}
						revo_lib::obs::obs_data_set_obj(settings, font_key.as_ptr(), font_settings);
						revo_lib::obs::obs_data_release(font_settings);
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

					if let Ok(key_c) = std::ffi::CString::new(key.as_str()) {
						if editable_list_keys.is_some_and(|keys| keys.contains(key)) {
							let items = parse_list_entries(value);
							let arr = build_obs_string_array(&items);
							if !arr.is_null() {
								revo_lib::obs::obs_data_set_array(settings, key_c.as_ptr(), arr);
								revo_lib::obs::obs_data_array_release(arr);
								continue;
							}
						}

						let normalized = value.trim().to_ascii_lowercase();
						let is_color_like = normalized_key.contains("color");

						if is_color_like {
							if let Some(color) = parse_color_abgr(value) {
								revo_lib::obs::obs_data_set_int(settings, key_c.as_ptr(), color as i64);
								continue;
							}
						}

						if normalized == "true" || normalized == "false" {
							revo_lib::obs::obs_data_set_bool(
								settings,
								key_c.as_ptr(),
								normalized == "true",
							);
						} else if let Ok(int_val) = value.parse::<i64>() {
							revo_lib::obs::obs_data_set_int(settings, key_c.as_ptr(), int_val);
						} else if let Ok(float_val) = value.parse::<f64>() {
							revo_lib::obs::obs_data_set_double(settings, key_c.as_ptr(), float_val);
						} else if let Ok(val_c) = std::ffi::CString::new(value.as_str()) {
							revo_lib::obs::obs_data_set_string(
								settings,
								key_c.as_ptr(),
								val_c.as_ptr(),
							);
						}
					}
				}
			}
			"image_source" => {
				if let Some(file) = params.get("file") {
					let key = std::ffi::CString::new("file").unwrap();
					let val = std::ffi::CString::new(file.as_str()).unwrap();
					revo_lib::obs::obs_data_set_string(settings, key.as_ptr(), val.as_ptr());
				}
			}
			"ffmpeg_source" => {
				if let Some(file) = params.get("local_file").or_else(|| params.get("file")) {
					let key = std::ffi::CString::new("local_file").unwrap();
					let val = std::ffi::CString::new(file.as_str()).unwrap();
					revo_lib::obs::obs_data_set_string(settings, key.as_ptr(), val.as_ptr());
				}

				if let Some(input) = params.get("input").or_else(|| params.get("url")) {
					if let Ok(key) = std::ffi::CString::new("input") {
						if let Ok(val) = std::ffi::CString::new(input.as_str()) {
							revo_lib::obs::obs_data_set_string(settings, key.as_ptr(), val.as_ptr());
						}
					}
				}

				if let Some(is_local_file) = params.get("is_local_file") {
					if let Ok(key) = std::ffi::CString::new("is_local_file") {
						let normalized = is_local_file.trim().to_ascii_lowercase();
						let as_bool = normalized == "true"
							|| normalized == "1"
							|| normalized == "yes"
							|| normalized == "on";
						revo_lib::obs::obs_data_set_bool(settings, key.as_ptr(), as_bool);
					}
				}

				for (key, value) in params {
					if key == "file" || key == "url" {
						continue;
					}

					if let Ok(key_c) = std::ffi::CString::new(key.as_str()) {
						if editable_list_keys.is_some_and(|keys| keys.contains(key)) {
							let items = parse_list_entries(value);
							let arr = build_obs_string_array(&items);
							if !arr.is_null() {
								revo_lib::obs::obs_data_set_array(settings, key_c.as_ptr(), arr);
								revo_lib::obs::obs_data_array_release(arr);
								continue;
							}
						}

						let normalized = value.trim().to_ascii_lowercase();
						if normalized == "true" || normalized == "false" {
							revo_lib::obs::obs_data_set_bool(
								settings,
								key_c.as_ptr(),
								normalized == "true",
							);
						} else if let Ok(int_val) = value.parse::<i64>() {
							revo_lib::obs::obs_data_set_int(settings, key_c.as_ptr(), int_val);
						} else if let Ok(float_val) = value.parse::<f64>() {
							revo_lib::obs::obs_data_set_double(settings, key_c.as_ptr(), float_val);
						} else if let Ok(val_c) = std::ffi::CString::new(value.as_str()) {
							revo_lib::obs::obs_data_set_string(
								settings,
								key_c.as_ptr(),
								val_c.as_ptr(),
							);
						}
					}
				}
			}
			"browser_source" => {
				if let Some(url) = params.get("url") {
					let key = std::ffi::CString::new("url").unwrap();
					let val = std::ffi::CString::new(url.as_str()).unwrap();
					revo_lib::obs::obs_data_set_string(settings, key.as_ptr(), val.as_ptr());
				}
				if let Some(width) = params.get("width").and_then(|v| v.parse::<i64>().ok()) {
					let key = std::ffi::CString::new("width").unwrap();
					revo_lib::obs::obs_data_set_int(settings, key.as_ptr(), width);
				}
				if let Some(height) = params.get("height").and_then(|v| v.parse::<i64>().ok()) {
					let key = std::ffi::CString::new("height").unwrap();
					revo_lib::obs::obs_data_set_int(settings, key.as_ptr(), height);
				}
			}
			"window_capture" | "xcomposite_input" => {
				if let Some(window) = params.get("window").or_else(|| params.get("capture_window")) {
					let val = std::ffi::CString::new(window.as_str()).unwrap();

					let key_window = std::ffi::CString::new("window").unwrap();
					revo_lib::obs::obs_data_set_string(settings, key_window.as_ptr(), val.as_ptr());

					let key_capture_window = std::ffi::CString::new("capture_window").unwrap();
					revo_lib::obs::obs_data_set_string(settings, key_capture_window.as_ptr(), val.as_ptr());
				}
			}
			"pulse_input_capture" | "pulse_output_capture" => {
				if let Some(device) = params.get("device") {
					let key = std::ffi::CString::new("device_id").unwrap();
					let val = std::ffi::CString::new(device.as_str()).unwrap();
					revo_lib::obs::obs_data_set_string(settings, key.as_ptr(), val.as_ptr());
				}
			}
			_ => {
				for (key, value) in params {
					if let Ok(key_c) = std::ffi::CString::new(key.as_str()) {
						if editable_list_keys.is_some_and(|keys| keys.contains(key)) {
							let items = parse_list_entries(value);
							let arr = build_obs_string_array(&items);
							if !arr.is_null() {
								revo_lib::obs::obs_data_set_array(settings, key_c.as_ptr(), arr);
								revo_lib::obs::obs_data_array_release(arr);
								continue;
							}
						}

						let normalized = value.trim().to_ascii_lowercase();
						let is_color_like = key
							.chars()
							.filter(|c| c.is_ascii_alphanumeric())
							.collect::<String>()
							.to_ascii_lowercase()
							.contains("color");

						if is_color_like {
							if let Some(color) = parse_color_abgr(value) {
								revo_lib::obs::obs_data_set_int(settings, key_c.as_ptr(), color as i64);
								continue;
							}
						}

						if normalized == "true" || normalized == "false" {
							revo_lib::obs::obs_data_set_bool(
								settings,
								key_c.as_ptr(),
								normalized == "true",
							);
						} else if let Ok(int_val) = value.parse::<i64>() {
							revo_lib::obs::obs_data_set_int(settings, key_c.as_ptr(), int_val);
						} else if let Ok(float_val) = value.parse::<f64>() {
							revo_lib::obs::obs_data_set_double(settings, key_c.as_ptr(), float_val);
						} else if let Ok(val_c) = std::ffi::CString::new(value.as_str()) {
							revo_lib::obs::obs_data_set_string(
								settings,
								key_c.as_ptr(),
								val_c.as_ptr(),
							);
						}
					}
				}
			}
		}
	}
}
