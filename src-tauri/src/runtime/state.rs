pub(crate) type ObsRuntime = crate::ObsRuntime;
pub(crate) type ObsState = crate::ObsState;
pub(crate) type SceneState = crate::SceneState;

pub(crate) fn is_initialized(state: tauri::State<crate::ObsState>) -> Result<bool, String> {
	let runtime = state
		.runtime
		.lock()
		.map_err(|_| "state poisoned".to_string())?;
	Ok(runtime.initialized)
}

pub(crate) fn version() -> Result<String, String> {
	unsafe {
		let ptr = revo_lib::obs::obs_get_version_string();
		if ptr.is_null() {
			return Err("version string is null".to_string());
		}
		let version = std::ffi::CStr::from_ptr(ptr).to_string_lossy().to_string();
		let tag = revo_lib::runtime::libobs_git_describe();
		if tag.is_empty() || tag == "unknown" || version.contains(tag) {
			Ok(version)
		} else {
			Ok(format!("{version} ({tag})"))
		}
	}
}
