//! Typed draft state for the native settings modal.
//!
//! The draft mirrors `revo_core::models::AppSettings` with a saved baseline so
//! dirty tracking is a value comparison. Field setters normalize and validate
//! exactly like the legacy Svelte modal did (URL scheme checks, resolution
//! parsing, encoder/quality option membership).

use revo_core::models::AppSettings;
use revo_core::planner::parse_resolution;

pub use crate::components::text_field::apply_line_keystroke as apply_field_keystroke;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsSection {
    General,
    Look,
    Broadcast,
    Output,
    Audio,
    Video,
    Shortcuts,
    Accessibility,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputTab {
    Streaming,
    Recording,
    Audio,
}

/// Editable text fields; boolean and option fields are toggled/cycled
/// directly and need no keystroke routing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextField {
    StreamUrl,
    StreamKey,
    RecordPath,
    SceneResolution,
    ObsPluginsPath,
    ObsWebsocketPassword,
    ActiveProfile,
    RootDir,
    /// Path used by scene import/export actions (not persisted).
    ImportExportPath,
    /// A `ui_profile`-backed key (encoder settings, audio channels,
    /// shortcuts, accessibility…). Stored as strings inside the profile
    /// document so new options persist without schema churn.
    ProfileKey(&'static str),
}

#[derive(Clone, Debug, PartialEq)]
pub enum SettingsEvent {
    /// The user confirmed the draft; carries the normalized settings to save.
    Saved(AppSettings),
    Cancelled,
    /// Export all scenes to a RevoStream collection JSON file at the given path.
    ExportRevoSceneCollection {
        path: String,
    },
    /// Import scenes from a RevoStream collection JSON file at the given path.
    ImportRevoSceneCollection {
        path: String,
    },
    /// Export all scenes as an OBS scene collection JSON file at the given path.
    ExportObsSceneCollection {
        path: String,
    },
    /// Import scenes from an OBS scene collection JSON file at the given path.
    ImportObsSceneCollection {
        path: String,
    },
    /// Extract the default theme under the selected root directory (plan §4.2).
    ExtractDefaultTheme {
        root_dir: String,
    },
    /// Export the active OBS profile to the given file path (plan §11.2).
    ExportProfile {
        path: String,
    },
    /// Import an OBS profile from the given file path (plan §11.2).
    ImportProfile {
        path: String,
    },
}

pub const PREVIEW_QUALITIES: [&str; 3] = ["low", "medium", "high"];
pub const ENCODER_PREFERENCES: [&str; 4] = ["hardware", "nvenc", "nvenclegacy", "software"];

#[derive(Debug)]
pub struct SettingsDraft {
    settings: AppSettings,
    saved: AppSettings,
    pub active_section: SettingsSection,
    pub output_tab: OutputTab,
    pub field_error: Option<String>,
    /// Transient modal notice (import/export status, restart hints); not
    /// persisted.
    pub notice: Option<String>,
    /// Path used by scene import/export actions; not persisted.
    pub import_export_path: String,
}

impl SettingsDraft {
    pub fn load(settings: AppSettings) -> Self {
        Self {
            saved: settings.clone(),
            settings,
            active_section: SettingsSection::General,
            output_tab: OutputTab::Streaming,
            field_error: None,
            notice: None,
            import_export_path: String::new(),
        }
    }

    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }

    pub fn is_dirty(&self) -> bool {
        self.settings != self.saved
    }

    pub fn select_section(&mut self, section: SettingsSection) {
        self.notice = None;
        self.active_section = section;
    }

    pub fn select_output_tab(&mut self, tab: OutputTab) {
        self.notice = None;
        self.output_tab = tab;
    }

    /// Applies one keystroke to a text field, marking the draft dirty.
    pub fn field_keystroke(
        &mut self,
        field: TextField,
        key: &str,
        key_char: Option<&str>,
        control: bool,
        alt: bool,
        platform: bool,
    ) {
        let current = self.field_text(field);
        if let Some(next) = apply_field_keystroke(&current, key, key_char, control, alt, platform) {
            self.set_field_text(field, next);
        }
    }

    pub fn field_text(&self, field: TextField) -> String {
        match field {
            TextField::StreamUrl => self.settings.stream_url.clone(),
            TextField::StreamKey => self.settings.stream_key.clone(),
            TextField::RecordPath => self.settings.record_path.clone(),
            TextField::SceneResolution => self
                .settings
                .scene_resolution
                .as_deref()
                .unwrap_or("")
                .to_string(),
            TextField::ObsPluginsPath => self
                .settings
                .obs_plugins_path
                .as_deref()
                .unwrap_or("")
                .to_string(),
            TextField::ObsWebsocketPassword => self
                .settings
                .obs_websocket_password
                .as_deref()
                .unwrap_or("")
                .to_string(),
            TextField::ActiveProfile => self
                .settings
                .active_profile
                .as_deref()
                .unwrap_or("")
                .to_string(),
            TextField::RootDir => self.settings.root_dir.as_deref().unwrap_or("").to_string(),
            TextField::ImportExportPath => self.import_export_path.clone(),
            TextField::ProfileKey(key) => self.profile_value(key),
        }
    }

    pub fn set_field_text(&mut self, field: TextField, value: String) {
        self.field_error = None;
        match field {
            // Scheme checks happen at commit time; validating per keystroke
            // would reject every partial value while typing.
            TextField::StreamUrl => self.settings.stream_url = value,
            TextField::StreamKey => self.settings.stream_key = value,
            TextField::RecordPath => {
                let trimmed = value.trim().to_string();
                if trimmed.is_empty() {
                    self.field_error = Some("record path is required".into());
                } else {
                    self.settings.record_path = trimmed;
                }
            }
            // WidthxHEIGHT checks happen at commit time; validating per
            // keystroke would reject every partial value while typing.
            TextField::SceneResolution => {
                let trimmed = value.trim().to_string();
                self.settings.scene_resolution = (!trimmed.is_empty()).then_some(trimmed);
            }
            TextField::ObsPluginsPath => {
                let trimmed = value.trim().to_string();
                self.settings.obs_plugins_path = (!trimmed.is_empty()).then_some(trimmed);
            }
            TextField::ObsWebsocketPassword => {
                let trimmed = value.trim().to_string();
                self.settings.obs_websocket_password = (!trimmed.is_empty()).then_some(trimmed);
            }
            TextField::ActiveProfile => {
                let trimmed = value.trim().to_string();
                if trimmed.is_empty() {
                    self.field_error = Some("active profile is required".into());
                } else {
                    self.settings.active_profile = Some(trimmed);
                }
            }
            TextField::RootDir => {
                let trimmed = value.trim().to_string();
                self.settings.root_dir = (!trimmed.is_empty()).then_some(trimmed);
            }
            TextField::ImportExportPath => {
                self.import_export_path = value.trim().to_string();
            }
            TextField::ProfileKey(key) => {
                self.set_profile_value(key, value);
            }
        }
    }

    /// Reads one `ui_profile`-backed value as a string (empty when unset).
    pub fn profile_value(&self, key: &str) -> String {
        match self.settings.ui_profile.as_ref() {
            Some(serde_json::Value::Object(map)) => map
                .get(key)
                .and_then(|value| match value {
                    serde_json::Value::String(text) => Some(text.clone()),
                    serde_json::Value::Bool(flag) => Some(flag.to_string()),
                    serde_json::Value::Number(number) => Some(number.to_string()),
                    _ => None,
                })
                .unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// Writes one `ui_profile`-backed value as a string.
    pub fn set_profile_value(&mut self, key: &str, value: String) {
        let profile = self
            .settings
            .ui_profile
            .get_or_insert_with(|| serde_json::Value::Object(Default::default()));
        if !profile.is_object() {
            *profile = serde_json::Value::Object(Default::default());
        }
        if let serde_json::Value::Object(map) = profile {
            map.insert(key.to_owned(), serde_json::Value::String(value));
        }
        self.field_error = None;
    }

    /// Flips a `ui_profile`-backed boolean ("true"/"false").
    pub fn toggle_profile(&mut self, key: &str) {
        let next = !self.profile_value(key).eq_ignore_ascii_case("true");
        self.set_profile_value(key, next.to_string());
    }

    /// Cycles a `ui_profile`-backed value through `options`.
    pub fn cycle_profile(&mut self, key: &str, options: &[&str]) {
        if options.is_empty() {
            return;
        }
        let current = self.profile_value(key);
        let index = options
            .iter()
            .position(|option| option.eq_ignore_ascii_case(&current))
            .map_or(0, |index| (index + 1) % options.len());
        self.set_profile_value(key, options[index].to_string());
    }

    /// Cycles the streaming/recording video encoder preference through the
    /// legacy option list.
    pub fn cycle_encoder_preference(&mut self) {
        let current = self
            .settings
            .encoder_preference
            .as_deref()
            .unwrap_or("hardware");
        let index = ENCODER_PREFERENCES
            .iter()
            .position(|option| *option == current)
            .map(|index| (index + 1) % ENCODER_PREFERENCES.len())
            .unwrap_or(0);
        self.settings.encoder_preference = Some(ENCODER_PREFERENCES[index].to_string());
    }

    /// Cycles the preview quality through low/medium/high.
    pub fn cycle_preview_quality(&mut self) {
        let current = self.settings.preview_quality.as_deref().unwrap_or("medium");
        let index = PREVIEW_QUALITIES
            .iter()
            .position(|option| *option == current)
            .map(|index| (index + 1) % PREVIEW_QUALITIES.len())
            .unwrap_or(1);
        self.settings.preview_quality = Some(PREVIEW_QUALITIES[index].to_string());
    }

    /// Boolean field values as currently drafted; `None` reads as `false`.
    pub fn bool_value(&self, read: impl Fn(&AppSettings) -> Option<bool>) -> bool {
        read(&self.settings).unwrap_or(false)
    }

    /// Toggles a boolean field where `None` reads as `false` (the serde
    /// default for optional flags), writing an explicit value.
    fn toggle_flag(
        &mut self,
        read: impl Fn(&AppSettings) -> Option<bool>,
        write: impl Fn(&mut AppSettings, bool),
    ) {
        self.field_error = None;
        let current = read(&self.settings).unwrap_or(false);
        write(&mut self.settings, !current);
    }

    pub fn toggle_auto_retry_preview(&mut self) {
        self.toggle_flag(
            |s| s.auto_retry_preview,
            |s, v| s.auto_retry_preview = Some(v),
        );
    }

    pub fn toggle_autorescale_inputs(&mut self) {
        self.toggle_flag(
            |s| s.autorescale_inputs,
            |s, v| s.autorescale_inputs = Some(v),
        );
    }

    pub fn toggle_minimize_to_tray(&mut self) {
        self.toggle_flag(|s| s.minimize_to_tray, |s, v| s.minimize_to_tray = Some(v));
    }

    pub fn toggle_built_in_browser(&mut self) {
        self.toggle_flag(
            |s| s.built_in_browser_enabled,
            |s, v| s.built_in_browser_enabled = Some(v),
        );
    }

    pub fn toggle_obs_vst(&mut self) {
        self.toggle_flag(|s| s.obs_vst_enabled, |s, v| s.obs_vst_enabled = Some(v));
    }

    pub fn toggle_obs_websocket(&mut self) {
        self.toggle_flag(
            |s| s.obs_websocket_enabled,
            |s, v| s.obs_websocket_enabled = Some(v),
        );
    }

    /// Confirms the draft: returns the settings to persist and rebases the
    /// dirty baseline onto them.
    pub fn commit(&mut self) -> Result<AppSettings, String> {
        if let Some(message) = &self.field_error {
            return Err(message.clone());
        }
        if let Err(message) = validate_url(&self.settings.stream_url) {
            return Err(message);
        }
        if let Some(resolution) = &self.settings.scene_resolution {
            if parse_resolution(resolution).is_none() {
                return Err("resolution must be WIDTHxHEIGHT, e.g. 1920x1080".into());
            }
        }
        let settings = self.settings.clone();
        self.saved = settings.clone();
        Ok(settings)
    }

    /// Discards edits and restores the saved baseline.
    pub fn cancel(&mut self) {
        self.settings = self.saved.clone();
        self.field_error = None;
    }

    /// Restores every field to the framework-neutral defaults.
    pub fn reset_defaults(&mut self) {
        self.settings = revo_core::settings::default_settings();
        self.field_error = None;
    }
}

/// Validates an http/https URL for streaming/WHEP/WHIP fields; empty is
/// allowed (unset).
pub fn validate_url(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        Ok(())
    } else {
        Err("URL must start with http:// or https://".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revo_core::settings::default_settings;

    fn draft() -> SettingsDraft {
        SettingsDraft::load(default_settings())
    }

    #[test]
    fn load_is_clean_and_commit_rebases() {
        let mut state = draft();
        assert!(!state.is_dirty());

        state.settings.record_path = "/tmp/other.mp4".into();
        assert!(state.is_dirty());

        let saved = state.commit().expect("draft should commit");
        assert_eq!(saved.record_path, "/tmp/other.mp4");
        assert!(!state.is_dirty());
    }

    #[test]
    fn cancel_restores_saved_baseline() {
        let mut state = draft();
        state.settings.stream_url = "rtmp://bad.example".into();
        state.cancel();
        assert_eq!(state.settings.stream_url, default_settings().stream_url);
        assert!(!state.is_dirty());
    }

    #[test]
    fn url_fields_reject_non_http_schemes() {
        assert!(validate_url("rtmp://example/live").is_err());
        assert!(validate_url("https://example/live").is_ok());
        assert!(validate_url("http://127.0.0.1:8080/whep").is_ok());
        assert!(validate_url("   ").is_ok());
    }

    #[test]
    fn field_keystroke_routes_into_stream_key() {
        let mut state = draft();
        state.field_keystroke(TextField::StreamKey, "k", Some("k"), false, false, false);
        assert_eq!(state.settings.stream_key, "k");
        assert!(state.is_dirty());
    }

    #[test]
    fn resolution_field_accepts_partial_typing_and_validates_at_commit() {
        let mut state = draft();
        // Typing must be free: partial values are accepted while typing and
        // the WIDTHxHEIGHT check happens at commit time.
        state.field_keystroke(
            TextField::SceneResolution,
            "x",
            Some("x"),
            false,
            false,
            false,
        );
        assert_eq!(
            state.settings.scene_resolution.as_deref(),
            Some("1920x1080x")
        );
        assert!(state.commit().is_err());

        state.set_field_text(TextField::SceneResolution, " 1280x720 ".into());
        assert_eq!(state.settings.scene_resolution.as_deref(), Some("1280x720"));

        state.set_field_text(TextField::SceneResolution, String::new());
        assert_eq!(state.settings.scene_resolution, None);
    }

    #[test]
    fn url_field_accepts_partial_typing_and_validates_at_commit() {
        let mut state = draft();
        // Each intermediate keystroke of a URL must be accepted while typing.
        for ch in "https://".chars() {
            state.field_keystroke(
                TextField::StreamUrl,
                &ch.to_string(),
                Some(&ch.to_string()),
                false,
                false,
                false,
            );
        }
        assert_eq!(state.settings.stream_url, "https://");

        state.field_keystroke(TextField::StreamUrl, "e", Some("e"), false, false, false);
        assert_eq!(state.settings.stream_url, "https://e");
        assert!(state.commit().is_ok());

        state.set_field_text(TextField::StreamUrl, "rtmp://example/live".into());
        assert!(state.commit().is_err());

        state.set_field_text(TextField::StreamUrl, "https://example/live".into());
        assert!(state.commit().is_ok());
    }

    #[test]
    fn cycling_options_matches_legacy_lists() {
        let mut state = draft();
        state.settings.encoder_preference = Some("software".into());
        state.cycle_encoder_preference();
        assert_eq!(
            state.settings.encoder_preference.as_deref(),
            Some("hardware")
        );

        state.settings.preview_quality = Some("high".into());
        state.cycle_preview_quality();
        assert_eq!(state.settings.preview_quality.as_deref(), Some("low"));
    }

    #[test]
    fn boolean_flags_toggle_from_none_semantics() {
        let mut state = draft();
        assert_eq!(state.settings.auto_retry_preview, Some(true));
        state.toggle_auto_retry_preview();
        assert_eq!(state.settings.auto_retry_preview, Some(false));
        state.toggle_auto_retry_preview();
        assert_eq!(state.settings.auto_retry_preview, Some(true));

        // None reads as false, so one toggle writes Some(true).
        assert_eq!(state.settings.minimize_to_tray, Some(false));
        state.settings.minimize_to_tray = None;
        state.toggle_minimize_to_tray();
        assert_eq!(state.settings.minimize_to_tray, Some(true));
    }

    #[test]
    fn reset_defaults_restores_framework_defaults() {
        let mut state = draft();
        state.settings.record_path = "/tmp/other.mp4".into();
        state.reset_defaults();
        assert_eq!(state.settings.record_path, default_settings().record_path);
        assert!(!state.is_dirty());
    }
}
