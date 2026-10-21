// macOS privacy permissions (TCC) helpers powering the first-run onboarding.

#[derive(serde::Serialize)]
pub struct PermissionsStatus {
    pub macos: bool,
    pub screen_capture: bool,
    pub accessibility: bool,
}

#[cfg(target_os = "macos")]
mod imp {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }

    pub fn preflight_screen_capture() -> bool {
        unsafe { CGPreflightScreenCaptureAccess() }
    }

    pub fn request_screen_capture() -> bool {
        // Posts the macOS Screen Recording consent dialog; safe off the main
        // thread, the system presents its own window.
        unsafe { CGRequestScreenCaptureAccess() }
    }

    pub fn accessibility_trusted() -> bool {
        unsafe { AXIsProcessTrusted() }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn preflight_screen_capture() -> bool {
        true
    }

    pub fn request_screen_capture() -> bool {
        true
    }

    pub fn accessibility_trusted() -> bool {
        true
    }
}

pub(crate) fn status() -> PermissionsStatus {
    PermissionsStatus {
        macos: cfg!(target_os = "macos"),
        screen_capture: imp::preflight_screen_capture(),
        accessibility: imp::accessibility_trusted(),
    }
}

#[tauri::command]
pub fn macos_permissions_status() -> PermissionsStatus {
    status()
}

#[tauri::command]
pub fn macos_request_screen_capture() -> PermissionsStatus {
    #[cfg(target_os = "macos")]
    {
        imp::request_screen_capture();
    }
    status()
}

#[tauri::command]
pub fn macos_open_privacy_pane(pane: String) -> Result<(), String> {
    let allowed = [
        "ScreenCapture",
        "Microphone",
        "Camera",
        "Accessibility",
        "ListenEvent",
        "AllFiles",
    ];
    if !allowed.contains(&pane.as_str()) {
        return Err(format!("unsupported privacy pane: {pane}"));
    }
    let url = format!("x-apple.systempreferences:com.apple.preference.security?Privacy_{pane}");
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("failed to open System Settings: {e}"))?;
        return Ok(());
    }
    #[cfg(not(target_os = "macos"))]
    Err("privacy panes are only available on macOS".to_string())
}
