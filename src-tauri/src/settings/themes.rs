use std::io::Read;

pub(crate) fn sanitize_theme_id(raw: &str) -> String {
	raw.chars()
		.filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
		.collect::<String>()
}

pub(crate) fn themes_dir_for_root(root_dir: Option<String>) -> Result<std::path::PathBuf, String> {
	let mut candidates: Vec<std::path::PathBuf> = Vec::new();

	if let Some(root) = root_dir.as_ref().map(|v| v.trim()).filter(|v| !v.is_empty()) {
		let root_path = std::path::PathBuf::from(root);
		candidates.push(root_path.join("data").join("themes"));
		candidates.push(root_path.join("themes"));
	}

	let cwd = crate::utils::fs::startup_cwd()?;
	if cwd.file_name().and_then(|s| s.to_str()) == Some("src-tauri") {
		if let Some(parent) = cwd.parent() {
			candidates.push(parent.join("data").join("themes"));
		}
	}

	candidates.push(cwd.join("data").join("themes"));
	candidates.push(cwd.join("src-tauri").join("..").join("data").join("themes"));

	for candidate in candidates {
		if candidate.is_dir() {
			return Ok(candidate);
		}
	}

	let dir = crate::settings::core::runtime_data_dir()?.join("themes");
	std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create themes dir: {e}"))?;
	Ok(dir)
}

pub(crate) fn normalize_hex_color(raw: Option<String>, fallback: &str) -> String {
	let Some(value) = raw else {
		return fallback.to_string();
	};
	let mut s = value.trim().to_string();
	if s.is_empty() {
		return fallback.to_string();
	}
	if !s.starts_with('#') {
		s = format!("#{s}");
	}
	let body = s.trim_start_matches('#');
	let is_valid = (body.len() == 6 || body.len() == 8)
		&& body.chars().all(|c| c.is_ascii_hexdigit());
	if is_valid {
		format!("#{body}")
	} else {
		fallback.to_string()
	}
}

pub(crate) fn themes_list(root_dir: Option<String>) -> Result<Vec<crate::ThemeInfo>, String> {
	let dir = themes_dir_for_root(root_dir)?;
	let mut result: Vec<crate::ThemeInfo> = Vec::new();

	for entry in std::fs::read_dir(&dir).map_err(|e| format!("failed to read themes dir: {e}"))? {
		let Ok(entry) = entry else { continue };
		let path = entry.path();
		if !path.is_dir() {
			continue;
		}

		let Some(folder_name) = path.file_name().and_then(|v| v.to_str()) else {
			continue;
		};
		let theme_id = sanitize_theme_id(folder_name);
		if theme_id.is_empty() {
			continue;
		}

		let css_file = path.join("theme.css");
		let cfg_file = path.join("config.revo");
		if !css_file.is_file() || !cfg_file.is_file() {
			continue;
		}

		let mut theme_name = folder_name.to_string();
		let mut author = "Unknown".to_string();
		let mut version = "0.0.0".to_string();

		if let Ok(raw_cfg) = std::fs::read_to_string(&cfg_file) {
			if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw_cfg) {
				if let Some(v) = parsed.get("name").and_then(|v| v.as_str()) {
					if !v.trim().is_empty() {
						theme_name = v.trim().to_string();
					}
				}
				if let Some(v) = parsed.get("author").and_then(|v| v.as_str()) {
					if !v.trim().is_empty() {
						author = v.trim().to_string();
					}
				}
				if let Some(v) = parsed.get("version").and_then(|v| v.as_str()) {
					if !v.trim().is_empty() {
						version = v.trim().to_string();
					}
				}
			}
		}

		result.push(crate::ThemeInfo {
			id: theme_id,
			name: theme_name,
			author,
			version,
		});
	}

	result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
	Ok(result)
}

pub(crate) fn themes_get_css(theme_id: String, root_dir: Option<String>) -> Result<String, String> {
	let safe = sanitize_theme_id(&theme_id);
	if safe.is_empty() {
		return Ok(String::new());
	}
	let themes_dir = themes_dir_for_root(root_dir)?;
	let css_path = themes_dir.join(safe).join("theme.css");
	if !css_path.is_file() {
		return Err("theme.css not found for selected theme".to_string());
	}
	std::fs::read_to_string(&css_path).map_err(|e| format!("failed to read theme.css: {e}"))
}

pub(crate) fn themes_extract_default(
	root_dir: Option<String>,
	bg: Option<String>,
	surface: Option<String>,
	surface_2: Option<String>,
	surface_3: Option<String>,
	border: Option<String>,
	border_strong: Option<String>,
	text: Option<String>,
	text_muted: Option<String>,
	accent: Option<String>,
	accent_strong: Option<String>,
	success: Option<String>,
	danger: Option<String>,
	warning: Option<String>,
) -> Result<String, String> {
	let themes_dir = themes_dir_for_root(root_dir)?;
	let target = themes_dir.join("default_local");
	std::fs::create_dir_all(&target)
		.map_err(|e| format!("failed to create default theme dir: {e}"))?;

	let bg = normalize_hex_color(bg, "#0f1115");
	let surface = normalize_hex_color(surface, "#151820");
	let surface_2 = normalize_hex_color(surface_2, "#1b1f2a");
	let surface_3 = normalize_hex_color(surface_3, "#232838");
	let border = normalize_hex_color(border, "#2b3142");
	let border_strong = normalize_hex_color(border_strong, "#3a4258");
	let text = normalize_hex_color(text, "#e5e7eb");
	let text_muted = normalize_hex_color(text_muted, "#a1a1aa");
	let accent = normalize_hex_color(accent, "#5b7cfa");
	let accent_strong = normalize_hex_color(accent_strong, "#3b5bdb");
	let success = normalize_hex_color(success, "#22c55e");
	let danger = normalize_hex_color(danger, "#ef4444");
	let warning = normalize_hex_color(warning, "#f59e0b");

	let css = format!(
		":root {{\n  color-scheme: dark;\n  --bg: {bg};\n  --surface: {surface};\n  --surface-2: {surface_2};\n  --surface-3: {surface_3};\n  --border: {border};\n  --border-strong: {border_strong};\n  --text: {text};\n  --text-muted: {text_muted};\n  --accent: {accent};\n  --accent-strong: {accent_strong};\n  --success: {success};\n  --danger: {danger};\n  --warning: {warning};\n  --revo-disable-rounded: 0;\n}}\n"
	);
	std::fs::write(target.join("theme.css"), css)
		.map_err(|e| format!("failed to write default theme.css: {e}"))?;

	let cfg = serde_json::json!({
		"name": "Default Theme (generated)",
		"author": "RevoProject authors",
		"version": "0.1.0"
	});
	let cfg_raw = serde_json::to_string_pretty(&cfg)
		.map_err(|e| format!("failed to serialize default config.revo: {e}"))?;
	std::fs::write(target.join("config.revo"), cfg_raw)
		.map_err(|e| format!("failed to write default config.revo: {e}"))?;

	Ok(format!("Default theme extracted to {}", target.to_string_lossy()))
}

pub(crate) fn transitions_validate_sequence_dir(path: String) -> Result<String, String> {
	let trimmed = path.trim();
	if trimmed.is_empty() {
		return Err("folder path is empty".to_string());
	}

	let dir = std::path::PathBuf::from(trimmed);
	if !dir.is_dir() {
		return Err("selected path is not a directory".to_string());
	}

	let allowed_ext = [
		"png", "jpg", "jpeg", "webp", "bmp", "gif", "tif", "tiff", "tga", "exr", "svg",
	];

	let mut image_count = 0usize;
	for entry in std::fs::read_dir(&dir).map_err(|e| format!("failed to read directory: {e}"))? {
		let entry = entry.map_err(|e| format!("failed to read entry: {e}"))?;
		let entry_path = entry.path();

		if entry_path.is_dir() {
			return Err("sequence folder cannot contain subfolders".to_string());
		}

		if !entry_path.is_file() {
			continue;
		}

		let ext = entry_path
			.extension()
			.and_then(|s| s.to_str())
			.map(|s| s.to_ascii_lowercase())
			.unwrap_or_default();

		if !allowed_ext.iter().any(|allowed| *allowed == ext) {
			let bad = entry_path
				.file_name()
				.and_then(|s| s.to_str())
				.unwrap_or("unknown");
			return Err(format!("folder contains non-image file: {bad}"));
		}

		image_count += 1;
	}

	if image_count == 0 {
		return Err("folder does not contain any image files".to_string());
	}

	Ok(format!("ok ({image_count} images)"))
}

pub(crate) fn themes_import(
	root_dir: Option<String>,
	theme_id: String,
	name: Option<String>,
	author: Option<String>,
	version: Option<String>,
	css: String,
) -> Result<crate::ThemeInfo, String> {
	write_theme_to_runtime(
		root_dir,
		&theme_id,
		name.as_deref(),
		author.as_deref(),
		version.as_deref(),
		&css,
	)
}

pub(crate) fn write_theme_to_runtime(
	root_dir: Option<String>,
	theme_id: &str,
	name: Option<&str>,
	author: Option<&str>,
	version: Option<&str>,
	css: &str,
) -> Result<crate::ThemeInfo, String> {
	let mut safe_id = sanitize_theme_id(&theme_id);
	if safe_id.is_empty() {
		safe_id = "imported_theme".to_string();
	}

	let css_trimmed = css.trim();
	if css_trimmed.is_empty() {
		return Err("theme.css cannot be empty".to_string());
	}

	let display_name = name
		.map(|v| v.trim())
		.filter(|v| !v.is_empty())
		.unwrap_or("Imported Theme")
		.to_string();
	let display_author = author
		.map(|v| v.trim())
		.filter(|v| !v.is_empty())
		.unwrap_or("Imported")
		.to_string();
	let display_version = version
		.map(|v| v.trim())
		.filter(|v| !v.is_empty())
		.unwrap_or("0.1.0")
		.to_string();

	let themes_dir = themes_dir_for_root(root_dir)?;
	let target = themes_dir.join(&safe_id);
	std::fs::create_dir_all(&target).map_err(|e| format!("failed to create theme dir: {e}"))?;

	std::fs::write(target.join("theme.css"), css_trimmed)
		.map_err(|e| format!("failed to write theme.css: {e}"))?;

	let cfg = serde_json::json!({
		"name": display_name,
		"author": display_author,
		"version": display_version
	});
	let cfg_raw = serde_json::to_string_pretty(&cfg)
		.map_err(|e| format!("failed to serialize config.revo: {e}"))?;
	std::fs::write(target.join("config.revo"), cfg_raw)
		.map_err(|e| format!("failed to write config.revo: {e}"))?;

	Ok(crate::ThemeInfo {
		id: safe_id,
		name: cfg
			.get("name")
			.and_then(|v| v.as_str())
			.unwrap_or("Imported Theme")
			.to_string(),
		author: cfg
			.get("author")
			.and_then(|v| v.as_str())
			.unwrap_or("Imported")
			.to_string(),
		version: cfg
			.get("version")
			.and_then(|v| v.as_str())
			.unwrap_or("0.1.0")
			.to_string(),
	})
}

pub(crate) fn themes_import_archive(
	root_dir: Option<String>,
	file_name: String,
	data: Vec<u8>,
) -> Result<crate::ThemeInfo, String> {
	if !file_name.to_ascii_lowercase().ends_with(".revotheme") {
		return Err("Only .revotheme files are supported".to_string());
	}
	if data.is_empty() {
		return Err(".revotheme file is empty".to_string());
	}

	let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data))
		.map_err(|e| format!("invalid .revotheme archive: {e}"))?;

	let mut file_paths: Vec<String> = Vec::new();
	for i in 0..archive.len() {
		let entry = archive
			.by_index(i)
			.map_err(|e| format!("failed to read archive entry: {e}"))?;
		if entry.is_dir() {
			continue;
		}

		let name = entry.name().replace('\\', "/");
		if name.starts_with('/') || name.contains("../") {
			return Err("invalid .revotheme archive path".to_string());
		}
		file_paths.push(name);
	}

	if file_paths.is_empty() {
		return Err(".revotheme does not contain files".to_string());
	}

	let root_cfg = file_paths.iter().find(|p| p.as_str() == "config.revo").cloned();
	let root_css = file_paths.iter().find(|p| p.as_str() == "theme.css").cloned();

	let (cfg_path, css_path, folder_hint): (String, String, Option<String>) =
		if let (Some(cfg), Some(css)) = (root_cfg, root_css) {
			(cfg, css, None)
		} else {
			let cfg_candidates: Vec<String> = file_paths
				.iter()
				.filter(|p| p.ends_with("/config.revo"))
				.cloned()
				.collect();
			let css_candidates: Vec<String> = file_paths
				.iter()
				.filter(|p| p.ends_with("/theme.css"))
				.cloned()
				.collect();

			if cfg_candidates.len() != 1 || css_candidates.len() != 1 {
				return Err("invalid .revotheme: expected config.revo and theme.css in root or a single folder".to_string());
			}

			let cfg = cfg_candidates[0].clone();
			let css = css_candidates[0].clone();
			let cfg_parent = cfg
				.rsplit_once('/')
				.map(|(parent, _)| parent.to_string())
				.ok_or_else(|| "invalid .revotheme config path".to_string())?;
			let css_parent = css
				.rsplit_once('/')
				.map(|(parent, _)| parent.to_string())
				.ok_or_else(|| "invalid .revotheme css path".to_string())?;

			if cfg_parent != css_parent || cfg_parent.contains('/') {
				return Err("invalid .revotheme: files must be in one top-level folder".to_string());
			}

			for path in &file_paths {
				if !path.starts_with(&format!("{cfg_parent}/")) {
					return Err("invalid .revotheme: archive must contain only one top-level folder".to_string());
				}
			}

			(cfg, css, Some(cfg_parent))
		};

	let mut cfg_raw = String::new();
	archive
		.by_name(&cfg_path)
		.map_err(|e| format!("failed to read config.revo: {e}"))?
		.read_to_string(&mut cfg_raw)
		.map_err(|e| format!("failed to parse config.revo as UTF-8: {e}"))?;

	let mut css = String::new();
	archive
		.by_name(&css_path)
		.map_err(|e| format!("failed to read theme.css: {e}"))?
		.read_to_string(&mut css)
		.map_err(|e| format!("failed to parse theme.css as UTF-8: {e}"))?;

	if css.trim().is_empty() {
		return Err("theme.css cannot be empty".to_string());
	}

	let cfg: serde_json::Value =
		serde_json::from_str(&cfg_raw).map_err(|e| format!("invalid config.revo JSON: {e}"))?;

	let name = cfg
		.get("name")
		.and_then(|v| v.as_str())
		.map(|v| v.trim().to_string())
		.filter(|v| !v.is_empty())
		.unwrap_or_else(|| "Imported Theme".to_string());
	let author = cfg
		.get("author")
		.and_then(|v| v.as_str())
		.map(|v| v.trim().to_string())
		.filter(|v| !v.is_empty())
		.unwrap_or_else(|| "Imported".to_string());
	let version = cfg
		.get("version")
		.and_then(|v| v.as_str())
		.map(|v| v.trim().to_string())
		.filter(|v| !v.is_empty())
		.unwrap_or_else(|| "0.1.0".to_string());

	let file_stem = file_name
		.trim_end_matches(".revotheme")
		.trim_end_matches(".REVOTHEME")
		.trim();
	let unix = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.as_secs())
		.unwrap_or(0);

	let theme_id = if let Some(folder) = folder_hint {
		let safe = sanitize_theme_id(&folder);
		if safe.is_empty() {
			format!("theme_{unix}")
		} else {
			safe
		}
	} else {
		let from_name = sanitize_theme_id(&name);
		let from_file = sanitize_theme_id(file_stem);
		if !from_name.is_empty() {
			format!("{}_{}", from_name, unix)
		} else if !from_file.is_empty() {
			format!("{}_{}", from_file, unix)
		} else {
			format!("theme_{unix}")
		}
	};

	write_theme_to_runtime(
		root_dir,
		&theme_id,
		Some(&name),
		Some(&author),
		Some(&version),
		&css,
	)
}
