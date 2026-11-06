use crate::models::AppSettings;

pub fn user_profile_dir() -> Option<std::path::PathBuf> {
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

pub fn default_record_path() -> String {
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

pub fn default_preview_quality() -> Option<String> {
    Some("medium".to_string())
}

pub fn default_encoder_preference() -> Option<String> {
    Some("hardware".to_string())
}

pub fn default_scene_resolution() -> Option<String> {
    Some("1920x1080".to_string())
}

pub fn default_whep_url() -> Option<String> {
    Some("http://127.0.0.1:8080/whep".to_string())
}

pub fn default_whip_url() -> Option<String> {
    Some("http://127.0.0.1:8080/whip".to_string())
}

pub fn default_active_profile() -> Option<String> {
    Some("default".to_string())
}

/// Normalizes a profile name to a filesystem-safe identifier: trims the
/// input, keeps ASCII alphanumerics, hyphens and underscores, converts other
/// whitespace to underscores, collapses repeated underscores, and trims
/// leading/trailing underscores.
pub fn sanitize_profile_name(name: &str) -> String {
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

pub fn default_settings() -> AppSettings {
    AppSettings {
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
        minimize_to_tray: Some(false),
        built_in_browser_enabled: Some(false),
        obs_plugins_path: None,
        obs_vst_enabled: Some(false),
        obs_websocket_enabled: Some(false),
        obs_websocket_password: None,
        ui_profile: None,
        active_profile: default_active_profile(),
    }
}

pub fn normalize_optional_trimmed(value: &mut Option<String>) {
    if let Some(raw) = value {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            *value = None;
        } else if trimmed != raw.as_str() {
            *raw = trimmed.to_string();
        }
    }
}

pub fn is_valid_resolution(value: &str) -> bool {
    crate::planner::parse_resolution(value).is_some()
}

pub fn normalize_settings(settings: &mut AppSettings) {
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
    settings.preview_quality = if matches!(
        preview_quality.as_str(),
        "very_low" | "low" | "medium" | "high"
    ) {
        Some(preview_quality)
    } else {
        default_preview_quality()
    };

    let encoder_pref = settings
        .encoder_preference
        .as_deref()
        .map(|v| v.trim().to_lowercase())
        .unwrap_or_else(|| "hardware".to_string());
    settings.encoder_preference = if matches!(
        encoder_pref.as_str(),
        "hardware" | "nvenc" | "nvenc_legacy" | "software"
    ) {
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
    settings.minimize_to_tray = Some(settings.minimize_to_tray.unwrap_or(false));
    settings.built_in_browser_enabled = Some(settings.built_in_browser_enabled.unwrap_or(false));
    settings.obs_vst_enabled = Some(settings.obs_vst_enabled.unwrap_or(false));
    settings.obs_websocket_enabled = Some(settings.obs_websocket_enabled.unwrap_or(false));

    let websocket_password = settings
        .obs_websocket_password
        .as_deref()
        .unwrap_or("")
        .trim()
        .replace(['\r', '\n'], "")
        .trim()
        .to_string();
    settings.obs_websocket_password = if websocket_password.is_empty() {
        None
    } else {
        Some(websocket_password)
    };

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

pub fn parse_settings_raw(raw: &str) -> AppSettings {
    let parsed_value = match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(v) => v,
        Err(_) => {
            let mut settings = default_settings();
            normalize_settings(&mut settings);
            return settings;
        }
    };

    if let Ok(mut typed) = serde_json::from_value::<AppSettings>(parsed_value.clone()) {
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
        if let Some(v) = obj.get("minimize_to_tray").and_then(|v| v.as_bool()) {
            settings.minimize_to_tray = Some(v);
        }
        if let Some(v) = obj
            .get("built_in_browser_enabled")
            .and_then(|v| v.as_bool())
        {
            settings.built_in_browser_enabled = Some(v);
        }
        if let Some(v) = obj.get("obs_plugins_path").and_then(|v| v.as_str()) {
            settings.obs_plugins_path = Some(v.to_string());
        }
        if let Some(v) = obj.get("obs_vst_enabled").and_then(|v| v.as_bool()) {
            settings.obs_vst_enabled = Some(v);
        }
        if let Some(v) = obj.get("obs_websocket_enabled").and_then(|v| v.as_bool()) {
            settings.obs_websocket_enabled = Some(v);
        }
        if let Some(v) = obj.get("obs_websocket_password").and_then(|v| v.as_str()) {
            settings.obs_websocket_password = Some(v.to_string());
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn default_settings_match_current_behavior() {
        let settings = default_settings();

        assert_eq!(settings.record_path, default_record_path());
        assert!(settings.record_path.ends_with("record.mp4"));
        assert_eq!(settings.root_dir, None);
        assert_eq!(settings.stream_url, "");
        assert_eq!(settings.stream_key, "");
        assert_eq!(settings.preview_quality.as_deref(), Some("medium"));
        assert_eq!(settings.encoder_preference.as_deref(), Some("hardware"));
        assert_eq!(settings.scene_resolution.as_deref(), Some("1920x1080"));
        assert_eq!(
            settings.whep_url.as_deref(),
            Some("http://127.0.0.1:8080/whep")
        );
        assert_eq!(
            settings.whip_url.as_deref(),
            Some("http://127.0.0.1:8080/whip")
        );
        assert_eq!(settings.auto_retry_preview, Some(true));
        assert_eq!(settings.autorescale_inputs, Some(false));
        assert_eq!(settings.minimize_to_tray, Some(false));
        assert_eq!(settings.built_in_browser_enabled, Some(false));
        assert_eq!(settings.obs_plugins_path, None);
        assert_eq!(settings.obs_vst_enabled, Some(false));
        assert_eq!(settings.obs_websocket_enabled, Some(false));
        assert_eq!(settings.obs_websocket_password, None);
        assert_eq!(settings.ui_profile, None);
        assert_eq!(settings.active_profile.as_deref(), Some("default"));
    }

    #[test]
    fn serde_defaults_apply_only_to_missing_fields() {
        let settings: AppSettings =
            serde_json::from_value(json!({})).expect("empty settings should deserialize");

        assert_eq!(settings.record_path, default_record_path());
        assert_eq!(settings.preview_quality.as_deref(), Some("medium"));
        assert_eq!(settings.active_profile.as_deref(), Some("default"));
        assert_eq!(settings.encoder_preference, None);
        assert_eq!(settings.scene_resolution, None);
        assert_eq!(settings.whep_url, None);
        assert_eq!(settings.whip_url, None);
        assert_eq!(settings.stream_url, "");
        assert_eq!(settings.stream_key, "");
        assert_eq!(settings.root_dir, None);
        assert_eq!(settings.auto_retry_preview, None);
        assert_eq!(settings.ui_profile, None);
    }

    #[test]
    fn sanitize_profile_name_matches_current_behavior() {
        assert_eq!(sanitize_profile_name("  My Profile  "), "My_Profile");
        assert_eq!(sanitize_profile_name("a__b"), "a_b");
        assert_eq!(sanitize_profile_name("win-dshow"), "win-dshow");
        assert_eq!(sanitize_profile_name("_leading_"), "leading");
        assert_eq!(sanitize_profile_name("!!!"), "");
        assert_eq!(sanitize_profile_name("   "), "");
        assert_eq!(sanitize_profile_name("Ähem"), "hem");
    }

    #[test]
    fn normalize_settings_trims_and_defaults_all_supported_values() {
        let mut settings = default_settings();
        settings.root_dir = Some("  /opt/revo  ".into());
        settings.record_path = "   ".into();
        settings.stream_url = "  rtmp://example.test/live  ".into();
        settings.stream_key = "  key-1  ".into();
        settings.preview_quality = Some(" HIGH ".into());
        settings.encoder_preference = Some(" NVENC_LEGACY ".into());
        settings.scene_resolution = Some(" 1280x720 ".into());
        settings.whep_url = Some("   ".into());
        settings.whip_url = Some("  http://example.test/whip  ".into());
        settings.auto_retry_preview = None;
        settings.autorescale_inputs = None;
        settings.minimize_to_tray = None;
        settings.built_in_browser_enabled = None;
        settings.obs_vst_enabled = None;
        settings.obs_websocket_enabled = None;
        settings.obs_websocket_password = Some("  line1\r\nline2  ".into());
        settings.ui_profile = Some(json!(["not", "an", "object"]));
        settings.active_profile = Some("  My   Profile!  ".into());

        normalize_settings(&mut settings);

        assert_eq!(settings.root_dir.as_deref(), Some("/opt/revo"));
        assert_eq!(settings.record_path, default_record_path());
        assert_eq!(settings.stream_url, "rtmp://example.test/live");
        assert_eq!(settings.stream_key, "key-1");
        assert_eq!(settings.preview_quality.as_deref(), Some("high"));
        assert_eq!(settings.encoder_preference.as_deref(), Some("nvenc_legacy"));
        assert_eq!(settings.scene_resolution.as_deref(), Some("1280x720"));
        assert_eq!(settings.whep_url, default_whep_url());
        assert_eq!(
            settings.whip_url.as_deref(),
            Some("http://example.test/whip")
        );
        assert_eq!(settings.auto_retry_preview, Some(true));
        assert_eq!(settings.autorescale_inputs, Some(false));
        assert_eq!(settings.minimize_to_tray, Some(false));
        assert_eq!(settings.built_in_browser_enabled, Some(false));
        assert_eq!(settings.obs_vst_enabled, Some(false));
        assert_eq!(settings.obs_websocket_enabled, Some(false));
        assert_eq!(
            settings.obs_websocket_password.as_deref(),
            Some("line1line2")
        );
        assert_eq!(settings.ui_profile, None);
        assert_eq!(settings.active_profile.as_deref(), Some("My_Profile"));

        settings.root_dir = Some(" \t ".into());
        settings.preview_quality = Some("invalid".into());
        settings.encoder_preference = Some("invalid".into());
        settings.scene_resolution = Some("invalid".into());
        settings.obs_websocket_password = Some(" \r\n ".into());
        settings.ui_profile = Some(json!({"general": {}}));
        settings.active_profile = Some("!!!".into());

        normalize_settings(&mut settings);

        assert_eq!(settings.root_dir, None);
        assert_eq!(settings.preview_quality, default_preview_quality());
        assert_eq!(settings.encoder_preference, default_encoder_preference());
        assert_eq!(settings.scene_resolution, default_scene_resolution());
        assert_eq!(settings.obs_websocket_password, None);
        assert_eq!(settings.ui_profile, Some(json!({"general": {}})));
        assert_eq!(settings.active_profile, default_active_profile());
    }

    #[test]
    fn normalize_settings_is_idempotent() {
        let mut settings = parse_settings_raw(
            r#"{
                "root_dir": " /opt/revo ",
                "stream_url": " rtmp://example.test/live ",
                "preview_quality": " HIGH ",
                "obs_websocket_password": " secret\nvalue ",
                "active_profile": " My Profile "
            }"#,
        );
        let once = serde_json::to_value(&settings).expect("settings should serialize");

        normalize_settings(&mut settings);

        assert_eq!(
            serde_json::to_value(settings).expect("settings should serialize"),
            once
        );
    }

    #[test]
    fn malformed_json_falls_back_to_normalized_defaults() {
        let parsed = parse_settings_raw(r#"{"stream_url":"unterminated"#);
        let mut expected = default_settings();
        normalize_settings(&mut expected);

        assert_eq!(
            serde_json::to_value(parsed).expect("settings should serialize"),
            serde_json::to_value(expected).expect("settings should serialize")
        );
    }

    #[test]
    fn non_object_json_falls_back_to_normalized_defaults() {
        let parsed = parse_settings_raw("42");
        let mut expected = default_settings();
        normalize_settings(&mut expected);

        assert_eq!(
            serde_json::to_value(parsed).expect("settings should serialize"),
            serde_json::to_value(expected).expect("settings should serialize")
        );
    }

    #[test]
    fn wrong_field_type_recovers_other_valid_fields() {
        let settings = parse_settings_raw(
            r#"{
                "record_path": "/tmp/capture.mp4",
                "stream_url": 42,
                "stream_key": " recovered-key ",
                "preview_quality": " LOW ",
                "auto_retry_preview": false,
                "obs_websocket_password": " pass\nword ",
                "ui_profile": {"general": {"theme": "RevoFuture"}},
                "active_profile": " Recovered Profile "
            }"#,
        );

        assert_eq!(settings.record_path, "/tmp/capture.mp4");
        assert_eq!(settings.stream_url, "");
        assert_eq!(settings.stream_key, "recovered-key");
        assert_eq!(settings.preview_quality.as_deref(), Some("low"));
        assert_eq!(settings.auto_retry_preview, Some(false));
        assert_eq!(settings.obs_websocket_password.as_deref(), Some("password"));
        assert_eq!(
            settings.ui_profile,
            Some(json!({"general": {"theme": "RevoFuture"}}))
        );
        assert_eq!(
            settings.active_profile.as_deref(),
            Some("Recovered_Profile")
        );
    }

    #[test]
    fn settings_preserve_complete_wire_shape() {
        let settings = AppSettings {
            root_dir: Some("/opt/revo".into()),
            record_path: "/tmp/record.mp4".into(),
            stream_url: "rtmp://example.test/live".into(),
            stream_key: "key-1".into(),
            preview_quality: Some("high".into()),
            encoder_preference: Some("software".into()),
            scene_resolution: Some("1280x720".into()),
            whep_url: Some("http://127.0.0.1:9000/whep".into()),
            whip_url: None,
            auto_retry_preview: Some(false),
            autorescale_inputs: Some(true),
            minimize_to_tray: Some(true),
            built_in_browser_enabled: Some(true),
            obs_plugins_path: Some("/opt/plugins".into()),
            obs_vst_enabled: Some(true),
            obs_websocket_enabled: Some(true),
            obs_websocket_password: Some("secret".into()),
            ui_profile: Some(json!({"general": {}})),
            active_profile: Some("streaming".into()),
        };

        assert_eq!(
            serde_json::to_value(settings).expect("settings should serialize"),
            json!({
                "root_dir": "/opt/revo",
                "record_path": "/tmp/record.mp4",
                "stream_url": "rtmp://example.test/live",
                "stream_key": "key-1",
                "preview_quality": "high",
                "encoder_preference": "software",
                "scene_resolution": "1280x720",
                "whep_url": "http://127.0.0.1:9000/whep",
                "whip_url": null,
                "auto_retry_preview": false,
                "autorescale_inputs": true,
                "minimize_to_tray": true,
                "built_in_browser_enabled": true,
                "obs_plugins_path": "/opt/plugins",
                "obs_vst_enabled": true,
                "obs_websocket_enabled": true,
                "obs_websocket_password": "secret",
                "ui_profile": {"general": {}},
                "active_profile": "streaming"
            })
        );
    }
}
