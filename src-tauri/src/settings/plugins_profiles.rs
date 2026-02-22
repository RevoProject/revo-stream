pub(crate) fn plugin_display_name_from_module(module_name: &str) -> String {
	let base = module_name.trim_start_matches("obs-");
	let words = base.replace(['-', '_'], " ");
	crate::settings::core::title_case_words(words.trim())
}

pub(crate) fn list_runtime_plugins_internal() -> Result<Vec<crate::PluginInfo>, String> {
	crate::settings::core::ensure_runtime_data_dirs()?;
	let primary = crate::settings::core::runtime_plugins_dir()?;
	let legacy = crate::settings::core::legacy_runtime_plugins_dir()?;
	let mut dirs = vec![primary.clone()];
	if legacy != primary {
		dirs.push(legacy);
	}

	let mut by_module: std::collections::HashMap<String, crate::PluginInfo> =
		std::collections::HashMap::new();
	for dir in dirs {
		if !dir.exists() {
			continue;
		}

		for entry in std::fs::read_dir(&dir).map_err(|e| format!("failed to read plugins dir: {e}"))? {
			let Ok(entry) = entry else { continue };
			let path = entry.path();
			if !path.is_file() {
				continue;
			}
			let ext = path
				.extension()
				.and_then(|e| e.to_str())
				.unwrap_or_default()
				.to_ascii_lowercase();
			if ext != "so" && ext != "dll" && ext != "dylib" {
				continue;
			}
			let Some(file_name) = path
				.file_name()
				.and_then(|s| s.to_str())
				.map(|s| s.to_string())
			else {
				continue;
			};
			let Some(module_name) = path
				.file_stem()
				.and_then(|s| s.to_str())
				.map(|s| s.to_string())
			else {
				continue;
			};
			if module_name.is_empty() {
				continue;
			}

			by_module
				.entry(module_name.clone())
				.or_insert(crate::PluginInfo {
					name: plugin_display_name_from_module(&module_name),
					file_name,
					module_name,
				});
		}
	}

	let mut plugins: Vec<crate::PluginInfo> = by_module.into_values().collect();

	plugins.sort_by(|a, b| a.name.cmp(&b.name).then(a.file_name.cmp(&b.file_name)));
	Ok(plugins)
}

pub(crate) fn plugin_profile_active_name_internal() -> Result<String, String> {
	let file = crate::settings::core::runtime_plugin_active_profile_file()?;
	if !file.exists() {
		return Ok("default".to_string());
	}
	let raw = std::fs::read_to_string(&file)
		.map_err(|e| format!("failed to read active plugin profile: {e}"))?;
	let safe = crate::settings::core::sanitize_profile_name(raw.trim());
	if safe.is_empty() {
		return Ok("default".to_string());
	}
	Ok(safe)
}

pub(crate) fn plugin_profile_save_internal(
	name: &str,
	profile: &crate::PluginProfile,
) -> Result<(), String> {
	let file = crate::settings::core::runtime_plugin_profile_file(name)?;
	if let Some(parent) = file.parent() {
		std::fs::create_dir_all(parent)
			.map_err(|e| format!("failed to create plugin profile dir: {e}"))?;
	}
	let raw = serde_json::to_string_pretty(profile)
		.map_err(|e| format!("failed to serialize plugin profile: {e}"))?;
	std::fs::write(&file, raw).map_err(|e| format!("failed to write plugin profile: {e}"))?;
	Ok(())
}

pub(crate) fn plugin_profile_get_internal(name: &str) -> Result<crate::PluginProfile, String> {
	let safe = crate::settings::core::sanitize_profile_name(name);
	if safe.is_empty() {
		return Err("Invalid plugin profile name".to_string());
	}

	let plugins = list_runtime_plugins_internal()?;
	let available: std::collections::HashSet<String> =
		plugins.into_iter().map(|p| p.module_name).collect();
	let normalized_to_available: std::collections::HashMap<String, String> = available
		.iter()
		.map(|m| (crate::settings::core::sanitize_profile_name(m), m.clone()))
		.collect();
	let file = crate::settings::core::runtime_plugin_profile_file(&safe)?;

	if !file.exists() {
		return Ok(crate::PluginProfile {
			enabled_modules: available.iter().cloned().collect(),
		});
	}

	let raw = std::fs::read_to_string(&file)
		.map_err(|e| format!("failed to read plugin profile: {e}"))?;
	let parsed = serde_json::from_str::<crate::PluginProfile>(&raw)
		.unwrap_or(crate::PluginProfile { enabled_modules: vec![] });

	let mut filtered: Vec<String> = parsed
		.enabled_modules
		.into_iter()
		.filter_map(|n| {
			let trimmed = n.trim().to_string();
			if trimmed.is_empty() {
				return None;
			}
			if available.contains(&trimmed) {
				return Some(trimmed);
			}
			let normalized = crate::settings::core::sanitize_profile_name(&trimmed);
			if normalized.is_empty() {
				return None;
			}
			normalized_to_available.get(&normalized).cloned()
		})
		.collect();
	filtered.sort();
	filtered.dedup();

	Ok(crate::PluginProfile {
		enabled_modules: filtered,
	})
}

pub(crate) fn plugins_list() -> Result<Vec<crate::PluginInfo>, String> {
	crate::logging::debug::push_debug_log_entry("[plugins] plugins_list:start".to_string(), None);
	let result = list_runtime_plugins_internal();
	match &result {
		Ok(items) => crate::logging::debug::push_debug_log_entry(
			"[plugins] plugins_list:ok".to_string(),
			Some(serde_json::json!({ "count": items.len() })),
		),
		Err(err) => crate::logging::debug::push_debug_log_entry(
			"[plugins] plugins_list:err".to_string(),
			Some(serde_json::json!({ "error": err })),
		),
	}
	result
}

pub(crate) fn plugin_profiles_list() -> Result<Vec<String>, String> {
	crate::logging::debug::push_debug_log_entry("[plugins] plugin_profiles_list:start".to_string(), None);
	crate::settings::core::ensure_runtime_data_dirs()?;
	let dir = crate::settings::core::runtime_plugin_profiles_dir()?;
	if !dir.exists() {
		crate::logging::debug::push_debug_log_entry("[plugins] plugin_profiles_list:ok default-only".to_string(), None);
		return Ok(vec!["default".to_string()]);
	}

	let mut names: Vec<String> = std::fs::read_dir(&dir)
		.map_err(|e| format!("failed to read plugin profiles dir: {e}"))?
		.filter_map(|entry| entry.ok())
		.filter_map(|entry| {
			let path = entry.path();
			if !path.is_dir() {
				return None;
			}
			if !path.join("profile.json").is_file() {
				return None;
			}
			path.file_name()
				.and_then(|s| s.to_str())
				.map(|s| s.to_string())
		})
		.collect();

	if !names.iter().any(|n| n == "default") {
		names.push("default".to_string());
	}
	names.sort();
	names.dedup();
	crate::logging::debug::push_debug_log_entry(
		"[plugins] plugin_profiles_list:ok".to_string(),
		Some(serde_json::json!({ "count": names.len() })),
	);
	Ok(names)
}

pub(crate) fn plugin_profile_get_active() -> Result<String, String> {
	crate::logging::debug::push_debug_log_entry("[plugins] plugin_profile_get_active:start".to_string(), None);
	let result = plugin_profile_active_name_internal();
	match &result {
		Ok(name) => crate::logging::debug::push_debug_log_entry(
			"[plugins] plugin_profile_get_active:ok".to_string(),
			Some(serde_json::json!({ "name": name })),
		),
		Err(err) => crate::logging::debug::push_debug_log_entry(
			"[plugins] plugin_profile_get_active:err".to_string(),
			Some(serde_json::json!({ "error": err })),
		),
	}
	result
}

pub(crate) fn plugin_profile_set_active(name: String) -> Result<String, String> {
	crate::settings::core::ensure_runtime_data_dirs()?;
	let safe = crate::settings::core::sanitize_profile_name(&name);
	if safe.is_empty() {
		return Err("Invalid plugin profile name".to_string());
	}
	let active_file = crate::settings::core::runtime_plugin_active_profile_file()?;
	if let Some(parent) = active_file.parent() {
		std::fs::create_dir_all(parent)
			.map_err(|e| format!("failed to create plugin profiles dir: {e}"))?;
	}
	std::fs::write(&active_file, &safe)
		.map_err(|e| format!("failed to write active plugin profile: {e}"))?;
	Ok(safe)
}

pub(crate) fn plugin_profile_get(name: String) -> Result<crate::PluginProfileState, String> {
	crate::logging::debug::push_debug_log_entry(
		"[plugins] plugin_profile_get:start".to_string(),
		Some(serde_json::json!({ "raw_name": name })),
	);
	let safe = crate::settings::core::sanitize_profile_name(&name);
	if safe.is_empty() {
		crate::logging::debug::push_debug_log_entry("[plugins] plugin_profile_get:err invalid-name".to_string(), None);
		return Err("Invalid plugin profile name".to_string());
	}
	let profile = plugin_profile_get_internal(&safe)?;
	crate::logging::debug::push_debug_log_entry(
		"[plugins] plugin_profile_get:ok".to_string(),
		Some(serde_json::json!({
			"name": safe,
			"enabled_count": profile.enabled_modules.len()
		})),
	);
	Ok(crate::PluginProfileState {
		profile_name: safe,
		enabled_modules: profile.enabled_modules,
	})
}

pub(crate) fn plugin_profile_save(
	name: String,
	enabled_modules: Vec<String>,
) -> Result<String, String> {
	crate::logging::debug::push_debug_log_entry(
		"[plugins] plugin_profile_save:start".to_string(),
		Some(serde_json::json!({
			"raw_name": name,
			"incoming_enabled_count": enabled_modules.len()
		})),
	);
	let safe = crate::settings::core::sanitize_profile_name(&name);
	if safe.is_empty() {
		crate::logging::debug::push_debug_log_entry("[plugins] plugin_profile_save:err invalid-name".to_string(), None);
		return Err("Invalid plugin profile name".to_string());
	}

	let available: std::collections::HashSet<String> = list_runtime_plugins_internal()?
		.into_iter()
		.map(|p| p.module_name)
		.collect();
	let normalized_to_available: std::collections::HashMap<String, String> = available
		.iter()
		.map(|m| (crate::settings::core::sanitize_profile_name(m), m.clone()))
		.collect();

	let mut modules: Vec<String> = enabled_modules
		.into_iter()
		.filter_map(|m| {
			let trimmed = m.trim().to_string();
			if trimmed.is_empty() {
				return None;
			}
			if available.contains(&trimmed) {
				return Some(trimmed);
			}
			let normalized = crate::settings::core::sanitize_profile_name(&trimmed);
			if normalized.is_empty() {
				return None;
			}
			normalized_to_available.get(&normalized).cloned()
		})
		.collect();
	modules.sort();
	modules.dedup();

	let profile = crate::PluginProfile {
		enabled_modules: modules,
	};
	plugin_profile_save_internal(&safe, &profile)?;
	crate::logging::debug::push_debug_log_entry(
		"[plugins] plugin_profile_save:ok".to_string(),
		Some(serde_json::json!({
			"name": safe,
			"stored_enabled_count": profile.enabled_modules.len()
		})),
	);
	Ok(safe)
}

pub(crate) fn profiles_list() -> Result<Vec<String>, String> {
	crate::settings::core::ensure_runtime_data_dirs()?;
	let dir = crate::settings::core::runtime_profiles_dir()?;
	if !dir.exists() {
		return Ok(vec!["default".to_string()]);
	}

	let mut names: Vec<String> = std::fs::read_dir(&dir)
		.map_err(|e| format!("failed to read profiles dir: {e}"))?
		.filter_map(|entry| entry.ok())
		.filter_map(|entry| {
			let path = entry.path();
			if path.is_dir() {
				let profile_json = path.join("profile.json");
				if profile_json.is_file() {
					return path
						.file_name()
						.and_then(|s| s.to_str())
						.map(|s| s.to_string());
				}
				return None;
			}

			if path.extension().and_then(|e| e.to_str()) == Some("json") {
				return path
					.file_stem()
					.and_then(|s| s.to_str())
					.map(|s| s.to_string());
			}
			None
		})
		.collect();

	if !names.iter().any(|n| n == "default") {
		names.push("default".to_string());
	}
	names.sort();
	names.dedup();
	Ok(names)
}

pub(crate) fn profiles_create(name: String) -> Result<String, String> {
	crate::settings::core::ensure_runtime_data_dirs()?;
	let safe = crate::settings::core::sanitize_profile_name(&name);
	if safe.is_empty() {
		return Err("Profile name is empty or invalid".to_string());
	}

	let file = crate::settings::core::runtime_profile_file(&safe)?;
	let legacy_file = crate::settings::core::legacy_runtime_profile_file(&safe)?;
	if file.exists() || legacy_file.exists() {
		return Err(format!("Profile '{}' already exists", safe));
	}

	if let Some(parent) = file.parent() {
		std::fs::create_dir_all(parent)
			.map_err(|e| format!("failed to create profile directory: {e}"))?;
	}

	let mut settings = crate::settings::core::settings_get()?;
	settings.active_profile = Some(safe.clone());
	crate::settings::core::normalize_settings(&mut settings);

	let raw = serde_json::to_string_pretty(&settings)
		.map_err(|e| format!("failed to serialize profile: {e}"))?;
	std::fs::write(&file, raw).map_err(|e| format!("failed to create profile: {e}"))?;
	crate::settings::core::settings_save(settings)?;
	Ok(safe)
}

pub(crate) fn profiles_activate(name: String) -> Result<crate::AppSettings, String> {
	crate::settings::core::ensure_runtime_data_dirs()?;
	let safe = crate::settings::core::sanitize_profile_name(&name);
	if safe.is_empty() {
		return Err("Invalid profile name".to_string());
	}

	let file = crate::settings::core::runtime_profile_file(&safe)?;
	let legacy_file = crate::settings::core::legacy_runtime_profile_file(&safe)?;

	let source_file = if file.exists() {
		file.clone()
	} else if legacy_file.exists() {
		if let Some(parent) = file.parent() {
			std::fs::create_dir_all(parent)
				.map_err(|e| format!("failed to create profile directory: {e}"))?;
		}
		let _ = std::fs::copy(&legacy_file, &file);
		file.clone()
	} else {
		return Err(format!("Profile '{}' does not exist", safe));
	};

	let raw = std::fs::read_to_string(&source_file)
		.map_err(|e| format!("failed to read profile: {e}"))?;
	let mut settings = crate::settings::core::parse_settings_raw(&raw);
	settings.active_profile = Some(safe);
	crate::settings::core::settings_save(settings.clone())?;
	Ok(settings)
}
