use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AppSettings {
    #[serde(default)]
    pub root_dir: Option<String>,
    #[serde(default = "crate::settings::default_record_path")]
    pub record_path: String,
    #[serde(default)]
    pub stream_url: String,
    #[serde(default)]
    pub stream_key: String,
    #[serde(default = "crate::settings::default_preview_quality")]
    pub preview_quality: Option<String>,
    #[serde(default)]
    pub encoder_preference: Option<String>,
    #[serde(default)]
    pub scene_resolution: Option<String>,
    #[serde(default)]
    pub whep_url: Option<String>,
    #[serde(default)]
    pub whip_url: Option<String>,
    #[serde(default)]
    pub auto_retry_preview: Option<bool>,
    #[serde(default)]
    pub autorescale_inputs: Option<bool>,
    #[serde(default)]
    pub minimize_to_tray: Option<bool>,
    #[serde(default)]
    pub built_in_browser_enabled: Option<bool>,
    #[serde(default)]
    pub obs_plugins_path: Option<String>,
    #[serde(default)]
    pub obs_vst_enabled: Option<bool>,
    #[serde(default)]
    pub obs_websocket_enabled: Option<bool>,
    #[serde(default)]
    pub obs_websocket_password: Option<String>,
    #[serde(default)]
    pub ui_profile: Option<JsonValue>,
    #[serde(default = "crate::settings::default_active_profile")]
    pub active_profile: Option<String>,
}

#[derive(Serialize)]
pub struct SceneInfo {
    pub name: String,
    pub active: bool,
    pub locked: bool,
}

#[derive(Serialize)]
pub struct ThemeInfo {
    pub id: String,
    pub name: String,
    pub author: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub file_name: String,
    pub module_name: String,
    pub window_capabilities: PluginWindowCapabilities,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginWindowCapabilities {
    pub can_list: bool,
    pub can_open: bool,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginProfile {
    pub enabled_modules: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PluginProfileState {
    pub profile_name: String,
    pub enabled_modules: Vec<String>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoEncoderPreference {
    Hardware,
    Nvenc,
    NvencLegacy,
    Software,
}

impl Default for VideoEncoderPreference {
    fn default() -> Self {
        VideoEncoderPreference::Hardware
    }
}

#[derive(Serialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Clone)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

#[derive(Serialize)]
pub struct ObsSettingsSelectOptions {
    pub streaming_audio_encoders: Vec<SelectOption>,
    pub streaming_video_encoders: Vec<SelectOption>,
    pub recording_audio_encoders: Vec<SelectOption>,
    pub recording_video_encoders: Vec<SelectOption>,
    pub recording_formats: Vec<SelectOption>,
    pub audio_sample_rates: Vec<SelectOption>,
    pub audio_channels: Vec<SelectOption>,
    pub video_resolutions: Vec<SelectOption>,
    pub video_downscale_filters: Vec<SelectOption>,
    pub video_common_fps: Vec<SelectOption>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SourceInfo {
    pub id: String,
    pub name: String,
    pub visible: bool,
    pub source_type: String,
    #[serde(default)]
    pub audio_capable: bool,
    pub params: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SourceTypeItem {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub audio_capable: bool,
}

#[derive(Clone, Serialize)]
pub struct ReusableSourceInfo {
    pub name: String,
    pub source_type: String,
}

#[derive(Serialize)]
pub struct SourcePropertyOption {
    pub value: String,
    pub label: String,
}

#[derive(Serialize)]
pub struct SourcePropertySpec {
    pub key: String,
    pub label: String,
    pub kind: String,
    pub hint: String,
    pub options: Vec<SourcePropertyOption>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PlannerInit {
    pub sources: Vec<SourceInfo>,
    pub scene_resolution: String,
    pub selected_source_id: Option<String>,
    #[serde(default)]
    pub scene_name: Option<String>,
}

#[derive(Deserialize)]
pub struct PlannerInitInput {
    pub sources: Vec<SourceInfo>,
    pub scene_resolution: String,
    pub selected_source_id: Option<String>,
    #[serde(default)]
    pub scene_name: Option<String>,
}

#[derive(Deserialize)]
pub struct SourceUpdate {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub params: HashMap<String, String>,
}

#[derive(Deserialize)]
pub struct SourceCreate {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub params: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SourceFilterItem {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub params: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn encoder_preference_preserves_default_and_wire_spelling() {
        assert!(matches!(
            VideoEncoderPreference::default(),
            VideoEncoderPreference::Hardware
        ));

        let cases = [
            (VideoEncoderPreference::Hardware, "hardware"),
            (VideoEncoderPreference::Nvenc, "nvenc"),
            (VideoEncoderPreference::NvencLegacy, "nvenclegacy"),
            (VideoEncoderPreference::Software, "software"),
        ];
        for (preference, expected) in cases {
            assert_eq!(
                serde_json::to_value(preference).expect("preference should serialize"),
                json!(expected)
            );
        }

        let legacy: VideoEncoderPreference =
            serde_json::from_value(json!("nvenclegacy")).expect("legacy NVENC should deserialize");
        assert!(matches!(legacy, VideoEncoderPreference::NvencLegacy));
    }

    #[test]
    fn media_choice_models_preserve_wire_shapes() {
        let device = AudioDevice {
            id: "alsa_input.usb-device".into(),
            name: "USB Microphone".into(),
        };
        let option = SelectOption {
            value: "h264_nvenc".into(),
            label: "NVIDIA NVENC".into(),
        };

        assert_eq!(
            serde_json::to_value(device).expect("audio device should serialize"),
            json!({
                "id": "alsa_input.usb-device",
                "name": "USB Microphone"
            })
        );
        assert_eq!(
            serde_json::to_value(option).expect("select option should serialize"),
            json!({
                "value": "h264_nvenc",
                "label": "NVIDIA NVENC"
            })
        );
    }

    #[test]
    fn settings_select_options_preserve_all_wire_fields() {
        let options = ObsSettingsSelectOptions {
            streaming_audio_encoders: vec![],
            streaming_video_encoders: vec![SelectOption {
                value: "h264_nvenc".into(),
                label: "NVIDIA NVENC".into(),
            }],
            recording_audio_encoders: vec![],
            recording_video_encoders: vec![],
            recording_formats: vec![],
            audio_sample_rates: vec![],
            audio_channels: vec![],
            video_resolutions: vec![],
            video_downscale_filters: vec![],
            video_common_fps: vec![],
        };

        assert_eq!(
            serde_json::to_value(options).expect("settings options should serialize"),
            json!({
                "streaming_audio_encoders": [],
                "streaming_video_encoders": [{
                    "value": "h264_nvenc",
                    "label": "NVIDIA NVENC"
                }],
                "recording_audio_encoders": [],
                "recording_video_encoders": [],
                "recording_formats": [],
                "audio_sample_rates": [],
                "audio_channels": [],
                "video_resolutions": [],
                "video_downscale_filters": [],
                "video_common_fps": []
            })
        );
    }

    #[test]
    fn source_type_defaults_audio_capability() {
        let item: SourceTypeItem = serde_json::from_value(json!({
            "id": "image_source",
            "label": "Image"
        }))
        .expect("source type should deserialize");

        assert!(!item.audio_capable);
    }

    #[test]
    fn source_filter_defaults_optional_fields() {
        let filter: SourceFilterItem = serde_json::from_value(json!({
            "id": "filter-1",
            "name": "Color Correction",
            "kind": "color_filter",
            "enabled": true
        }))
        .expect("source filter should deserialize");

        assert!(!filter.locked);
        assert!(filter.params.is_empty());
    }

    #[test]
    fn planner_init_defaults_scene_name() {
        let planner: PlannerInit = serde_json::from_value(json!({
            "sources": [],
            "scene_resolution": "1920x1080",
            "selected_source_id": null
        }))
        .expect("planner state should deserialize");

        assert!(planner.scene_name.is_none());
    }

    #[test]
    fn source_info_preserves_wire_shape() {
        let source = SourceInfo {
            id: "camera-1".into(),
            name: "Camera".into(),
            visible: true,
            source_type: "v4l2_input".into(),
            audio_capable: false,
            params: HashMap::from([("device_id".into(), "/dev/video0".into())]),
        };

        assert_eq!(
            serde_json::to_value(source).expect("source should serialize"),
            json!({
                "id": "camera-1",
                "name": "Camera",
                "visible": true,
                "source_type": "v4l2_input",
                "audio_capable": false,
                "params": { "device_id": "/dev/video0" }
            })
        );
    }

    #[test]
    fn source_commands_keep_snake_case_payloads() {
        let payload = json!({
            "id": "source-1",
            "name": "Source",
            "source_type": "image_source",
            "params": { "file": "/tmp/image.png" }
        });

        let create: SourceCreate =
            serde_json::from_value(payload.clone()).expect("create payload should deserialize");
        let update: SourceUpdate =
            serde_json::from_value(payload).expect("update payload should deserialize");

        assert_eq!(create.source_type, "image_source");
        assert_eq!(
            update.params.get("file").map(String::as_str),
            Some("/tmp/image.png")
        );
    }

    #[test]
    fn planner_payload_round_trips_without_field_changes() {
        let payload = json!({
            "sources": [{
                "id": "source-1",
                "name": "Source",
                "visible": true,
                "source_type": "image_source",
                "audio_capable": false,
                "params": {}
            }],
            "scene_resolution": "1280x720",
            "selected_source_id": "source-1",
            "scene_name": "Scene 1"
        });
        let planner: PlannerInit =
            serde_json::from_value(payload.clone()).expect("planner payload should deserialize");
        let serialized: Value =
            serde_json::to_value(planner).expect("planner payload should serialize");

        assert_eq!(serialized, payload);
    }

    #[test]
    fn scene_and_theme_models_preserve_wire_shapes() {
        let scene = SceneInfo {
            name: "Scene 1".into(),
            active: true,
            locked: false,
        };
        let theme = ThemeInfo {
            id: "revo-future".into(),
            name: "RevoFuture".into(),
            author: "RevoProject".into(),
            version: "1.0.0".into(),
        };

        assert_eq!(
            serde_json::to_value(scene).expect("scene should serialize"),
            json!({
                "name": "Scene 1",
                "active": true,
                "locked": false
            })
        );
        assert_eq!(
            serde_json::to_value(theme).expect("theme should serialize"),
            json!({
                "id": "revo-future",
                "name": "RevoFuture",
                "author": "RevoProject",
                "version": "1.0.0"
            })
        );
    }

    #[test]
    fn plugin_models_preserve_wire_shapes() {
        let info = PluginInfo {
            name: "Websocket".into(),
            file_name: "obs-websocket.so".into(),
            module_name: "obs-websocket".into(),
            window_capabilities: PluginWindowCapabilities {
                can_list: false,
                can_open: false,
                reason: "unsupported".into(),
            },
        };

        assert_eq!(
            serde_json::to_value(info).expect("plugin info should serialize"),
            json!({
                "name": "Websocket",
                "file_name": "obs-websocket.so",
                "module_name": "obs-websocket",
                "window_capabilities": {
                    "can_list": false,
                    "can_open": false,
                    "reason": "unsupported"
                }
            })
        );

        let profile = PluginProfile {
            enabled_modules: vec!["obs-websocket".into()],
        };
        let state = PluginProfileState {
            profile_name: "default".into(),
            enabled_modules: vec![],
        };

        assert_eq!(
            serde_json::to_value(profile).expect("plugin profile should serialize"),
            json!({ "enabled_modules": ["obs-websocket"] })
        );
        assert_eq!(
            serde_json::to_value(state).expect("plugin profile state should serialize"),
            json!({
                "profile_name": "default",
                "enabled_modules": []
            })
        );
    }
}
