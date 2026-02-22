// Scene-item helper bridge module.

pub(crate) unsafe extern "C" fn remove_scene_item_cb(
	_scene: *mut revo_lib::obs::obs_scene,
	item: *mut revo_lib::obs::obs_scene_item,
	_param: *mut std::os::raw::c_void,
) -> bool {
	if !item.is_null() {
		revo_lib::obs::obs_sceneitem_remove(item);
	}
	true
}

pub(crate) unsafe extern "C" fn collect_scene_items_cb(
	_scene: *mut revo_lib::obs::obs_scene,
	item: *mut revo_lib::obs::obs_scene_item,
	param: *mut std::os::raw::c_void,
) -> bool {
	if !param.is_null() {
		let list = &mut *(param as *mut Vec<*mut revo_lib::obs::obs_scene_item>);
		list.push(item);
	}
	true
}

pub(crate) fn resolve_scene_item(
	scene: &crate::SceneState,
	id: &str,
) -> Option<*mut revo_lib::obs::obs_scene_item> {
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

pub(crate) fn scene_item_id(
	scene: &crate::SceneState,
	item: *mut revo_lib::obs::obs_scene_item,
) -> Option<String> {
	if item.is_null() {
		return None;
	}
	if item == scene.item_accent {
		return Some("accent".to_string());
	}
	if item == scene.item_text {
		return Some("title".to_string());
	}
	if let Some((custom_id, _)) = scene.custom_items.iter().find(|(_, v)| **v == item) {
		return Some(custom_id.clone());
	}
	unsafe {
		let source = revo_lib::obs::obs_sceneitem_get_source(item);
		if source.is_null() {
			return None;
		}
		let name_ptr = revo_lib::obs::obs_source_get_name(source);
		if name_ptr.is_null() {
			return None;
		}
		let name = std::ffi::CStr::from_ptr(name_ptr).to_string_lossy().to_string();
		if name.is_empty() {
			None
		} else {
			Some(name)
		}
	}
}

pub(crate) fn find_scene_item_by_name(
	scene: &crate::SceneState,
	name: &str,
) -> Option<*mut revo_lib::obs::obs_scene_item> {
	if scene.scene.is_null() {
		return None;
	}
	let mut items: Vec<*mut revo_lib::obs::obs_scene_item> = Vec::new();
	unsafe {
		revo_lib::obs::obs_scene_enum_items(
			scene.scene,
			Some(collect_scene_items_cb),
			&mut items as *mut _ as *mut std::os::raw::c_void,
		);
	}
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
			if name_ptr.is_null() {
				continue;
			}
			let item_name = std::ffi::CStr::from_ptr(name_ptr).to_string_lossy();
			if item_name == name {
				return Some(item);
			}
		}
	}
	None
}
