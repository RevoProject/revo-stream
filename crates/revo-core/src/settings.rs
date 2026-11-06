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
