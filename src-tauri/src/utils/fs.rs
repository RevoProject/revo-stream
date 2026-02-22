use std::path::PathBuf;

static APP_START_CWD: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

pub(crate) fn set_startup_cwd(path: PathBuf) {
    let _ = APP_START_CWD.set(path);
}

pub(crate) fn startup_cwd() -> Result<PathBuf, String> {
    if let Some(path) = APP_START_CWD.get() {
        return Ok(path.clone());
    }
    std::env::current_dir().map_err(|e| format!("failed to get current dir: {e}"))
}

pub(crate) fn runtime_logs_root_dir() -> Result<PathBuf, String> {
    let cwd = startup_cwd()?;
    if cwd.file_name().and_then(|s| s.to_str()) == Some("src-tauri") {
        if let Some(parent) = cwd.parent() {
            return Ok(parent.join("logs"));
        }
        return Ok(cwd.join("..").join("logs"));
    }

    let src_tauri_dir = cwd.join("src-tauri");
    if src_tauri_dir.is_dir() {
        return Ok(cwd.join("logs"));
    }

    let data_dir = crate::settings::core::runtime_data_dir()?;
    if let Some(parent) = data_dir.parent() {
        if parent.file_name().and_then(|s| s.to_str()) == Some("src-tauri") {
            if let Some(grand_parent) = parent.parent() {
                return Ok(grand_parent.join("logs"));
            }
        }
        return Ok(parent.join("logs"));
    }
    Ok(data_dir.join("..").join("logs"))
}
