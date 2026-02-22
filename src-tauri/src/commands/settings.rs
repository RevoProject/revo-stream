use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub root_dir: Option<String>,
    pub record_path: String,
    pub stream_url: String,
    pub preview_quality: Option<String>,
}

fn data_dir_from_root(root: &PathBuf) -> PathBuf {
    if root.join("data/share/obs/libobs/default.effect").exists() {
        root.join("data")
    } else {
        root.clone()
    }
}

fn settings_path(root: &PathBuf) -> PathBuf {
    data_dir_from_root(root).join("conf").join("settings.json")
}

fn user_profile_dir() -> Option<PathBuf> {
    if let Ok(val) = env::var("USERPROFILE") {
        return Some(PathBuf::from(val));
    }
    if let Ok(val) = env::var("HOME") {
        return Some(PathBuf::from(val));
    }
    let drive = env::var("HOMEDRIVE").ok();
    let path = env::var("HOMEPATH").ok();
    match (drive, path) {
        (Some(d), Some(p)) => Some(PathBuf::from(format!("{d}{p}"))),
        _ => None,
    }
}

fn default_record_path() -> String {
    if let Some(profile) = user_profile_dir() {
        return profile
            .join("Videos")
            .join("RevoStream")
            .join("record.mp4")
            .to_string_lossy()
            .to_string();
    }
    PathBuf::from("Videos")
        .join("RevoStream")
        .join("record.mp4")
        .to_string_lossy()
        .to_string()
}

fn default_settings() -> AppSettings {
    AppSettings {
        root_dir: None,
        record_path: default_record_path(),
        stream_url: String::new(),
        preview_quality: Some("medium".to_string()),
    }
}

fn is_valid_root(root: &PathBuf) -> bool {
    let data_effect = root.join("data/share/obs/libobs/default.effect");
    if data_effect.exists() {
        return true;
    }
    let direct_effect = root.join("share/obs/libobs/default.effect");
    direct_effect.exists()
}

fn find_root_from(start: &PathBuf) -> Option<PathBuf> {
    for ancestor in start.ancestors() {
        let candidate = ancestor.to_path_buf();
        if is_valid_root(&candidate) {
            return Some(candidate);
        }
    }
    None
}

pub fn resolve_root_dir(root_dir: Option<String>) -> Result<PathBuf, String> {
    if let Some(root) = root_dir {
        let path = PathBuf::from(root);
        if path.exists() && is_valid_root(&path) {
            return Ok(path);
        }
        return Err(format!("Root path does not exist or is invalid: {path:?}"));
    }

    if let Ok(root) = env::var("REVO_ROOT") {
        let path = PathBuf::from(root);
        if path.exists() && is_valid_root(&path) {
            return Ok(path);
        }
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest.join("..").join("..");
    if is_valid_root(&repo_root) {
        return Ok(repo_root);
    }

    if let Ok(cwd) = env::current_dir() {
        if let Some(found) = find_root_from(&cwd) {
            return Ok(found);
        }
    }

    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            if let Some(found) = find_root_from(&dir.to_path_buf()) {
                return Ok(found);
            }
        }
    }

    Err("Unable to resolve RevoStream root directory containing libobs effects".to_string())
}

#[tauri::command]
pub fn settings_get() -> Result<AppSettings, String> {
    let root = resolve_root_dir(None)?;
    let path = settings_path(&root);
    if path.exists() {
        let raw = fs::read_to_string(&path).map_err(|e| format!("failed to read settings: {e}"))?;
        let parsed: AppSettings = serde_json::from_str(&raw)
            .map_err(|e| format!("failed to parse settings: {e}"))?;
        return Ok(parsed);
    }

    let settings = default_settings();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let raw = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("failed to serialize settings: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("failed to write settings: {e}"))?;
    Ok(settings)
}

#[tauri::command]
pub fn settings_save(settings: AppSettings) -> Result<String, String> {
    let root = resolve_root_dir(settings.root_dir.clone())?;
    let path = settings_path(&root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("failed to create settings dir: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("failed to serialize settings: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("failed to write settings: {e}"))?;
    Ok(format!("Settings saved to {}", path.to_string_lossy()))
}
