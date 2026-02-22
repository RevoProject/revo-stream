pub(crate) fn set_scene_resolution(
	state: tauri::State<crate::ObsState>,
	resolution: String,
) -> Result<String, String> {
	let mut runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	if !runtime.initialized {
		return Err("OBS not initialized".to_string());
	}
	if !runtime.output_record.is_null() || !runtime.output_stream.is_null() {
		return Err("Cannot change resolution while recording/streaming".to_string());
	}

	let parts: Vec<&str> = resolution.split('x').collect();
	if parts.len() != 2 {
		return Err("Invalid resolution format".to_string());
	}
	let width: u32 = parts[0]
		.trim()
		.parse()
		.map_err(|_| "Invalid width".to_string())?;
	let height: u32 = parts[1]
		.trim()
		.parse()
		.map_err(|_| "Invalid height".to_string())?;

	let ok = crate::runtime::helpers::reset_video_with_resolution(width, height);
	if !ok {
		return Err("obs_reset_video failed".to_string());
	}
	runtime.scene_resolution = format!("{width}x{height}");
	Ok(format!("Resolution set to {width}x{height}"))
}
