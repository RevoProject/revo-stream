pub(crate) fn data_dir_from_root(root: &std::path::PathBuf) -> std::path::PathBuf {
	if root.join("data/share/obs/libobs/default.effect").exists() {
		root.join("data")
	} else {
		root.clone()
	}
}

pub(crate) fn settings_path(root: &std::path::PathBuf) -> std::path::PathBuf {
	data_dir_from_root(root).join("conf").join("profile.json")
}

pub(crate) fn legacy_settings_path(root: &std::path::PathBuf) -> std::path::PathBuf {
	data_dir_from_root(root).join("conf").join("settings.json")
}

pub(crate) fn user_profile_dir() -> Option<std::path::PathBuf> {
	if let Ok(val) = std::env::var("USERPROFILE") {
		return Some(std::path::PathBuf::from(val));
	}
	if let Ok(val) = std::env::var("HOME") {
		return Some(std::path::PathBuf::from(val));
	}
	let drive = std::env::var("HOMEDRIVE").ok();
	let path = std::env::var("HOMEPATH").ok();
	match (drive, path) {
		(Some(d), Some(p)) => Some(std::path::PathBuf::from(format!("{d}{p}"))),
		_ => None,
	}
}

pub(crate) fn default_record_path() -> String {
	if let Some(profile) = user_profile_dir() {
		return profile
			.join("Videos")
			.join("RevoStream")
			.join("Output")
			.join("record.mp4")
			.to_string_lossy()
			.to_string();
	}
	std::path::PathBuf::from("Videos")
		.join("RevoStream")
		.join("Output")
		.join("record.mp4")
		.to_string_lossy()
		.to_string()
}

pub(crate) fn default_preview_quality() -> Option<String> {
	Some("medium".to_string())
}

pub(crate) fn default_encoder_preference() -> Option<String> {
	Some("hardware".to_string())
}

pub(crate) fn default_scene_resolution() -> Option<String> {
	Some("1920x1080".to_string())
}

pub(crate) fn default_whep_url() -> Option<String> {
	Some("http://127.0.0.1:8080/whep".to_string())
}

pub(crate) fn default_whip_url() -> Option<String> {
	Some("http://127.0.0.1:8080/whip".to_string())
}

pub(crate) fn default_active_profile() -> Option<String> {
	Some("default".to_string())
}

pub(crate) fn sanitize_profile_name(name: &str) -> String {
	let mut out = String::new();
	for ch in name.trim().chars() {
		if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
			out.push(ch);
		} else if ch.is_whitespace() {
			out.push('_');
		}
	}
	while out.contains("__") {
		out = out.replace("__", "_");
	}
	out.trim_matches('_').to_string()
}

pub(crate) fn normalize_optional_trimmed(value: &mut Option<String>) {
	if let Some(raw) = value {
		let trimmed = raw.trim();
		if trimmed.is_empty() {
			*value = None;
		} else if trimmed != raw.as_str() {
			*raw = trimmed.to_string();
		}
	}
}

pub(crate) fn is_valid_resolution(value: &str) -> bool {
	let mut parts = value.split('x');
	let w = parts.next().and_then(|v| v.trim().parse::<u32>().ok());
	let h = parts.next().and_then(|v| v.trim().parse::<u32>().ok());
	if parts.next().is_some() {
		return false;
	}
	matches!((w, h), (Some(wv), Some(hv)) if wv > 0 && hv > 0)
}

pub(crate) fn normalize_settings(settings: &mut crate::AppSettings) {
	normalize_optional_trimmed(&mut settings.root_dir);

	let record_path = settings.record_path.trim();
	settings.record_path = if record_path.is_empty() {
		default_record_path()
	} else {
		record_path.to_string()
	};

	settings.stream_url = settings.stream_url.trim().to_string();
	settings.stream_key = settings.stream_key.trim().to_string();

	let preview_quality = settings
		.preview_quality
		.as_deref()
		.map(|v| v.trim().to_lowercase())
		.unwrap_or_else(|| "medium".to_string());
	settings.preview_quality =
		if matches!(preview_quality.as_str(), "very_low" | "low" | "medium" | "high") {
			Some(preview_quality)
		} else {
			default_preview_quality()
		};

	let encoder_pref = settings
		.encoder_preference
		.as_deref()
		.map(|v| v.trim().to_lowercase())
		.unwrap_or_else(|| "hardware".to_string());
	settings.encoder_preference = if matches!(encoder_pref.as_str(), "hardware" | "software") {
		Some(encoder_pref)
	} else {
		default_encoder_preference()
	};

	let scene_resolution = settings
		.scene_resolution
		.as_deref()
		.map(|v| v.trim().to_string());
	settings.scene_resolution = match scene_resolution {
		Some(v) if is_valid_resolution(&v) => Some(v),
		_ => default_scene_resolution(),
	};

	let whep = settings.whep_url.as_deref().map(|v| v.trim().to_string());
	settings.whep_url = match whep {
		Some(v) if !v.is_empty() => Some(v),
		_ => default_whep_url(),
	};

	let whip = settings.whip_url.as_deref().map(|v| v.trim().to_string());
	settings.whip_url = match whip {
		Some(v) if !v.is_empty() => Some(v),
		_ => default_whip_url(),
	};

	settings.auto_retry_preview = Some(settings.auto_retry_preview.unwrap_or(true));
	settings.autorescale_inputs = Some(settings.autorescale_inputs.unwrap_or(false));

	if settings.ui_profile.as_ref().is_some_and(|v| !v.is_object()) {
		settings.ui_profile = None;
	}

	let active = settings
		.active_profile
		.clone()
		.unwrap_or_else(|| "default".to_string());
	let clean = sanitize_profile_name(&active);
	settings.active_profile = if clean.is_empty() {
		default_active_profile()
	} else {
		Some(clean)
	};
}

pub(crate) fn parse_settings_raw(raw: &str) -> crate::AppSettings {
	let parsed_value = match serde_json::from_str::<serde_json::Value>(raw) {
		Ok(v) => v,
		Err(_) => {
			let mut settings = default_settings();
			normalize_settings(&mut settings);
			return settings;
		}
	};

	if let Ok(mut typed) = serde_json::from_value::<crate::AppSettings>(parsed_value.clone()) {
		normalize_settings(&mut typed);
		return typed;
	}

	let mut settings = default_settings();
	if let Some(obj) = parsed_value.as_object() {
		if let Some(v) = obj.get("root_dir") {
			settings.root_dir = v.as_str().map(|s| s.to_string());
		}
		if let Some(v) = obj.get("record_path").and_then(|v| v.as_str()) {
			settings.record_path = v.to_string();
		}
		if let Some(v) = obj.get("stream_url").and_then(|v| v.as_str()) {
			settings.stream_url = v.to_string();
		}
		if let Some(v) = obj.get("stream_key").and_then(|v| v.as_str()) {
			settings.stream_key = v.to_string();
		}
		if let Some(v) = obj.get("preview_quality").and_then(|v| v.as_str()) {
			settings.preview_quality = Some(v.to_string());
		}
		if let Some(v) = obj.get("encoder_preference").and_then(|v| v.as_str()) {
			settings.encoder_preference = Some(v.to_string());
		}
		if let Some(v) = obj.get("scene_resolution").and_then(|v| v.as_str()) {
			settings.scene_resolution = Some(v.to_string());
		}
		if let Some(v) = obj.get("whep_url").and_then(|v| v.as_str()) {
			settings.whep_url = Some(v.to_string());
		}
		if let Some(v) = obj.get("whip_url").and_then(|v| v.as_str()) {
			settings.whip_url = Some(v.to_string());
		}
		if let Some(v) = obj.get("auto_retry_preview").and_then(|v| v.as_bool()) {
			settings.auto_retry_preview = Some(v);
		}
		if let Some(v) = obj.get("autorescale_inputs").and_then(|v| v.as_bool()) {
			settings.autorescale_inputs = Some(v);
		}
		if let Some(v) = obj.get("ui_profile") {
			settings.ui_profile = if v.is_object() { Some(v.clone()) } else { None };
		}
		if let Some(v) = obj.get("active_profile").and_then(|v| v.as_str()) {
			settings.active_profile = Some(v.to_string());
		}
	}

	normalize_settings(&mut settings);
	settings
}

pub(crate) fn default_settings() -> crate::AppSettings {
	crate::AppSettings {
		root_dir: None,
		record_path: default_record_path(),
		stream_url: String::new(),
		stream_key: String::new(),
		preview_quality: default_preview_quality(),
		encoder_preference: default_encoder_preference(),
		scene_resolution: default_scene_resolution(),
		whep_url: default_whep_url(),
		whip_url: default_whip_url(),
		auto_retry_preview: Some(true),
		autorescale_inputs: Some(false),
		ui_profile: None,
		active_profile: default_active_profile(),
	}
}

pub(crate) fn find_file_upwards(
	start: &std::path::PathBuf,
	relative_path: &str,
) -> Option<std::path::PathBuf> {
	for ancestor in start.ancestors() {
		let candidate = ancestor.join(relative_path);
		if candidate.is_file() {
			return Some(candidate);
		}
	}
	None
}

pub(crate) fn resolve_rtmp_services_json_path() -> Option<std::path::PathBuf> {
	let relative_candidates = [
		"data/conf/rtmp-services/services.json",
		"revo-ui/src-tauri/resources/revo-root/data/conf/rtmp-services/services.json",
		"src-tauri/resources/revo-root/data/conf/rtmp-services/services.json",
	];

	let mut starts: Vec<std::path::PathBuf> = Vec::new();

	if let Ok(cwd) = std::env::current_dir() {
		starts.push(cwd);
	}

	let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	starts.push(manifest.clone());
	starts.push(manifest.join(".."));
	starts.push(manifest.join("..").join(".."));

	if let Ok(exe) = std::env::current_exe() {
		if let Some(dir) = exe.parent() {
			starts.push(dir.to_path_buf());
		}
	}

	for start in starts {
		for rel in relative_candidates {
			if let Some(found) = find_file_upwards(&start, rel) {
				return Some(found);
			}
		}
	}

	None
}

pub(crate) fn load_rtmp_services_json() -> Result<String, String> {
	let path = resolve_rtmp_services_json_path().ok_or_else(|| {
		"Could not locate data/conf/rtmp-services/services.json from current runtime directory"
			.to_string()
	})?;

	std::fs::read_to_string(&path)
		.map_err(|e| format!("failed to read services.json from {}: {e}", path.to_string_lossy()))
}

pub(crate) fn settings_get() -> Result<crate::AppSettings, String> {
	let root = resolve_root_dir(None)?;
	let path = settings_path(&root);
	let legacy_path = legacy_settings_path(&root);

	if path.exists() {
		let raw = std::fs::read_to_string(&path).map_err(|e| format!("failed to read profile: {e}"))?;
		let parsed = parse_settings_raw(&raw);
		if let Some(parent) = path.parent() {
			let _ = std::fs::create_dir_all(parent);
		}
		let normalized = serde_json::to_string_pretty(&parsed)
			.map_err(|e| format!("failed to serialize normalized profile: {e}"))?;
		std::fs::write(&path, normalized)
			.map_err(|e| format!("failed to write normalized profile: {e}"))?;
		let _ = sync_active_profile_file(&parsed);
		return Ok(parsed);
	}

	if legacy_path.exists() {
		let raw = std::fs::read_to_string(&legacy_path)
			.map_err(|e| format!("failed to read settings: {e}"))?;
		let parsed = parse_settings_raw(&raw);
		if let Some(parent) = path.parent() {
			let _ = std::fs::create_dir_all(parent);
		}
		let migrated = serde_json::to_string_pretty(&parsed)
			.map_err(|e| format!("failed to serialize migrated profile: {e}"))?;
		std::fs::write(&path, migrated)
			.map_err(|e| format!("failed to write migrated profile: {e}"))?;
		let _ = sync_active_profile_file(&parsed);
		return Ok(parsed);
	}

	let settings = default_settings();
	if let Some(parent) = path.parent() {
		let _ = std::fs::create_dir_all(parent);
	}
	let raw = serde_json::to_string_pretty(&settings)
		.map_err(|e| format!("failed to serialize profile: {e}"))?;
	std::fs::write(&path, raw).map_err(|e| format!("failed to write profile: {e}"))?;
	let _ = sync_active_profile_file(&settings);
	Ok(settings)
}

pub(crate) fn settings_save(settings: crate::AppSettings) -> Result<String, String> {
	let mut normalized_settings = settings;
	normalize_settings(&mut normalized_settings);

	let root = resolve_root_dir(normalized_settings.root_dir.clone())?;
	let path = settings_path(&root);
	if let Some(parent) = path.parent() {
		std::fs::create_dir_all(parent)
			.map_err(|e| format!("failed to create profile dir: {e}"))?;
	}
	let raw = serde_json::to_string_pretty(&normalized_settings)
		.map_err(|e| format!("failed to serialize profile: {e}"))?;
	std::fs::write(&path, raw).map_err(|e| format!("failed to write profile: {e}"))?;
	let _ = sync_active_profile_file(&normalized_settings);
	Ok(format!("Profile saved to {}", path.to_string_lossy()))
}

pub(crate) fn runtime_data_dir() -> Result<std::path::PathBuf, String> {
	let cwd = crate::utils::fs::startup_cwd()?;
	if cwd.file_name().and_then(|s| s.to_str()) == Some("src-tauri") {
		if let Some(parent) = cwd.parent() {
			return Ok(parent.join("data"));
		}
	}
	Ok(cwd.join("data"))
}

pub(crate) fn runtime_plugins_dir() -> Result<std::path::PathBuf, String> {
	let cwd = crate::utils::fs::startup_cwd()?;
	if cfg!(debug_assertions) {
		if cwd.file_name().and_then(|s| s.to_str()) == Some("src-tauri") {
			if let Some(parent) = cwd.parent() {
				return Ok(parent.join("data").join("plugins"));
			}
		}
	}

	Ok(runtime_data_dir()?.join("plugins"))
}

pub(crate) fn legacy_runtime_plugins_dir() -> Result<std::path::PathBuf, String> {
	let cwd = crate::utils::fs::startup_cwd()?;
	if cfg!(debug_assertions) {
		if cwd.file_name().and_then(|s| s.to_str()) == Some("src-tauri") {
			return Ok(cwd.join("data").join("plugins"));
		}

		let src_tauri_data = cwd.join("src-tauri").join("data");
		if src_tauri_data.is_dir() {
			return Ok(src_tauri_data.join("plugins"));
		}
	}
	Ok(cwd.join("data").join("plugins"))
}

pub(crate) fn runtime_plugin_profiles_dir() -> Result<std::path::PathBuf, String> {
	Ok(runtime_plugins_dir()?.join("profiles"))
}

pub(crate) fn runtime_plugin_profile_dir(name: &str) -> Result<std::path::PathBuf, String> {
	let safe = sanitize_profile_name(name);
	if safe.is_empty() {
		return Err("Invalid plugin profile name".to_string());
	}
	Ok(runtime_plugin_profiles_dir()?.join(safe))
}

pub(crate) fn runtime_plugin_profile_file(name: &str) -> Result<std::path::PathBuf, String> {
	Ok(runtime_plugin_profile_dir(name)?.join("profile.json"))
}

pub(crate) fn runtime_plugin_active_profile_file() -> Result<std::path::PathBuf, String> {
	Ok(runtime_plugin_profiles_dir()?.join("active_profile.txt"))
}

pub(crate) fn title_case_words(input: &str) -> String {
	input
		.split_whitespace()
		.map(|w| {
			let mut chars = w.chars();
			match chars.next() {
				Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
				None => String::new(),
			}
		})
		.collect::<Vec<String>>()
		.join(" ")
}

pub(crate) fn runtime_profiles_dir() -> Result<std::path::PathBuf, String> {
	Ok(runtime_data_dir()?.join("profiles"))
}

pub(crate) fn runtime_profile_dir(name: &str) -> Result<std::path::PathBuf, String> {
	let safe = sanitize_profile_name(name);
	if safe.is_empty() {
		return Err("Invalid profile name".to_string());
	}
	Ok(runtime_profiles_dir()?.join(safe))
}

pub(crate) fn runtime_profile_file(name: &str) -> Result<std::path::PathBuf, String> {
	Ok(runtime_profile_dir(name)?.join("profile.json"))
}

pub(crate) fn legacy_runtime_profile_file(name: &str) -> Result<std::path::PathBuf, String> {
	let safe = sanitize_profile_name(name);
	if safe.is_empty() {
		return Err("Invalid profile name".to_string());
	}
	Ok(runtime_profiles_dir()?.join(format!("{safe}.json")))
}

pub(crate) fn sync_active_profile_file(settings: &crate::AppSettings) -> Result<(), String> {
	let profile_name = settings
		.active_profile
		.clone()
		.unwrap_or_else(|| "default".to_string());
	let file = runtime_profile_file(&profile_name)?;
	if let Some(parent) = file.parent() {
		std::fs::create_dir_all(parent)
			.map_err(|e| format!("failed to create profiles dir: {e}"))?;
	}
	let raw = serde_json::to_string_pretty(settings)
		.map_err(|e| format!("failed to serialize active profile: {e}"))?;
	std::fs::write(&file, raw).map_err(|e| format!("failed to write active profile: {e}"))?;
	Ok(())
}

pub(crate) fn ensure_runtime_data_dirs() -> Result<(), String> {
	let data_dir = runtime_data_dir()?;
	std::fs::create_dir_all(&data_dir).map_err(|e| format!("failed to create data dir: {e}"))?;

	for name in ["plugins", "scenes", "profiles", "themes", "templates"] {
		let dir = data_dir.join(name);
		std::fs::create_dir_all(&dir)
			.map_err(|e| format!("failed to create data subdir '{}': {e}", name))?;
	}

	let primary_plugins_dir = runtime_plugins_dir()?;
	std::fs::create_dir_all(primary_plugins_dir.join("data"))
		.map_err(|e| format!("failed to create plugins data subdir: {e}"))?;

	let legacy_plugins_dir = legacy_runtime_plugins_dir()?;
	if legacy_plugins_dir != primary_plugins_dir {
		std::fs::create_dir_all(legacy_plugins_dir.join("data"))
			.map_err(|e| format!("failed to create legacy plugins data subdir: {e}"))?;
	}

	let logs_root = crate::utils::fs::runtime_logs_root_dir()?;
	std::fs::create_dir_all(&logs_root).map_err(|e| format!("failed to create logs dir: {e}"))?;

	Ok(())
}

pub(crate) fn is_valid_root(root: &std::path::PathBuf) -> bool {
	let data_effect = root.join("data/share/obs/libobs/default.effect");
	if data_effect.exists() {
		return true;
	}
	let direct_effect = root.join("share/obs/libobs/default.effect");
	direct_effect.exists()
}

pub(crate) fn find_root_from(start: &std::path::PathBuf) -> Option<std::path::PathBuf> {
	for ancestor in start.ancestors() {
		let candidate = ancestor.to_path_buf();
		if is_valid_root(&candidate) {
			return Some(candidate);
		}
	}
	None
}

pub(crate) fn resolve_root_dir(root_dir: Option<String>) -> Result<std::path::PathBuf, String> {
	if let Some(root) = root_dir {
		let path = std::path::PathBuf::from(root);
		if path.exists() && is_valid_root(&path) {
			return Ok(path);
		}
		return Err(format!("Root path does not exist or is invalid: {path:?}"));
	}

	if let Ok(root) = std::env::var("REVO_ROOT") {
		let path = std::path::PathBuf::from(root);
		if path.exists() && is_valid_root(&path) {
			return Ok(path);
		}
	}

	let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let repo_root = manifest.join("..").join("..");
	if is_valid_root(&repo_root) {
		return Ok(repo_root);
	}

	if let Ok(cwd) = std::env::current_dir() {
		if let Some(found) = find_root_from(&cwd) {
			return Ok(found);
		}
	}

	if let Ok(exe) = std::env::current_exe() {
		if let Some(dir) = exe.parent() {
			if let Some(found) = find_root_from(&dir.to_path_buf()) {
				return Ok(found);
			}
		}
	}

	Err("Unable to resolve RevoStream root directory containing libobs effects".to_string())
}
