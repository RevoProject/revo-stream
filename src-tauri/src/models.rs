use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum VideoEncoderPreference {
	Hardware,
	Software,
}

impl Default for VideoEncoderPreference {
	fn default() -> Self {
		VideoEncoderPreference::Hardware
	}
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct LibObsActionEvent {
	pub(crate) timestamp_ms: u64,
	pub(crate) action: String,
	pub(crate) detail: Option<JsonValue>,
}

#[derive(Serialize)]
pub(crate) struct CefBridgeInfo {
	pub(crate) compiled: bool,
	pub(crate) major: i32,
	pub(crate) minor: i32,
	pub(crate) patch: i32,
	pub(crate) commit: i32,
}

#[derive(Serialize)]
pub(crate) struct BrowserDockState {
	pub(crate) is_open: bool,
	pub(crate) url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct SourceInfo {
	pub(crate) id: String,
	pub(crate) name: String,
	pub(crate) visible: bool,
	pub(crate) source_type: String,
	pub(crate) params: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct SourceTypeItem {
	pub(crate) id: String,
	pub(crate) label: String,
}

#[derive(Serialize)]
pub(crate) struct AudioDevice {
	pub(crate) id: String,
	pub(crate) name: String,
}

#[derive(Serialize, Clone)]
pub(crate) struct SelectOption {
	pub(crate) value: String,
	pub(crate) label: String,
}

#[derive(Serialize)]
pub(crate) struct ObsSettingsSelectOptions {
	pub(crate) streaming_audio_encoders: Vec<SelectOption>,
	pub(crate) streaming_video_encoders: Vec<SelectOption>,
	pub(crate) recording_audio_encoders: Vec<SelectOption>,
	pub(crate) recording_video_encoders: Vec<SelectOption>,
	pub(crate) recording_formats: Vec<SelectOption>,
	pub(crate) audio_sample_rates: Vec<SelectOption>,
	pub(crate) audio_channels: Vec<SelectOption>,
	pub(crate) video_resolutions: Vec<SelectOption>,
	pub(crate) video_downscale_filters: Vec<SelectOption>,
	pub(crate) video_common_fps: Vec<SelectOption>,
}

#[derive(Serialize)]
pub(crate) struct SceneInfo {
	pub(crate) name: String,
	pub(crate) active: bool,
	pub(crate) locked: bool,
}

#[derive(Serialize)]
pub(crate) struct SourcePropertyOption {
	pub(crate) value: String,
	pub(crate) label: String,
}

#[derive(Serialize)]
pub(crate) struct SourcePropertySpec {
	pub(crate) key: String,
	pub(crate) label: String,
	pub(crate) kind: String,
	pub(crate) hint: String,
	pub(crate) options: Vec<SourcePropertyOption>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct AppSettings {
	#[serde(default)]
	pub(crate) root_dir: Option<String>,
	#[serde(default = "crate::settings::core::default_record_path")]
	pub(crate) record_path: String,
	#[serde(default)]
	pub(crate) stream_url: String,
	#[serde(default)]
	pub(crate) stream_key: String,
	#[serde(default = "crate::settings::core::default_preview_quality")]
	pub(crate) preview_quality: Option<String>,
	#[serde(default)]
	pub(crate) encoder_preference: Option<String>,
	#[serde(default)]
	pub(crate) scene_resolution: Option<String>,
	#[serde(default)]
	pub(crate) whep_url: Option<String>,
	#[serde(default)]
	pub(crate) whip_url: Option<String>,
	#[serde(default)]
	pub(crate) auto_retry_preview: Option<bool>,
	#[serde(default)]
	pub(crate) autorescale_inputs: Option<bool>,
	#[serde(default)]
	pub(crate) ui_profile: Option<JsonValue>,
	#[serde(default = "crate::settings::core::default_active_profile")]
	pub(crate) active_profile: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct PluginInfo {
	pub(crate) name: String,
	pub(crate) file_name: String,
	pub(crate) module_name: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct PluginProfile {
	pub(crate) enabled_modules: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct PluginProfileState {
	pub(crate) profile_name: String,
	pub(crate) enabled_modules: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct ThemeInfo {
	pub(crate) id: String,
	pub(crate) name: String,
	pub(crate) author: String,
	pub(crate) version: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct PlannerInit {
	pub(crate) sources: Vec<SourceInfo>,
	pub(crate) scene_resolution: String,
	pub(crate) selected_source_id: Option<String>,
	#[serde(default)]
	pub(crate) scene_name: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct PlannerInitInput {
	pub(crate) sources: Vec<SourceInfo>,
	pub(crate) scene_resolution: String,
	pub(crate) selected_source_id: Option<String>,
	#[serde(default)]
	pub(crate) scene_name: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct SourceUpdate {
	pub(crate) id: String,
	pub(crate) name: String,
	pub(crate) source_type: String,
	pub(crate) params: HashMap<String, String>,
}

#[derive(Deserialize)]
pub(crate) struct SourceCreate {
	pub(crate) id: String,
	pub(crate) name: String,
	pub(crate) source_type: String,
	pub(crate) params: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct SourceFilterItem {
	pub(crate) id: String,
	pub(crate) name: String,
	pub(crate) kind: String,
	pub(crate) enabled: bool,
	#[serde(default)]
	pub(crate) locked: bool,
	#[serde(default)]
	pub(crate) params: HashMap<String, String>,
}
