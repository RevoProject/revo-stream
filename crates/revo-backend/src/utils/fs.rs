use std::path::PathBuf;

static APP_START_CWD: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
static APP_RESOURCE_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
static APP_DATA_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

pub fn set_app_paths(resource_dir: PathBuf, data_dir: PathBuf) {
    let _ = APP_RESOURCE_DIR.set(resource_dir);
    let _ = APP_DATA_DIR.set(data_dir);
}

// Development-mode workspace anchor, injected by the GUI adapter (the legacy
// adapter's source directory name). unset for non-adapter embedders.
static ADAPTER_DEV_DIR_NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn set_adapter_dev_dir_name(name: &str) {
    let _ = ADAPTER_DEV_DIR_NAME.set(name.to_string());
}

pub fn adapter_dev_dir_name() -> Option<String> {
    ADAPTER_DEV_DIR_NAME.get().cloned()
}

/// Returns `Some(name)` when the startup cwd is the adapter's development
/// source directory.
fn startup_cwd_is_dev_dir(cwd: &std::path::Path) -> bool {
    let Some(name) = adapter_dev_dir_name() else {
        return false;
    };
    cwd.file_name().and_then(|s| s.to_str()).map(|s| s == name).unwrap_or(false)
}

pub fn resource_dir() -> Option<&'static PathBuf> {
    APP_RESOURCE_DIR.get()
}

pub fn app_data_dir() -> Option<&'static PathBuf> {
    APP_DATA_DIR.get()
}

fn runtime_app_data_dir_for(
    development: bool,
    data_dir: Option<&PathBuf>,
) -> Result<Option<&PathBuf>, String> {
    if development {
        return Ok(None);
    }
    data_dir
        .map(Some)
        .ok_or_else(|| "app data directory is not initialized".to_string())
}

pub fn runtime_app_data_dir() -> Result<Option<&'static PathBuf>, String> {
    runtime_app_data_dir_for(cfg!(debug_assertions), app_data_dir())
}

pub fn set_startup_cwd(path: PathBuf) {
    let _ = APP_START_CWD.set(path);
}

pub fn startup_cwd() -> Result<PathBuf, String> {
    if let Some(path) = APP_START_CWD.get() {
        return Ok(path.clone());
    }
    std::env::current_dir().map_err(|e| format!("failed to get current dir: {e}"))
}

pub fn runtime_logs_root_dir() -> Result<PathBuf, String> {
    if let Some(data_dir) = runtime_app_data_dir()? {
        return Ok(data_dir.join("logs"));
    }
    let cwd = startup_cwd()?;
    if startup_cwd_is_dev_dir(&cwd) {
        if let Some(parent) = cwd.parent() {
            return Ok(parent.join("logs"));
        }
        return Ok(cwd.join("..").join("logs"));
    }

    if let Some(dev_name) = adapter_dev_dir_name() {
        let dev_dir = cwd.join(&dev_name);
        if dev_dir.is_dir() {
            return Ok(cwd.join("logs"));
        }
    }

    let data_dir = crate::settings::core::runtime_data_dir()?;
    if let Some(parent) = data_dir.parent() {
        if startup_cwd_is_dev_dir(parent) {
            if let Some(grand_parent) = parent.parent() {
                return Ok(grand_parent.join("logs"));
            }
        }
        return Ok(parent.join("logs"));
    }
    Ok(data_dir.join("..").join("logs"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_requires_initialized_app_data_without_cwd_fallback() {
        let data = PathBuf::from("/user/app-data");
        assert_eq!(runtime_app_data_dir_for(false, Some(&data)).unwrap(), Some(&data));
        assert!(runtime_app_data_dir_for(false, None).is_err());
    }

    #[test]
    fn development_keeps_existing_paths() {
        let data = PathBuf::from("/user/app-data");
        assert_eq!(runtime_app_data_dir_for(true, Some(&data)).unwrap(), None);
        assert_eq!(runtime_app_data_dir_for(true, None).unwrap(), None);
    }
}
