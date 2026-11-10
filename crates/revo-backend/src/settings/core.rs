pub fn data_dir_from_root(root: &std::path::PathBuf) -> std::path::PathBuf {
    if root.join("data/share/obs/libobs/default.effect").exists() {
        root.join("data")
    } else {
        root.clone()
    }
}

pub fn settings_path(root: &std::path::PathBuf) -> Result<std::path::PathBuf, String> {
    let data_dir = match crate::utils::fs::runtime_app_data_dir()? {
        Some(dir) => dir.join("data"),
        None => data_dir_from_root(root),
    };
    Ok(data_dir.join("conf").join("profile.json"))
}

pub fn legacy_settings_path(root: &std::path::PathBuf) -> std::path::PathBuf {
    data_dir_from_root(root).join("conf").join("settings.json")
}

pub use revo_core::settings::{
    default_active_profile, default_encoder_preference, default_preview_quality,
    default_record_path, default_scene_resolution, default_settings, default_whep_url,
    default_whip_url, is_valid_resolution, normalize_optional_trimmed, normalize_settings,
    parse_settings_raw, sanitize_profile_name,
};

pub fn find_file_upwards(
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

pub fn resolve_rtmp_services_json_path() -> Option<std::path::PathBuf> {
    let mut relative_candidates = vec!["data/conf/rtmp-services/services.json".to_string()];
    if let Some(dev_name) = crate::utils::fs::adapter_dev_dir_name() {
        relative_candidates.push(format!(
            "revo-ui/{dev_name}/resources/revo-root/data/conf/rtmp-services/services.json"
        ));
        relative_candidates.push(format!(
            "{dev_name}/resources/revo-root/data/conf/rtmp-services/services.json"
        ));
    }

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
        for rel in &relative_candidates {
            if let Some(found) = find_file_upwards(&start, rel) {
                return Some(found);
            }
        }
    }

    None
}

pub fn load_rtmp_services_json() -> Result<String, String> {
    let path = resolve_rtmp_services_json_path().ok_or_else(|| {
        "Could not locate data/conf/rtmp-services/services.json from current runtime directory"
            .to_string()
    })?;

    std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "failed to read services.json from {}: {e}",
            path.to_string_lossy()
        )
    })
}

pub fn settings_get() -> Result<crate::AppSettings, String> {
    let root = resolve_root_dir(None)?;
    let path = settings_path(&root)?;
    let resource_profile = data_dir_from_root(&root).join("conf/profile.json");
    let legacy_path = legacy_settings_path(&root);
    let settings = load_settings_from_paths(&path, &resource_profile, &legacy_path)?;
    let _ = sync_active_profile_file(&settings);
    Ok(settings)
}

fn load_settings_from_paths(
    path: &std::path::PathBuf,
    resource_profile: &std::path::PathBuf,
    legacy_path: &std::path::PathBuf,
) -> Result<crate::AppSettings, String> {
    // Installed resource profiles are seeds only; normalization always writes user state.
    let source = [path, resource_profile, legacy_path]
        .into_iter()
        .find(|p| p.exists());
    let settings = match source {
        Some(source) => {
            let raw = std::fs::read_to_string(source)
                .map_err(|e| format!("failed to read profile {}: {e}", source.display()))?;
            parse_settings_raw(&raw)
        }
        None => default_settings(),
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create profile dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("failed to serialize profile: {e}"))?;
    std::fs::write(&path, raw).map_err(|e| format!("failed to write profile: {e}"))?;
    Ok(settings)
}

pub fn settings_save(settings: crate::AppSettings) -> Result<String, String> {
    let mut normalized_settings = settings;
    normalize_settings(&mut normalized_settings);

    let root = resolve_root_dir(normalized_settings.root_dir.clone())?;
    let path = settings_path(&root)?;
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

pub fn runtime_data_dir() -> Result<std::path::PathBuf, String> {
    if let Some(data_dir) = crate::utils::fs::runtime_app_data_dir()? {
        return Ok(data_dir.join("data"));
    }
    let cwd = crate::utils::fs::startup_cwd()?;
    if crate::utils::fs::adapter_dev_dir_name()
        .as_deref()
        .and_then(|dev| cwd.file_name().map(|n| n == std::ffi::OsStr::new(dev)))
        .unwrap_or(false)
    {
        if let Some(parent) = cwd.parent() {
            return Ok(parent.join("data"));
        }
    }
    Ok(cwd.join("data"))
}

pub fn runtime_plugins_dir() -> Result<std::path::PathBuf, String> {
    if cfg!(debug_assertions) {
        let cwd = crate::utils::fs::startup_cwd()?;
        if crate::utils::fs::adapter_dev_dir_name()
            .as_deref()
            .map(|dev| {
                cwd.file_name()
                    .is_some_and(|n| n == std::ffi::OsStr::new(dev))
            })
            .unwrap_or(false)
        {
            if let Some(parent) = cwd.parent() {
                return Ok(parent.join("data").join("plugins"));
            }
        }
    }

    Ok(runtime_data_dir()?.join("plugins"))
}

pub fn legacy_runtime_plugins_dir() -> Result<std::path::PathBuf, String> {
    if !cfg!(debug_assertions) {
        return runtime_plugins_dir();
    }
    let cwd = crate::utils::fs::startup_cwd()?;
    let dev_name = crate::utils::fs::adapter_dev_dir_name();
    if dev_name.is_some() {
        if let Some(dev) = dev_name.as_deref() {
            if cwd
                .file_name()
                .is_some_and(|n| n == std::ffi::OsStr::new(dev))
            {
                return Ok(cwd.join("data").join("plugins"));
            }
        }
    }
    if cfg!(debug_assertions) {
        if let Some(dev) = dev_name.as_deref() {
            let dev_data = cwd.join(dev).join("data");
            if dev_data.is_dir() {
                return Ok(dev_data.join("plugins"));
            }
        }
    }
    Ok(cwd.join("data").join("plugins"))
}

pub fn runtime_plugin_profiles_dir() -> Result<std::path::PathBuf, String> {
    Ok(runtime_plugins_dir()?.join("profiles"))
}

pub fn runtime_plugin_profile_dir(name: &str) -> Result<std::path::PathBuf, String> {
    let safe = sanitize_profile_name(name);
    if safe.is_empty() {
        return Err("Invalid plugin profile name".to_string());
    }
    Ok(runtime_plugin_profiles_dir()?.join(safe))
}

pub fn runtime_plugin_profile_file(name: &str) -> Result<std::path::PathBuf, String> {
    Ok(runtime_plugin_profile_dir(name)?.join("profile.json"))
}

pub fn runtime_plugin_active_profile_file() -> Result<std::path::PathBuf, String> {
    Ok(runtime_plugin_profiles_dir()?.join("active_profile.txt"))
}

pub fn runtime_profiles_dir() -> Result<std::path::PathBuf, String> {
    Ok(runtime_data_dir()?.join("profiles"))
}

pub fn runtime_profile_dir(name: &str) -> Result<std::path::PathBuf, String> {
    let safe = sanitize_profile_name(name);
    if safe.is_empty() {
        return Err("Invalid profile name".to_string());
    }
    Ok(runtime_profiles_dir()?.join(safe))
}

pub fn runtime_profile_file(name: &str) -> Result<std::path::PathBuf, String> {
    Ok(runtime_profile_dir(name)?.join("profile.json"))
}

pub fn legacy_runtime_profile_file(name: &str) -> Result<std::path::PathBuf, String> {
    let safe = sanitize_profile_name(name);
    if safe.is_empty() {
        return Err("Invalid profile name".to_string());
    }
    Ok(runtime_profiles_dir()?.join(format!("{safe}.json")))
}

pub fn sync_active_profile_file(settings: &crate::AppSettings) -> Result<(), String> {
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

pub fn ensure_runtime_data_dirs() -> Result<(), String> {
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

pub fn is_valid_root(root: &std::path::PathBuf) -> bool {
    let data_effect = root.join("data/share/obs/libobs/default.effect");
    if data_effect.exists() {
        return true;
    }
    let direct_effect = root.join("share/obs/libobs/default.effect");
    if direct_effect.exists() {
        return true;
    }
    // macOS staging mirrors the framework-hosted effects into core/share.
    let core_effect = root.join("core/share/obs/libobs/default.effect");
    core_effect.exists()
}

pub fn find_root_from(start: &std::path::PathBuf) -> Option<std::path::PathBuf> {
    for ancestor in start.ancestors() {
        let candidate = ancestor.to_path_buf();
        if is_valid_root(&candidate) {
            return Some(candidate);
        }
    }
    None
}

pub fn resolve_root_dir(root_dir: Option<String>) -> Result<std::path::PathBuf, String> {
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

    if let Some(resources) = crate::utils::fs::resource_dir() {
        for relative in ["resources/revo-root", "revo-root", ""] {
            let candidate = resources.join(relative);
            if is_valid_root(&candidate) {
                return Ok(candidate);
            }
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // Packaged layouts without a Tauri resource directory: the runtime
            // sits next to the binary (portable tree, AppImage, FHS install)
            // or inside a macOS bundle one level up.
            for relative in [
                "revo-root",
                "../Resources/revo-root",
                "../Resources/resources/revo-root",
                "../lib/revo-stream/revo-root",
            ] {
                let candidate = dir.join(relative);
                if is_valid_root(&candidate) {
                    return Ok(candidate);
                }
            }
            if let Some(found) = find_root_from(&dir.to_path_buf()) {
                return Ok(found);
            }
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        if let Some(found) = find_root_from(&cwd) {
            return Ok(found);
        }
    }

    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.join("..").join("..");
    if is_valid_root(&repo_root) {
        return Ok(repo_root);
    }

    Err("Unable to resolve RevoStream root directory containing libobs effects".to_string())
}

#[cfg(test)]
mod writable_state_tests {
    use super::*;

    #[test]
    fn shipped_settings_seed_user_config_without_modifying_resources() {
        let temp = std::env::temp_dir().join(format!("revo-settings-{}", uuid::Uuid::new_v4()));
        let resources = temp.join("resources/conf");
        std::fs::create_dir_all(&resources).unwrap();
        let user_profile = temp.join("user/data/conf/profile.json");
        let resource_profile = resources.join("profile.json");
        let legacy = resources.join("settings.json");
        let legacy_raw = r#"{"stream_url":" legacy "}"#;
        std::fs::write(&legacy, legacy_raw).unwrap();
        let migrated = load_settings_from_paths(&user_profile, &resource_profile, &legacy).unwrap();
        assert_eq!(migrated.stream_url, "legacy");
        assert_eq!(std::fs::read_to_string(&legacy).unwrap(), legacy_raw);

        std::fs::remove_file(&user_profile).unwrap();
        let shipped_raw = r#"{"stream_url":" shipped "}"#;
        std::fs::write(&resource_profile, shipped_raw).unwrap();
        let seeded = load_settings_from_paths(&user_profile, &resource_profile, &legacy).unwrap();
        assert_eq!(seeded.stream_url, "shipped");
        assert!(user_profile.is_file());
        std::fs::write(&user_profile, r#"{"stream_url":" user "}"#).unwrap();
        let loaded = load_settings_from_paths(&user_profile, &resource_profile, &legacy).unwrap();
        assert_eq!(loaded.stream_url, "user");
        assert_eq!(
            std::fs::read_to_string(&resource_profile).unwrap(),
            shipped_raw
        );
        assert_eq!(std::fs::read_to_string(&legacy).unwrap(), legacy_raw);
        std::fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    #[cfg(debug_assertions)]
    fn development_config_preserves_resource_root_layout() {
        let root = std::env::temp_dir().join(format!("revo-paths-{}", uuid::Uuid::new_v4()));
        assert_eq!(
            settings_path(&root).unwrap(),
            root.join("conf/profile.json")
        );
        let effects = root.join("data/share/obs/libobs");
        std::fs::create_dir_all(&effects).unwrap();
        std::fs::write(effects.join("default.effect"), "").unwrap();
        assert_eq!(
            settings_path(&root).unwrap(),
            root.join("data/conf/profile.json")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
