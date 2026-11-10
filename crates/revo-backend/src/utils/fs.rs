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
    cwd.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s == name)
        .unwrap_or(false)
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

/// Bundle identifier used for the standalone (non-Tauri) data directory, so a
/// packaged GPUI install keeps the same per-user location the Tauri app used.
pub const STANDALONE_APP_ID: &str = "com.klubuntuprojects.revo";

/// Per-user data directory for a process that has no GUI framework to ask:
/// the environment variable wins, then the platform standard location.
fn user_data_dir_from(
    override_dir: Option<std::ffi::OsString>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(dir) = override_dir {
        let path = PathBuf::from(dir);
        if path.as_os_str() != "" {
            return Some(path);
        }
    }
    let home = home?;
    #[cfg(target_os = "macos")]
    {
        return Some(
            home.join("Library")
                .join("Application Support")
                .join(STANDALONE_APP_ID),
        );
    }
    #[cfg(target_os = "windows")]
    {
        return Some(home.join(STANDALONE_APP_ID));
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Some(home.join(".local").join("share").join(STANDALONE_APP_ID))
    }
}

fn standalone_home() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return Some(PathBuf::from(appdata));
    }
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Resource directory for a standalone process: inside a macOS `.app` the
/// runtime lives in `Contents/Resources`, elsewhere next to the binary.
fn standalone_resource_dir(exe_dir: &std::path::Path) -> PathBuf {
    if exe_dir.file_name().and_then(|name| name.to_str()) == Some("MacOS") {
        if let Some(parent) = exe_dir.parent() {
            let resources = parent.join("Resources");
            if resources.is_dir() {
                return resources;
            }
        }
    }
    exe_dir.to_path_buf()
}

/// Wires the paths a GUI adapter would normally provide. Called by the engine
/// process (and by any future standalone host) before it touches settings or
/// logs; a release build refuses to write without them.
pub fn init_standalone_app_paths() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("failed to locate exe: {e}"))?;
    let exe_dir = exe.parent().ok_or("executable has no parent directory")?;
    let data_dir = user_data_dir_from(std::env::var_os("REVO_DATA_DIR"), standalone_home())
        .ok_or("no per-user data directory is available")?;
    set_startup_cwd(
        std::env::current_dir().unwrap_or_else(|_| exe_dir.to_path_buf()),
    );
    set_app_paths(
        std::env::var_os("REVO_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| standalone_resource_dir(exe_dir)),
        data_dir,
    );
    Ok(())
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
        assert_eq!(
            runtime_app_data_dir_for(false, Some(&data)).unwrap(),
            Some(&data)
        );
        assert!(runtime_app_data_dir_for(false, None).is_err());
    }

    #[test]
    fn standalone_data_dir_prefers_the_override() {
        let home = PathBuf::from("/home/user");
        let override_dir = std::ffi::OsString::from("/mnt/revo-data");
        assert_eq!(
            user_data_dir_from(Some(override_dir), Some(home.clone())).unwrap(),
            PathBuf::from("/mnt/revo-data")
        );
    }

    #[test]
    fn standalone_data_dir_falls_back_to_the_platform_standard() {
        let home = PathBuf::from("/home/user");
        let resolved = user_data_dir_from(None, Some(home.clone()))
            .expect("a home directory resolves");
        #[cfg(target_os = "macos")]
        let expected = home.join("Library").join("Application Support");
        #[cfg(target_os = "windows")]
        let expected = home.clone();
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let expected = home.join(".local").join("share");
        assert_eq!(resolved, expected.join(STANDALONE_APP_ID));
    }

    #[test]
    fn standalone_data_dir_needs_a_home_or_an_override() {
        assert!(user_data_dir_from(None, None).is_none());
    }

    #[test]
    fn a_macos_bundle_takes_its_resources_directory() {
        let root = std::env::temp_dir().join(format!(
            "revo-app-paths-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let macos = root.join("Contents").join("MacOS");
        let resources = root.join("Contents").join("Resources");
        std::fs::create_dir_all(&macos).unwrap();
        std::fs::create_dir_all(&resources).unwrap();
        assert_eq!(standalone_resource_dir(&macos), resources);
        // A loose binary keeps its own directory as the resource root.
        assert_eq!(standalone_resource_dir(&root), root);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn development_keeps_existing_paths() {
        let data = PathBuf::from("/user/app-data");
        assert_eq!(runtime_app_data_dir_for(true, Some(&data)).unwrap(), None);
        assert_eq!(runtime_app_data_dir_for(true, None).unwrap(), None);
    }
}
