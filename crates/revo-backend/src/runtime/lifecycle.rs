pub trait ObsPlatform {
	fn startup(&self, locale: &std::ffi::CStr, config: &std::ffi::CStr) -> Result<(), String>;
	fn shutdown(&self) -> Result<(), String>;
}

pub fn version() -> Result<String, String> {
	// SAFETY: libobs owns the returned static version string. It is checked for
	// null before being read and copied into an owned Rust string.
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
